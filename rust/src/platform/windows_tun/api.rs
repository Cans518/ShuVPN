use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::Arc;

use crate::platform::model::PlatformFailure;
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GetProcAddress, LoadLibraryExW,
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

pub(super) const DLL_SHA256: &str =
    "E5DA8447DC2C320EDC0FC52FA01885C103DE8C118481F683643CACC3220DAFCE";
pub(super) type Handle = *mut c_void;
type Create =
    unsafe extern "system" fn(*const u16, *const u16, *const windows_sys::core::GUID) -> Handle;
type Close = unsafe extern "system" fn(Handle);
type Luid = unsafe extern "system" fn(Handle, *mut NET_LUID_LH);
type Start = unsafe extern "system" fn(Handle, u32) -> Handle;
type End = unsafe extern "system" fn(Handle);
type Event = unsafe extern "system" fn(Handle) -> Handle;
type Receive = unsafe extern "system" fn(Handle, *mut u32) -> *mut u8;
type Release = unsafe extern "system" fn(Handle, *const u8);
type Allocate = unsafe extern "system" fn(Handle, u32) -> *mut u8;
type SendPacket = unsafe extern "system" fn(Handle, *const u8);

pub(super) struct Api {
    _module: Module,
    pub create: Create,
    pub close: Close,
    pub luid: Luid,
    pub start: Start,
    pub end: End,
    pub event: Event,
    pub receive: Receive,
    pub release: Release,
    pub allocate: Allocate,
    pub send: SendPacket,
}

struct Module(HMODULE);
impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.0);
        }
    }
}
// The module is immutable and retained until every session/adapter lease ends.
unsafe impl Send for Module {}
unsafe impl Sync for Module {}

impl Api {
    pub(super) fn load(path: &Path) -> Result<Arc<Self>, PlatformFailure> {
        if !path.is_absolute() {
            return Err(error("driver", "Wintun DLL 必须是绝对路径"));
        }
        // Deny concurrent writers/deletion while checking and mapping the DLL.
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(path)
            .map_err(|_| error("driver", "无法打开已固定的 Wintun DLL"))?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 2 * 1024 * 1024
            || sangfor_core::crypto::hex_upper(&sangfor_core::crypto::sha256(&bytes)) != DLL_SHA256
        {
            return Err(error("driver", "Wintun DLL 版本或 SHA-256 不匹配"));
        }
        let wide = wide_path(path)?;
        let module = Module(unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        });
        if module.0.is_null() {
            return Err(system("加载 Wintun DLL 失败"));
        }
        macro_rules! symbol {
            ($name:literal, $kind:ty) => {{
                let address = unsafe { GetProcAddress(module.0, concat!($name, "\0").as_ptr()) }
                    .ok_or_else(|| error("driver", concat!("Wintun 缺少接口 ", $name)))?;
                unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, $kind>(address)
                }
            }};
        }
        let api = Self {
            create: symbol!("WintunCreateAdapter", Create),
            close: symbol!("WintunCloseAdapter", Close),
            luid: symbol!("WintunGetAdapterLUID", Luid),
            start: symbol!("WintunStartSession", Start),
            end: symbol!("WintunEndSession", End),
            event: symbol!("WintunGetReadWaitEvent", Event),
            receive: symbol!("WintunReceivePacket", Receive),
            release: symbol!("WintunReleaseReceivePacket", Release),
            allocate: symbol!("WintunAllocateSendPacket", Allocate),
            send: symbol!("WintunSendPacket", SendPacket),
            _module: module,
        };
        drop(file);
        Ok(Arc::new(api))
    }

    pub(super) fn default_path() -> Result<PathBuf, PlatformFailure> {
        if let Some(path) = std::env::var_os("SHUVPN_WINTUN_DLL") {
            return Ok(PathBuf::from(path));
        }
        let mut module = null_mut();
        if unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                Self::default_path as *const () as *const u16,
                &mut module,
            )
        } == 0
        {
            return Err(system("无法定位原生核心模块"));
        }
        let mut buffer = vec![0u16; 32768];
        let length =
            unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 || length as usize >= buffer.len() {
            return Err(system("原生核心模块路径无效"));
        }
        let path = PathBuf::from(
            String::from_utf16(&buffer[..length as usize])
                .map_err(|_| error("driver", "原生核心路径编码无效"))?,
        );
        Ok(path
            .parent()
            .ok_or_else(|| error("driver", "原生核心目录无效"))?
            .join("wintun.dll"))
    }
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn wide_path(path: &Path) -> Result<Vec<u16>, PlatformFailure> {
    use std::os::windows::ffi::OsStrExt;
    let value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(error("driver", "Wintun 路径含空字符"));
    }
    Ok(value.into_iter().chain(std::iter::once(0)).collect())
}
pub(super) fn error(code: &str, message: &str) -> PlatformFailure {
    PlatformFailure::new(code, message)
}
pub(super) fn system(message: &str) -> PlatformFailure {
    let cause = std::io::Error::last_os_error();
    let code = if cause.kind() == std::io::ErrorKind::PermissionDenied {
        "permission"
    } else {
        "system"
    };
    PlatformFailure::new(code, &format!("{message}：{cause}"))
}
