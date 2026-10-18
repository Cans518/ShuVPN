use super::model::{validate_packet, PlatformFailure};
use std::{
    os::fd::{AsRawFd, OwnedFd},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{io::unix::AsyncFd, sync::Notify};
use tokio_util::sync::CancellationToken;

pub(crate) struct PacketFd {
    fd: Mutex<Option<Arc<AsyncFd<OwnedFd>>>>,
    cancellation: CancellationToken,
    active: AtomicUsize,
    idle: Notify,
    mtu: u32,
}

pub(crate) struct Operation<'a> {
    device: &'a PacketFd,
    pub(crate) fd: Option<Arc<AsyncFd<OwnedFd>>>,
}
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.fd.take();
        self.device.active.fetch_sub(1, Ordering::AcqRel);
        self.device.idle.notify_waiters();
    }
}

impl PacketFd {
    #[cfg(test)]
    pub(crate) fn new(fd: OwnedFd, mtu: u32) -> Result<Self, PlatformFailure> {
        Ok(Self::registered(AsyncFd::new(fd)?, mtu))
    }
    pub(crate) fn registered(descriptor: AsyncFd<OwnedFd>, mtu: u32) -> Self {
        Self {
            fd: Mutex::new(Some(Arc::new(descriptor))),
            cancellation: CancellationToken::new(),
            active: AtomicUsize::new(0),
            idle: Notify::new(),
            mtu,
        }
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.cancellation.is_cancelled()
    }
    pub(crate) fn request_close(&self) -> Result<(), PlatformFailure> {
        self.cancellation.cancel();
        self.fd
            .lock()
            .map_err(|_| PlatformFailure::new("lock", "TUN 状态不可用"))?
            .take();
        Ok(())
    }
    pub(crate) fn operation(&self) -> Result<Operation<'_>, PlatformFailure> {
        let guard = self
            .fd
            .lock()
            .map_err(|_| PlatformFailure::new("lock", "TUN 状态不可用"))?;
        if self.cancellation.is_cancelled() {
            return Err(closed());
        }
        let fd = guard.as_ref().ok_or_else(closed)?.clone();
        self.active.fetch_add(1, Ordering::AcqRel);
        Ok(Operation {
            device: self,
            fd: Some(fd),
        })
    }

    pub(crate) async fn read(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        if !(1..=5000).contains(&wait_ms) {
            return Err(PlatformFailure::new(
                "timeout",
                "读取等待必须为 1–5000 毫秒",
            ));
        }
        let operation = self.operation()?;
        let descriptor = operation.fd.as_deref().ok_or_else(closed)?;
        let result = async {
            loop {
                let mut ready = descriptor.readable().await?;
                let mut packet = vec![0u8; 65536];
                let result = ready.try_io(|descriptor| {
                    // SAFETY: OwnedFd stays alive through the operation lease and
                    // the buffer is writable for exactly its allocated length.
                    let read = unsafe {
                        libc::read(
                            descriptor.as_raw_fd(),
                            packet.as_mut_ptr().cast(),
                            packet.len(),
                        )
                    };
                    if read < 0 {
                        Err(std::io::Error::last_os_error())
                    } else {
                        Ok(read as usize)
                    }
                });
                match result {
                    Ok(Ok(0)) => return Err(closed()),
                    Ok(Ok(length)) => {
                        packet.truncate(length);
                        return Ok(Some(packet));
                    }
                    Ok(Err(error)) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Ok(Err(error)) => return Err(error.into()),
                    Err(_) => continue,
                }
            }
        };
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(closed()),
            result = tokio::time::timeout(Duration::from_millis(u64::from(wait_ms)), result) =>
                match result { Ok(result) => result, Err(_) => Ok(None) },
        }
    }

    pub(crate) async fn write(&self, packet: &[u8]) -> Result<(), PlatformFailure> {
        validate_packet(packet, self.mtu)?;
        let operation = self.operation()?;
        let descriptor = operation.fd.as_deref().ok_or_else(closed)?;
        let result = async {
            loop {
                let mut ready = descriptor.writable().await?;
                let written = ready.try_io(|descriptor| {
                    // SAFETY: the immutable packet is live and OwnedFd is leased
                    // until this syscall and readiness loop have completed.
                    let written = unsafe {
                        libc::write(descriptor.as_raw_fd(), packet.as_ptr().cast(), packet.len())
                    };
                    if written < 0 {
                        Err(std::io::Error::last_os_error())
                    } else {
                        Ok(written as usize)
                    }
                });
                match written {
                    Ok(Ok(length)) if length == packet.len() => return Ok(()),
                    Ok(Ok(_)) => {
                        return Err(PlatformFailure::new("packet", "内核没有接受完整 IP 包"))
                    }
                    Ok(Err(error)) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Ok(Err(error)) => return Err(error.into()),
                    Err(_) => continue,
                }
            }
        };
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(closed()),
            result = tokio::time::timeout(Duration::from_secs(5), result) =>
                result.map_err(|_| PlatformFailure::new("timeout", "TUN 写入超时"))?,
        }
    }

    pub(crate) async fn close(&self) -> Result<(), PlatformFailure> {
        self.request_close()?;
        loop {
            let notified = self.idle.notified();
            if self.active.load(Ordering::Acquire) == 0 {
                break;
            }
            notified.await;
        }
        Ok(())
    }
}

impl Drop for PacketFd {
    fn drop(&mut self) {
        self.cancellation.cancel();
        // OwnedFd closes after the last operation lease is gone.
        if let Ok(mut fd) = self.fd.lock() {
            fd.take();
        }
    }
}

fn closed() -> PlatformFailure {
    PlatformFailure::new("closed", "TUN 已关闭")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixDatagram;
    use tokio::time::{timeout, Duration};

    fn packet() -> Vec<u8> {
        let mut bytes = vec![0u8; 20];
        bytes[0] = 0x45;
        bytes[2..4].copy_from_slice(&20u16.to_be_bytes());
        bytes
    }

    fn pair() -> anyhow::Result<(Arc<PacketFd>, tokio::net::UnixDatagram)> {
        let (owned, peer) = UnixDatagram::pair()?;
        owned.set_nonblocking(true)?;
        peer.set_nonblocking(true)?;
        Ok((
            Arc::new(PacketFd::new(owned.into(), 1400)?),
            tokio::net::UnixDatagram::from_std(peer)?,
        ))
    }

    fn weak_descriptor(device: &PacketFd) -> anyhow::Result<std::sync::Weak<AsyncFd<OwnedFd>>> {
        let slot = device.fd.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        Ok(Arc::downgrade(
            slot.as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing descriptor"))?,
        ))
    }

    async fn assert_closed(
        descriptor: std::sync::Weak<AsyncFd<OwnedFd>>,
        peer: &tokio::net::UnixDatagram,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            descriptor.upgrade().is_none(),
            "close retained a Rust descriptor owner"
        );
        // A concurrent fork can briefly retain a CLOEXEC descriptor until exec.
        // Local ownership must already be gone; also require kernel peer closure.
        timeout(Duration::from_secs(2), async {
            while peer.send(&packet()).await.is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn kernel_packet_io_preserves_bytes_and_validates_before_write() -> anyhow::Result<()> {
        let (device, peer) = pair()?;
        let descriptor = weak_descriptor(&device)?;
        let expected = packet();
        peer.send(&expected).await?;
        anyhow::ensure!(
            device.read(1000).await? == Some(expected.clone()),
            "kernel read changed bytes"
        );
        device.write(&expected).await?;
        let mut buffer = [0u8; 64];
        let length = peer.recv(&mut buffer).await?;
        anyhow::ensure!(buffer[..length] == expected, "kernel write changed bytes");
        anyhow::ensure!(
            device.write(&[1, 2, 3]).await.is_err(),
            "invalid packet accepted"
        );
        anyhow::ensure!(device.read(1).await?.is_none(), "timeout invented bytes");
        device.close().await?;
        device.close().await?;
        assert_closed(descriptor, &peer).await?;
        Ok(())
    }

    #[tokio::test]
    async fn closing_interrupts_a_pending_read_and_rejects_future_operations() -> anyhow::Result<()>
    {
        let (device, _peer) = pair()?;
        let reader = device.clone();
        let pending = tokio::spawn(async move { reader.read(5000).await });
        tokio::task::yield_now().await;
        device.request_close()?;
        timeout(Duration::from_secs(1), device.close()).await??;
        anyhow::ensure!(
            pending
                .await?
                .err()
                .is_some_and(|error| error.code == "closed"),
            "pending read survived close"
        );
        anyhow::ensure!(
            device
                .write(&packet())
                .await
                .err()
                .is_some_and(|error| error.code == "closed"),
            "closed descriptor accepted write"
        );
        Ok(())
    }

    #[tokio::test]
    async fn cancelling_close_does_not_release_an_operation_fd_early() -> anyhow::Result<()> {
        let (device, peer) = pair()?;
        let descriptor = weak_descriptor(&device)?;
        let operation = device.operation()?;
        let closer = device.clone();
        let pending = tokio::spawn(async move { closer.close().await });
        device.cancellation.cancelled().await;
        anyhow::ensure!(!pending.is_finished(), "close ignored a live operation");
        pending.abort();
        let _ = pending.await;
        peer.send(&packet()).await?;
        drop(operation);
        timeout(Duration::from_secs(1), device.close()).await??;
        assert_closed(descriptor, &peer).await?;
        Ok(())
    }
}
