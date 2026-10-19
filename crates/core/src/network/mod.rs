#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

pub(crate) mod dns;
pub(crate) mod http;
#[cfg(any(target_os = "windows", target_os = "linux"))]
mod http_tunnel;
pub(crate) mod interface;
#[cfg(target_os = "linux")]
mod retirement;
mod socket;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(all(target_os = "windows", feature = "egress-probe"))]
pub(crate) mod windows_probe;
pub(crate) use socket::{connect_address, connect_host, udp_socket, Tcp, Udp};
#[cfg(target_os = "windows")]
pub(crate) fn windows_capture_snapshot(
) -> Result<interface::CaptureSnapshot, crate::connection::model::VpnFailure> {
    windows::capture_snapshot()
}

use crate::connection::model::VpnFailure;
use socket2::Socket;
use std::net::{IpAddr, SocketAddr};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock, Weak,
};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

pub(crate) struct Egress {
    pub interface: String,
    pub resolvers: Vec<SocketAddr>,
    epoch: u64,
    retired: AtomicBool,
    #[cfg(target_os = "windows")]
    adapter: windows::Interface,

    #[cfg(target_os = "linux")]
    broker: Option<Arc<crate::platform::linux_tun::helper::Broker>>,
    #[cfg(target_os = "linux")]
    operations: std::sync::atomic::AtomicUsize,
    #[cfg(target_os = "linux")]
    idle: tokio::sync::Notify,
    #[cfg(target_os = "linux")]
    cancellation: tokio_util::sync::CancellationToken,
    #[cfg(target_os = "linux")]
    closed: AtomicBool,
    #[cfg(all(test, target_os = "linux"))]
    binding_probe: Option<Arc<tests::BindingProbe>>,
}

impl Egress {
    pub(crate) fn interface_for(&self, peer: IpAddr) -> &str {
        if peer.is_loopback() {
            "lo"
        } else {
            &self.interface
        }
    }

    #[cfg(target_os = "linux")]
    fn operation(&self) -> Result<Operation<'_>, VpnFailure> {
        if self.retired.load(Ordering::SeqCst) {
            return Err(VpnFailure::new("closed", "物理出口正在停止"));
        }
        self.operations.fetch_add(1, Ordering::SeqCst);
        let operation = Operation(self);
        if self.retired.load(Ordering::SeqCst) {
            return Err(VpnFailure::new("closed", "物理出口正在停止"));
        }
        Ok(operation)
    }

    #[cfg(target_os = "linux")]
    fn retire(&self) {
        self.retired.store(true, Ordering::SeqCst);
        self.cancellation.cancel();
    }

    #[cfg(target_os = "linux")]
    async fn wait_idle(&self) {
        loop {
            let idle = self.idle.notified();
            if self.operations.load(Ordering::SeqCst) == 0 {
                return;
            }
            idle.await;
        }
    }

    #[cfg(target_os = "linux")]
    async fn bind(&self, socket: &Socket, interface: Option<&[u8]>) -> Result<(), VpnFailure> {
        #[cfg(test)]
        if let Some(probe) = &self.binding_probe {
            return probe.bind(interface).await;
        }
        match &self.broker {
            Some(broker) => {
                let interface = interface
                    .map(|name| {
                        std::str::from_utf8(name)
                            .map(str::to_owned)
                            .map_err(|_| VpnFailure::new("interface", "物理接口名称无效"))
                    })
                    .transpose()?;
                broker
                    .bind(socket, interface)
                    .await
                    .map_err(|error| VpnFailure::new(&error.code, &error.message))
            }
            None => socket.bind_device(interface).map_err(Into::into),
        }
    }
}

#[cfg(target_os = "linux")]
struct Operation<'a>(&'a Egress);
#[cfg(target_os = "linux")]
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.operations.fetch_sub(1, Ordering::SeqCst);
        self.0.idle.notify_waiters();
    }
}

#[derive(Default)]
struct Registry {
    active: Option<Arc<Egress>>,
    sockets: Vec<Weak<Record>>,
    epoch: u64,
}
impl Registry {
    fn current(&self) -> Result<Option<Arc<Egress>>, VpnFailure> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.retired.load(Ordering::Acquire))
        {
            return Err(VpnFailure::new(
                "egress",
                "物理出口清理尚未完成，不能建立未保护连接",
            ));
        }
        Ok(self.active.clone())
    }
}
static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
static GATE: OnceLock<Arc<RwLock<()>>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}
pub(crate) async fn read_gate() -> Arc<OwnedRwLockReadGuard<()>> {
    Arc::new(
        GATE.get_or_init(|| Arc::new(RwLock::new(())))
            .clone()
            .read_owned()
            .await,
    )
}
pub(crate) fn current() -> Result<Option<Arc<Egress>>, VpnFailure> {
    registry().lock().map_err(|_| failure())?.current()
}

pub(crate) struct Record {
    socket: Socket,

    peer: IpAddr,
    #[cfg(target_os = "linux")]
    previous: tokio::sync::Mutex<Option<(u64, Option<Vec<u8>>)>>,
    #[cfg(target_os = "windows")]
    previous: Mutex<Option<(u64, u32)>>,
}

impl Record {
    async fn protect(&self, egress: &Egress) -> Result<(), VpnFailure> {
        #[cfg(target_os = "linux")]
        {
            let mut previous = self.previous.lock().await;
            if egress.retired.load(Ordering::SeqCst) {
                return Err(VpnFailure::new("closed", "物理出口保护已停止"));
            }
            if previous.is_none() {
                *previous = Some((egress.epoch, self.socket.device()?));
            }
            egress
                .bind(
                    &self.socket,
                    Some(egress.interface_for(self.peer).as_bytes()),
                )
                .await?;
        }
        #[cfg(target_os = "windows")]
        {
            let mut previous = self.previous.lock().map_err(|_| failure())?;
            if egress.retired.load(Ordering::Acquire) {
                return Err(VpnFailure::new("closed", "物理出口保护已停止"));
            }
            if previous.is_none() {
                *previous = Some((egress.epoch, windows::index(&self.socket, self.peer)?));
            }
            let adapter = windows::selected(&egress.interface, self.peer, Some(egress))?;
            windows::set_index(&self.socket, self.peer, adapter.index)?;
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = egress;
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    async fn restore(&self, egress: &Egress) -> Result<(), VpnFailure> {
        let mut previous = self.previous.lock().await;
        if let Some((owner, interface)) = previous.as_ref() {
            if *owner == egress.epoch {
                // A cancelled protect may still be queued in the broker. Always
                // enqueue restoration behind it; getsockopt can observe the
                // original state before that queued mutation has been applied.
                egress.bind(&self.socket, interface.as_deref()).await?;
                *previous = None;
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    fn restore(&self, epoch: u64) -> Result<(), VpnFailure> {
        #[cfg(target_os = "windows")]
        {
            let mut previous = self.previous.lock().map_err(|_| failure())?;
            if let Some((owner, index)) = *previous {
                if owner == epoch {
                    windows::set_index(&self.socket, self.peer, index)?;
                    *previous = None;
                }
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = epoch;
        }
        Ok(())
    }
}

pub(crate) async fn register(
    socket: Socket,
    peer: IpAddr,
    original: Option<&str>,
    egress: Option<&Egress>,
) -> Result<Arc<Record>, VpnFailure> {
    #[cfg(target_os = "linux")]
    if egress.is_some_and(|egress| egress.retired.load(Ordering::Acquire)) {
        if let Some(egress) = egress {
            egress.bind(&socket, original.map(str::as_bytes)).await?;
        }
    }
    #[cfg(target_os = "windows")]
    let original_index = original
        .map(|alias| windows::selected(alias, peer, None))
        .transpose()?
        .map(|adapter| adapter.index)
        .unwrap_or(0);
    #[cfg(target_os = "windows")]
    if egress.is_some_and(|egress| egress.retired.load(Ordering::Acquire)) {
        windows::set_index(&socket, peer, original_index)?;
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = (original, egress);
    let record = Arc::new(Record {
        socket,

        peer,
        #[cfg(target_os = "linux")]
        previous: tokio::sync::Mutex::new(None),
        #[cfg(target_os = "windows")]
        previous: Mutex::new(None),
    });
    let active = {
        let mut registry = registry().lock().map_err(|_| failure())?;
        let active = registry.current()?;
        if let Some(egress) = &active {
            #[cfg(target_os = "linux")]
            {
                *record.previous.try_lock().map_err(|_| failure())? =
                    Some((egress.epoch, original.map(|name| name.as_bytes().to_vec())));
            }
            #[cfg(target_os = "windows")]
            {
                *record.previous.lock().map_err(|_| failure())? =
                    Some((egress.epoch, original_index));
            }
        }
        registry.sockets.retain(|socket| socket.strong_count() > 0);
        registry.sockets.push(Arc::downgrade(&record));
        active
    };
    if let Some(egress) = &active {
        record.protect(egress).await?;
    }
    if active.is_none() && egress.is_some_and(|egress| egress.retired.load(Ordering::Acquire)) {
        return Err(VpnFailure::new("closed", "连接完成时物理出口已停止"));
    }

    Ok(record)
}

pub(crate) struct Activation {
    _gate: OwnedRwLockWriteGuard<()>,
    pub egress: Arc<Egress>,
    lease: Arc<Lease>,
    published: bool,
    committed: bool,
}

impl Activation {
    pub(crate) fn cleanup_owner(&self) -> Arc<Lease> {
        Arc::clone(&self.lease)
    }
    #[cfg(not(target_os = "linux"))]
    pub(crate) async fn prepare(
        interface: String,
        resolvers: Option<Vec<IpAddr>>,
    ) -> Result<Self, VpnFailure> {
        Self::prepare_inner(interface, resolvers).await
    }

    #[cfg(target_os = "linux")]
    pub(crate) async fn prepare_with_broker(
        interface: String,
        resolvers: Option<Vec<IpAddr>>,
        broker: Option<Arc<crate::platform::linux_tun::helper::Broker>>,
    ) -> Result<Self, VpnFailure> {
        Self::prepare_inner(interface, resolvers, broker).await
    }

    async fn prepare_inner(
        interface: String,
        resolvers: Option<Vec<IpAddr>>,
        #[cfg(target_os = "linux")] broker: Option<Arc<crate::platform::linux_tun::helper::Broker>>,
    ) -> Result<Self, VpnFailure> {
        interface::validate(Some(&interface))?;
        #[cfg(target_os = "windows")]
        let adapter = windows::Interface::resolve(&interface)?;

        #[cfg(target_os = "windows")]
        let resolvers = match resolvers {
            Some(resolvers) => Some(resolvers),
            None => Some(adapter.dns_servers()?),
        };
        #[cfg(target_os = "linux")]
        retirement::drain().await?;
        let gate = GATE
            .get_or_init(|| Arc::new(RwLock::new(())))
            .clone()
            .write_owned()
            .await;
        let pending = registry()
            .lock()
            .map_err(|_| failure())?
            .active
            .as_ref()
            .filter(|active| active.retired.load(Ordering::Acquire))
            .cloned();
        if let Some(active) = pending {
            #[cfg(target_os = "linux")]
            retirement::close(active).await?;
            #[cfg(not(target_os = "linux"))]
            deactivate(active.epoch)?;
        }
        let mut registry = registry().lock().map_err(|_| failure())?;
        if registry.active.is_some() {
            return Err(VpnFailure::new("busy", "进程已有 TUN 物理出口保护"));
        }
        registry.epoch = registry.epoch.checked_add(1).ok_or_else(failure)?;
        let egress = Arc::new(Egress {
            interface,
            resolvers: dns::servers(resolvers)?,
            epoch: registry.epoch,
            retired: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            adapter,

            #[cfg(target_os = "linux")]
            broker,
            #[cfg(target_os = "linux")]
            operations: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(target_os = "linux")]
            idle: tokio::sync::Notify::new(),
            #[cfg(target_os = "linux")]
            cancellation: tokio_util::sync::CancellationToken::new(),
            #[cfg(target_os = "linux")]
            closed: AtomicBool::new(false),
            #[cfg(all(test, target_os = "linux"))]
            binding_probe: None,
        });
        drop(registry);
        let lease = Arc::new(Lease {
            #[cfg(not(target_os = "linux"))]
            epoch: egress.epoch,
            #[cfg(target_os = "linux")]
            egress: Arc::clone(&egress),
        });
        Ok(Self {
            _gate: gate,
            egress,
            lease,
            published: false,
            committed: false,
        })
    }

    pub(crate) async fn publish(&mut self) -> Result<(), VpnFailure> {
        let records = {
            let mut registry = registry().lock().map_err(|_| failure())?;

            registry.active = Some(Arc::clone(&self.egress));
            self.published = true;
            registry
                .sockets
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>()
        };
        for record in records {
            record.protect(&self.egress).await?;
        }
        Ok(())
    }

    pub(crate) fn commit(mut self) -> Arc<Lease> {
        self.committed = true;
        Arc::clone(&self.lease)
    }
    pub(crate) async fn rollback(&mut self) -> Result<(), VpnFailure> {
        if self.published {
            #[cfg(target_os = "linux")]
            retirement::close(Arc::clone(&self.egress)).await?;
            #[cfg(not(target_os = "linux"))]
            deactivate(self.egress.epoch)?;
            self.published = false;
        }
        Ok(())
    }
}
impl Drop for Activation {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if !self.committed {
            retirement::request(Arc::clone(&self.egress));
        }
        #[cfg(not(target_os = "linux"))]
        if self.published && !self.committed {
            let _ = deactivate(self.egress.epoch);
        }
    }
}

pub(crate) struct Lease {
    #[cfg(not(target_os = "linux"))]
    epoch: u64,
    #[cfg(target_os = "linux")]
    egress: Arc<Egress>,
}
impl Lease {
    pub(crate) async fn close(&self) -> Result<(), VpnFailure> {
        #[cfg(target_os = "linux")]
        return retirement::close(Arc::clone(&self.egress)).await;
        #[cfg(not(target_os = "linux"))]
        deactivate(self.epoch)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        retirement::request(Arc::clone(&self.egress));
        #[cfg(not(target_os = "linux"))]
        let _ = deactivate(self.epoch);
    }
}

#[cfg(not(target_os = "linux"))]
fn deactivate(epoch: u64) -> Result<(), VpnFailure> {
    let mut registry = registry().lock().map_err(|_| failure())?;
    deactivate_registry(&mut registry, epoch)
}

#[cfg(not(target_os = "linux"))]
fn deactivate_registry(registry: &mut Registry, epoch: u64) -> Result<(), VpnFailure> {
    if registry
        .active
        .as_ref()
        .is_none_or(|active| active.epoch != epoch)
    {
        return Ok(());
    }
    if let Some(active) = &registry.active {
        active.retired.store(true, Ordering::Release);
    }
    let mut failed = None;
    for record in registry.sockets.iter().filter_map(Weak::upgrade) {
        if let Err(error) = record.restore(epoch) {
            failed = Some(error);
        }
    }
    match failed {
        Some(error) => Err(error),
        None => {
            registry.active = None;
            Ok(())
        }
    }
}
fn failure() -> VpnFailure {
    VpnFailure::new("egress", "物理出口保护状态不可用")
}

pub(crate) fn status() -> Result<String, VpnFailure> {
    let registry = registry().lock().map_err(|_| failure())?;
    let active = registry.active.as_ref();
    let mut sockets = vec![];
    for record in registry.sockets.iter().filter_map(Weak::upgrade) {
        #[cfg(target_os = "linux")]
        let binding = record
            .socket
            .device()?
            .map(|name| String::from_utf8_lossy(&name).into_owned());
        #[cfg(target_os = "windows")]
        let binding = Some(windows::index(&record.socket, record.peer)?.to_string());

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let binding: Option<String> = None;
        sockets.push(serde_json::json!({"peer":record.peer,"binding":binding}));
    }
    serde_json::to_string(&serde_json::json!({
        "scope":"process",
        "active":active.is_some_and(|active| !active.retired.load(Ordering::Acquire)),
        "pending_cleanup":active.is_some_and(|active| active.retired.load(Ordering::Acquire)),
        "interface":active.map(|active| &active.interface),
        "dns_servers":active.map(|active| &active.resolvers),
        "sockets":sockets,
    }))
    .map_err(|_| VpnFailure::new("encoding", "物理出口状态编码失败"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    type BindingRequest = (
        Option<Vec<u8>>,
        tokio::sync::oneshot::Sender<Result<(), VpnFailure>>,
    );
    pub(super) struct BindingProbe {
        requests: tokio::sync::mpsc::Sender<BindingRequest>,
        restore_queued: Arc<tokio::sync::Notify>,
    }
    impl BindingProbe {
        pub(super) async fn bind(&self, interface: Option<&[u8]>) -> Result<(), VpnFailure> {
            let (reply, result) = tokio::sync::oneshot::channel();
            self.requests
                .send((interface.map(<[u8]>::to_vec), reply))
                .await
                .map_err(|_| failure())?;
            if interface.is_none() {
                self.restore_queued.notify_one();
            }
            result.await.map_err(|_| failure())?
        }
    }
    struct BindingTask(tokio::task::JoinHandle<()>);
    impl Drop for BindingTask {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    fn egress(epoch: u64) -> Arc<Egress> {
        Arc::new(Egress {
            interface: "lo".to_owned(),
            resolvers: vec![],
            epoch,
            retired: AtomicBool::new(false),
            broker: None,
            operations: std::sync::atomic::AtomicUsize::new(0),
            idle: tokio::sync::Notify::new(),
            cancellation: tokio_util::sync::CancellationToken::new(),
            closed: AtomicBool::new(false),
            binding_probe: None,
        })
    }

    #[tokio::test]
    async fn failed_restore_blocks_default_egress_until_the_same_owner_is_retried(
    ) -> anyhow::Result<()> {
        let egress = egress(71);
        let socket = Arc::new(Record {
            socket: Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, None)?,
            peer: "192.0.2.2".parse()?,
            previous: tokio::sync::Mutex::new(Some((71, Some(b"shumissing".to_vec())))),
        });
        let mut registry = Registry {
            active: Some(Arc::clone(&egress)),
            sockets: vec![Arc::downgrade(&socket)],
            epoch: 71,
        };
        assert!(retirement::deactivate_registry(&mut registry, 71)
            .await
            .is_err());
        assert!(registry.active.is_some());
        assert_eq!(
            registry.current().err().map(|error| error.code),
            Some("egress".to_owned())
        );
        // An unrelated cleanup cannot retire or clear this owner.
        retirement::deactivate_registry(&mut registry, 72).await?;
        assert!(registry.current().is_err());
        *socket.previous.lock().await = Some((71, None));
        retirement::deactivate_registry(&mut registry, 71).await?;
        assert!(registry.current()?.is_none());
        assert!(socket.previous.lock().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn retirement_waits_for_creation_and_rejects_late_protection() -> anyhow::Result<()> {
        let egress = egress(72);
        let operation = egress.operation()?;
        egress.retire();
        assert!(egress.operation().is_err());
        tokio::select! {
            biased;
            _ = egress.wait_idle() => anyhow::bail!("retirement ignored a live creation"),
            _ = tokio::task::yield_now() => {},
        }
        drop(operation);
        tokio::time::timeout(std::time::Duration::from_secs(1), egress.wait_idle()).await?;
        let socket = Record {
            socket: Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, None)?,
            peer: "127.0.0.1".parse()?,
            previous: tokio::sync::Mutex::new(None),
        };
        assert_eq!(
            socket.protect(&egress).await.err().map(|error| error.code),
            Some("closed".into())
        );
        assert!(socket.previous.lock().await.is_none());
        assert!(socket.socket.device()?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn cancelled_protect_restores_after_the_already_queued_bind() -> anyhow::Result<()> {
        let (requests, mut receiver) = tokio::sync::mpsc::channel::<BindingRequest>(4);
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let restore_queued = Arc::new(tokio::sync::Notify::new());
        let applied = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let worker_entered = Arc::clone(&entered);
        let worker_release = Arc::clone(&release);
        let worker_applied = Arc::clone(&applied);
        let _worker = BindingTask(tokio::spawn(async move {
            let mut first = true;
            while let Some((interface, reply)) = receiver.recv().await {
                if first {
                    first = false;
                    worker_entered.notify_one();
                    worker_release.notified().await;
                }
                worker_applied.lock().await.push(interface);
                let _ = reply.send(Ok(()));
            }
        }));
        let mut egress = egress(73);
        Arc::get_mut(&mut egress)
            .ok_or_else(|| anyhow::anyhow!("fixture owner shared"))?
            .binding_probe = Some(Arc::new(BindingProbe {
            requests,
            restore_queued: Arc::clone(&restore_queued),
        }));
        let record = Arc::new(Record {
            socket: Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, None)?,
            peer: "127.0.0.1".parse()?,
            previous: tokio::sync::Mutex::new(None),
        });
        let protecting_record = Arc::clone(&record);
        let protecting_egress = Arc::clone(&egress);
        let protecting =
            tokio::spawn(async move { protecting_record.protect(&protecting_egress).await });
        entered.notified().await;
        protecting.abort();
        assert!(protecting.await.is_err());
        egress.retire();
        // The real descriptor still has its original binding while the broker
        // owns an unapplied mutation. A getsockopt snapshot is insufficient.
        assert!(record.socket.device()?.is_none());
        let restoring_record = Arc::clone(&record);
        let restoring_egress = Arc::clone(&egress);
        let restoring =
            tokio::spawn(async move { restoring_record.restore(&restoring_egress).await });
        tokio::time::timeout(std::time::Duration::from_secs(1), restore_queued.notified()).await?;
        assert!(
            !restoring.is_finished(),
            "restore completed ahead of the queued protection"
        );
        release.notify_one();
        restoring.await??;
        assert_eq!(*applied.lock().await, vec![Some(b"lo".to_vec()), None]);
        assert!(record.previous.lock().await.is_none());
        Ok(())
    }
}
