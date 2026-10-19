use super::process::Process;
use anyhow::{ensure, Context};
use std::{
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows_sys::Win32::UI::{
    Shell::{
        ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
        SHELLEXECUTEINFOW,
    },
    WindowsAndMessaging::SW_HIDE,
};

pub(super) fn elevated(arguments: [String; 3]) -> anyhow::Result<Process> {
    // Shell's consent flow runs on a dedicated COM apartment in this blocking
    // task, keeping the UI runtime free to process cancellation.
    let _apartment = Apartment::new()?;
    let helper = installed_path()?;
    let directory = helper
        .parent()
        .context("application directory unavailable")?;
    let file = wide(&helper);
    let directory = wide(directory);
    let verb = crate::platform::windows_tun::api::wide("runas");
    // Arguments contain only a fixed pipe prefix, hex and decimal numbers.
    let parameters = crate::platform::windows_tun::api::wide(&arguments.join(" "));
    let mut request = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: parameters.as_ptr(),
        lpDirectory: directory.as_ptr(),
        nShow: SW_HIDE,
        ..Default::default()
    };
    // SAFETY: all strings and the writable structure remain live for this
    // synchronous call. Shell owns the UAC consent flow; we own hProcess.
    if unsafe { ShellExecuteExW(&mut request) } == 0 {
        return Err(std::io::Error::last_os_error()).context("launch TUN helper with UAC");
    }
    Process::from_owned(request.hProcess)
}

pub(crate) fn installed_path() -> anyhow::Result<PathBuf> {
    let executable = std::env::current_exe()?.canonicalize()?;
    let directory = executable
        .parent()
        .context("application directory unavailable")?;
    let helper = directory.join("shuvpn_tun_helper.exe").canonicalize()?;
    ensure!(
        helper.parent() == Some(directory),
        "helper is outside the application directory"
    );
    ensure!(helper.is_file(), "TUN helper is not a regular file");
    Ok(helper)
}

struct Apartment;
impl Apartment {
    fn new() -> anyhow::Result<Self> {
        // SAFETY: initialization applies only to the calling blocking thread;
        // successful initialization is paired with exactly one uninitialize.
        let result = unsafe {
            CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        ensure!(
            result >= 0,
            "cannot initialize helper launch apartment: {result:#x}"
        );
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: this guard never leaves the thread on which it was created.
        unsafe { CoUninitialize() };
    }
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
