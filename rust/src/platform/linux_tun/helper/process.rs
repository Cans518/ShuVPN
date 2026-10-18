//! Track helper lifetime independently of the host runtime's child reaper.
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
use tokio::{io::unix::AsyncFd, process::Child};

pub(super) struct Process {
    child: Child,
    exit: AsyncFd<Identity>,
}

struct Identity(OwnedFd);

impl AsRawFd for Identity {
    fn as_raw_fd(&self) -> i32 {
        self.0.as_raw_fd()
    }
}

impl Process {
    pub(super) fn open(child: Child, descriptor: OwnedFd) -> io::Result<Self> {
        Ok(Self {
            child,
            // Retain the kill guard even if async registration fails.
            exit: AsyncFd::new(Identity(descriptor))?,
        })
    }

    #[cfg(feature = "tun-probe")]
    pub(super) fn id(&self) -> Option<u32> {
        self.child.id()
    }

    pub(super) async fn wait(&mut self) -> io::Result<()> {
        // Dart's process-exit thread can reap children created through FFI.
        // A pidfd becomes readable only when this specific process exits,
        // even after another thread has consumed its wait status.
        let _ready = self.exit.readable().await?;
        match self.child.wait().await {
            Ok(_) => Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ECHILD) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(super) async fn kill(&mut self) -> io::Result<()> {
        self.exit.get_ref().request_kill()?;
        self.wait().await
    }
}

impl Identity {
    fn request_kill(&self) -> io::Result<()> {
        // SAFETY: the fd identifies the original process, never a reused PID;
        // a null siginfo requests the ordinary SIGKILL delivery semantics.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        Ok(())
    }
}

pub(super) fn current_exit_descriptor() -> io::Result<OwnedFd> {
    // SAFETY: getpid has no preconditions; the current process cannot have its
    // PID reused while making this syscall.
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful syscall transferred this new fd to us.
    Ok(unsafe { OwnedFd::from_raw_fd(descriptor as i32) })
}

impl Drop for Identity {
    fn drop(&mut self) {
        // Child's numeric PID may already have been reaped and reused.
        let _ = self.request_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{process::Stdio, time::Duration};
    use tokio::{process::Command, time::timeout};

    fn waiting_child() -> io::Result<Child> {
        Command::new("/bin/sh")
            .args(["-c", "read ignored"])
            .stdin(Stdio::piped())
            .kill_on_drop(false)
            .spawn()
    }

    fn tracked_child() -> io::Result<Process> {
        let child = waiting_child()?;
        let pid = child.id().ok_or_else(|| io::Error::other("missing PID"))?;
        // SAFETY: this fixture is blocked on our still-open stdin. Production
        // gets a self-opened pidfd through the helper handshake instead.
        let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful syscall transferred this new fd to us.
        Process::open(child, unsafe { OwnedFd::from_raw_fd(descriptor as i32) })
    }

    #[tokio::test]
    async fn confirms_exit_after_another_runtime_reaps_the_child() -> anyhow::Result<()> {
        let mut process = tracked_child()?;
        let pid = process
            .child
            .id()
            .ok_or_else(|| io::Error::other("missing PID"))?;
        drop(process.child.stdin.take());
        tokio::task::spawn_blocking(move || {
            let mut status = 0;
            // SAFETY: only wait for the child created by this test.
            let reaped = unsafe { libc::waitpid(pid as i32, &mut status, 0) };
            assert_eq!(reaped, pid as i32);
        })
        .await?;
        assert_eq!(
            process
                .child
                .wait()
                .await
                .err()
                .and_then(|error| error.raw_os_error()),
            Some(libc::ECHILD),
            "fixture must reproduce a consumed child wait status"
        );
        timeout(Duration::from_secs(2), process.wait()).await??;
        timeout(Duration::from_secs(2), process.wait()).await??;
        Ok(())
    }

    #[tokio::test]
    async fn a_live_process_is_not_confirmed_until_killed() -> anyhow::Result<()> {
        let mut process = tracked_child()?;
        assert!(timeout(Duration::from_millis(30), process.wait())
            .await
            .is_err());
        timeout(Duration::from_secs(2), process.kill()).await??;
        timeout(Duration::from_secs(2), process.wait()).await??;
        Ok(())
    }

    #[tokio::test]
    async fn dropping_the_owner_terminates_the_same_process() -> anyhow::Result<()> {
        let process = tracked_child()?;
        let monitor = AsyncFd::new(process.exit.get_ref().0.try_clone()?)?;
        drop(process);
        let _ready = timeout(Duration::from_secs(2), monitor.readable()).await??;
        Ok(())
    }
}
