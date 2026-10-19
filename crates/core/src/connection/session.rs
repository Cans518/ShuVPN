use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use sangfor_core::{l3, plan::SessionPlan};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot, watch, Mutex, Notify};
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout};
use tokio_util::sync::CancellationToken;

use super::{model::*, proxy, tunnel};

#[derive(Default)]
pub(crate) struct Traffic {
    pub upload: AtomicU64,
    pub download: AtomicU64,
    pub connections: AtomicU64,
    pub unrouted: AtomicU64,
    pub failures: AtomicU64,
    latency_us: AtomicU64,
}

impl Traffic {
    pub fn sample_latency(&self, elapsed: Duration) {
        let sample = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        let _ = self
            .latency_us
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                Some(if old == 0 {
                    sample.max(1)
                } else {
                    old.saturating_mul(3) / 4 + sample / 4
                })
            });
    }
}

#[derive(Clone)]
struct State {
    status: VpnState,
    vip: Option<String>,
    message: Option<String>,
}

pub(crate) struct Shared {
    pub plan: SessionPlan,
    pub tls: Arc<rustls::ClientConfig>,
    pub stats: Traffic,
    pub cancellation: CancellationToken,
    pub dial_timeout: Duration,
    pub routing: std::sync::RwLock<Arc<crate::routing::store::RoutingStore>>,
    pub decisions: std::sync::Mutex<std::collections::VecDeque<serde_json::Value>>,
    pub http_port: std::sync::atomic::AtomicU16,
    pub socks_port: std::sync::atomic::AtomicU16,
    pub dns: crate::routing::dns::Cache,
    pub packet_task: Mutex<Option<JoinHandle<()>>>,
    pub captures: std::sync::Mutex<Option<Arc<super::capture::Handle>>>,
    pub dns_proxy_servers: std::sync::RwLock<Vec<std::net::IpAddr>>,
    pub dns_capture_full: AtomicBool,
    pub full_capture_routes: std::sync::RwLock<Vec<ipnet::IpNet>>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    pub tun_service: Mutex<Option<super::packet::tun::Service>>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    pub tun_startup: std::sync::Mutex<Option<Arc<super::packet::tun::Startup>>>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    pub tun_cleanup: std::sync::Mutex<Option<Arc<crate::network::Lease>>>,
    pub virtual_addresses: std::sync::RwLock<Vec<std::net::IpAddr>>,
    state: watch::Sender<State>,
    events: broadcast::Sender<VpnEvent>,
    closed: AtomicBool,
    driver_failed: AtomicBool,
    explicit_stop: AtomicBool,
    cleanup_errors: std::sync::Mutex<Vec<VpnFailure>>,
    proxy_cleanup: Mutex<Option<crate::platform::system_proxy::Lease>>,
    done: Notify,
}

impl Shared {
    pub(crate) fn remember_cleanup_failure(&self, error: &VpnFailure) {
        if let Ok(mut saved) = self.cleanup_errors.lock() {
            if !saved
                .iter()
                .any(|previous| previous.code == error.code && previous.message == error.message)
            {
                saved.push(VpnFailure::new(&error.code, &error.message));
            }
        } else {
            self.driver_failed.store(true, Ordering::Release);
        }
        if error.code == "worker" {
            self.driver_failed.store(true, Ordering::Release);
            self.fail(VpnFailure::new(&error.code, &error.message));
        }
    }

    fn request_stop(&self) {
        self.explicit_stop.store(true, Ordering::Release);
        self.cancellation.cancel();
    }

    pub(crate) fn stop_requested(&self) -> bool {
        self.explicit_stop.load(Ordering::Acquire)
    }

    fn cleanup_result(&self) -> Result<(), VpnFailure> {
        let errors = self
            .cleanup_errors
            .lock()
            .map_err(|_| VpnFailure::new("cleanup", "无法读取清理结果"))?;
        if let Some(error) = errors.first() {
            return Err(VpnFailure::new(
                &error.code,
                &errors
                    .iter()
                    .map(|error| error.message.as_str())
                    .collect::<Vec<_>>()
                    .join("；"),
            ));
        }
        if self.driver_failed.load(Ordering::Acquire) {
            return Err(VpnFailure::new("worker", "网络工作线程退出失败"));
        }
        Ok(())
    }

    pub(crate) async fn wait_closed(&self) {
        loop {
            let notified = self.done.notified();
            if self.closed.load(Ordering::Acquire) || self.driver_failed.load(Ordering::Acquire) {
                break;
            }
            notified.await;
        }
    }
    pub(crate) fn snapshot(&self, kind: VpnEventKind, up_bps: f64, down_bps: f64) -> VpnEvent {
        let state = self.state.borrow().clone();
        let latency = self.stats.latency_us.load(Ordering::Relaxed);
        VpnEvent {
            kind,
            state: state.status,
            virtual_address: state.vip,
            dns_servers: self.plan.dns_servers.clone(),
            message: state.message,
            upload_bytes: self.stats.upload.load(Ordering::Relaxed),
            download_bytes: self.stats.download.load(Ordering::Relaxed),
            upload_bps: up_bps,
            download_bps: down_bps,
            latency_ms: if latency == 0 {
                None
            } else {
                Some(latency as f64 / 1000.0)
            },
            connections: self.stats.connections.load(Ordering::Relaxed),
            unrouted: self.stats.unrouted.load(Ordering::Relaxed),
            tunnel_failures: self.stats.failures.load(Ordering::Relaxed),
        }
    }

    fn publish(&self, kind: VpnEventKind, up_bps: f64, down_bps: f64) {
        let _ = self.events.send(self.snapshot(kind, up_bps, down_bps));
    }

    pub(crate) fn fail(&self, failure: VpnFailure) {
        self.state.send_modify(|state| {
            state.status = VpnState::Failed;
            state.message = Some(failure.message);
        });
        self.publish(VpnEventKind::State, 0.0, 0.0);
        self.cancellation.cancel();
    }
}

enum Command {
    ProxyStatus {
        reply: oneshot::Sender<super::desktop_services::ProxyServicesStatus>,
    },
    Start {
        kind: ProxyKind,
        address: String,
        port: u16,
        reply: oneshot::Sender<Result<ProxyBinding, VpnFailure>>,
    },
    Stop {
        kind: ProxyKind,
        reply: oneshot::Sender<Result<(), VpnFailure>>,
    },
    SystemProxy {
        directory: String,
        reply: oneshot::Sender<Result<(), VpnFailure>>,
    },
    RestoreSystemProxy {
        reply: oneshot::Sender<Result<bool, VpnFailure>>,
    },
}

struct Service {
    token: CancellationToken,
    task: JoinHandle<()>,
    binding: ProxyBinding,
}

struct DriverCompletion(Arc<Shared>);
impl Drop for DriverCompletion {
    fn drop(&mut self) {
        if !self.0.closed.load(Ordering::Acquire) {
            self.0
                .fail(VpnFailure::new("worker", "网络工作线程异常退出"));
            self.0.driver_failed.store(true, Ordering::Release);
            self.0.done.notify_waiters();
        }
    }
}

pub(crate) struct Engine {
    pub shared: Arc<Shared>,
    pub desktop_operations: Mutex<()>,
    commands: mpsc::Sender<Command>,
    driver: Mutex<Option<JoinHandle<()>>>,
    pub runtime: tokio::runtime::Handle,
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shared.cancellation.cancel();
    }
}

impl Engine {
    #[cfg(test)]
    pub(crate) async fn fixture_hold_driver_completion(
        &self,
        release: Arc<Notify>,
    ) -> Result<(), VpnFailure> {
        let mut driver = self.driver.lock().await;
        let original = driver
            .take()
            .ok_or_else(|| VpnFailure::new("fixture", "driver missing"))?;
        *driver = Some(tokio::spawn(async move {
            release.notified().await;
            assert!(original.await.is_ok(), "fixture driver failed");
        }));
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn fixture_driver_is_joining(&self) -> bool {
        self.driver.try_lock().is_err()
    }

    #[cfg(test)]
    pub(crate) async fn fixture_proxy_cleanup(
        &self,
        directory: &str,
        settings: Arc<std::sync::Mutex<BTreeMap<String, Option<String>>>>,
        failures: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Result<(), VpnFailure> {
        let mut lease =
            crate::platform::system_proxy::Lease::fixture(directory, settings, failures)?;
        lease.enable(3322)?;
        *self.shared.proxy_cleanup.lock().await = Some(lease);
        Ok(())
    }

    pub(crate) fn bind_authority(&self, authority: CancellationToken) {
        let shared = Arc::clone(&self.shared);
        if authority.is_cancelled() {
            shared.request_stop();
        }
        self.runtime.spawn(async move {
            tokio::select! {
                biased;
                _ = authority.cancelled() => shared.request_stop(),
                _ = shared.cancellation.cancelled() => {},
            }
        });
    }

    pub(crate) async fn connect(document: String, timeout_ms: u32) -> Result<Self, VpnFailure> {
        Self::connect_with_routing(
            document,
            timeout_ms,
            crate::routing::store::RoutingStore::legacy(),
        )
        .await
    }

    pub(crate) async fn connect_with_routing(
        document: String,
        timeout_ms: u32,
        routing: Arc<crate::routing::store::RoutingStore>,
    ) -> Result<Self, VpnFailure> {
        if document.len() > 1024 * 1024 || !(100..=120_000).contains(&timeout_ms) {
            return Err(VpnFailure::new("plan", "会话计划或超时参数无效"));
        }
        let plan = SessionPlan::decode(document.as_bytes())?;
        if plan.sid.is_empty()
            || plan.device_id.is_empty()
            || plan.connection_id.is_empty()
            || plan.sign_key().is_none_or(|key| key.len() != 32)
            || !plan.heartbeat_seconds.is_finite()
            || !(0.05..=3600.0).contains(&plan.heartbeat_seconds)
            || plan.routes.len() > 4096
        {
            return Err(VpnFailure::new("plan", "会话计划字段无效"));
        }
        let tls = tunnel::tls_config(&plan)?;
        let (stream, addresses, leftover) = timeout(
            Duration::from_millis(u64::from(timeout_ms)),
            tunnel::open_control(&plan, Arc::clone(&tls)),
        )
        .await
        .map_err(|_| VpnFailure::new("timeout", "建立 L3 通道超时"))??;
        let (state, _) = watch::channel(State {
            status: VpnState::Connected,
            vip: addresses.first().cloned(),
            message: None,
        });
        let (events, _) = broadcast::channel(64);
        let shared = Arc::new(Shared {
            plan,
            tls,
            stats: Traffic::default(),
            cancellation: CancellationToken::new(),
            dial_timeout: Duration::from_millis(u64::from(timeout_ms)),
            routing: std::sync::RwLock::new(routing),
            decisions: std::sync::Mutex::new(std::collections::VecDeque::new()),
            http_port: std::sync::atomic::AtomicU16::new(0),
            socks_port: std::sync::atomic::AtomicU16::new(0),
            dns: crate::routing::dns::Cache::default(),
            packet_task: Mutex::new(None),
            captures: std::sync::Mutex::new(None),
            dns_proxy_servers: std::sync::RwLock::new(vec![]),
            dns_capture_full: AtomicBool::new(false),
            full_capture_routes: std::sync::RwLock::new(Vec::new()),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            tun_service: Mutex::new(None),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            tun_startup: std::sync::Mutex::new(None),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            tun_cleanup: std::sync::Mutex::new(None),
            virtual_addresses: std::sync::RwLock::new(
                addresses
                    .iter()
                    .filter_map(|address| address.parse().ok())
                    .collect(),
            ),
            state,
            events,
            closed: AtomicBool::new(false),
            driver_failed: AtomicBool::new(false),
            explicit_stop: AtomicBool::new(false),
            cleanup_errors: std::sync::Mutex::new(vec![]),
            proxy_cleanup: Mutex::new(None),
            done: Notify::new(),
        });
        let (commands, receiver) = mpsc::channel(32);
        let driver = tokio::spawn(run_driver(Arc::clone(&shared), receiver, stream, leftover));
        Ok(Self {
            shared,
            desktop_operations: Mutex::new(()),
            commands,
            driver: Mutex::new(Some(driver)),
            runtime: tokio::runtime::Handle::current(),
        })
    }

    pub(crate) async fn start(
        &self,
        kind: ProxyKind,
        address: String,
        port: u16,
    ) -> Result<ProxyBinding, VpnFailure> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Start {
                kind,
                address,
                port,
                reply,
            })
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?;
        response
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?
    }

    pub(crate) async fn proxy_status(
        &self,
    ) -> Result<super::desktop_services::ProxyServicesStatus, VpnFailure> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::ProxyStatus { reply })
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?;
        response
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))
    }

    pub(crate) async fn stop_proxy(&self, kind: ProxyKind) -> Result<(), VpnFailure> {
        if self.shared.cancellation.is_cancelled() {
            self.shared.wait_closed().await;
            return self.shared.cleanup_result();
        }
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Stop { kind, reply })
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?;
        response
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?
    }

    pub(crate) async fn stop(&self) -> Result<(), VpnFailure> {
        self.shared.request_stop();
        let mut driver = self.driver.lock().await;
        if let Some(task) = driver.as_mut() {
            let result = task.await;
            driver.take();
            if result.is_err() {
                self.shared
                    .fail(VpnFailure::new("worker", "网络工作线程退出失败"));
                self.shared.driver_failed.store(true, Ordering::Release);
                self.shared.done.notify_waiters();
                return Err(VpnFailure::new("worker", "网络工作线程退出失败"));
            }
        } else {
            self.shared.wait_closed().await;
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let tun = super::packet::tun::stop(&self.shared).await;
            self.shared.cleanup_result()?;
            tun?;
        }
        self.shared.cleanup_result()
    }

    pub(crate) async fn finish_stop(&self) -> Result<Option<String>, VpnFailure> {
        let previous = self.stop().await.err();
        if self.shared.driver_failed.load(Ordering::Acquire)
            || previous
                .as_ref()
                .is_some_and(|error| error.code == "worker")
            || !self.shared.closed.load(Ordering::Acquire)
        {
            let message = match previous {
                Some(error) => format!("网络工作线程未证明安全结束：{}", error.message),
                None => "网络工作线程未证明安全结束".to_owned(),
            };
            return Err(VpnFailure::new("worker", &message));
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let retried = super::packet::tun::stop(&self.shared).await;
            if self.shared.tun_service.lock().await.is_some()
                || self
                    .shared
                    .tun_startup
                    .lock()
                    .map_err(|_| VpnFailure::new("worker", "TUN 启动状态不可用"))?
                    .is_some()
                || self
                    .shared
                    .tun_cleanup
                    .lock()
                    .map_err(|_| VpnFailure::new("egress", "出口清理状态不可用"))?
                    .is_some()
            {
                return Err(self
                    .shared
                    .cleanup_result()
                    .err()
                    .or_else(|| retried.err())
                    .unwrap_or_else(|| VpnFailure::new("cleanup", "TUN 清理尚未完成")));
            }
            if retried.as_ref().is_err_and(|error| error.code == "worker") {
                return Err(VpnFailure::new("worker", "TUN 工作线程未证明安全结束"));
            }
        }
        let mut proxy = self.shared.proxy_cleanup.lock().await;
        if let Some(lease) = proxy.as_mut() {
            if let Err(error) = lease.restore() {
                self.shared.remember_cleanup_failure(&error);
                return Err(self.shared.cleanup_result().err().unwrap_or(error));
            }
            proxy.take();
        }
        if self.shared.packet_task.lock().await.is_some() {
            return Err(VpnFailure::new("worker", "包工作线程尚未结束"));
        }
        Ok(self
            .shared
            .cleanup_result()
            .err()
            .or(previous)
            .map(|error| error.message))
    }

    pub(crate) async fn system_proxy(&self, directory: String) -> Result<(), VpnFailure> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::SystemProxy { directory, reply })
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?;
        response
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?
    }

    pub(crate) async fn restore_system_proxy(&self) -> Result<bool, VpnFailure> {
        if self.shared.cancellation.is_cancelled() {
            self.shared.wait_closed().await;
            self.shared.cleanup_result()?;
            return Ok(false);
        }
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::RestoreSystemProxy { reply })
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?;
        response
            .await
            .map_err(|_| VpnFailure::new("closed", "会话已断开"))?
    }

    pub(crate) fn events(&self) -> broadcast::Receiver<VpnEvent> {
        self.shared.events.subscribe()
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::Acquire)
    }
}

async fn run_driver(
    shared: Arc<Shared>,
    mut receiver: mpsc::Receiver<Command>,
    stream: tunnel::TunnelIo,
    leftover: Vec<u8>,
) {
    let _completion = DriverCompletion(Arc::clone(&shared));
    let control_shared = Arc::clone(&shared);
    let control = tokio::spawn(async move {
        let result = tokio::select! {
            _ = control_shared.cancellation.cancelled() => Ok(()),
            result = control_loop(&control_shared, stream, leftover) => result,
        };
        if let Err(failure) = result {
            control_shared.fail(failure);
        }
    });
    let telemetry_shared = Arc::clone(&shared);
    let telemetry = tokio::spawn(async move {
        let mut timer = interval(Duration::from_secs(1));
        timer.tick().await;
        let mut previous_up = 0;
        let mut previous_down = 0;
        let mut sampled_at = Instant::now();
        loop {
            tokio::select! {
                _ = telemetry_shared.cancellation.cancelled() => break,
                _ = timer.tick() => {
                    let up = telemetry_shared.stats.upload.load(Ordering::Relaxed);
                    let down = telemetry_shared.stats.download.load(Ordering::Relaxed);
                    let seconds = sampled_at.elapsed().as_secs_f64().max(0.001);
                    telemetry_shared.publish(VpnEventKind::Traffic,
                        up.saturating_sub(previous_up) as f64 / seconds,
                        down.saturating_sub(previous_down) as f64 / seconds);
                    previous_up = up;
                    previous_down = down;
                    sampled_at = Instant::now();
                }
            }
        }
    });
    let mut services: BTreeMap<ProxyKind, Service> = BTreeMap::new();
    let mut system_proxy: Option<crate::platform::system_proxy::Lease> = None;
    loop {
        tokio::select! {
            biased;
            _ = shared.cancellation.cancelled() => break,
            command = receiver.recv() => {
                match command {
                    Some(Command::ProxyStatus { reply }) => {
                        let enabled = match &system_proxy {
                            Some(lease) => lease.is_enabled(),
                            None => Ok(false),
                        };
                        let (system_proxy_enabled, system_proxy_error) = match enabled {
                            Ok(enabled) => (enabled, None),
                            Err(error) => (false, Some(error.message)),
                        };
                        let _ = reply.send(super::desktop_services::ProxyServicesStatus {
                            http: services.get(&ProxyKind::Http).map(|service| service.binding.clone()),
                            socks: services.get(&ProxyKind::Socks).map(|service| service.binding.clone()),
                            system_proxy_enabled,
                            system_proxy_error,
                        });
                    }
                    Some(Command::Start { kind, address, port, reply }) => {
                        if reply.is_closed() { continue; }
                        let existed = services.contains_key(&kind);
                        let result = if let Some(service) = services.get(&kind) {
                            Ok(service.binding.clone())
                        } else {
                            start_service(Arc::clone(&shared), kind, address, port).await.map(|service| {
                                let binding = service.binding.clone();
                                match kind { ProxyKind::Http => &shared.http_port, ProxyKind::Socks => &shared.socks_port }
                                    .store(binding.port, Ordering::Release);
                                services.insert(kind, service);
                                binding
                            })
                        };
                        if reply.send(result).is_err() && !existed {
                            if let Some(service) = services.remove(&kind) {
                                match kind { ProxyKind::Http => &shared.http_port, ProxyKind::Socks => &shared.socks_port }
                                    .store(0, Ordering::Release);
                                service.token.cancel();
                                if service.task.await.is_err() {
                                    shared.driver_failed.store(true, Ordering::Release);
                                    shared.fail(VpnFailure::new("worker", "代理工作线程退出失败"));
                                }
                            }
                        }
                    }
                    Some(Command::Stop { kind, reply }) => {
                        if kind == ProxyKind::Http {
                            if let Some(lease) = &mut system_proxy {
                                if let Err(error) = lease.restore() {
                                    let _ = reply.send(Err(error));
                                    continue;
                                }
                            }
                            system_proxy = None;
                        }
                        if let Some(service) = services.remove(&kind) {
                            match kind { ProxyKind::Http => &shared.http_port, ProxyKind::Socks => &shared.socks_port }
                                .store(0, Ordering::Release);
                            service.token.cancel();
                            if service.task.await.is_err() {
                                shared.driver_failed.store(true, Ordering::Release);
                                shared.fail(VpnFailure::new("worker", "代理工作线程退出失败"));
                                let _ = reply.send(Err(VpnFailure::new("worker", "代理工作线程退出失败")));
                                continue;
                            }
                        }
                        let _ = reply.send(Ok(()));
                    }
                    Some(Command::SystemProxy { directory, reply }) => {
                        if reply.is_closed() { continue; }
                        if let Some(lease) = &mut system_proxy {
                            match lease.is_enabled() {
                                Ok(true) => { let _ = reply.send(Ok(())); continue; }
                                Ok(false) => {}
                                Err(error) => { let _ = reply.send(Err(error)); continue; }
                            }
                            if let Err(error) = lease.restore() {
                                let _ = reply.send(Err(error));
                                continue;
                            }
                        }
                        system_proxy = None;
                        let Some(service) = services.get(&ProxyKind::Http) else {
                            let _ = reply.send(Err(VpnFailure::new("systemProxy", "系统代理需要已启动的 HTTP 监听")));
                            continue;
                        };
                        if !matches!(service.binding.address.as_str(), "127.0.0.1" | "0.0.0.0") {
                            let _ = reply.send(Err(VpnFailure::new("systemProxy", "系统代理需要可访问的 IPv4 本机监听")));
                            continue;
                        }
                        let port = service.binding.port;
                        let prepared = tokio::task::spawn_blocking(move || {
                            let mut lease = crate::platform::system_proxy::Lease::open(&directory)?;
                            let result = lease.enable(port);
                            Ok::<_, VpnFailure>((lease, result))
                        }).await;
                        let result = match prepared {
                            Ok(Ok((lease, result))) => { system_proxy = Some(lease); result }
                            Ok(Err(error)) => Err(error),
                            Err(_) => Err(VpnFailure::new("systemProxy", "系统代理配置线程退出失败")),
                        };
                        if reply.send(result).is_err() {
                            let restored = match &mut system_proxy {
                                Some(lease) => lease.restore(),
                                None => Ok(false),
                            };
                            match restored {
                                Ok(_) => system_proxy = None,
                                // Retain the lease until shutdown retries it and
                                // records any cleanup failure for stop callers.
                                Err(error) => shared.fail(error),
                            }
                        }
                    }
                    Some(Command::RestoreSystemProxy { reply }) => {
                        let result = match &mut system_proxy {
                            Some(lease) => lease.restore(),
                            None => Ok(false),
                        };
                        if result.is_ok() { system_proxy = None; }
                        let _ = reply.send(result);
                    }
                    None => { shared.cancellation.cancel(); break; }
                }
            }
        }
    }
    let mut proxy_cleanup_failed = false;
    if let Some(lease) = &mut system_proxy {
        if let Err(error) = lease.restore() {
            proxy_cleanup_failed = true;
            shared.remember_cleanup_failure(&error);
            shared.fail(error);
        }
    }
    if proxy_cleanup_failed {
        *shared.proxy_cleanup.lock().await = system_proxy.take();
    }
    drop(system_proxy);
    for (_, service) in services {
        service.token.cancel();
        if service.task.await.is_err() {
            shared.driver_failed.store(true, Ordering::Release);
            shared.fail(VpnFailure::new("worker", "代理工作线程退出失败"));
        }
    }
    shared.http_port.store(0, Ordering::Release);
    shared.socks_port.store(0, Ordering::Release);

    if control.await.is_err() {
        shared.driver_failed.store(true, Ordering::Release);
        shared.fail(VpnFailure::new("worker", "控制工作线程退出失败"));
    }

    if telemetry.await.is_err() {
        shared.driver_failed.store(true, Ordering::Release);
        shared.fail(VpnFailure::new("worker", "统计工作线程退出失败"));
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let tun_cleanup_failed = match super::packet::tun::stop(&shared).await {
        Ok(()) => false,
        Err(error) => {
            shared.remember_cleanup_failure(&error);
            shared.fail(error);
            true
        }
    };
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let tun_cleanup_failed = false;

    if let Some(task) = shared.packet_task.lock().await.take() {
        if task.await.is_err() {
            shared.driver_failed.store(true, Ordering::Release);
            shared.fail(VpnFailure::new("worker", "包数据面工作线程退出失败"));
        }
    }
    if let Err(error) = shared.dns.clear() {
        shared.fail(VpnFailure::new(&error.code, &error.message));
    }
    let cleanup_failed = shared.driver_failed.load(Ordering::Acquire)
        || tun_cleanup_failed
        || shared
            .cleanup_errors
            .lock()
            .map(|errors| !errors.is_empty())
            .unwrap_or(true);
    if !cleanup_failed
        && (shared.explicit_stop.load(Ordering::Acquire)
            || shared.state.borrow().status != VpnState::Failed)
    {
        shared.state.send_modify(|state| {
            state.status = VpnState::Disconnected;
            state.message = None;
        });
    }
    shared.publish(VpnEventKind::State, 0.0, 0.0);
    shared.closed.store(true, Ordering::Release);

    shared.done.notify_waiters();
}

async fn start_service(
    shared: Arc<Shared>,
    kind: ProxyKind,
    address: String,
    port: u16,
) -> Result<Service, VpnFailure> {
    if shared.cancellation.is_cancelled() {
        return Err(VpnFailure::new("closed", "会话已断开"));
    }
    let address: std::net::IpAddr = address
        .parse()
        .map_err(|_| VpnFailure::new("listen", "监听地址无效"))?;
    let listener = match TcpListener::bind((address, port)).await {
        Ok(listener) => listener,
        Err(error) if port != 0 && error.kind() == std::io::ErrorKind::AddrInUse => {
            TcpListener::bind((address, 0)).await?
        }
        Err(error) => return Err(error.into()),
    };
    let bound = listener.local_addr()?;
    let (display_endpoint, display_warning) =
        crate::network::interface::proxy_endpoint(bound).await;
    if shared.cancellation.is_cancelled() {
        return Err(VpnFailure::new("closed", "会话已断开"));
    }
    let token = shared.cancellation.child_token();
    let task = tokio::spawn(proxy::serve(listener, kind, shared, token.clone()));
    Ok(Service {
        token,
        task,
        binding: ProxyBinding {
            address: bound.ip().to_string(),
            port: bound.port(),
            display_endpoint,
            display_warning,
        },
    })
}

async fn control_loop(
    shared: &Shared,
    mut stream: tunnel::TunnelIo,
    leftover: Vec<u8>,
) -> Result<(), VpnFailure> {
    let mut decoder = l3::FrameDecoder::new();
    let mut timer = interval(Duration::from_secs_f64(shared.plan.heartbeat_seconds));
    // After scheduler delays, don't count a burst of overdue ticks as three
    // independent failed network probes before reading queued responses.
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    timer.tick().await;
    let mut misses = 0;
    handle_control_frames(shared, decoder.push(&leftover)?, &mut misses);
    let mut chunk = [0; 16 * 1024];
    loop {
        tokio::select! {
            read = stream.read(&mut chunk) => {
                let read = read?;
                if read == 0 { return Err(VpnFailure::new("closed", "L3 通道已关闭")); }
                handle_control_frames(shared, decoder.push(&chunk[..read])?, &mut misses);
            }
            _ = timer.tick() => {
                if misses >= 3 { return Err(VpnFailure::new("heartbeat", "L3 节点未回应心跳")); }
                stream.write_all(&l3::heartbeat_request()).await?;
                stream.flush().await?;
                misses += 1;
            }
        }
    }
}

fn handle_control_frames(shared: &Shared, frames: Vec<l3::Frame>, misses: &mut u32) {
    for frame in frames {
        if frame.command == l3::Command::HeartbeatResponse {
            *misses = 0;
        }
        if frame.command == l3::Command::SecondVipResponse {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&frame.payload) {
                let vip = value["vip"]
                    .as_str()
                    .or_else(|| value["data"]["vip"].as_str());
                if let Some(vip) = vip.filter(|vip| vip.parse::<std::net::IpAddr>().is_ok()) {
                    if let (Ok(address), Ok(mut addresses)) =
                        (vip.parse(), shared.virtual_addresses.write())
                    {
                        if !addresses.contains(&address) {
                            addresses.push(address);
                        }
                    }
                    shared
                        .state
                        .send_modify(|state| state.vip = Some(vip.to_owned()));
                    shared.publish(VpnEventKind::State, 0.0, 0.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_gateway as gateway;

    fn with_driver(engine: &Engine, task: JoinHandle<()>) -> Engine {
        Engine {
            shared: Arc::clone(&engine.shared),
            desktop_operations: Mutex::new(()),
            commands: engine.commands.clone(),
            driver: Mutex::new(Some(task)),
            runtime: engine.runtime.clone(),
        }
    }

    #[tokio::test]
    async fn finish_preserves_the_worker_cleanup_cause() -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = Engine::connect(gateway.plan(), 1000).await?;
        engine
            .shared
            .remember_cleanup_failure(&VpnFailure::new("worker", "specific worker cleanup cause"));
        for _ in 0..2 {
            let failure = engine
                .finish_stop()
                .await
                .err()
                .ok_or_else(|| anyhow::anyhow!("unsafe worker cleanup was accepted"))?;
            assert_eq!(failure.code, "worker");
            assert!(
                failure.message.contains("specific worker cleanup cause"),
                "cleanup cause was replaced: {}",
                failure.message
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn finish_retries_the_owned_proxy_lease_and_preserves_failure_diagnostics(
    ) -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = Engine::connect(gateway.plan(), 1000).await?;
        engine.stop().await?;
        let directory = tempfile::tempdir()?;
        let original = BTreeMap::from([
            ("mode".to_owned(), Some("auto".to_owned())),
            ("server".to_owned(), None),
        ]);
        let settings = Arc::new(std::sync::Mutex::new(original.clone()));
        let failures = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut lease = crate::platform::system_proxy::Lease::fixture(
            &directory.path().to_string_lossy(),
            Arc::clone(&settings),
            Arc::clone(&failures),
        )?;
        lease.enable(3322)?;
        *engine.shared.proxy_cleanup.lock().await = Some(lease);
        engine.shared.remember_cleanup_failure(&VpnFailure::new(
            "systemProxy",
            "retained restoration diagnostic",
        ));
        failures.store(1, Ordering::Release);
        assert!(engine.finish_stop().await.is_err());
        assert!(engine.shared.proxy_cleanup.lock().await.is_some());
        let diagnostic = engine
            .finish_stop()
            .await?
            .ok_or_else(|| anyhow::anyhow!("restoration diagnostic disappeared"))?;
        assert!(diagnostic.contains("retained restoration diagnostic"));
        assert!(diagnostic.contains("注入部分写入失败"));
        assert!(engine.shared.proxy_cleanup.lock().await.is_none());
        assert_eq!(
            *settings
                .lock()
                .map_err(|_| anyhow::anyhow!("settings lock"))?,
            original
        );
        assert!(engine.stop().await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn cancelled_proxy_operations_report_the_recorded_cleanup_failure() -> anyhow::Result<()>
    {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = Engine::connect(gateway.plan(), 1000).await?;
        engine.stop().await?;
        engine.shared.remember_cleanup_failure(&VpnFailure::new(
            "systemProxy",
            "fixture restoration failed",
        ));
        assert_eq!(
            engine
                .stop_proxy(ProxyKind::Http)
                .await
                .err()
                .map(|error| error.code),
            Some("systemProxy".to_owned())
        );
        assert_eq!(
            engine
                .restore_system_proxy()
                .await
                .err()
                .map(|error| error.code),
            Some("systemProxy".to_owned())
        );
        assert_eq!(
            engine.stop().await.err().map(|error| error.code),
            Some("systemProxy".to_owned())
        );
        Ok(())
    }

    #[tokio::test]
    async fn cancelling_stop_does_not_detach_the_registered_driver() -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = Engine::connect(gateway.plan(), 1000).await?;
        let release = Arc::new(Notify::new());
        let finished = Arc::new(AtomicBool::new(false));
        let worker_release = Arc::clone(&release);
        let worker_finished = Arc::clone(&finished);
        let shadow = with_driver(
            &engine,
            tokio::spawn(async move {
                worker_release.notified().await;
                worker_finished.store(true, Ordering::Release);
            }),
        );
        {
            let stopping = shadow.stop();
            tokio::pin!(stopping);
            tokio::select! {
                biased;
                _ = &mut stopping => anyhow::bail!("driver unexpectedly finished"),
                _ = tokio::task::yield_now() => {},
            }
        }
        assert!(shadow.driver.lock().await.is_some());
        let release_later = async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            release.notify_one();
        };
        let (stopped, _) = timeout(Duration::from_secs(2), async {
            tokio::join!(shadow.stop(), release_later)
        })
        .await?;
        stopped?;
        assert!(finished.load(Ordering::Acquire));
        engine.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn a_failed_driver_returns_a_repeatable_error_without_waiting_forever(
    ) -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = Engine::connect(gateway.plan(), 1000).await?;
        let shadow = with_driver(
            &engine,
            tokio::spawn(async {
                panic!("simulated driver failure");
            }),
        );
        for _ in 0..2 {
            let result = timeout(Duration::from_secs(2), shadow.stop()).await?;
            assert_eq!(
                result.err().map(|error| error.code),
                Some("worker".to_owned())
            );
        }
        // The real driver still joins its registered services despite the
        // recorded failure, and reports that failure to subsequent callers.
        assert!(engine.stop().await.is_err());
        assert_eq!(
            engine
                .stop_proxy(ProxyKind::Http)
                .await
                .err()
                .map(|error| error.code),
            Some("worker".to_owned())
        );
        assert!(engine.shared.closed.load(Ordering::Acquire));
        Ok(())
    }
}
