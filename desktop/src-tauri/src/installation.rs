/// Keep the installation directory protected while the application is running.
pub struct InstallationGuard {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

impl InstallationGuard {
    pub fn acquire() -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            let name: Vec<u16> = "ShuVPN.Desktop.Running\0".encode_utf16().collect();
            let handle = unsafe {
                windows_sys::Win32::System::Threading::CreateMutexW(
                    std::ptr::null(),
                    0,
                    name.as_ptr(),
                )
            };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self { handle })
        }
        #[cfg(not(windows))]
        Ok(Self {})
    }
}

impl Drop for InstallationGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}
