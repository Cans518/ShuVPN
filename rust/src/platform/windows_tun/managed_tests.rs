use super::{Remote, Transport, Worker};
use crate::platform::model::PlatformFailure;
use std::{
    net::IpAddr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{Mutex, Notify};

#[test]
fn helper_launch_errors_keep_os_cause_and_cancellation_code() {
    for (os_code, failure_code) in [(1223, "cancelled"), (5, "helper")] {
        let error = anyhow::Error::new(std::io::Error::from_raw_os_error(os_code))
            .context("launch TUN helper with UAC");
        let failure = super::helper_error(error);
        assert_eq!(failure.code, failure_code);
        assert!(failure.message.contains("launch TUN helper with UAC"));
        assert!(
            failure.message.contains(&format!("os error {os_code}")),
            "OS error was lost: {}",
            failure.message
        );
    }
}

#[tokio::test]
async fn cancelled_join_after_close_acknowledgement_does_not_send_another_close(
) -> anyhow::Result<()> {
    let (requests, receiver) = tokio::sync::mpsc::channel(1);
    drop(receiver);
    let (reply, closing) = tokio::sync::oneshot::channel();
    reply
        .send(Ok(()))
        .map_err(|_| anyhow::anyhow!("reply dropped"))?;
    let release = Arc::new(Notify::new());
    let worker_release = Arc::clone(&release);
    let remote = Remote {
        requests,
        cancellation: tokio_util::sync::CancellationToken::new(),
        stopping: AtomicBool::new(false),
        worker: Mutex::new(Worker {
            task: Some(tokio::spawn(async move {
                worker_release.notified().await;
                Ok(())
            })),
            closing: Some(closing),
            acknowledged: false,
            closed: false,
        }),
        #[cfg(feature = "tun-probe")]
        probe: Arc::default(),
    };
    tokio::select! {
        result = remote.close() => anyhow::bail!("join did not wait: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    assert!(remote.worker.lock().await.acknowledged);
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), remote.close()).await??;
    assert!(remote.worker.lock().await.closed);
    Ok(())
}

#[derive(Default)]
struct Signals {
    started: Notify,
    release: Notify,
    closed: AtomicBool,
    reads: AtomicUsize,
    writes: Mutex<Vec<Vec<u8>>>,
    close_failures: AtomicUsize,
}

struct Peer(Arc<Signals>);

impl Transport for Peer {
    async fn create(&self, _plan: String) -> Result<String, PlatformFailure> {
        self.0.started.notify_one();
        self.0.release.notified().await;
        Ok("{}".to_owned())
    }
    async fn read(&self, _wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        self.0.reads.fetch_add(1, Ordering::SeqCst);
        self.0.started.notify_one();
        self.0.release.notified().await;
        Ok(Some(vec![7, 8, 9]))
    }
    async fn write(&self, packet: Vec<u8>) -> Result<(), PlatformFailure> {
        self.0.writes.lock().await.push(packet);
        Ok(())
    }
    async fn routes(&self, _hosts: Vec<IpAddr>) -> Result<(), PlatformFailure> {
        self.0.started.notify_one();
        self.0.release.notified().await;
        Ok(())
    }
    async fn close(&self) -> Result<(), PlatformFailure> {
        if self
            .0
            .close_failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
        {
            return Err(PlatformFailure::new("cleanup", "injected cleanup failure"));
        }
        self.0.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn abandoned_create_is_drained_before_close_and_cleanup_can_be_retried() -> anyhow::Result<()>
{
    let signals = Arc::new(Signals::default());
    signals.close_failures.store(1, Ordering::SeqCst);
    let remote = Remote::new(Peer(Arc::clone(&signals)));
    tokio::select! {
        result = remote.create("{}".to_owned()) => anyhow::bail!("create finished early: {result:?}"),
        _ = signals.started.notified() => {}
    }
    tokio::select! {
        result = remote.close() => anyhow::bail!("close ignored create: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    assert!(!signals.closed.load(Ordering::SeqCst));
    signals.release.notify_one();
    let error = tokio::time::timeout(Duration::from_secs(2), remote.close())
        .await?
        .err()
        .ok_or_else(|| anyhow::anyhow!("cleanup failure was hidden"))?;
    assert_eq!(error.code, "cleanup");
    assert!(!signals.closed.load(Ordering::SeqCst));
    tokio::time::timeout(Duration::from_secs(2), remote.close()).await??;
    assert!(signals.closed.load(Ordering::SeqCst));
    Ok(())
}

#[tokio::test]
async fn abandoned_read_preserves_packet_and_keeps_transport_usable() -> anyhow::Result<()> {
    let signals = Arc::new(Signals::default());
    let remote = Remote::new(Peer(Arc::clone(&signals)));
    tokio::select! {
        result = remote.read(500) => anyhow::bail!("read completed early: {result:?}"),
        _ = signals.started.notified() => {}
    }
    signals.release.notify_one();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), remote.read(500)).await??,
        Some(vec![7, 8, 9])
    );
    assert_eq!(signals.reads.load(Ordering::SeqCst), 1);
    remote.write(vec![4, 5, 6]).await?;
    assert_eq!(*signals.writes.lock().await, vec![vec![4, 5, 6]]);
    remote.close().await?;
    assert!(signals.closed.load(Ordering::SeqCst));
    Ok(())
}

#[tokio::test]
async fn cancelled_route_and_close_keep_worker_until_cleanup_is_joined() -> anyhow::Result<()> {
    let signals = Arc::new(Signals::default());
    let remote = Remote::new(Peer(Arc::clone(&signals)));
    tokio::select! {
        result = remote.routes(vec!["192.0.2.3".parse()?]) => anyhow::bail!("route finished early: {result:?}"),
        _ = signals.started.notified() => {}
    }
    tokio::select! {
        result = remote.close() => anyhow::bail!("close ignored active exchange: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    assert!(!signals.closed.load(Ordering::SeqCst));
    signals.release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), remote.close()).await??;
    remote.close().await?;
    assert!(signals.closed.load(Ordering::SeqCst));
    assert!(remote.write(vec![1]).await.is_err());
    Ok(())
}
