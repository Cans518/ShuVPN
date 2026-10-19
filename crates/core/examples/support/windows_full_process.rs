//! Read-only process witnesses retained by kernel handle, never by PID alone.
use anyhow::{ensure, Context};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, os::windows::ffi::OsStringExt, path::Path, time::Duration};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, FILETIME, HANDLE, WAIT_TIMEOUT},
    Security::{
        Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenElevation, TokenUser,
        TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER,
    },
    System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, OpenProcessToken,
        QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SYNCHRONIZE,
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: every Handle exclusively owns one successful Open* result.
        unsafe { CloseHandle(self.0) };
    }
}

pub struct Process(Handle, u32);
// SAFETY: retained Windows query/wait handles support concurrent access.
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub created_ticks: String,
    pub image: String,
    pub sha256: String,
    pub elevated: bool,
    pub sid: String,
}

impl Process {
    pub fn open(pid: u32) -> anyhow::Result<Self> {
        ensure!(pid != 0, "zero process PID");
        // SAFETY: query/wait only; no termination or mutation rights requested.
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        ensure!(
            !handle.is_null(),
            "OpenProcess({pid}): {}",
            std::io::Error::last_os_error()
        );
        Ok(Self(Handle(handle), pid))
    }

    pub fn alive(&self) -> anyhow::Result<bool> {
        // SAFETY: the process handle remains owned and a zero timeout cannot block.
        match unsafe { WaitForSingleObject(self.0 .0, 0) } {
            WAIT_TIMEOUT => Ok(true),
            0 => Ok(false),
            _ => Err(std::io::Error::last_os_error()).context("process lifetime query"),
        }
    }

    pub fn created(&self) -> anyhow::Result<u64> {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: retained process and four writable FILETIME outputs.
        ensure!(
            unsafe {
                GetProcessTimes(self.0 .0, &mut created, &mut exited, &mut kernel, &mut user)
            } != 0,
            "GetProcessTimes failed"
        );
        Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    pub fn identity(&self) -> anyhow::Result<Identity> {
        ensure!(self.alive()?, "process exited before identity capture");
        let mut path = vec![0u16; 32768];
        let mut length = path.len() as u32;
        // SAFETY: the buffer has the advertised writable UTF-16 capacity.
        ensure!(
            unsafe { QueryFullProcessImageNameW(self.0 .0, 0, path.as_mut_ptr(), &mut length) }
                != 0,
            "process image query failed"
        );
        let path =
            std::path::PathBuf::from(std::ffi::OsString::from_wide(&path[..length as usize]));
        let path = path.canonicalize()?;
        let mut token = std::ptr::null_mut();
        // SAFETY: valid process handle; token is a writable HANDLE output.
        ensure!(
            unsafe { OpenProcessToken(self.0 .0, TOKEN_QUERY, &mut token) } != 0,
            "process token query failed"
        );
        let token = Handle(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0;
        // SAFETY: token is retained and output has exactly TOKEN_ELEVATION capacity.
        ensure!(
            unsafe {
                GetTokenInformation(
                    token.0,
                    TokenElevation,
                    std::ptr::from_mut(&mut elevation).cast(),
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut size,
                )
            } != 0,
            "token elevation query failed"
        );
        // SID maximum size fits this aligned buffer; reject any oversized response.
        let mut user_buffer = [0u64; 128];
        // SAFETY: u64 buffer aligns TOKEN_USER and has the stated byte capacity.
        ensure!(
            unsafe {
                GetTokenInformation(
                    token.0,
                    TokenUser,
                    user_buffer.as_mut_ptr().cast(),
                    std::mem::size_of_val(&user_buffer) as u32,
                    &mut size,
                )
            } != 0,
            "token user query failed"
        );
        // SAFETY: successful TokenUser query initialized TOKEN_USER in this live buffer.
        let user = unsafe { &*user_buffer.as_ptr().cast::<TOKEN_USER>() };
        let mut sid = std::ptr::null_mut();
        // SAFETY: SID points into the live token-user buffer; output is LocalAlloc-owned.
        ensure!(
            unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } != 0,
            "SID conversion failed"
        );
        let mut length = 0;
        // SAFETY: successful conversion returned a NUL-terminated allocated SID string.
        unsafe {
            while *sid.add(length) != 0 {
                length += 1;
            }
        }
        // SAFETY: the exact string length was read from the live allocation above.
        let sid_text = unsafe { String::from_utf16(std::slice::from_raw_parts(sid, length)) };
        // SAFETY: ConvertSidToStringSidW allocated this value with LocalAlloc.
        unsafe {
            LocalFree(sid.cast());
        }
        let identity = Identity {
            pid: self.1,
            created_ticks: self.created()?.to_string(),
            image: path.to_string_lossy().into_owned(),
            sha256: sha_file(&path)?,
            elevated: elevation.TokenIsElevated != 0,
            sid: sid_text?,
        };
        ensure!(self.alive()?, "process exited during identity capture");
        Ok(identity)
    }

    pub async fn successful_exit(&self) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(8), async {
            while self.alive()? {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let mut code = 0;
            // SAFETY: the same retained process handle is still valid after exit.
            ensure!(
                unsafe { GetExitCodeProcess(self.0 .0, &mut code) } != 0,
                "exit-code query failed"
            );
            ensure!(code == 0, "observed process exited with {code}");
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("observed process did not exit")?
    }
}

pub fn sha_file(path: &Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    #[tokio::test]
    async fn retains_the_exact_real_process_through_successful_exit() -> anyhow::Result<()> {
        use std::os::windows::process::CommandExt;
        let mut child = ChildGuard(
            std::process::Command::new("cmd.exe")
                .args(["/d", "/c", "pause >nul"])
                .stdin(std::process::Stdio::piped())
                .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
                .spawn()?,
        );
        let process = Process::open(child.0.id())?;
        let identity = process.identity()?;
        ensure!(
            identity.pid == child.0.id()
                && identity.created_ticks == process.created()?.to_string(),
            "retained identity changed"
        );
        use std::io::Write;
        child
            .0
            .stdin
            .as_mut()
            .context("child input")?
            .write_all(b"\n")?;
        process.successful_exit().await?;
        ensure!(
            !process.alive()? && process.identity().is_err(),
            "exited identity accepted"
        );
        child.0.wait()?;
        Ok(())
    }

    #[tokio::test]
    async fn a_real_nonzero_exit_cannot_confirm_successful_cleanup() -> anyhow::Result<()> {
        use std::{io::Write, os::windows::process::CommandExt};
        let mut child = ChildGuard(
            std::process::Command::new("cmd.exe")
                .args(["/d", "/c", "pause >nul & exit /b 7"])
                .stdin(std::process::Stdio::piped())
                .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
                .spawn()?,
        );
        let process = Process::open(child.0.id())?;
        child
            .0
            .stdin
            .as_mut()
            .context("child input")?
            .write_all(b"\n")?;
        assert!(process.successful_exit().await.is_err());
        ensure!(
            !process.alive()?,
            "nonzero witness timed out instead of observing exit"
        );
        assert_eq!(child.0.wait()?.code(), Some(7));
        Ok(())
    }
}
