mod campus;
mod codec;
mod dns_proxy;
mod dns_service;
mod tcp;
mod tcp_codec;
mod tcp_stack;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) mod tun;

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::connection::{model::VpnFailure, physical, session::Shared, tunnel};
use crate::routing::{
    engine::Evaluation,
    model::{Action, Flow, RouteDecision},
};
use codec::Tuple;

const MAX_FLOWS: usize = 128;

pub(crate) struct Port {
    input: mpsc::Sender<Vec<u8>>,
    output: Mutex<mpsc::Receiver<Vec<u8>>>,
    state: Arc<State>,
    shared: Arc<Shared>,
    sources: Vec<std::net::IpAddr>,
}

struct State {
    cancellation: CancellationToken,
    closed: AtomicBool,
    done: Notify,
}

struct Completion(Arc<State>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.cancellation.cancel();
        self.0.closed.store(true, Ordering::Release);
        self.0.done.notify_waiters();
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        self.state.cancellation.cancel();
    }
}

impl Port {
    pub(crate) async fn open(
        shared: Arc<Shared>,
        physical_interface: Option<String>,
    ) -> Result<Self, VpnFailure> {
        let sources = shared
            .virtual_addresses
            .read()
            .map_err(|_| VpnFailure::new("address", "无法读取虚拟地址"))?
            .clone();
        Self::open_with_sources(shared, physical_interface, sources).await
    }

    pub(crate) async fn open_with_sources(
        shared: Arc<Shared>,
        physical_interface: Option<String>,
        sources: Vec<std::net::IpAddr>,
    ) -> Result<Self, VpnFailure> {
        physical::validate_interface(physical_interface.as_deref())?;
        let mut task = shared.packet_task.lock().await;
        if shared.cancellation.is_cancelled() {
            return Err(closed());
        }
        if task.as_ref().is_some_and(|task| !task.is_finished()) {
            return Err(VpnFailure::new("busy", "此会话已有包数据面"));
        }
        if let Some(previous) = task.take() {
            previous
                .await
                .map_err(|_| VpnFailure::new("worker", "旧包工作线程退出失败"))?;
        }
        let (input, incoming) = mpsc::channel(64);
        let (outgoing, output) = mpsc::channel(64);
        let state = Arc::new(State {
            cancellation: shared.cancellation.child_token(),
            closed: AtomicBool::new(false),
            done: Notify::new(),
        });
        let completion = Completion(Arc::clone(&state));
        let worker_shared = Arc::clone(&shared);
        let worker_token = state.cancellation.clone();
        *task = Some(tokio::spawn(async move {
            let _completion = completion;
            run(
                worker_shared,
                incoming,
                outgoing,
                physical_interface,
                worker_token,
            )
            .await;
        }));
        drop(task);
        Ok(Self {
            input,
            output: Mutex::new(output),
            state,
            shared,
            sources,
        })
    }

    pub(crate) async fn write(&self, packet: Vec<u8>) -> Result<(), VpnFailure> {
        if packet.len() > usize::from(self.shared.plan.mtu) {
            return Err(VpnFailure::new("packet", "包长度超过会话 MTU"));
        }
        let (_, tuple) = packet_tuple(&packet)?;
        if !self.sources.contains(&tuple.source.ip()) {
            return Err(VpnFailure::new(
                "source",
                "源地址不属于此会话允许的本机入口",
            ));
        }
        tokio::select! {
            biased;
            _ = self.state.cancellation.cancelled() => Err(closed()),
            result = self.input.send(packet) => result.map_err(|_| closed()),
        }
    }

    pub(crate) async fn read(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, VpnFailure> {
        if !(1..=5000).contains(&wait_ms) {
            return Err(VpnFailure::new("timeout", "包读取等待必须为 1–5000 毫秒"));
        }
        let receive = async { self.output.lock().await.recv().await.ok_or_else(closed) };
        tokio::select! {
            biased;
            _ = self.state.cancellation.cancelled() => Err(closed()),
            result = tokio::time::timeout(Duration::from_millis(u64::from(wait_ms)), receive) =>
                match result { Ok(result) => result.map(Some), Err(_) => Ok(None) },
        }
    }

    pub(crate) async fn close(&self) {
        self.state.cancellation.cancel();
        loop {
            let done = self.state.done.notified();
            if self.state.closed.load(Ordering::Acquire) {
                break;
            }
            done.await;
        }
    }
}

async fn run(
    shared: Arc<Shared>,
    mut incoming: mpsc::Receiver<Vec<u8>>,
    outgoing: mpsc::Sender<Vec<u8>>,
    physical_interface: Option<String>,
    token: CancellationToken,
) {
    let mut flows = HashMap::<(u8, Tuple), mpsc::Sender<Vec<u8>>>::new();
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => break,
            Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                match result {
                    Ok(tuple) => { flows.remove(&tuple); }
                    Err(_) => { shared.fail(VpnFailure::new("worker", "包数据面流工作线程异常退出")); break; }
                }
            }
            packet = incoming.recv() => {
                let Some(packet) = packet else { break; };
                let Ok((protocol, tuple)) = packet_tuple(&packet) else { continue; };
                let key = (protocol, tuple);
                if let Some(flow) = flows.get(&key) {
                    if flow.try_send(packet).is_err() {
                        shared.stats.failures.fetch_add(1, Ordering::Relaxed);
                    }
                    continue;
                }
                if protocol == 6 && !tcp_codec::parse(&packet).is_ok_and(|segment| segment.is_opening()) {
                    if let Ok(Some(reset)) = tcp_codec::parse(&packet).and_then(|segment| segment.reset()) {
                        tokio::select! {
                            biased;
                            _ = token.cancelled() => break,
                            _ = outgoing.send(reset) => {},
                        }
                    }
                    continue;
                }
                if flows.len() >= MAX_FLOWS {
                    shared.stats.failures.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                if protocol == 6 {
                    let snapshot = match shared.routing.read().ok()
                        .and_then(|store| store.snapshot().ok())
                    {
                        Some(snapshot) => snapshot,
                        None => { shared.stats.failures.fetch_add(1, Ordering::Relaxed); continue; }
                    };
                    let acquired = tokio::select! {
                        biased;
                        _ = token.cancelled() => break,
                        result = crate::connection::capture::acquire(&shared, tuple.destination.ip(), snapshot.info.version) => result,
                    };
                    let binding = match acquired {
                        Ok(binding) => binding,
                        Err(_) => {
                            if let Ok(Some(reset)) = tcp_codec::parse(&packet).and_then(|segment| segment.reset()) {
                                tokio::select! { _ = token.cancelled() => break, _ = outgoing.send(reset) => {} }
                            }
                            shared.stats.failures.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                    };
                    // A TCP receive window can arrive as a burst of roughly
                    // 50 MTU-sized segments before this actor runs again.
                    let (sender, receiver) = mpsc::channel(64);
                    if sender.try_send(packet).is_err() { continue; }
                    flows.insert(key, sender);
                    let shared = Arc::clone(&shared);
                    let outgoing = outgoing.clone();
                    let interface = physical_interface.clone();
                    let token = token.child_token();
                    tasks.spawn(async move {
                        let result = tcp::serve(&shared, tuple, snapshot, receiver, outgoing, interface.as_deref(), token, binding).await;
                        if result.is_err() { shared.stats.failures.fetch_add(1, Ordering::Relaxed); }
                        key
                    });
                    continue;
                }
                if tuple.destination.port() == 53 && (shared.dns_capture_full.load(Ordering::Acquire) || shared.dns_proxy_servers.read()
                    .is_ok_and(|servers| servers.contains(&tuple.destination.ip())))
                {
                    let (sender, receiver) = mpsc::channel(16);
                    if sender.try_send(packet).is_err() { continue; }
                    flows.insert(key, sender);
                    let shared = Arc::clone(&shared);
                    let outgoing = outgoing.clone();
                    let token = token.child_token();
                    tasks.spawn(async move {
                        let result = tokio::select! {
                            biased;
                            _ = token.cancelled() => Ok(()),
                            result = dns_proxy::serve(&shared, tuple, receiver, outgoing) => result,
                        };
                        if result.is_err() { shared.stats.failures.fetch_add(1, Ordering::Relaxed); }
                        key
                    });
                    continue;
                }
                let snapshot = match shared.routing.read().ok().and_then(|store| store.snapshot().ok()) {
                    Some(snapshot) => snapshot,
                    None => { shared.stats.failures.fetch_add(1, Ordering::Relaxed); continue; }
                };
                let acquired = tokio::select! {
                    biased;
                    _ = token.cancelled() => break,
                    result = crate::connection::capture::acquire(&shared, tuple.destination.ip(), snapshot.info.version) => result,
                };
                let binding = match acquired {
                    Ok(binding) => binding,
                    Err(_) => { shared.stats.failures.fetch_add(1, Ordering::Relaxed); continue; }
                };
                let mut flow = tuple.flow();
                flow.domain_candidates = binding.domain_candidates;
                if let Some(host) = binding.host { flow.host = host; }
                let (sender, receiver) = mpsc::channel(16);
                if sender.try_send(packet).is_err() { continue; }
                flows.insert(key, sender);
                let shared = Arc::clone(&shared);
                let outgoing = outgoing.clone();
                let interface = physical_interface.clone();
                let token = token.child_token();
                tasks.spawn(async move {
                    let _lease = binding._lease;
                    let result = tokio::select! {
                        biased;
                        _ = token.cancelled() => Ok(()),
                        result = serve(&shared, tuple, flow, snapshot, receiver, outgoing, interface.as_deref()) => result,
                    };
                    if result.is_err() { shared.stats.failures.fetch_add(1, Ordering::Relaxed); }
                    key
                });
            }
        }
    }
    token.cancel();

    incoming.close();

    while let Some(joined) = tasks.join_next().await {
        let _ = &joined;
    }
}

fn packet_tuple(packet: &[u8]) -> Result<(u8, Tuple), VpnFailure> {
    let protocol = codec::parse_ip(packet)?.protocol;
    let tuple = match protocol {
        6 => tcp_codec::parse(packet)?.tuple,
        17 => codec::parse(packet)?.tuple,
        _ => {
            return Err(VpnFailure::new(
                "packet_protocol",
                "包端口仅支持未分片 TCP/UDP",
            ))
        }
    };
    Ok((protocol, tuple))
}

fn decide(
    shared: &Shared,
    flow: &Flow,
    snapshot: &crate::routing::store::Snapshot,
) -> Result<RouteDecision, VpnFailure> {
    match snapshot
        .evaluate(&shared.plan.routes, flow)
        .map_err(|error| VpnFailure::new(&error.code, &error.message))?
    {
        Evaluation::Decided(decision) => Ok(decision),
        Evaluation::NeedAddress => {
            Err(VpnFailure::new("routing", "IP 包决策不应要求额外 DNS 查询"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn serve(
    shared: &Shared,
    tuple: Tuple,
    mut flow: Flow,
    snapshot: Arc<crate::routing::store::Snapshot>,
    mut incoming: mpsc::Receiver<Vec<u8>>,
    outgoing: mpsc::Sender<Vec<u8>>,
    interface: Option<&str>,
) -> Result<(), VpnFailure> {
    let diagnostic = crate::connection::process_identity::attach(
        &snapshot,
        &mut flow,
        crate::platform::process::Query {
            network: crate::routing::model::Network::Udp,
            source: tuple.source,
            destination: tuple.destination,
        },
    )
    .await;
    let decision = match decide(shared, &flow, &snapshot) {
        Ok(decision) => decision,
        Err(mut error) => {
            if error.code == "process" {
                if let Some(diagnostic) = diagnostic {
                    error.message = format!("{}：{diagnostic}", error.message);
                }
            }
            tunnel::record_setup_failure(shared, &flow, snapshot.info.version, &error);
            return Err(error);
        }
    };
    if let Err(error) = tunnel::validate_captured_dns_transport(
        shared,
        tuple.destination.ip(),
        tuple.destination.port(),
    ) {
        let result: Result<(), _> = Err(error);
        tunnel::record_decision(shared, &flow, decision, &[], &result);
        return result;
    }
    match decision.action {
        Action::Reject => {
            let result: Result<(), _> = Err(VpnFailure::new("rejected", "UDP 流被规则拒绝"));
            tunnel::record_decision(shared, &flow, decision, &[], &result);
            result
        }
        Action::ShuVpn => campus::serve(shared, tuple, flow, decision, incoming, outgoing).await,
        Action::Direct => {
            let socket = physical::udp_socket(tuple.destination, interface).await;
            tunnel::record_decision(shared, &flow, decision.clone(), &[], &socket);
            let socket = socket?;
            shared.stats.connections.fetch_add(1, Ordering::Relaxed);
            let mut response = vec![0; 9001];
            loop {
                let result = tokio::time::timeout(Duration::from_secs(30), async {
                    tokio::select! {
                        packet = incoming.recv() => {
                            let Some(packet) = packet else { return Ok(false); };
                            let datagram = codec::parse(&packet)?;
                            let written = socket.send(datagram.payload).await?;
                            if written != datagram.payload.len() {
                                return Err(VpnFailure::new("network", "UDP 数据报未完整发送"));
                            }
                            shared.stats.upload.fetch_add(packet.len() as u64, Ordering::Relaxed);
                        }
                        received = socket.recv(&mut response) => {
                            let length = received?;
                            let packet = tuple.reverse().packet(&response[..length])?;
                            if packet.len() > usize::from(shared.plan.mtu) {
                                return Err(VpnFailure::new("packet", "物理 UDP 回包超过会话 MTU"));
                            }
                            shared.stats.download.fetch_add(packet.len() as u64, Ordering::Relaxed);
                            outgoing.send(packet).await.map_err(|_| closed())?;
                        }
                    }
                    Ok::<_, VpnFailure>(true)
                })
                .await;
                match result {
                    Ok(Ok(true)) => {}
                    Ok(Ok(false)) | Err(_) => return Ok(()),
                    Ok(Err(error)) => {
                        let failed: Result<(), _> = Err(error);
                        tunnel::record_decision(shared, &flow, decision, &[], &failed);
                        return failed;
                    }
                }
            }
        }
    }
}

fn closed() -> VpnFailure {
    VpnFailure::new("closed", "包数据面已关闭")
}
