//! Session adapter: retain helper exchanges independently of packet-loop cancellation.
use std::{
    collections::BTreeSet,
    net::IpAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{administrator, helper, Device as LocalDevice};
use crate::platform::model::{
    validate_dynamic, validate_packet, PlatformFailure, TunInfo, ValidatedTunPlan,
};

pub(crate) enum Prepared {
    Local,
    Remote(helper::Client),
}

impl Prepared {
    pub(crate) async fn prepare() -> Result<Self, PlatformFailure> {
        if administrator()? {
            return Ok(Self::Local);
        }
        let endpoint = helper::Endpoint::new().map_err(helper_error)?;
        let client = endpoint.launch().await.map_err(helper_error)?;
        if !client.elevated() {
            return Err(failure("permission", "TUN helper 未获得管理员权限"));
        }
        Ok(Self::Remote(client))
    }

    pub(crate) async fn create(
        self,
        plan: ValidatedTunPlan,
        owner: &std::sync::Mutex<Option<Arc<Device>>>,
    ) -> Result<Arc<Device>, PlatformFailure> {
        match self {
            Self::Local => {
                let protected = plan.protected_ips.clone();
                let local = LocalDevice::create(plan).await?;
                let device = Arc::new(Device {
                    info: RwLock::new(local.info()),
                    protected,
                    backend: Backend::Local(Arc::clone(&local)),
                });
                *owner
                    .lock()
                    .map_err(|_| failure("lock", "TUN 清理状态不可用"))? =
                    Some(Arc::clone(&device));
                local.wait_ready().await?;
                Ok(device)
            }
            Self::Remote(client) => {
                #[cfg(feature = "tun-probe")]
                let process = client.probe_identity().map_err(helper_error)?;
                let mut document = serde_json::json!({
                    "name": plan.name,
                    "mtu": plan.mtu,
                    "addresses": plan.addresses.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "routes": plan.routes.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "protected_ips": plan.protected_ips,
                    "physical_dns_servers": plan.physical_dns_servers,
                    "dns_capture": plan.dns_capture,
                });
                if plan.capture_mode == crate::platform::model::CaptureMode::Full {
                    document["capture_mode"] = serde_json::json!(plan.capture_mode);
                }
                let document = document.to_string();
                let remote = Remote::new(client);
                #[cfg(feature = "tun-probe")]
                remote
                    .probe
                    .process
                    .set(process)
                    .map_err(|_| failure("identity", "helper probe identity already set"))?;
                let device = Arc::new(Device {
                    info: RwLock::new(TunInfo {
                        name: plan.name.clone(),
                        index: 0,
                        mtu: plan.mtu,
                        addresses: plan.addresses.iter().map(ToString::to_string).collect(),
                        routes: plan.routes.iter().map(ToString::to_string).collect(),
                        closed: false,
                    }),
                    protected: plan.protected_ips.clone(),
                    backend: Backend::Remote(remote),
                });
                *owner
                    .lock()
                    .map_err(|_| failure("lock", "TUN 清理状态不可用"))? =
                    Some(Arc::clone(&device));
                let created = async {
                    let Backend::Remote(remote) = &device.backend else {
                        return Err(failure("state", "TUN helper 状态无效"));
                    };
                    let document = remote.create(document).await?;
                    let info: TunInfo = serde_json::from_str(&document)
                        .map_err(|_| failure("identity", "helper TUN 信息无效"))?;
                    if info.closed
                        || info.index == 0
                        || info.name != plan.name
                        || info.mtu != plan.mtu
                        || info.addresses
                            != plan
                                .addresses
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                        || info.routes
                            != plan
                                .routes
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                    {
                        return Err(failure("identity", "helper TUN 配置与会话不一致"));
                    }
                    Ok(info)
                }
                .await;
                match created {
                    Ok(info) => {
                        *device
                            .info
                            .write()
                            .map_err(|_| failure("lock", "TUN 信息不可用"))? = info;
                        Ok(device)
                    }
                    Err(error) => Err(error),
                }
            }
        }
    }
}

enum Backend {
    Local(Arc<LocalDevice>),
    Remote(Remote),
}

pub(crate) struct Device {
    info: RwLock<TunInfo>,
    protected: Vec<IpAddr>,
    backend: Backend,
}

impl Device {
    #[cfg(feature = "tun-probe")]
    pub(crate) fn probe_status(&self) -> serde_json::Value {
        match &self.backend {
            Backend::Local(_) => serde_json::json!({"mode": "local"}),
            Backend::Remote(remote) => remote.probe.snapshot(),
        }
    }

    pub(crate) fn info(&self) -> Result<TunInfo, PlatformFailure> {
        self.info
            .read()
            .map(|info| info.clone())
            .map_err(|_| failure("lock", "TUN 信息不可用"))
    }

    pub(crate) async fn read(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        match &self.backend {
            Backend::Local(device) => device.read(wait_ms).await,
            Backend::Remote(remote) => remote.read(wait_ms).await,
        }
    }

    pub(crate) async fn write(&self, packet: &[u8]) -> Result<(), PlatformFailure> {
        validate_packet(packet, self.info()?.mtu)?;
        match &self.backend {
            Backend::Local(device) => device.write(packet).await,
            Backend::Remote(remote) => remote.write(packet.to_vec()).await,
        }
    }

    pub(crate) async fn sync_dynamic(
        &self,
        mut hosts: BTreeSet<IpAddr>,
        cancellation: &CancellationToken,
    ) -> Result<(), PlatformFailure> {
        validate_dynamic(&self.info()?, &self.protected, &mut hosts)?;
        match &self.backend {
            Backend::Local(device) => device.sync_dynamic(hosts, cancellation).await,
            Backend::Remote(remote) => {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => Err(failure("cancelled", "动态路由操作已取消")),
                    result = remote.routes(hosts.into_iter().collect()) => result,
                }
            }
        }
    }

    pub(crate) async fn close(&self) -> Result<(), PlatformFailure> {
        match &self.backend {
            Backend::Local(device) => device.close().await,
            Backend::Remote(remote) => remote.close().await,
        }
    }
}

type Reply<T> = oneshot::Sender<Result<T, PlatformFailure>>;
enum Request {
    Create(String, Reply<String>),
    Read(u32, Reply<Option<Vec<u8>>>),
    Write(Vec<u8>, Reply<()>),
    Routes(Vec<IpAddr>, Reply<()>),
    Close(Reply<()>),
}

trait Transport: Send + Sync + 'static {
    fn create(
        &self,
        plan: String,
    ) -> impl std::future::Future<Output = Result<String, PlatformFailure>> + Send;
    fn read(
        &self,
        wait_ms: u32,
    ) -> impl std::future::Future<Output = Result<Option<Vec<u8>>, PlatformFailure>> + Send;
    fn write(
        &self,
        packet: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<(), PlatformFailure>> + Send;
    fn routes(
        &self,
        hosts: Vec<IpAddr>,
    ) -> impl std::future::Future<Output = Result<(), PlatformFailure>> + Send;
    fn close(&self) -> impl std::future::Future<Output = Result<(), PlatformFailure>> + Send;
}

impl Transport for helper::Client {
    async fn create(&self, plan: String) -> Result<String, PlatformFailure> {
        self.create(plan).await
    }
    async fn read(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        self.read(wait_ms).await
    }
    async fn write(&self, packet: Vec<u8>) -> Result<(), PlatformFailure> {
        self.write(packet).await
    }
    async fn routes(&self, hosts: Vec<IpAddr>) -> Result<(), PlatformFailure> {
        self.sync_routes(hosts).await
    }
    async fn close(&self) -> Result<(), PlatformFailure> {
        self.close().await
    }
}

struct Worker {
    task: Option<JoinHandle<Result<(), PlatformFailure>>>,
    closing: Option<oneshot::Receiver<Result<(), PlatformFailure>>>,
    acknowledged: bool,
    closed: bool,
}

#[cfg(feature = "tun-probe")]
#[derive(Default)]
struct Probe {
    process: std::sync::OnceLock<(u32, u64)>,
    read_requests: std::sync::atomic::AtomicUsize,
    packets_received: std::sync::atomic::AtomicUsize,
    packet_replies_accepted: std::sync::atomic::AtomicUsize,
    packets_delivered: std::sync::atomic::AtomicUsize,
    read_errors: std::sync::atomic::AtomicUsize,
    write_requests: std::sync::atomic::AtomicUsize,
    packets_written: std::sync::atomic::AtomicUsize,
    write_errors: std::sync::atomic::AtomicUsize,
}

#[cfg(feature = "tun-probe")]
impl Probe {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": "helper",
            "process": self.process.get().map(|(pid, created)| serde_json::json!({
                "pid":pid,"created_ticks":created.to_string(),
            })),
            "readRequests": self.read_requests.load(Ordering::Relaxed),
            "packetsReceived": self.packets_received.load(Ordering::Relaxed),
            "packetRepliesAccepted": self.packet_replies_accepted.load(Ordering::Relaxed),
            "packetsDelivered": self.packets_delivered.load(Ordering::Relaxed),
            "readErrors": self.read_errors.load(Ordering::Relaxed),
            "writeRequests": self.write_requests.load(Ordering::Relaxed),
            "packetsWritten": self.packets_written.load(Ordering::Relaxed),
            "writeErrors": self.write_errors.load(Ordering::Relaxed),
        })
    }
}

struct Remote {
    requests: mpsc::Sender<Request>,
    cancellation: CancellationToken,
    stopping: AtomicBool,
    worker: Mutex<Worker>,
    #[cfg(feature = "tun-probe")]
    probe: Arc<Probe>,
}

impl Drop for Remote {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl Remote {
    fn new(transport: impl Transport) -> Self {
        let (requests, receiver) = mpsc::channel(32);
        let cancellation = CancellationToken::new();
        let token = cancellation.clone();
        #[cfg(feature = "tun-probe")]
        let probe = Arc::new(Probe::default());
        let task = tokio::spawn(run(
            transport,
            receiver,
            token,
            #[cfg(feature = "tun-probe")]
            Arc::clone(&probe),
        ));
        Self {
            requests,
            cancellation,
            stopping: AtomicBool::new(false),
            worker: Mutex::new(Worker {
                task: Some(task),
                closing: None,
                acknowledged: false,
                closed: false,
            }),
            #[cfg(feature = "tun-probe")]
            probe,
        }
    }

    async fn send(&self, request: Request) -> Result<(), PlatformFailure> {
        if self.stopping.load(Ordering::Acquire) {
            return Err(closed());
        }
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(closed()),
            result = self.requests.send(request) => result.map_err(|_| closed()),
        }
    }

    async fn create(&self, plan: String) -> Result<String, PlatformFailure> {
        let (reply, result) = oneshot::channel();
        self.send(Request::Create(plan, reply)).await?;
        result.await.map_err(|_| closed())?
    }

    async fn read(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        if !(1..=5000).contains(&wait_ms) {
            return Err(failure("timeout", "TUN 读取预算无效"));
        }
        let (reply, result) = oneshot::channel();
        // Bound head-of-line blocking for outgoing packets and route changes.
        self.send(Request::Read(wait_ms.min(20), reply)).await?;
        let packet = result.await.map_err(|_| closed())??;
        #[cfg(feature = "tun-probe")]
        if packet.is_some() {
            self.probe.packets_delivered.fetch_add(1, Ordering::Relaxed);
        }
        Ok(packet)
    }

    async fn write(&self, packet: Vec<u8>) -> Result<(), PlatformFailure> {
        let (reply, result) = oneshot::channel();
        self.send(Request::Write(packet, reply)).await?;
        result.await.map_err(|_| closed())?
    }

    async fn routes(&self, hosts: Vec<IpAddr>) -> Result<(), PlatformFailure> {
        let (reply, result) = oneshot::channel();
        self.send(Request::Routes(hosts, reply)).await?;
        result.await.map_err(|_| closed())?
    }

    async fn close(&self) -> Result<(), PlatformFailure> {
        self.stopping.store(true, Ordering::Release);
        let mut worker = self.worker.lock().await;
        if worker.closed {
            return Ok(());
        }
        if !worker.acknowledged && worker.closing.is_none() {
            let (reply, result) = oneshot::channel();
            self.requests
                .send(Request::Close(reply))
                .await
                .map_err(|_| closed())?;
            worker.closing = Some(result);
        }
        if let Some(reply) = worker.closing.as_mut() {
            let result = reply.await.map_err(|_| closed());
            worker.closing.take();
            result??;
            worker.acknowledged = true;
        }
        if let Some(task) = worker.task.as_mut() {
            task.await
                .map_err(|_| failure("worker", "TUN helper 工作线程退出失败"))??;
            worker.task.take();
        }
        worker.closed = true;
        Ok(())
    }
}

async fn run(
    transport: impl Transport,
    mut requests: mpsc::Receiver<Request>,
    cancellation: CancellationToken,
    #[cfg(feature = "tun-probe")] probe: Arc<Probe>,
) -> Result<(), PlatformFailure> {
    let mut pending = None;
    loop {
        let request = tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            request = requests.recv() => match request {
                Some(request) => request,
                None => break,
            },
        };
        // Complete each exchange even if its caller stops waiting. Dropping an
        // in-flight Client future would invalidate the named pipe.
        match request {
            Request::Create(plan, reply) if !reply.is_closed() => {
                let _ = reply.send(transport.create(plan).await);
            }
            Request::Read(wait_ms, reply) if !reply.is_closed() => {
                #[cfg(feature = "tun-probe")]
                probe.read_requests.fetch_add(1, Ordering::Relaxed);
                let result = match pending.take() {
                    Some(packet) => Ok(Some(packet)),
                    None => {
                        let result = transport.read(wait_ms).await;
                        #[cfg(feature = "tun-probe")]
                        match &result {
                            Ok(Some(_)) => {
                                probe.packets_received.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                probe.read_errors.fetch_add(1, Ordering::Relaxed);
                            }
                            _ => {}
                        }
                        result
                    }
                };
                #[cfg(feature = "tun-probe")]
                let has_packet = matches!(&result, Ok(Some(_)));
                let sent = reply.send(result);
                #[cfg(feature = "tun-probe")]
                if has_packet && sent.is_ok() {
                    probe
                        .packet_replies_accepted
                        .fetch_add(1, Ordering::Relaxed);
                }
                if let Err(Ok(Some(packet))) = sent {
                    pending = Some(packet);
                }
            }
            Request::Write(packet, reply) if !reply.is_closed() => {
                #[cfg(feature = "tun-probe")]
                probe.write_requests.fetch_add(1, Ordering::Relaxed);
                let result = transport.write(packet).await;
                #[cfg(feature = "tun-probe")]
                if result.is_ok() {
                    probe.packets_written.fetch_add(1, Ordering::Relaxed);
                } else {
                    probe.write_errors.fetch_add(1, Ordering::Relaxed);
                }
                let _ = reply.send(result);
            }
            Request::Routes(hosts, reply) if !reply.is_closed() => {
                let _ = reply.send(transport.routes(hosts).await);
            }
            Request::Close(reply) => {
                let result = transport.close().await;
                let closed = result.is_ok();
                let _ = reply.send(result);
                if closed {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
    transport.close().await
}

fn failure(code: &str, message: &str) -> PlatformFailure {
    PlatformFailure::new(code, message)
}
fn closed() -> PlatformFailure {
    failure("closed", "TUN helper 已关闭")
}
fn helper_error(error: anyhow::Error) -> PlatformFailure {
    let cancelled = error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.raw_os_error() == Some(1223))
    });
    failure(
        if cancelled { "cancelled" } else { "helper" },
        &format!("{error:#}"),
    )
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
