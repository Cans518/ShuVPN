#![cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]
use crate::{
    connection::{model::VpnFailure, session::Shared},
    routing::{
        authorization,
        dns::{Epoch, Resolution, Source},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    sync::Arc,
    time::Instant,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Name {
    addresses: Vec<IpAddr>,
    expires: Instant,
}
#[derive(Clone, Default)]
struct Book {
    names: BTreeMap<String, Name>,
    flows: BTreeMap<IpAddr, usize>,
    version: u64,
    epoch: Option<Epoch>,
}
impl Book {
    fn reset(&mut self, version: u64, epoch: Epoch) {
        if self.version != version || self.epoch != Some(epoch) {
            self.names.clear();
            self.version = version;
            self.epoch = Some(epoch);
        }
    }
    fn targets(&self, now: Instant) -> BTreeSet<IpAddr> {
        self.names
            .values()
            .filter(|name| name.expires > now)
            .flat_map(|name| name.addresses.iter().copied())
            .chain(self.flows.keys().copied())
            .collect()
    }
    fn host(&self, ip: IpAddr, now: Instant) -> Result<Option<String>, VpnFailure> {
        let names = self.candidates(ip, now);
        match names.len() {
            0 => Ok(None),
            1 => Ok(names.into_iter().next()),
            _ => Err(VpnFailure::new(
                "dns_ambiguous",
                "同一地址有多个有效校园域名，不能伪造域名身份",
            )),
        }
    }
    fn candidates(&self, ip: IpAddr, now: Instant) -> Vec<String> {
        self.names
            .iter()
            .filter(|(_, name)| name.expires > now && name.addresses.contains(&ip))
            .map(|(host, _)| host.clone())
            .collect()
    }
}

enum Command {
    Register {
        resolution: Resolution,
        epoch: Epoch,
        reply: oneshot::Sender<Result<(), VpnFailure>>,
    },
    Acquire {
        ip: IpAddr,
        version: u64,
        reply: oneshot::Sender<Result<Binding, VpnFailure>>,
    },
    Status(oneshot::Sender<Result<String, VpnFailure>>),
}
pub(crate) struct Handle {
    commands: mpsc::Sender<Command>,
    releases: mpsc::UnboundedSender<IpAddr>,
    cancellation: CancellationToken,
}
pub(crate) struct Binding {
    pub host: Option<String>,
    pub domain_candidates: Vec<String>,
    pub _lease: Option<Lease>,
}
pub(crate) struct Lease {
    ip: IpAddr,
    release: mpsc::UnboundedSender<IpAddr>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.release.send(self.ip);
    }
}

fn active(shared: &Shared) -> Result<Option<Arc<Handle>>, VpnFailure> {
    Ok(shared.captures.lock().map_err(|_| failure())?.clone())
}
impl Handle {
    async fn send(&self, command: Command) -> Result<(), VpnFailure> {
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(closed()),
            result = self.commands.send(command) => result.map_err(|_| closed()),
        }
    }
}
pub(crate) async fn advertise(
    shared: &Shared,
    resolution: Resolution,
    epoch: Epoch,
) -> Result<(), VpnFailure> {
    let Some(handle) = active(shared)? else {
        return Ok(());
    };
    let (reply, result) = oneshot::channel();
    handle
        .send(Command::Register {
            resolution,
            epoch,
            reply,
        })
        .await?;
    tokio::select! {
        biased;
        _ = handle.cancellation.cancelled() => Err(closed()),
        result = result => result.map_err(|_| closed())?,
    }
}
pub(crate) async fn acquire(
    shared: &Shared,
    ip: IpAddr,
    version: u64,
) -> Result<Binding, VpnFailure> {
    let Some(handle) = active(shared)? else {
        return Ok(Binding {
            host: None,
            domain_candidates: Vec::new(),
            _lease: None,
        });
    };
    let (reply, result) = oneshot::channel();
    handle.send(Command::Acquire { ip, version, reply }).await?;
    tokio::select! {
        biased;
        _ = handle.cancellation.cancelled() => Err(closed()),
        result = result => result.map_err(|_| closed())?,
    }
}
pub(crate) async fn status(shared: &Shared) -> Result<String, VpnFailure> {
    let Some(handle) = active(shared)? else {
        return Ok("{\"active\":false}".to_owned());
    };
    let (reply, result) = oneshot::channel();
    handle.send(Command::Status(reply)).await?;
    tokio::select! {
        biased;
        _ = handle.cancellation.cancelled() => Err(closed()),
        result = result => result.map_err(|_| closed())?,
    }
}

pub(crate) trait RouteCapture: Send + Sync + 'static {
    fn set_routes(
        &self,
        wanted: BTreeSet<IpAddr>,
        cancellation: &CancellationToken,
    ) -> impl std::future::Future<Output = Result<(), VpnFailure>> + Send;
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(crate) fn start<D: RouteCapture>(
    shared: Arc<Shared>,
    device: D,
    cancellation: CancellationToken,
) -> (Arc<Handle>, tokio::task::JoinHandle<Result<(), VpnFailure>>) {
    let (commands, incoming) = mpsc::channel(64);
    let (releases, released) = mpsc::unbounded_channel();
    let handle = Arc::new(Handle {
        commands,
        releases,
        cancellation: cancellation.clone(),
    });
    let worker_handle = Arc::clone(&handle);
    let task = tokio::spawn(async move {
        run(
            shared,
            device,
            worker_handle,
            incoming,
            released,
            cancellation,
        )
        .await
    });
    (handle, task)
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn generation(shared: &Shared) -> Result<(u64, Epoch), VpnFailure> {
    let routing = shared.routing.read().map_err(|_| failure())?;
    Ok((
        routing.snapshot().map_err(|_| failure())?.info.version,
        shared.dns.epoch().map_err(|_| failure())?,
    ))
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
async fn run<D: RouteCapture>(
    shared: Arc<Shared>,
    device: D,
    handle: Arc<Handle>,
    mut commands: mpsc::Receiver<Command>,
    mut released: mpsc::UnboundedReceiver<IpAddr>,
    cancellation: CancellationToken,
) -> Result<(), VpnFailure> {
    let mut book = Book::default();
    let mut routes = BTreeSet::new();
    let mut last_error: Option<String> = None;
    let mut timer = tokio::time::interval(std::time::Duration::from_millis(100));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let (version, epoch) = generation(&shared)?;
        book.reset(version, epoch);
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            Some(ip) = released.recv() => {
                if let Some(count) = book.flows.get_mut(&ip) {
                    *count -= 1;
                    if *count == 0 { book.flows.remove(&ip); }
                }
            }
            command = commands.recv() => match command {
                Some(Command::Register { resolution, epoch: requested_epoch, reply }) => {
                    // The actor may have waited across a routing replacement.
                    // Validate against the live generation, not the pre-wait book.
                    let (version, epoch) = generation(&shared)?;
                    book.reset(version, epoch);
                    let mut candidate = book.clone();
                    let requested_host = resolution.host.clone();
                    let result = register(&shared, &mut candidate, resolution, requested_epoch);
                    let result = match result {
                        Ok(()) => {
                            let wanted = candidate.targets(Instant::now());
                            match device.set_routes(wanted.clone(), &cancellation).await {
                                Ok(()) => {
                                    let (latest_version, latest_epoch) = generation(&shared)?;
                                    if candidate.names.get(&requested_host).is_some_and(|name| name.expires <= Instant::now()) {
                                        let restored = book.targets(Instant::now());
                                        device.set_routes(restored.clone(), &cancellation).await
                                            .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
                                        routes = restored;
                                        Err(VpnFailure::new("capture_ttl", "DNS 路由安装完成时记录已过期"))
                                    } else if latest_version != candidate.version || candidate.epoch != Some(latest_epoch) {
                                        book.reset(latest_version, latest_epoch);
                                        let restored = book.targets(Instant::now());
                                        device.set_routes(restored.clone(), &cancellation).await
                                            .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
                                        routes = restored;
                                        Err(VpnFailure::new("conflict", "DNS 安装期间规则版本已变化"))
                                    } else {
                                        book = candidate; routes = wanted; last_error = None; Ok(())
                                    }
                                }
                                Err(error) if error.code == "route_rollback" => return Err(VpnFailure::new(&error.code, &error.message)),
                                Err(error) => { last_error = Some(error.message.clone()); Err(VpnFailure::new(&error.code, &error.message)) }
                            }
                        }
                        Err(error) => Err(error),
                    };
                    let _ = reply.send(result);
                }
                Some(Command::Acquire { ip, version: requested_version, reply }) => {
                    let (latest_version, latest_epoch) = generation(&shared)?;
                    book.reset(latest_version, latest_epoch);
                    let result = if requested_version != book.version {
                        Err(VpnFailure::new("conflict", "DNS 捕获版本已变化"))
                    } else {
                        let found = match book.host(ip, Instant::now()) {
                            Ok(host) => Ok((host, Vec::new())),
                            Err(error) if error.code == "dns_ambiguous" && shared.dns_capture_full.load(std::sync::atomic::Ordering::Acquire) => {
                                let names = book.candidates(ip, Instant::now());
                                if names.len() > 32 { Err(VpnFailure::new("dns_ambiguous", "DNS 候选名称超过预算")) }
                                else { Ok((None, names)) }
                            }
                            Err(error) => Err(error),
                        };
                        found.map(|(host, domain_candidates)| {
                            let lease = if routes.contains(&ip) {
                                *book.flows.entry(ip).or_default() += 1;
                                Some(Lease { ip, release:handle.releases.clone() })
                            } else { None };
                            Binding { host, domain_candidates, _lease:lease }
                        })
                    };
                    let _ = reply.send(result);
                }
                Some(Command::Status(reply)) => {
                    let names: Vec<_> = book.names.iter().filter(|(_, name)| name.expires > Instant::now())
                        .map(|(host, name)| serde_json::json!({"host":host,"addresses":name.addresses,
                            "remaining_ms":name.expires.saturating_duration_since(Instant::now()).as_millis()})).collect();
                    let result = serde_json::to_string(&serde_json::json!({"active":true,
                        "version":book.version,"mappings":names,"dynamic_addresses":routes,
                        "leased_addresses":book.flows,"error":last_error})).map_err(|_| failure());
                    let _ = reply.send(result);
                }
                None => break,
            },
            _ = timer.tick() => {},
        }
        let wanted = book.targets(Instant::now());
        if wanted != routes {
            match device.set_routes(wanted.clone(), &cancellation).await {
                Ok(()) => {
                    routes = wanted;
                    last_error = None;
                }
                Err(error) if error.code == "route_rollback" => {
                    return Err(VpnFailure::new(&error.code, &error.message))
                }
                Err(error) => last_error = Some(error.message),
            }
        }
        book.names.retain(|_, name| name.expires > Instant::now());
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn register(
    shared: &Shared,
    book: &mut Book,
    resolution: Resolution,
    epoch: Epoch,
) -> Result<(), VpnFailure> {
    if resolution.version != book.version || book.epoch != Some(epoch) {
        return Err(VpnFailure::new("conflict", "迟到 DNS 不能覆盖当前捕获版本"));
    }
    let full = shared
        .dns_capture_full
        .load(std::sync::atomic::Ordering::Acquire);
    if !full && !matches!(resolution.source, Source::Campus { .. }) {
        return Ok(());
    }
    if !full
        && !shared.plan.routes.iter().any(|route| {
            authorization::domain_match(&route.host, &resolution.host)
                && route.host.parse::<IpAddr>().is_err()
                && route.host.parse::<ipnet::IpNet>().is_err()
        })
    {
        return Ok(());
    }
    let mut expires = resolution
        .expires
        .filter(|deadline| *deadline > Instant::now())
        .filter(|_| resolution.ttl_seconds != Some(0))
        .ok_or_else(|| {
            VpnFailure::new("capture_ttl", "域名资源需要尚未过期的正 TTL 才能动态捕获")
        })?;
    if book.names.len() >= 512 && !book.names.contains_key(&resolution.host) {
        return Err(VpnFailure::new("capture_limit", "DNS 捕获域名数量超过上限"));
    }
    let mut addresses = resolution.addresses;
    if full {
        if let Some(previous) = book
            .names
            .get(&resolution.host)
            .filter(|name| name.expires > Instant::now())
        {
            for address in &previous.addresses {
                if !addresses.contains(address) {
                    addresses.push(*address);
                }
            }
            expires = expires.min(previous.expires);
        }
        if addresses.len() > 64 {
            return Err(VpnFailure::new(
                "capture_limit",
                "单个 DNS 名称地址超过预算",
            ));
        }
    }
    book.names
        .insert(resolution.host, Name { addresses, expires });
    if book.targets(Instant::now()).len() > 1024 {
        return Err(VpnFailure::new("capture_limit", "动态捕获地址数量超过上限"));
    }
    Ok(())
}
fn closed() -> VpnFailure {
    VpnFailure::new("closed", "DNS 捕获服务已关闭")
}
fn failure() -> VpnFailure {
    VpnFailure::new("capture", "DNS 捕获状态不可用")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };
    use std::time::Duration;
    use tokio::sync::Notify;

    use crate::test_gateway as gateway;
    mod datagram {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/datagram.rs"
        ));
    }

    #[tokio::test]
    async fn closing_packet_port_cancels_a_queued_capture_reply() -> anyhow::Result<()> {
        let gateway =
            gateway::Gateway::start(gateway::Mode::DomainDns(gateway::DnsMode::Mapping)).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let sink = Arc::new(Sink::default());
        sink.pause.store(true, Ordering::Release);
        let cancellation = engine.shared.cancellation.child_token();
        let (handle, task) = start(
            Arc::clone(&engine.shared),
            Arc::clone(&sink),
            cancellation.clone(),
        );
        *engine.shared.captures.lock().map_err(|_| failure())? = Some(Arc::clone(&handle));
        let shared = Arc::clone(&engine.shared);
        let answer = resolution(&shared, vec!["10.99.0.7".parse()?])?;
        let epoch = shared.dns.epoch()?;
        let advertising = tokio::spawn(async move { advertise(&shared, answer, epoch).await });
        sink.entered.notified().await;
        let port = crate::connection::packet::Port::open(Arc::clone(&engine.shared), None).await?;
        port.write(datagram::packet(
            "10.99.0.2:53001".parse()?,
            "10.99.0.7:9001".parse()?,
            b"queued",
        ))
        .await?;
        tokio::time::timeout(Duration::from_secs(2), async {
            while handle.commands.capacity() == handle.commands.max_capacity() {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        let stopped = tokio::time::timeout(Duration::from_millis(100), port.close()).await;
        // Always release the paused actor, including the failing pre-fix case.
        sink.resume.notify_one();
        advertising.await??;
        port.close().await;
        cancellation.cancel();
        task.await??;
        engine.shared.captures.lock().map_err(|_| failure())?.take();
        engine.stop().await?;
        assert!(
            stopped.is_ok(),
            "port close waited for an unrelated capture reply"
        );
        Ok(())
    }

    #[derive(Default)]
    struct Sink {
        routes: Mutex<BTreeSet<IpAddr>>,
        failure: Mutex<Option<String>>,
        pause: AtomicBool,
        entered: Notify,
        resume: Notify,
    }
    impl RouteCapture for Arc<Sink> {
        async fn set_routes(
            &self,
            wanted: BTreeSet<IpAddr>,
            cancellation: &CancellationToken,
        ) -> Result<(), VpnFailure> {
            if cancellation.is_cancelled() {
                return Err(closed());
            }
            if let Some(code) = self.failure.lock().map_err(|_| failure())?.take() {
                return Err(VpnFailure::new(&code, "injected route failure"));
            }
            *self.routes.lock().map_err(|_| failure())? = wanted;
            if self.pause.swap(false, Ordering::AcqRel) {
                self.entered.notify_one();
                self.resume.notified().await;
            }
            Ok(())
        }
    }

    fn resolution(shared: &Shared, addresses: Vec<IpAddr>) -> anyhow::Result<Resolution> {
        let now = Instant::now();
        Ok(Resolution {
            host: "fixture-dns.shu.edu.cn".to_owned(),
            addresses,
            source: Source::Campus {
                server: "10.99.0.53".to_owned(),
            },
            ttl_seconds: Some(5),
            version: shared
                .routing
                .read()
                .map_err(|_| failure())?
                .snapshot()?
                .info
                .version,
            cached: false,
            age_ms: 0,
            cache_ttl_remaining_ms: Some(5000),
            warnings: vec![],
            observed: now,
            expires: Some(now + Duration::from_secs(5)),
        })
    }

    async fn wait_routes(sink: &Sink, wanted: BTreeSet<IpAddr>) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if *sink.routes.lock().map_err(|_| failure())? == wanted {
                    return Ok::<_, VpnFailure>(());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await??;
        Ok(())
    }

    #[tokio::test]
    async fn a_fresh_answer_after_replacing_routing_does_not_wait_for_the_capture_timer(
    ) -> anyhow::Result<()> {
        let gateway =
            gateway::Gateway::start(gateway::Mode::DomainDns(gateway::DnsMode::Mapping)).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let directory = tempfile::tempdir()?;
        let routing = crate::routing::store::RoutingStore::open(
            directory.path().to_string_lossy().into_owned(),
        )?;
        routing
            .apply(
                "automatic_campus: false\nrules: ['MATCH,SHU-VPN']".into(),
                "{}".into(),
                0,
            )
            .await?;
        tokio::time::pause();
        let sink = Arc::new(Sink::default());
        let cancellation = engine.shared.cancellation.child_token();
        let (handle, task) = start(
            Arc::clone(&engine.shared),
            Arc::clone(&sink),
            cancellation.clone(),
        );
        *engine.shared.captures.lock().map_err(|_| failure())? = Some(handle);
        let before: serde_json::Value = serde_json::from_str(&status(&engine.shared).await?)?;
        assert_eq!(before["version"], 0);
        *engine.shared.routing.write().map_err(|_| failure())? = routing;
        engine.shared.dns.clear()?;
        let answer = resolution(&engine.shared, vec!["10.99.0.7".parse()?])?;
        let result = advertise(&engine.shared, answer, engine.shared.dns.epoch()?).await;
        cancellation.cancel();
        task.await??;
        engine.shared.captures.lock().map_err(|_| failure())?.take();
        engine.stop().await?;
        drop(engine);
        assert!(
            result.is_ok(),
            "fresh answer rejected before timer refresh: {result:?}"
        );
        assert!(sink
            .routes
            .lock()
            .map_err(|_| failure())?
            .contains(&"10.99.0.7".parse()?));
        Ok(())
    }

    #[tokio::test]
    async fn shared_worker_installs_before_reply_and_keeps_routes_until_the_last_flow_lease(
    ) -> anyhow::Result<()> {
        let gateway =
            gateway::Gateway::start(gateway::Mode::DomainDns(gateway::DnsMode::Mapping)).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let sink = Arc::new(Sink::default());
        let cancellation = engine.shared.cancellation.child_token();
        let (handle, task) = start(
            Arc::clone(&engine.shared),
            Arc::clone(&sink),
            cancellation.clone(),
        );
        *engine.shared.captures.lock().map_err(|_| failure())? = Some(handle);
        let v4: IpAddr = "10.99.0.7".parse()?;
        let v6: IpAddr = "2001:db8::7".parse()?;
        let mut reply = resolution(&engine.shared, vec![v4, v6])?;
        let version = reply.version;
        reply.expires = Some(Instant::now() + Duration::from_millis(500));
        advertise(&engine.shared, reply, engine.shared.dns.epoch()?).await?;
        assert_eq!(
            *sink.routes.lock().map_err(|_| failure())?,
            BTreeSet::from([v4, v6])
        );
        let binding = acquire(&engine.shared, v4, version).await?;
        assert_eq!(binding.host.as_deref(), Some("fixture-dns.shu.edu.cn"));
        wait_routes(&sink, BTreeSet::from([v4])).await?;
        drop(binding);
        wait_routes(&sink, BTreeSet::new()).await?;
        cancellation.cancel();
        task.await??;
        engine.shared.captures.lock().map_err(|_| failure())?.take();
        engine.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn failed_install_preserves_previous_identity_and_fatal_rollback_ends_the_worker(
    ) -> anyhow::Result<()> {
        let gateway =
            gateway::Gateway::start(gateway::Mode::DomainDns(gateway::DnsMode::Mapping)).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let sink = Arc::new(Sink::default());
        let cancellation = engine.shared.cancellation.child_token();
        let (handle, task) = start(
            Arc::clone(&engine.shared),
            Arc::clone(&sink),
            cancellation.clone(),
        );
        *engine.shared.captures.lock().map_err(|_| failure())? = Some(handle);
        let old: IpAddr = "10.99.0.7".parse()?;
        let new: IpAddr = "10.99.0.8".parse()?;
        let reply = resolution(&engine.shared, vec![old])?;
        let version = reply.version;
        advertise(&engine.shared, reply, engine.shared.dns.epoch()?).await?;
        *sink.failure.lock().map_err(|_| failure())? = Some("conflict".to_owned());
        assert_eq!(
            advertise(
                &engine.shared,
                resolution(&engine.shared, vec![new])?,
                engine.shared.dns.epoch()?
            )
            .await
            .err()
            .map(|error| error.code),
            Some("conflict".to_owned())
        );
        assert_eq!(
            *sink.routes.lock().map_err(|_| failure())?,
            BTreeSet::from([old])
        );
        assert_eq!(
            acquire(&engine.shared, old, version).await?.host.as_deref(),
            Some("fixture-dns.shu.edu.cn")
        );
        assert!(acquire(&engine.shared, new, version).await?.host.is_none());
        *sink.failure.lock().map_err(|_| failure())? = Some("route_rollback".to_owned());
        assert!(advertise(
            &engine.shared,
            resolution(&engine.shared, vec![new])?,
            engine.shared.dns.epoch()?
        )
        .await
        .is_err());
        assert_eq!(
            task.await?.err().map(|error| error.code),
            Some("route_rollback".to_owned())
        );
        engine.shared.captures.lock().map_err(|_| failure())?.take();
        engine.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn an_epoch_or_ttl_change_during_install_restores_routes_without_publishing(
    ) -> anyhow::Result<()> {
        for expire in [false, true] {
            let gateway =
                gateway::Gateway::start(gateway::Mode::DomainDns(gateway::DnsMode::Mapping))
                    .await?;
            let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
            let sink = Arc::new(Sink::default());
            sink.pause.store(true, Ordering::Release);
            let cancellation = engine.shared.cancellation.child_token();
            let (handle, task) = start(
                Arc::clone(&engine.shared),
                Arc::clone(&sink),
                cancellation.clone(),
            );
            *engine.shared.captures.lock().map_err(|_| failure())? = Some(handle);
            let mut reply = resolution(&engine.shared, vec!["10.99.0.7".parse()?])?;
            if expire {
                reply.expires = Some(Instant::now() + Duration::from_millis(500));
            }
            let shared = Arc::clone(&engine.shared);
            let epoch = shared.dns.epoch()?;
            let pending = tokio::spawn(async move { advertise(&shared, reply, epoch).await });
            tokio::time::timeout(Duration::from_secs(2), sink.entered.notified()).await?;
            if expire {
                tokio::time::sleep(Duration::from_millis(550)).await;
            } else {
                engine.shared.dns.clear()?;
            }
            sink.resume.notify_one();
            let result = pending.await?;
            assert_eq!(
                result.err().map(|error| error.code),
                Some(if expire { "capture_ttl" } else { "conflict" }.to_owned())
            );
            assert!(sink.routes.lock().map_err(|_| failure())?.is_empty());
            cancellation.cancel();
            task.await??;
            engine.shared.captures.lock().map_err(|_| failure())?.take();
            engine.stop().await?;
        }
        Ok(())
    }

    #[test]
    fn expiration_and_version_changes_do_not_remove_routes_leased_by_existing_flows(
    ) -> anyhow::Result<()> {
        let cache = crate::routing::dns::Cache::default();
        let mut book = Book::default();
        book.reset(1, cache.epoch()?);
        let ip: IpAddr = "10.99.0.7".parse()?;
        let now = Instant::now();
        book.names.insert(
            "one.internal.invalid".to_owned(),
            Name {
                addresses: vec![ip],
                expires: now + Duration::from_secs(1),
            },
        );
        book.flows.insert(ip, 1);
        assert_eq!(book.host(ip, now)?, Some("one.internal.invalid".to_owned()));
        assert_eq!(book.host(ip, now + Duration::from_secs(2))?, None);
        assert!(book.targets(now + Duration::from_secs(2)).contains(&ip));
        cache.clear()?;
        book.reset(1, cache.epoch()?);
        assert!(book.names.is_empty());
        assert!(book.targets(now).contains(&ip));
        book.flows.remove(&ip);
        assert!(book.targets(now).is_empty());
        Ok(())
    }

    #[test]
    fn aliases_are_not_guessed_and_each_address_family_has_its_own_capture_target(
    ) -> anyhow::Result<()> {
        let now = Instant::now();
        let v4: IpAddr = "10.99.0.7".parse()?;
        let v6: IpAddr = "2001:db8::7".parse()?;
        let mut book = Book::default();
        book.names.insert(
            "one.internal.invalid".to_owned(),
            Name {
                addresses: vec![v4, v6],
                expires: now + Duration::from_secs(2),
            },
        );
        book.names.insert(
            "two.internal.invalid".to_owned(),
            Name {
                addresses: vec![v4],
                expires: now + Duration::from_secs(1),
            },
        );
        assert_eq!(book.targets(now), BTreeSet::from([v4, v6]));
        assert_eq!(
            book.host(v4, now).err().map(|error| error.code),
            Some("dns_ambiguous".to_owned())
        );
        assert_eq!(book.host(v6, now)?, Some("one.internal.invalid".to_owned()));
        assert_eq!(
            book.host(v4, now + Duration::from_millis(1500))?,
            Some("one.internal.invalid".to_owned())
        );
        Ok(())
    }
}
