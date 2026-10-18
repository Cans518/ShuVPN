use anyhow::{ensure, Context};
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_TIMEOUT},
    System::{
        Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
        Threading::{
            GetExitCodeProcess, GetProcessId, GetProcessTimes, OpenProcess, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        },
    },
};

pub(super) struct Process(HANDLE);
// SAFETY: owned process handles support concurrent Windows wait/query calls.
// CloseHandle runs only after the last Rust owner has dropped the wrapper.
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns the handle.
        unsafe { CloseHandle(self.0) };
    }
}

impl Process {
    pub(super) fn open(pid: u32) -> anyhow::Result<Self> {
        ensure!(pid != 0, "invalid helper peer PID");
        // SAFETY: noninheritable handle; no memory pointers are passed.
        let handle = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        ensure!(
            !handle.is_null(),
            "cannot retain helper peer: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self(handle))
    }

    pub(super) fn from_owned(handle: HANDLE) -> anyhow::Result<Self> {
        ensure!(
            !handle.is_null(),
            "helper launch did not return a process handle"
        );
        Ok(Self(handle))
    }

    pub(super) fn id(&self) -> anyhow::Result<u32> {
        // SAFETY: handle is live and exclusively owned by this wrapper.
        let pid = unsafe { GetProcessId(self.0) };
        ensure!(pid != 0, "cannot read helper process identity");
        Ok(pid)
    }

    pub(super) fn alive(&self) -> anyhow::Result<bool> {
        // SAFETY: zero timeout never blocks and handle remains retained.
        match unsafe { WaitForSingleObject(self.0, 0) } {
            WAIT_TIMEOUT => Ok(true),
            0 => Ok(false),
            _ => Err(std::io::Error::last_os_error()).context("query helper peer lifetime"),
        }
    }

    pub(super) fn created(&self) -> anyhow::Result<u64> {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: handle is retained; each output points to a live FILETIME.
        ensure!(
            unsafe { GetProcessTimes(self.0, &mut created, &mut exited, &mut kernel, &mut user) }
                != 0,
            "cannot query helper peer creation time"
        );
        Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    pub(super) fn successful_exit(&self) -> anyhow::Result<()> {
        ensure!(!self.alive()?, "helper is still running");
        let mut status = 0;
        // SAFETY: the retained process handle is live and status is writable.
        ensure!(
            unsafe { GetExitCodeProcess(self.0, &mut status) } != 0,
            "cannot query helper exit status: {}",
            std::io::Error::last_os_error()
        );
        ensure!(
            status == 0,
            "helper exited without confirmed cleanup: {status}"
        );
        Ok(())
    }
}

pub(super) fn verify_peer(
    pipe: &impl AsRawHandle,
    expected: u32,
    side: Side,
) -> anyhow::Result<()> {
    let mut actual = 0;
    // SAFETY: borrowed pipe handle remains live; Windows writes one u32.
    let result = unsafe {
        match side {
            Side::Client => GetNamedPipeClientProcessId(pipe.as_raw_handle().cast(), &mut actual),
            Side::Server => GetNamedPipeServerProcessId(pipe.as_raw_handle().cast(), &mut actual),
        }
    };
    ensure!(
        result != 0 && actual == expected && expected != 0,
        "helper pipe peer identity mismatch"
    );
    Ok(())
}

pub(super) enum Side {
    Client,
    Server,
}
