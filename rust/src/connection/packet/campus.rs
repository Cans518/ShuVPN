use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};
use std::time::Duration;

use sangfor_core::l3::{self, AuthRequest, FrameDecoder, IpInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time::{interval, timeout};

use super::codec::{self, Tuple};
use crate::connection::{model::VpnFailure, session::Shared, tunnel};
use crate::routing::{
    authorization,
    model::{Flow, Network, RouteDecision},
};

pub(super) async fn serve(
    shared: &Shared,
    tuple: Tuple,
    flow: Flow,
    decision: RouteDecision,
    mut incoming: mpsc::Receiver<Vec<u8>>,
    outgoing: mpsc::Sender<Vec<u8>>,
) -> Result<(), VpnFailure> {
    let opened = timeout(shared.dial_timeout, async {
        let wire = wire_tuple(shared, tuple)?;
        authenticate(shared, wire, &flow.host)
            .await
            .map(|(stream, decoder, token)| (stream, decoder, token, wire))
    })
    .await
    .map_err(|_| VpnFailure::new("timeout", "校园 UDP 流授权超时"))
    .and_then(|result| result);
    tunnel::record_decision(shared, &flow, decision.clone(), &[], &opened);
    let (mut stream, mut decoder, token, wire) = opened?;
    shared.stats.connections.fetch_add(1, Ordering::Relaxed);
    let mut heartbeat = interval(Duration::from_secs_f64(shared.plan.heartbeat_seconds));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let mut misses = 0;
    let mut chunk = [0; 16 * 1024];
    let mut packets = Vec::new();
    let mut idle_at = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let result = timeout(Duration::from_secs(30), async {
            tokio::select! {
                packet = incoming.recv() => {
                    let Some(packet) = packet else { return Ok(false); };
                    let rewritten;
                    let bytes = if wire == tuple { &packet } else {
                        let datagram = codec::parse(&packet)?;
                        if datagram.tuple != tuple { return Err(VpnFailure::new("packet", "UDP 流入口发生变化")); }
                        rewritten = wire.packet(datagram.payload)?;
                        &rewritten
                    };
                    stream.write_all(&l3::data_request(&token, bytes)?).await?;
                    stream.flush().await?;
                    shared.stats.upload.fetch_add(packet.len() as u64, Ordering::Relaxed);
                    idle_at = tokio::time::Instant::now() + Duration::from_secs(30);
                }
                read = stream.read(&mut chunk) => {
                    let length = read?;
                    if length == 0 { return Err(VpnFailure::new("closed", "校园 UDP 通道已关闭")); }
                    for frame in decoder.push(&chunk[..length])? {
                        match frame.command {
                            l3::Command::HeartbeatResponse => misses = 0,
                            l3::Command::DataResponse => {
                                packets.extend_from_slice(&frame.payload);
                                if packets.len() > 32 * 1024 {
                                    return Err(VpnFailure::new("packet", "校园数据包缓冲超过限制"));
                                }
                                deliver(shared, tuple, wire, &mut packets, &outgoing).await?;
                                idle_at = tokio::time::Instant::now() + Duration::from_secs(30);
                            }
                            _ => return Err(VpnFailure::new("protocol", "校园 UDP 通道返回意外命令")),
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    if misses >= 3 {
                        return Err(VpnFailure::new("heartbeat", "校园 UDP 节点未回应心跳"));
                    }
                    stream.write_all(&l3::heartbeat_request()).await?;
                    stream.flush().await?;
                    misses += 1;
                }
                _ = tokio::time::sleep_until(idle_at) => return Ok(false),
            }
            Ok::<_, VpnFailure>(true)
        }).await;
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

fn wire_tuple(shared: &Shared, tuple: Tuple) -> Result<Tuple, VpnFailure> {
    let addresses = shared
        .virtual_addresses
        .read()
        .map_err(|_| VpnFailure::new("source", "无法读取获授权虚拟地址"))?;
    if addresses.contains(&tuple.source.ip()) {
        return Ok(tuple);
    }
    let address = addresses
        .iter()
        .find(|address| address.is_ipv4() == tuple.source.is_ipv4())
        .ok_or_else(|| VpnFailure::new("unrouted", "校园 UDP 没有同地址族的网关虚拟源地址"))?;
    Ok(Tuple {
        source: std::net::SocketAddr::new(*address, tuple.source.port()),
        ..tuple
    })
}

async fn authenticate(
    shared: &Shared,
    tuple: Tuple,
    host: &str,
) -> Result<(tunnel::TunnelIo, FrameDecoder, String), VpnFailure> {
    let target = tuple.destination.ip().to_string();
    let route = authorization::route_for(
        &shared.plan.routes,
        host,
        tuple.destination.port(),
        Network::Udp,
    )
    .ok_or_else(|| {
        shared.stats.unrouted.fetch_add(1, Ordering::Relaxed);
        VpnFailure::new("unrouted", "规则选择校园 UDP，但网关未授权目标")
    })?;
    if !tuple.destination.is_ipv4() {
        return Err(VpnFailure::new(
            "capability",
            "校园 IPv6 原始 UDP 出站尚未验证",
        ));
    }
    let (mut stream, _, leftover) =
        tunnel::open_control_for(&shared.plan, &route.node_group_id, Arc::clone(&shared.tls))
            .await?;
    static NEXT_FLOW: AtomicI64 = AtomicI64::new(1);
    let flow_id = NEXT_FLOW
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| VpnFailure::new("flow", "流标识已耗尽"))?;
    let request = AuthRequest {
        sid: shared.plan.sid.clone(),
        app_id: route.app_id.clone(),
        url: format!("udp:{target}:{}", tuple.destination.port()),
        device_id: shared.plan.device_id.clone(),
        connection_id: shared.plan.connection_id.clone(),
        lang: shared.plan.lang.clone(),
        conntrack_hash: flow_id,
        ip: IpInfo {
            atype: 0x0800,
            protocol: 17,
            destination_address: target,
            destination_port: tuple.destination.port(),
            source_address: tuple.source.ip().to_string(),
            source_port: tuple.source.port(),
        },
        proc_hash: Some(shared.plan.fingerprint()),
        app_token: None,
        rc_applied_info: 1,
        process: Some(shared.plan.process()),
        domain: if host.parse::<std::net::IpAddr>().is_err() {
            Some(host.to_owned())
        } else {
            None
        },
    };
    let key = shared
        .plan
        .sign_key()
        .ok_or_else(|| VpnFailure::new("plan", "请求签名密钥无效"))?;
    stream.write_all(&request.frame(&key)?).await?;
    stream.flush().await?;
    let mut decoder = FrameDecoder::new();
    let mut frames = decoder.push(&leftover)?;
    let mut chunk = [0; 16 * 1024];
    loop {
        for frame in frames {
            match frame.command {
                l3::Command::HeartbeatResponse => {}
                l3::Command::AuthResponse => {
                    let value: serde_json::Value = serde_json::from_slice(&frame.payload)
                        .map_err(|_| VpnFailure::new("protocol", "流授权响应 JSON 无效"))?;
                    let data = &value["data"];
                    if data["conntrackHash"].as_i64() != Some(flow_id) {
                        return Err(VpnFailure::new("protocol", "流授权响应标识不匹配"));
                    }
                    let code = data["code"].as_i64().or_else(|| value["code"].as_i64());
                    if frame.status != 0
                        || code != Some(0)
                        || value["code"].as_i64().is_some_and(|code| code != 0)
                    {
                        return Err(VpnFailure::new("auth", "网关拒绝 UDP 流授权"));
                    }
                    let token = data["connectToken"]
                        .as_str()
                        .filter(|token| !token.is_empty() && token.len() <= 255)
                        .ok_or_else(|| VpnFailure::new("protocol", "流授权 token 无效"))?;
                    return Ok((stream, decoder, token.to_owned()));
                }
                _ => return Err(VpnFailure::new("protocol", "流授权前收到未授权的数据")),
            }
        }
        let length = stream.read(&mut chunk).await?;
        if length == 0 {
            return Err(VpnFailure::new("closed", "UDP 授权期间节点关闭连接"));
        }
        frames = decoder.push(&chunk[..length])?;
    }
}

async fn deliver(
    shared: &Shared,
    tuple: Tuple,
    wire: Tuple,
    packets: &mut Vec<u8>,
    outgoing: &mpsc::Sender<Vec<u8>>,
) -> Result<(), VpnFailure> {
    loop {
        if packets.len() < 4 {
            return Ok(());
        }
        if packets[0] >> 4 != 4 {
            return Err(VpnFailure::new("packet", "此校园 UDP 流返回不支持的地址族"));
        }
        let length = usize::from(u16::from_be_bytes([packets[2], packets[3]]));
        if !(28..=usize::from(shared.plan.mtu)).contains(&length) {
            return Err(VpnFailure::new("packet", "校园 UDP 包长度无效"));
        }
        if packets.len() < length {
            return Ok(());
        }
        let packet: Vec<u8> = packets.drain(..length).collect();
        let datagram = codec::parse(&packet)?;
        if datagram.tuple != wire.reverse() {
            return Err(VpnFailure::new("packet", "校园回包不属于已授权 UDP 流"));
        }
        let packet = if tuple == wire {
            packet
        } else {
            tuple.reverse().packet(datagram.payload)?
        };
        outgoing
            .send(packet)
            .await
            .map_err(|_| VpnFailure::new("closed", "包端口已关闭"))?;
        shared
            .stats
            .download
            .fetch_add(length as u64, Ordering::Relaxed);
    }
}
