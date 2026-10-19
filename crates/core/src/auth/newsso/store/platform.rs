use super::failure;
use crate::{auth::model::AuthFailure, platform::files};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub(super) fn directory(path: &Path) -> Result<File, AuthFailure> {
    files::directory(path).map_err(|error| failure(&error.to_string()))
}
pub(super) fn check_directory(path: &Path, handle: &File) -> Result<(), AuthFailure> {
    files::check_directory(path, handle).map_err(|error| failure(&error.to_string()))
}
pub(super) fn options() -> OpenOptions {
    files::options()
}
pub(super) fn regular(path: &Path) -> Result<bool, AuthFailure> {
    files::regular(path).map_err(|error| failure(&error.to_string()))
}
pub(super) fn same_file(handle: &File, path: &Path) -> Result<(), AuthFailure> {
    files::same_file(handle, path).map_err(|error| failure(&error.to_string()))
}
pub(super) fn replace(source: &Path, target: &Path) -> Result<(), AuthFailure> {
    files::replace(source, target).map_err(|error| failure(&error.to_string()))
}
pub(super) fn sync(directory: &File) -> Result<(), AuthFailure> {
    files::sync(directory).map_err(|error| failure(&error.to_string()))
}

#[cfg(not(target_os = "windows"))]
pub(super) fn seal(bytes: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>, AuthFailure> {
    Ok(zeroize::Zeroizing::new(bytes.to_vec()))
}
#[cfg(not(target_os = "windows"))]
pub(super) fn unseal(bytes: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>, AuthFailure> {
    Ok(zeroize::Zeroizing::new(bytes.to_vec()))
}

#[cfg(target_os = "windows")]
mod dpapi {
    use super::*;
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    const ENTROPY: &[u8] = b"ShuVPN/newsso-state/v1";
    struct Output(CRYPT_INTEGER_BLOB);
    impl Drop for Output {
        fn drop(&mut self) {
            if !self.0.pbData.is_null() {
                // SAFETY: DPAPI owns this LocalAlloc buffer; erase then free once.
                unsafe {
                    use zeroize::Zeroize;
                    std::slice::from_raw_parts_mut(self.0.pbData, self.0.cbData as usize).zeroize();
                    LocalFree(self.0.pbData.cast());
                }
            }
        }
    }
    pub(super) fn transform(
        bytes: &[u8],
        protect: bool,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, AuthFailure> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes
                .len()
                .try_into()
                .map_err(|_| failure("会话数据过大"))?,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let entropy = CRYPT_INTEGER_BLOB {
            cbData: ENTROPY.len() as u32,
            pbData: ENTROPY.as_ptr() as *mut u8,
        };
        let mut output = Output(CRYPT_INTEGER_BLOB::default());
        // SAFETY: input is borrowed for the call; DPAPI returns a LocalAlloc
        // buffer. Default scope is the current user, with all UI forbidden.
        let success = unsafe {
            if protect {
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output.0,
                )
            } else {
                CryptUnprotectData(
                    &input,
                    std::ptr::null_mut(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output.0,
                )
            }
        };
        if success == 0 || output.0.pbData.is_null() {
            return Err(failure("无法保护或恢复当前用户会话数据"));
        }
        // SAFETY: successful DPAPI returned cbData bytes, still retained by Output.
        Ok(zeroize::Zeroizing::new(
            unsafe { std::slice::from_raw_parts(output.0.pbData, output.0.cbData as usize) }
                .to_vec(),
        ))
    }
}

#[cfg(target_os = "windows")]
pub(super) fn seal(bytes: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>, AuthFailure> {
    dpapi::transform(bytes, true)
}
#[cfg(target_os = "windows")]
pub(super) fn unseal(bytes: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>, AuthFailure> {
    dpapi::transform(bytes, false)
}
