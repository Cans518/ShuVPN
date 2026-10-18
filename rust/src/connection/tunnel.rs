use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use sangfor_core::{l3, plan::SessionPlan, tcp_tunnel};
use sangfor_tls::{PinVerifier, TrustPolicy};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;
use tokio_rustls::{client::TlsStream, TlsConnector};

use super::model::VpnFailure;
use super::session::Shared;
#[cfg(test)]
use crate::routing::authorization::{address_match, domain_match};
use crate::routing::{
    dns::{Answer, Epoch, Resolution, Source},
    engine::Evaluation,
    model::{Action, Flow, Inbound, Network, RouteDecision},
};

pub(crate) type TunnelIo = TlsStream<crate::network::Tcp>;
pub(crate) const HANDSHAKE_LIMIT: usize = 128 * 1024;

pub(crate) fn tls_config(plan: &SessionPlan) -> Result<Arc<rustls::ClientConfig>, VpnFailure> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let policy = TrustPolicy {
        pins: plan.certificate_digests.clone(),
        accept_unpinned: plan.accept_any_certificate,
    };
    let config = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(|_| VpnFailure::new("tls", "无法配置 TLS 协议"))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinVerifier::new(policy, provider)))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

pub(crate) async fn connect_node(
    plan: &SessionPlan,
    group: &str,
    tls: Arc<rustls::ClientConfig>,
) -> Result<TunnelIo, VpnFailure> {
    let candidates = plan
        .nodes
        .get(group)
        .or_else(|| plan.nodes.get(&plan.major_node_group))
        .ok_or_else(|| VpnFailure::new("nodes", "网关没有下发节点"))?;
    let mut last = None;
    for candidate in candidates {
        let Some((host, port)) = sangfor_core::plan::parse_endpoint(candidate) else {
            continue;
        };
        if host.is_empty() || port == 0 {
            continue;
        }
        let attempt = async {
            let socket = crate::network::connect_host(&host, port).await?;
            socket.set_nodelay(true)?;
            let name = ServerName::try_from(host.clone())
                .map_err(|_| VpnFailure::new("nodes", "节点主机名无效"))?;
            Ok::<_, VpnFailure>(
                TlsConnector::from(Arc::clone(&tls))
                    .connect(name, socket)
                    .await?,
            )
        };
        match timeout(Duration::from_secs(10), attempt).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last = Some(error),
            Err(_) => last = Some(VpnFailure::new("timeout", "连接隧道节点超时")),
        }
    }
    Err(last.unwrap_or_else(|| VpnFailure::new("nodes", "没有可用的隧道节点")))
}

pub(crate) async fn open_control(
    plan: &SessionPlan,
    tls: Arc<rustls::ClientConfig>,
) -> Result<(TunnelIo, Vec<String>, Vec<u8>), VpnFailure> {
    open_control_for(plan, &plan.major_node_group, tls).await
}

pub(crate) async fn open_control_for(
    plan: &SessionPlan,
    group: &str,
    tls: Arc<rustls::ClientConfig>,
) -> Result<(TunnelIo, Vec<String>, Vec<u8>), VpnFailure> {
    let mut stream = connect_node(plan, group, tls).await?;
    stream
        .write_all(&l3::auth_tunnel_request(&plan.sid)?)
        .await?;
    stream.flush().await?;
    let mut parser = l3::HandshakeParser::new();
    let mut received = 0;
    let mut chunk = [0; 16 * 1024];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(VpnFailure::new("closed", "L3 握手期间节点关闭了连接"));
        }
        received += read;
        if received > HANDSHAKE_LIMIT {
            return Err(VpnFailure::new("protocol", "L3 握手超过缓冲限制"));
        }
        if let Some((response, leftover)) = parser.push(&chunk[..read])? {
            if response.virtual_ip.is_empty() {
                return Err(VpnFailure::new("address", "网关未下发虚拟地址"));
            }
            return Ok((stream, response.virtual_ip, leftover));
        }
    }
}

fn route_for<'a>(
    plan: &'a SessionPlan,
    host: &str,
    port: u16,
) -> Option<&'a sangfor_core::route::Route> {
    crate::routing::authorization::route_for(&plan.routes, host, port, Network::Tcp)
}

pub(crate) trait ProxyIo:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> ProxyIo for T {}
pub(crate) struct Dialed {
    pub io: Box<dyn ProxyIo>,
    pub leftover: Vec<u8>,
}

struct DialContext<'a> {
    physical_interface: Option<&'a str>,
    pinned: Option<Arc<crate::routing::store::Snapshot>>,
    packet_ip: Option<IpAddr>,
    process_query: crate::platform::process::Query,
    domain_candidates: Vec<String>,
}

pub(crate) struct PacketIngress {
    pub source: std::net::SocketAddr,
    pub destination: std::net::SocketAddr,
    pub domain_candidates: Vec<String>,
}

pub(crate) async fn dial_proxy(
    shared: &Shared,
    host: &str,
    port: u16,
    inbound: Inbound,
    source: std::net::SocketAddr,
    destination: std::net::SocketAddr,
) -> Result<Dialed, VpnFailure> {
    dial_resolved(
        shared,
        host,
        port,
        inbound,
        DialContext {
            physical_interface: None,
            pinned: None,
            packet_ip: None,
            process_query: crate::platform::process::Query {
                network: Network::Tcp,
                source,
                destination,
            },
            domain_candidates: Vec::new(),
        },
    )
    .await
}

pub(crate) async fn dial_packet(
    shared: &Shared,
    host: &str,
    physical_interface: Option<&str>,
    snapshot: Arc<crate::routing::store::Snapshot>,
    ingress: PacketIngress,
) -> Result<Dialed, VpnFailure> {
    let ip = ingress.destination.ip();
    let port = ingress.destination.port();
    if let Err(error) = validate_captured_dns_transport(shared, ip, port) {
        // Android probes opportunistic DoT for VPN-provided DNS servers.
        // Encrypted replies cannot populate this capture's domain mappings;
        // never silently send a campus resolver through a DIRECT fallback.
        let flow = Flow {
            host: host.to_owned(),
            ips: vec![ip],
            port,
            network: Network::Tcp,
            inbound: Inbound::Tun,
            process: None,
            domain_candidates: Vec::new(),
        };
        record_setup_failure(shared, &flow, snapshot.info.version, &error);
        return Err(error);
    }
    dial_resolved(
        shared,
        host,
        port,
        Inbound::Tun,
        DialContext {
            physical_interface,
            pinned: Some(snapshot),
            packet_ip: Some(ip),
            process_query: crate::platform::process::Query {
                network: Network::Tcp,
                source: ingress.source,
                destination: ingress.destination,
            },
            domain_candidates: ingress.domain_candidates,
        },
    )
    .await
}

pub(crate) fn validate_captured_dns_transport(
    shared: &Shared,
    ip: IpAddr,
    port: u16,
) -> Result<(), VpnFailure> {
    if port == 853
        && shared
            .dns_proxy_servers
            .read()
            .map_err(|_| VpnFailure::new("dns", "无法读取 DNS 捕获配置"))?
            .contains(&ip)
    {
        return Err(VpnFailure::new(
            "dns_protocol",
            "校园 DNS 捕获尚不支持 DoT/DoQ，不能回落到物理出口",
        ));
    }
    Ok(())
}

async fn dial_resolved(
    shared: &Shared,
    host: &str,
    port: u16,
    inbound: Inbound,
    context: DialContext<'_>,
) -> Result<Dialed, VpnFailure> {
    let DialContext {
        physical_interface,
        pinned,
        packet_ip,
        process_query,
        domain_candidates,
    } = context;
    let normalized = match host.parse::<IpAddr>() {
        Ok(address) => address.to_string(),
        Err(_) => crate::routing::model::normalize_domain(host).map_err(routing_failure)?,
    };
    let host = normalized.as_str();
    let (routing, epoch) = {
        let routing = shared
            .routing
            .read()
            .map_err(|_| VpnFailure::new("routing", "无法读取规则管理器"))?;
        (
            Arc::clone(&routing),
            shared.dns.epoch().map_err(routing_failure)?,
        )
    };
    let snapshot = match pinned {
        Some(snapshot) => snapshot,
        None => routing.snapshot().map_err(routing_failure)?,
    };
    let mut flow = Flow {
        host: host.to_owned(),
        ips: packet_ip
            .or_else(|| host.parse::<IpAddr>().ok())
            .into_iter()
            .collect(),
        port,
        network: Network::Tcp,
        inbound,
        process: None,
        domain_candidates,
    };
    let process_error = super::process_identity::attach(&snapshot, &mut flow, process_query).await;
    let evaluate = |flow: &Flow| {
        snapshot
            .evaluate(&shared.plan.routes, flow)
            .map_err(|error| {
                let error = super::process_identity::failure(error, process_error.as_deref());
                record_setup_failure(shared, flow, snapshot.info.version, &error);
                error
            })
    };
    let mut resolutions = Vec::new();
    let decision = match evaluate(&flow)? {
        Evaluation::Decided(decision) => decision,
        Evaluation::NeedAddress => {
            let resolved = match resolve_for_rules(shared, host, snapshot.info.version, epoch).await
            {
                Ok(resolved) => resolved,
                Err(error) => {
                    record_setup_failure(shared, &flow, snapshot.info.version, &error);
                    return Err(error);
                }
            };
            flow.ips = resolved.addresses.clone();
            resolutions.push(resolved);
            match evaluate(&flow)? {
                Evaluation::Decided(decision) => decision,
                Evaluation::NeedAddress => {
                    return Err(VpnFailure::new("dns", "DNS 没有返回可用于匹配的地址"))
                }
            }
        }
    };
    let result = async {
        match decision.action {
            Action::Reject => Err(VpnFailure::new("rejected", "流量被规则拒绝")),
            Action::Direct => {
                let started = Instant::now();
                let mut direct = flow.clone();
                if host.parse::<IpAddr>().is_err() && packet_ip.is_none() {
                    let resolved = if let Some(resolved) = resolutions
                        .iter()
                        .find(|resolved| resolved.source == Source::System)
                    {
                        resolved.clone()
                    } else {
                        physical_dns(shared, host, snapshot.info.version, epoch).await?
                    };
                    direct.ips = resolved.addresses.clone();
                    if !resolutions
                        .iter()
                        .any(|existing| existing.source == Source::System)
                    {
                        resolutions.push(resolved);
                    }
                }
                let socket = super::physical::tcp_socket(&direct, physical_interface).await?;
                let peer = socket.peer_addr()?;
                let local = socket.local_addr()?;
                let own_port = peer.port()
                    == shared.http_port.load(std::sync::atomic::Ordering::Acquire)
                    || peer.port() == shared.socks_port.load(std::sync::atomic::Ordering::Acquire);
                if own_port && (peer.ip().is_loopback() || peer.ip() == local.ip()) {
                    Err(VpnFailure::new(
                        "protected",
                        "规则不能将流量回送自身代理监听",
                    ))
                } else {
                    socket.set_nodelay(true)?;
                    shared
                        .stats
                        .connections
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    shared.stats.sample_latency(started.elapsed());
                    Ok(Dialed {
                        io: Box::new(socket),
                        leftover: vec![],
                    })
                }
            }
            Action::ShuVpn => {
                let target = if host.parse::<IpAddr>().is_err()
                    && route_for(&shared.plan, host, port).is_none()
                {
                    if flow.ips.is_empty() {
                        let resolved =
                            match resolve_for_rules(shared, host, snapshot.info.version, epoch)
                                .await
                            {
                                Ok(resolved) => resolved,
                                Err(_) if snapshot.compiled.is_none() && !campus_name(host) => {
                                    shared
                                        .stats
                                        .unrouted
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    return Err(VpnFailure::new(
                                        "unrouted",
                                        "目标不在网关资源表内",
                                    ));
                                }
                                Err(error) => return Err(error),
                            };
                        flow.ips = resolved.addresses.clone();
                        resolutions.push(resolved);
                    }
                    flow.ips
                        .iter()
                        .find(|address| {
                            route_for(&shared.plan, &address.to_string(), port).is_some()
                        })
                        .map(ToString::to_string)
                        .ok_or_else(|| {
                            shared
                                .stats
                                .unrouted
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            VpnFailure::new("unrouted", "规则选择校园隧道，但网关未授权目标")
                        })?
                } else {
                    host.to_owned()
                };
                dial_tunnel_pinned(shared, &target, port, packet_ip).await
            }
        }
    }
    .await;
    record_decision(shared, &flow, decision, &resolutions, &result);
    result
}

fn routing_failure(error: crate::routing::model::RoutingFailure) -> VpnFailure {
    VpnFailure::new(&error.code, &error.message)
}

fn campus_name(host: &str) -> bool {
    host == "shu.edu.cn" || host.ends_with(".shu.edu.cn")
}

pub(crate) fn record_decision<T>(
    shared: &Shared,
    flow: &Flow,
    decision: RouteDecision,
    resolutions: &[Resolution],
    result: &Result<T, VpnFailure>,
) {
    if let Ok(mut events) = shared.decisions.lock() {
        if events.len() >= 128 {
            events.pop_front();
        }
        events.push_back(serde_json::json!({"flow":flow, "decision":decision,
            "dns":resolutions,
            "outbound":if result.is_ok() { Some(decision.action) } else { None },
            "error":result.as_ref().err().map(|error| error.code.clone()),
            "message":result.as_ref().err().map(|error| error.message.clone())}));
    }
}

pub(crate) fn record_setup_failure(shared: &Shared, flow: &Flow, version: u64, error: &VpnFailure) {
    if let Ok(mut events) = shared.decisions.lock() {
        if events.len() >= 128 {
            events.pop_front();
        }
        events.push_back(
            serde_json::json!({"flow":flow,"version":version,"decision":null,
            "dns":[],"outbound":null,"error":error.code,"message":error.message}),
        );
    }
}

async fn resolve_for_rules(
    shared: &Shared,
    host: &str,
    version: u64,
    epoch: Epoch,
) -> Result<Resolution, VpnFailure> {
    let host = crate::routing::model::normalize_domain(host).map_err(routing_failure)?;
    if campus_name(&host) {
        let server = shared
            .plan
            .dns_servers
            .iter()
            .find(|server| route_for(&shared.plan, server, 53).is_some())
            .ok_or_else(|| VpnFailure::new("dns", "没有可用且获授权的校园 TCP DNS 通道"))?;
        let source = Source::Campus {
            server: server.clone(),
        };
        let (cached, epoch) = shared
            .dns
            .lookup(&host, &source, version, epoch)
            .map_err(routing_failure)?;
        if let Some(cached) = cached {
            crate::connection::capture::advertise(shared, cached.clone(), epoch).await?;
            return Ok(cached);
        }
        let resolved = campus_dns(shared, &host, server, version).await?;
        shared
            .dns
            .insert(&resolved, epoch)
            .map_err(routing_failure)?;
        crate::connection::capture::advertise(shared, resolved.clone(), epoch).await?;
        return Ok(resolved);
    }
    physical_dns(shared, &host, version, epoch).await
}

async fn physical_dns(
    shared: &Shared,
    host: &str,
    version: u64,
    epoch: Epoch,
) -> Result<Resolution, VpnFailure> {
    shared
        .dns
        .lookup(host, &Source::System, version, epoch)
        .map_err(routing_failure)?;
    shared
        .dns
        .system_lookups
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let result = crate::network::dns::resolve(host).await?;
    if result.is_empty() {
        return Err(VpnFailure::new("dns", "物理 DNS 没有返回地址"));
    }
    Ok(Resolution::new(
        host.to_owned(),
        Source::System,
        version,
        vec![Answer {
            addresses: result,
            ttl: None,
            observed: Instant::now(),
        }],
        vec![],
    ))
}

async fn campus_dns(
    shared: &Shared,
    host: &str,
    server: &str,
    version: u64,
) -> Result<Resolution, VpnFailure> {
    use hickory_proto::rr::RecordType;
    let queries = tokio::join!(
        campus_query(shared, host, server, RecordType::A),
        campus_query(shared, host, server, RecordType::AAAA)
    );
    let mut answers = Vec::new();
    let mut warnings = Vec::new();
    let mut last_error = None;
    for (kind, resolved) in [(RecordType::A, queries.0), (RecordType::AAAA, queries.1)] {
        match resolved {
            Ok(answer) => {
                if answer.expired(Instant::now()) && !answer.addresses.is_empty() {
                    match campus_query(shared, host, server, kind).await {
                        Ok(fresh) => answers.push(fresh),
                        Err(error) => {
                            warnings.push(format!("{kind} 过期后重新查询失败：{}", error.message));
                            last_error = Some(error);
                        }
                    }
                } else {
                    answers.push(answer);
                }
            }
            Err(error) => {
                warnings.push(format!("{kind}：{}", error.message));
                last_error = Some(error);
            }
        }
    }
    let resolution = Resolution::new(
        host.to_owned(),
        Source::Campus {
            server: server.to_owned(),
        },
        version,
        answers,
        warnings,
    );
    if resolution.addresses.is_empty() {
        return Err(last_error.unwrap_or_else(|| VpnFailure::new("dns", "校园 DNS 没有返回地址")));
    }
    Ok(resolution)
}

pub(crate) async fn resolve_campus(
    shared: &Shared,
    host: &str,
    selected_server: Option<&str>,
) -> Result<Resolution, VpnFailure> {
    let host = crate::routing::model::normalize_domain(host).map_err(routing_failure)?;
    let (version, epoch) = {
        let routing = shared
            .routing
            .read()
            .map_err(|_| VpnFailure::new("routing", "无法读取规则配置"))?;
        (
            routing.snapshot().map_err(routing_failure)?.info.version,
            shared.dns.epoch().map_err(routing_failure)?,
        )
    };
    let server = shared
        .plan
        .dns_servers
        .iter()
        .find(|server| {
            selected_server.is_none_or(|selected| selected == server.as_str())
                && route_for(&shared.plan, server, 53).is_some()
        })
        .ok_or_else(|| VpnFailure::new("dns", "没有获授权的校园 TCP DNS 服务"))?;
    let source = Source::Campus {
        server: server.clone(),
    };
    let (cached, epoch) = shared
        .dns
        .lookup(&host, &source, version, epoch)
        .map_err(routing_failure)?;
    let resolution = match cached {
        Some(cached) => cached,
        None => campus_dns(shared, &host, server, version).await?,
    };
    if shared.dns.epoch().map_err(routing_failure)? != epoch {
        return Err(VpnFailure::new("conflict", "DNS 请求所属配置已被替换"));
    }
    shared
        .dns
        .insert(&resolution, epoch)
        .map_err(routing_failure)?;
    crate::connection::capture::advertise(shared, resolution.clone(), epoch).await?;
    let current_version = shared
        .routing
        .read()
        .map_err(|_| VpnFailure::new("routing", "无法读取规则配置"))?
        .snapshot()
        .map_err(routing_failure)?
        .info
        .version;
    if current_version != version || shared.dns.epoch().map_err(routing_failure)? != epoch {
        return Err(VpnFailure::new("conflict", "DNS 回答发布前配置已变化"));
    }
    Ok(resolution)
}

async fn campus_query(
    shared: &Shared,
    host: &str,
    server: &str,
    record_type: hickory_proto::rr::RecordType,
) -> Result<Answer, VpnFailure> {
    use hickory_proto::{
        op::{Message, MessageType, Query},
        rr::Name,
    };
    let resolved = async {
        let mut random = [0; 2];
        getrandom::getrandom(&mut random)
            .map_err(|_| VpnFailure::new("dns", "无法生成 DNS 请求标识"))?;
        let id = u16::from_be_bytes(random);
        let mut query = Message::new();
        query
            .set_id(id)
            .set_message_type(MessageType::Query)
            .set_recursion_desired(true)
            .add_query(Query::query(
                Name::from_ascii(format!("{host}."))
                    .map_err(|_| VpnFailure::new("dns", "DNS 名称无效"))?,
                record_type,
            ));
        let response = campus_wire(shared, server, &query).await?;
        crate::routing::dns::parse_response(&query, &response).map_err(routing_failure)
    };
    let result = timeout(shared.dial_timeout / 2, resolved)
        .await
        .map_err(|_| VpnFailure::new("dns", "校园 DNS 查询超时"))
        .and_then(|result| result);
    if result.is_err() {
        shared
            .dns
            .failed_queries
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    result
}

async fn campus_wire(
    shared: &Shared,
    server: &str,
    query: &hickory_proto::op::Message,
) -> Result<hickory_proto::op::Message, VpnFailure> {
    let bytes = query
        .to_vec()
        .map_err(|_| VpnFailure::new("dns", "DNS 请求编码失败"))?;
    if bytes.len() > u16::MAX as usize {
        return Err(VpnFailure::new("dns", "DNS 请求过大"));
    }
    let mut channel = dial_tunnel(shared, server, 53).await?;
    channel.io.write_u16(bytes.len() as u16).await?;
    channel.io.write_all(&bytes).await?;
    channel.io.flush().await?;
    shared
        .dns
        .campus_queries
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut reply = channel.leftover;
    let mut prefix = [0; 2];
    let copied = reply.len().min(2);
    prefix[..copied].copy_from_slice(&reply[..copied]);
    reply.drain(..copied);
    if copied < 2 {
        channel.io.read_exact(&mut prefix[copied..]).await?;
    }
    let length = u16::from_be_bytes(prefix) as usize;
    if length < 12 {
        return Err(VpnFailure::new("dns", "DNS 响应过短"));
    }
    while reply.len() < length {
        let mut chunk = [0; 4096];
        let read = channel.io.read(&mut chunk).await?;
        if read == 0 {
            return Err(VpnFailure::new("dns", "校园 DNS 提前关闭连接"));
        }
        reply.extend_from_slice(&chunk[..read]);
    }
    let response = hickory_proto::op::Message::from_vec(&reply[..length])
        .map_err(|_| VpnFailure::new("dns", "校园 DNS 响应无效"))?;
    crate::network::dns::validate_reply(query, &response)?;
    if response.truncated() {
        return Err(VpnFailure::new("dns", "校园 TCP DNS 返回截断回答"));
    }
    Ok(response)
}

pub(crate) async fn exchange_campus_dns(
    shared: &Shared,
    query: &hickory_proto::op::Message,
) -> Result<(hickory_proto::op::Message, String), VpnFailure> {
    let server = shared
        .plan
        .dns_servers
        .iter()
        .find(|server| route_for(&shared.plan, server, 53).is_some())
        .ok_or_else(|| VpnFailure::new("dns", "没有获授权的校园 TCP DNS 服务"))?;
    let response = timeout(shared.dial_timeout / 2, campus_wire(shared, server, query))
        .await
        .map_err(|_| VpnFailure::new("dns", "校园 DNS 查询超时"))??;
    Ok((response, server.clone()))
}

#[cfg(test)]
fn dns_addresses(
    query: &hickory_proto::op::Message,
    response: &hickory_proto::op::Message,
) -> Result<Vec<IpAddr>, VpnFailure> {
    crate::routing::dns::parse_response(query, response)
        .map(|answer| answer.addresses)
        .map_err(routing_failure)
}

async fn dial_tunnel(shared: &Shared, host: &str, port: u16) -> Result<Dialed, VpnFailure> {
    dial_tunnel_pinned(shared, host, port, None).await
}

async fn dial_tunnel_pinned(
    shared: &Shared,
    host: &str,
    port: u16,
    packet_ip: Option<IpAddr>,
) -> Result<Dialed, VpnFailure> {
    if host.is_empty()
        || host.len() > 255
        || host.bytes().any(|byte| byte.is_ascii_control())
        || port == 0
    {
        return Err(VpnFailure::new("destination", "代理目标地址无效"));
    }
    let started = Instant::now();
    let target = host
        .parse::<IpAddr>()
        .map(|address| address.to_string())
        .map_or_else(
            |_| crate::routing::model::normalize_domain(host).map_err(routing_failure),
            Ok,
        )?;
    let route = route_for(&shared.plan, &target, port);
    let Some(route) = route else {
        shared
            .stats
            .unrouted
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Err(VpnFailure::new("unrouted", "目标不在网关资源表内"));
    };
    let mut stream =
        connect_node(&shared.plan, &route.node_group_id, Arc::clone(&shared.tls)).await?;
    let target_address = if target.contains(':') {
        format!("[{target}]:{port}")
    } else {
        format!("{target}:{port}")
    };
    let request = tcp_tunnel::AuthRequest {
        sid: shared.plan.sid.clone(),
        app_id: route.app_id.clone(),
        url: format!("tcp://{target_address}"),
        device_id: shared.plan.device_id.clone(),
        connection_id: shared.plan.connection_id.clone(),
        proc_hash: shared.plan.fingerprint(),
        user_name: shared.plan.username.clone(),
        lang: shared.plan.lang.clone(),
        dest_addr: target_address,
        dest_ip: packet_ip
            .or_else(|| target.parse::<IpAddr>().ok())
            .map(|address| address.to_string()),
        rc_applied_info: 1,
        process: Some(shared.plan.process()),
    };
    let key = shared
        .plan
        .sign_key()
        .ok_or_else(|| VpnFailure::new("plan", "请求签名密钥无效"))?;
    // Raw mode matches the current app's non-zero-RTT TCP dials.
    stream
        .write_all(&tcp_tunnel::protocol::handshake_message(
            &request, &key, &target, port, false,
        )?)
        .await?;
    stream.flush().await?;
    let mut parser = tcp_tunnel::HandshakeDecoder::new();
    let mut received = 0;
    let mut chunk = [0; 16 * 1024];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(VpnFailure::new("closed", "TCP 隧道握手期间节点关闭了连接"));
        }
        received += read;
        if received > HANDSHAKE_LIMIT {
            return Err(VpnFailure::new("protocol", "TCP 握手超过缓冲限制"));
        }
        if let Some(leftover) = parser.push(&chunk[..read])? {
            let response = parser
                .response()
                .ok_or_else(|| VpnFailure::new("protocol", "TCP 握手未完成"))?;
            if response.auth_code != 0 || response.connect_status != 0 {
                return Err(VpnFailure::new("refused", "网关拒绝了 TCP 通道"));
            }
            shared
                .stats
                .connections
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            shared.stats.sample_latency(started.elapsed());
            return Ok(Dialed {
                io: Box::new(stream),
                leftover,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    mod gateway {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/gateway.rs"
        ));
    }

    #[tokio::test]
    async fn captured_dns_tls_cannot_fall_back_to_a_direct_outbound() -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::L3Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let directory = tempfile::tempdir()?;
        let routing = crate::routing::store::RoutingStore::open(
            directory.path().to_string_lossy().into_owned(),
        )?;
        routing
            .apply(
                "automatic_campus: false\nrules: ['MATCH,DIRECT']".to_owned(),
                "{}".to_owned(),
                0,
            )
            .await?;
        let snapshot = routing.snapshot()?;
        // Loopback keeps the failing pre-fix reproducer off every real network.
        let ip: IpAddr = "127.0.0.1".parse()?;
        *engine
            .shared
            .dns_proxy_servers
            .write()
            .map_err(|_| anyhow::anyhow!("poisoned"))? = vec![ip];
        let result = dial_packet(
            &engine.shared,
            "127.0.0.1",
            None,
            snapshot,
            PacketIngress {
                source: (ip, 12345).into(),
                destination: (ip, 853).into(),
                domain_candidates: Vec::new(),
            },
        )
        .await;
        assert_eq!(
            result.err().map(|error| error.code),
            Some("dns_protocol".to_owned())
        );
        {
            let decisions = engine
                .shared
                .decisions
                .lock()
                .map_err(|_| anyhow::anyhow!("poisoned"))?;
            let event = decisions
                .back()
                .ok_or_else(|| anyhow::anyhow!("missing rejection event"))?;
            assert_eq!(event["error"], "dns_protocol");
            assert_eq!(event["flow"]["port"], 853);
            assert!(event["outbound"].is_null());
        }
        validate_captured_dns_transport(&engine.shared, ip, 53)?;
        validate_captured_dns_transport(&engine.shared, "127.0.0.2".parse()?, 853)?;
        engine
            .shared
            .dns_proxy_servers
            .write()
            .map_err(|_| anyhow::anyhow!("poisoned"))?
            .clear();
        validate_captured_dns_transport(&engine.shared, ip, 853)?;
        engine.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn direct_tries_remaining_resolved_addresses() -> anyhow::Result<()> {
        let endpoint = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let socket = super::super::physical::tcp_socket(
            &Flow {
                host: "multi.example".to_owned(),
                ips: vec!["127.0.0.2".parse()?, "127.0.0.1".parse()?],
                port: endpoint.local_addr()?.port(),
                network: Network::Tcp,
                inbound: Inbound::Http,
                process: None,
                domain_candidates: Vec::new(),
            },
            None,
        )
        .await?;
        assert_eq!(socket.peer_addr()?, endpoint.local_addr()?);
        Ok(())
    }

    #[test]
    fn dns_answers_must_belong_to_the_question_or_its_cname_chain() -> anyhow::Result<()> {
        use hickory_proto::{
            op::{Message, MessageType, Query, ResponseCode},
            rr::{
                rdata::{A, CNAME},
                Name, RData, Record, RecordType,
            },
        };
        let host = Name::from_ascii("oa.shu.edu.cn.")?;
        let alias = Name::from_ascii("alias.shu.edu.cn.")?;
        let unrelated = Name::from_ascii("unrelated.example.")?;
        let mut query = Message::new();
        query
            .set_id(123)
            .add_query(Query::query(host.clone(), RecordType::A));
        let mut response = query.clone();
        response
            .set_message_type(MessageType::Response)
            .add_answer(Record::from_rdata(
                unrelated,
                60,
                RData::A(A("10.0.0.7".parse()?)),
            ));
        assert!(dns_addresses(&query, &response)?.is_empty());
        response
            .add_answer(Record::from_rdata(
                alias.clone(),
                60,
                RData::A(A("10.0.0.8".parse()?)),
            ))
            .add_answer(Record::from_rdata(host, 60, RData::CNAME(CNAME(alias))));
        assert_eq!(
            dns_addresses(&query, &response)?,
            vec!["10.0.0.8".parse::<IpAddr>()?]
        );
        response.set_response_code(ResponseCode::ServFail);
        assert!(dns_addresses(&query, &response).is_err());
        response
            .set_response_code(ResponseCode::NoError)
            .set_id(124);
        assert!(dns_addresses(&query, &response).is_err());
        Ok(())
    }

    #[test]
    fn wildcard_domains_require_a_dot_boundary_and_exclude_the_apex() {
        assert!(domain_match("*.shu.edu.cn", "oa.shu.edu.cn"));
        assert!(domain_match("*.SHU.edu.cn", "OA.shu.edu.cn."));
        assert!(!domain_match("*.shu.edu.cn", "shu.edu.cn"));
        assert!(!domain_match("*.shu.edu.cn", "evilshu.edu.cn"));
        assert!(!domain_match(
            "*.shu.edu.cn",
            "oa.shu.edu.cn.attacker.invalid"
        ));
    }

    #[test]
    fn address_routes_cover_both_families_and_reject_invalid_prefixes(
    ) -> Result<(), std::net::AddrParseError> {
        assert!(address_match("10.0.0.0/8", "10.2.3.4".parse()?));
        assert!(!address_match("10.0.0.0/8", "192.0.2.1".parse()?));
        assert!(address_match("10.0.0.1~10.0.0.10", "10.0.0.5".parse()?));
        assert!(address_match("2001:db8::/32", "2001:db8::12".parse()?));
        assert!(!address_match("10.0.0.0/99", "10.2.3.4".parse()?));
        Ok(())
    }
}
