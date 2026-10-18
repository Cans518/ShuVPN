pub use crate::connection::desktop_services::{
    CaptureMode, DesktopServicesReport, ProxyStartup, TunOperation, TunStartup,
};
pub use crate::connection::model::{
    ProxyBinding, ProxyKind, VpnEvent, VpnEventKind, VpnFailure, VpnState,
};
use crate::{connection::session::Engine, frb_generated::StreamSink};

pub struct SessionIdentity {
    pub connection_id: String,
    pub sign_key: Vec<u8>,
}

#[flutter_rust_bridge::frb(sync)]
pub fn session_identity(device_id: String) -> Result<SessionIdentity, VpnFailure> {
    let identity = crate::connection::identity::new(&device_id)?;
    Ok(SessionIdentity {
        connection_id: identity.connection_id,
        sign_key: identity.sign_key,
    })
}

#[flutter_rust_bridge::frb(opaque)]
pub struct VpnSession {
    engine: std::sync::Arc<Engine>,
    run_id: u64,
    preparation_warnings: Vec<String>,
    startup: Option<DesktopServicesReport>,
}

#[flutter_rust_bridge::frb(opaque)]
pub struct VpnConnectAttempt {
    cancellation: tokio_util::sync::CancellationToken,
    owns_cancellation: bool,
}

impl Default for VpnConnectAttempt {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for VpnConnectAttempt {
    fn drop(&mut self) {
        if self.owns_cancellation {
            self.cancellation.cancel();
        }
    }
}

impl VpnConnectAttempt {
    #[flutter_rust_bridge::frb(ignore)]
    pub(crate) fn for_owner(cancellation: tokio_util::sync::CancellationToken) -> Self {
        Self {
            cancellation,
            owns_cancellation: false,
        }
    }
    /// Authentication and tunnel establishment share one native deadline.
    pub async fn connect_account(
        &self,
        account: &crate::api::account::AccountSession,
        gateway_origin: String,
        strict_anti_mitm: bool,
        timeout_ms: u32,
    ) -> Result<VpnSession, VpnFailure> {
        self.connect_account_using_routing(
            account,
            gateway_origin,
            strict_anti_mitm,
            timeout_ms,
            crate::routing::store::RoutingStore::legacy(),
        )
        .await
    }

    /// Recovery finishes before authentication; routing is installed before any
    /// session worker or listener is published to the caller.
    pub async fn connect_desktop_account(
        &self,
        account: &crate::api::account::AccountSession,
        gateway_origin: String,
        strict_anti_mitm: bool,
        timeout_ms: u32,
        routing: &crate::api::routing::RoutingManager,
        recovery_directory: Option<String>,
    ) -> Result<VpnSession, VpnFailure> {
        if !(100..=120_000).contains(&timeout_ms) {
            return Err(VpnFailure::new("plan", "会话超时参数无效"));
        }
        let gateway_origin = crate::connection::gateway::normalize(&gateway_origin)?;
        if self.cancellation.is_cancelled() {
            return Err(VpnFailure::new("cancelled", "连接已取消"));
        }
        // Do not detach a blocking OS recovery transaction on cancellation.
        if let Some(directory) = recovery_directory {
            crate::api::system_proxy::recover_system_proxy(directory).await?;
        }
        self.connect_account_using_routing(
            account,
            gateway_origin,
            strict_anti_mitm,
            timeout_ms,
            std::sync::Arc::clone(&routing.store),
        )
        .await
    }

    #[flutter_rust_bridge::frb(ignore)]
    async fn connect_account_using_routing(
        &self,
        account: &crate::api::account::AccountSession,
        gateway_origin: String,
        strict_anti_mitm: bool,
        timeout_ms: u32,
        routing: std::sync::Arc<crate::routing::store::RoutingStore>,
    ) -> Result<VpnSession, VpnFailure> {
        self.with_deadline(timeout_ms, async {
            let gateway_origin = crate::connection::gateway::normalize(&gateway_origin)?;
            let prepared = self
                .prepare_account(account, gateway_origin, strict_anti_mitm)
                .await?;
            let mut session = self
                .connect_prepared_using_routing(&prepared, timeout_ms, routing)
                .await?;
            session.preparation_warnings = prepared.warnings();
            Ok(session)
        })
        .await
    }

    #[flutter_rust_bridge::frb(ignore)]
    async fn with_deadline<T>(
        &self,
        timeout_ms: u32,
        operation: impl std::future::Future<Output = Result<T, VpnFailure>>,
    ) -> Result<T, VpnFailure> {
        if !(100..=120_000).contains(&timeout_ms) {
            return Err(VpnFailure::new("plan", "会话超时参数无效"));
        }
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(VpnFailure::new("cancelled", "连接已取消")),
            result = tokio::time::timeout(
                std::time::Duration::from_millis(u64::from(timeout_ms)),
                operation,
            ) => match result {
                Ok(result) => result,
                Err(_) => {
                    self.cancellation.cancel();
                    Err(VpnFailure::new("timeout", "认证或建立校园连接超时"))
                }
            },
        }
    }

    pub async fn prepare_account(
        &self,
        account: &crate::api::account::AccountSession,
        gateway_origin: String,
        strict_anti_mitm: bool,
    ) -> Result<crate::api::newsso::PreparedATrustTunnel, VpnFailure> {
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(VpnFailure::new("cancelled", "连接已取消")),
            result = account.inner.prepare_for_origin(gateway_origin, strict_anti_mitm) => result
                .map(|inner| crate::api::newsso::PreparedATrustTunnel { inner })
                .map_err(|error| VpnFailure::new(&error.code, &error.message)),
        }
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn new() -> Self {
        Self {
            cancellation: tokio_util::sync::CancellationToken::new(),
            owns_cancellation: true,
        }
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub async fn connect(
        &self,
        plan_json: String,
        timeout_ms: u32,
    ) -> Result<VpnSession, VpnFailure> {
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(VpnFailure::new("cancelled", "连接已取消")),
            result = VpnSession::connect(plan_json, timeout_ms) => result,
        }
    }

    pub async fn connect_prepared(
        &self,
        prepared: &crate::api::newsso::PreparedATrustTunnel,
        timeout_ms: u32,
    ) -> Result<VpnSession, VpnFailure> {
        self.connect_prepared_using_routing(
            prepared,
            timeout_ms,
            crate::routing::store::RoutingStore::legacy(),
        )
        .await
    }

    #[flutter_rust_bridge::frb(ignore)]
    async fn connect_prepared_using_routing(
        &self,
        prepared: &crate::api::newsso::PreparedATrustTunnel,
        timeout_ms: u32,
        routing: std::sync::Arc<crate::routing::store::RoutingStore>,
    ) -> Result<VpnSession, VpnFailure> {
        if !(100..=120_000).contains(&timeout_ms) {
            return Err(VpnFailure::new("plan", "会话超时参数无效"));
        }
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(VpnFailure::new("cancelled", "连接已取消")),
            _ = prepared.inner.validity().cancelled() => Err(VpnFailure::new("cancelled", "账号会话已改变")),
            result = async {
                let plan = prepared.inner.take().map_err(|failure| VpnFailure::new(&failure.code, &failure.message))?;
                let session = VpnSession::connect_using_routing(plan, timeout_ms, routing).await?;
                session.engine.bind_authority(prepared.inner.validity().clone());
                Ok(session)
            } => result,
        }
    }
}

impl VpnSession {
    #[flutter_rust_bridge::frb(ignore)]
    pub(crate) fn into_owned(self) -> crate::connection::owner::Session {
        crate::connection::owner::Session {
            engine: self.engine,
            run_id: self.run_id,
            warnings: self.preparation_warnings,
            startup: self.startup,
        }
    }
    #[flutter_rust_bridge::frb(ignore)]
    pub(crate) fn from_owned(session: crate::connection::owner::Session) -> Self {
        Self {
            engine: session.engine,
            run_id: session.run_id,
            preparation_warnings: session.warnings,
            startup: session.startup,
        }
    }
    pub fn startup_services(&self) -> Option<DesktopServicesReport> {
        self.startup.clone()
    }

    pub async fn start_desktop_services(
        &self,
        proxies: ProxyStartup,
        tun: Option<TunStartup>,
        system_proxy_directory: Option<String>,
    ) -> Result<DesktopServicesReport, VpnFailure> {
        crate::connection::desktop_services::start(
            &self.engine,
            proxies,
            tun,
            system_proxy_directory,
        )
        .await
    }

    pub async fn start_tun_service(&self, request: TunStartup) -> Result<TunOperation, VpnFailure> {
        crate::connection::desktop_tun::start_service(&self.engine, request).await
    }

    pub async fn stop_tun_service(&self) -> Result<TunOperation, VpnFailure> {
        crate::connection::desktop_tun::stop_service(&self.engine).await
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn preparation_warnings(&self) -> Vec<String> {
        self.preparation_warnings.clone()
    }

    /// Stable identity of this native engine instance, independent of interface replacement.
    pub fn run_id(&self) -> u64 {
        self.run_id
    }
    pub async fn enable_system_proxy(&self, directory: String) -> Result<(), VpnFailure> {
        let _operation = self.engine.desktop_operations.lock().await;
        self.engine.system_proxy(directory).await
    }

    pub async fn restore_system_proxy(&self) -> Result<bool, VpnFailure> {
        let _operation = self.engine.desktop_operations.lock().await;
        self.engine.restore_system_proxy().await
    }

    pub async fn resolve_campus_dns(&self, host: String) -> Result<String, VpnFailure> {
        let resolution =
            crate::connection::tunnel::resolve_campus(&self.engine.shared, &host, None).await?;
        serde_json::to_string(&resolution)
            .map_err(|_| VpnFailure::new("encoding", "DNS 解析结果编码失败"))
    }
    pub async fn capture_status(&self) -> Result<String, VpnFailure> {
        crate::connection::capture::status(&self.engine.shared).await
    }
    /// Physical protection is shared by native requests in this process.
    pub fn egress_status(&self) -> Result<String, VpnFailure> {
        crate::network::status()
    }

    pub async fn start_tun(
        &self,
        plan_json: String,
        physical_interface: String,
    ) -> Result<String, VpnFailure> {
        let _operation = self.engine.desktop_operations.lock().await;
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            crate::connection::packet::tun::start(
                std::sync::Arc::clone(&self.engine.shared),
                plan_json,
                physical_interface,
            )
            .await
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = (plan_json, physical_interface);
            Err(VpnFailure::new("platform", "此平台尚未接入 TUN 数据面"))
        }
    }

    /// Preview contains only addresses and capture configuration, never credentials.
    pub async fn desktop_tun_plan(
        &self,
        mtu: u32,
        physical_dns: Option<String>,
        capture_mode: Option<crate::platform::model::CaptureMode>,
        physical_interface: Option<String>,
    ) -> Result<String, VpnFailure> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            crate::connection::desktop_tun::plan_for_mode(
                &self.engine.shared,
                mtu,
                physical_dns.as_deref(),
                capture_mode.unwrap_or_default(),
                physical_interface.as_deref().unwrap_or(""),
            )
            .await
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = (mtu, physical_dns, capture_mode, physical_interface);
            Err(VpnFailure::new("platform", "当前平台没有桌面 TUN 入口"))
        }
    }

    pub async fn start_desktop_tun(
        &self,
        mtu: u32,
        physical_interface: String,
        physical_dns: Option<String>,
        capture_mode: Option<crate::platform::model::CaptureMode>,
    ) -> Result<String, VpnFailure> {
        let _operation = self.engine.desktop_operations.lock().await;
        crate::connection::desktop_tun::start(
            std::sync::Arc::clone(&self.engine.shared),
            mtu,
            physical_interface,
            physical_dns,
            capture_mode,
        )
        .await
    }

    pub async fn stop_tun(&self) -> Result<(), VpnFailure> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            crate::connection::packet::tun::stop(&self.engine.shared).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Ok(())
        }
    }

    /// Acknowledges completed cleanup separately from the retained stop diagnostic.
    pub async fn finish_stop(&self) -> Result<Option<String>, VpnFailure> {
        self.engine.finish_stop().await
    }

    pub async fn tun_status(&self) -> Result<String, VpnFailure> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            crate::connection::packet::tun::status(&self.engine.shared).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err(VpnFailure::new("platform", "当前平台没有 TUN 状态入口"))
        }
    }

    /// Creates the raw IP ingress used by platform TUN adapters. Current
    /// capability is unfragmented TCP/UDP; unsupported protocols fail explicitly.
    pub async fn open_packet_port(
        &self,
        physical_interface: Option<String>,
    ) -> Result<VpnPacketPort, VpnFailure> {
        Ok(VpnPacketPort {
            port: crate::connection::packet::Port::open(
                std::sync::Arc::clone(&self.engine.shared),
                physical_interface,
            )
            .await?,
        })
    }
    pub fn attach_routing(
        &self,
        manager: &crate::api::routing::RoutingManager,
    ) -> Result<(), VpnFailure> {
        let mut routing = self
            .engine
            .shared
            .routing
            .write()
            .map_err(|_| VpnFailure::new("routing", "无法切换规则管理器"))?;
        *routing = std::sync::Arc::clone(&manager.store);
        self.engine
            .shared
            .dns
            .clear()
            .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
        Ok(())
    }

    pub fn decisions(&self) -> Result<String, VpnFailure> {
        let decisions = self
            .engine
            .shared
            .decisions
            .lock()
            .map_err(|_| VpnFailure::new("routing", "无法读取规则命中"))?;
        serde_json::to_string(&*decisions)
            .map_err(|_| VpnFailure::new("routing", "无法编码规则命中"))
    }

    pub fn dns_stats(&self) -> Result<String, VpnFailure> {
        let statistics = self
            .engine
            .shared
            .dns
            .statistics()
            .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
        serde_json::to_string(&statistics).map_err(|_| VpnFailure::new("dns", "DNS 状态编码失败"))
    }

    pub async fn connect(plan_json: String, timeout_ms: u32) -> Result<Self, VpnFailure> {
        Self::connect_using_routing(
            plan_json,
            timeout_ms,
            crate::routing::store::RoutingStore::legacy(),
        )
        .await
    }

    #[flutter_rust_bridge::frb(ignore)]
    async fn connect_using_routing(
        plan_json: String,
        timeout_ms: u32,
        routing: std::sync::Arc<crate::routing::store::RoutingStore>,
    ) -> Result<Self, VpnFailure> {
        static NEXT_RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let run_id = NEXT_RUN
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |run| run.checked_add(1),
            )
            .map_err(|_| VpnFailure::new("session", "原生会话标识已耗尽"))?;
        Ok(Self {
            engine: std::sync::Arc::new(
                Engine::connect_with_routing(plan_json, timeout_ms, routing).await?,
            ),
            run_id,
            preparation_warnings: Vec::new(),
            startup: None,
        })
    }

    pub async fn start_proxy(
        &self,
        kind: ProxyKind,
        address: String,
        port: u16,
    ) -> Result<ProxyBinding, VpnFailure> {
        let _operation = self.engine.desktop_operations.lock().await;
        self.engine.start(kind, address, port).await
    }

    pub async fn rebind_proxy(
        &self,
        kind: ProxyKind,
        address: String,
        port: u16,
        system_proxy_directory: Option<String>,
        system_proxy_enabled: bool,
    ) -> Result<DesktopServicesReport, VpnFailure> {
        crate::connection::desktop_services::rebind(
            &self.engine,
            kind,
            address,
            port,
            system_proxy_directory,
            system_proxy_enabled,
        )
        .await
    }

    pub async fn stop_proxy(&self, kind: ProxyKind) -> Result<(), VpnFailure> {
        let _operation = self.engine.desktop_operations.lock().await;
        self.engine.stop_proxy(kind).await
    }

    pub async fn stop(&self) -> Result<(), VpnFailure> {
        self.engine.stop().await
    }

    /// The task owns the receiver, so an event stream cannot retain the FFI session.
    pub fn events(&self, sink: StreamSink<VpnEvent>) {
        let already_closed = self.engine.is_closed();
        let mut receiver = self.engine.events();
        let shared = std::sync::Arc::clone(&self.engine.shared);
        self.engine.runtime.spawn(async move {
            if sink.add(shared.snapshot(VpnEventKind::State, 0.0, 0.0)).is_err() { return; }
            if already_closed { return; }
            loop {
                tokio::select! {
                    event = receiver.recv() => match event {
                        Ok(event) => {
                            let closed = matches!(event.state, VpnState::Disconnected | VpnState::Failed);
                            if sink.add(event).is_err() || closed { break; }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            if sink.add(shared.snapshot(VpnEventKind::Traffic, 0.0, 0.0)).is_err() { break; }
                        }
                        Err(_) => break,
                    },
                    _ = shared.cancellation.cancelled() => {
                        shared.wait_closed().await;
                        let _ = sink.add(shared.snapshot(VpnEventKind::State, 0.0, 0.0));
                        break;
                    }
                }
            }
        });
    }

    pub fn snapshot(&self) -> VpnEvent {
        self.engine.shared.snapshot(VpnEventKind::State, 0.0, 0.0)
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn cancel(&self) {
        self.engine.shared.cancellation.cancel();
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    #[tokio::test(start_paused = true)]
    async fn preparation_and_handshake_consume_one_deadline() {
        let attempt = VpnConnectAttempt::new();
        let started = tokio::time::Instant::now();
        let handshake_finished = AtomicBool::new(false);
        let result = attempt
            .with_deadline(1000, async {
                tokio::time::sleep(Duration::from_millis(600)).await;
                tokio::time::sleep(Duration::from_millis(600)).await;
                handshake_finished.store(true, Ordering::Relaxed);
                Ok(())
            })
            .await;
        assert!(result.is_err_and(|failure| failure.code == "timeout"));
        assert_eq!(started.elapsed(), Duration::from_millis(1000));
        assert!(!handshake_finished.load(Ordering::Relaxed));
        assert!(attempt.cancellation.is_cancelled());
    }

    #[tokio::test]
    async fn invalid_budget_and_pre_cancelled_attempt_never_poll_the_operation() {
        let attempt = VpnConnectAttempt::new();
        let polled = AtomicBool::new(false);
        for budget in [0, 99, 120_001] {
            let result = attempt
                .with_deadline(budget, async {
                    polled.store(true, Ordering::Relaxed);
                    Ok(())
                })
                .await;
            assert!(result.is_err_and(|failure| failure.code == "plan"));
        }
        attempt.cancel();
        let result = attempt
            .with_deadline(1000, async {
                polled.store(true, Ordering::Relaxed);
                Ok(())
            })
            .await;
        assert!(result.is_err_and(|failure| failure.code == "cancelled"));
        assert!(!polled.load(Ordering::Relaxed));
    }
}

#[flutter_rust_bridge::frb(opaque)]
pub struct VpnPacketPort {
    port: crate::connection::packet::Port,
}

impl VpnPacketPort {
    pub async fn write_packet(&self, packet: Vec<u8>) -> Result<(), VpnFailure> {
        self.port.write(packet).await
    }

    pub async fn read_packet(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, VpnFailure> {
        self.port.read(wait_ms).await
    }

    pub async fn close(&self) {
        self.port.close().await
    }
}
