use super::Egress;
use crate::connection::model::VpnFailure;
use hickory_proto::{
    op::{Message, Query},
    rr::{Name, RecordType},
};
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::OwnedRwLockReadGuard,
};

pub(crate) fn servers(explicit: Option<Vec<IpAddr>>) -> Result<Vec<SocketAddr>, VpnFailure> {
    let addresses = match explicit {
        Some(addresses) => addresses,
        None => {
            let addresses: Vec<IpAddr> = vec![];
            #[cfg(target_os = "linux")]
            let mut addresses = addresses;
            #[cfg(target_os = "linux")]
            for path in ["/etc/resolv.conf", "/run/systemd/resolve/resolv.conf"] {
                if let Ok(configuration) = std::fs::read_to_string(path) {
                    for line in configuration.lines() {
                        let mut words = line.split_whitespace();
                        if words.next() == Some("nameserver") {
                            if let Some(Ok(address)) = words.next().map(str::parse::<IpAddr>) {
                                // A system stub may issue its own unbound queries.
                                if !address.is_loopback() && !addresses.contains(&address) {
                                    addresses.push(address);
                                }
                            }
                        }
                    }
                }
                if !addresses.is_empty() {
                    break;
                }
            }
            addresses
        }
    };
    let addresses: Vec<_> = addresses
        .into_iter()
        .map(|address| match address {
            IpAddr::V6(address) => address
                .to_ipv4_mapped()
                .map(IpAddr::V4)
                .unwrap_or(IpAddr::V6(address)),
            address => address,
        })
        .collect();
    if addresses.len() > 16
        || addresses.iter().any(|address| {
            address.is_unspecified() || address.is_multicast() || address.is_loopback()
        })
    {
        return Err(VpnFailure::new("dns", "物理 DNS 服务器配置无效"));
    }
    Ok(addresses
        .into_iter()
        .map(|address| SocketAddr::new(address, 53))
        .collect())
}

pub(crate) fn explicit_upstream(value: Option<&str>) -> Result<Option<Vec<IpAddr>>, VpnFailure> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return Ok(None);
    }
    let address: IpAddr = value
        .parse()
        .map_err(|_| VpnFailure::new("dns", "请输入明确的非回环 DNS 上游 IP"))?;
    if matches!(address, IpAddr::V6(address) if address.is_unicast_link_local()) {
        return Err(VpnFailure::new(
            "dns",
            "本次 DNS 上游不支持带接口作用域的地址",
        ));
    }
    let normalized = servers(Some(vec![address]))?;
    Ok(Some(
        normalized.into_iter().map(|server| server.ip()).collect(),
    ))
}

pub(crate) async fn resolve(host: &str) -> Result<Vec<IpAddr>, VpnFailure> {
    let gate = super::read_gate().await;
    let egress = super::current()?;
    resolve_with(egress.as_deref(), host, Some(gate)).await
}

pub(crate) async fn resolve_with(
    egress: Option<&Egress>,
    host: &str,
    gate: Option<Arc<OwnedRwLockReadGuard<()>>>,
) -> Result<Vec<IpAddr>, VpnFailure> {
    if let Ok(address) = host.parse() {
        return Ok(vec![address]);
    }
    let host = crate::routing::model::normalize_domain(host)
        .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
    if host == "localhost" {
        return Ok(vec![
            "127.0.0.1".parse().map_err(|_| invalid())?,
            "::1".parse().map_err(|_| invalid())?,
        ]);
    }
    let Some(egress) = egress else {
        // The worker retains the transition gate even when the caller drops
        // this future; getaddrinfo cannot issue late unbound DNS after capture.
        return tokio::task::spawn_blocking(move || {
            let _gate = gate;
            let addresses = (host.as_str(), 0).to_socket_addrs()?;
            Ok::<_, std::io::Error>(addresses.map(|address| address.ip()).take(64).collect())
        })
        .await
        .map_err(|_| VpnFailure::new("worker", "系统解析线程退出失败"))?
        .map_err(VpnFailure::from);
    };

    let mut last = None;
    for server in &egress.resolvers {
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let (a, aaaa) = tokio::join!(
                query(egress, *server, &host, RecordType::A),
                query(egress, *server, &host, RecordType::AAAA),
            );
            let mut addresses = vec![];
            let mut last = None;
            for result in [a, aaaa] {
                match result {
                    Ok(answer) if !answer.expired(std::time::Instant::now()) => {
                        for address in answer.addresses {
                            if !addresses.contains(&address) && addresses.len() < 64 {
                                addresses.push(address);
                            }
                        }
                    }
                    Ok(_) => last = Some(VpnFailure::new("dns", "物理 DNS 回答已过期")),
                    Err(error) => last = Some(error),
                }
            }
            if addresses.is_empty() {
                return Err(last.unwrap_or_else(invalid));
            }
            Ok(addresses)
        })
        .await
        .map_err(|_| VpnFailure::new("timeout", "绑定物理出口的 DNS 查询超时"))
        .and_then(|result| result);
        match result {
            Ok(addresses) => return Ok(addresses),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| {
        VpnFailure::new(
            "dns",
            "TUN 域名解析需要明确的物理上游 DNS，不能回落到系统 stub",
        )
    }))
}

async fn query(
    egress: &Egress,
    server: SocketAddr,
    host: &str,
    kind: RecordType,
) -> Result<crate::routing::dns::Answer, VpnFailure> {
    let mut random = [0; 2];
    getrandom::getrandom(&mut random).map_err(|_| invalid())?;
    let mut question = Message::new();
    question
        .set_id(u16::from_be_bytes(random))
        .set_recursion_desired(true);
    question.add_query(Query::query(
        Name::from_ascii(format!("{host}.")).map_err(|_| invalid())?,
        kind,
    ));
    let response = exchange_server(egress, server, &question).await?;
    crate::routing::dns::parse_response(&question, &response)
        .map_err(|error| VpnFailure::new(&error.code, &error.message))
}

pub(crate) async fn exchange(egress: &Egress, question: &Message) -> Result<Message, VpnFailure> {
    let mut last = None;
    for server in &egress.resolvers {
        match exchange_server(egress, *server, question).await {
            Ok(response) => return Ok(response),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| VpnFailure::new("dns", "没有明确的物理 DNS 上游")))
}

pub(crate) fn validate_reply(question: &Message, response: &Message) -> Result<(), VpnFailure> {
    if response.id() != question.id()
        || response.queries() != question.queries()
        || response.message_type() != hickory_proto::op::MessageType::Response
        || response.op_code() != question.op_code()
    {
        return Err(invalid());
    }
    Ok(())
}

async fn exchange_server(
    egress: &Egress,
    server: SocketAddr,
    question: &Message,
) -> Result<Message, VpnFailure> {
    let bytes = question.to_vec().map_err(|_| invalid())?;
    if bytes.len() > u16::MAX as usize {
        return Err(invalid());
    }
    let operation = async {
        let socket = super::socket::udp_with(Some(egress), server, None).await?;
        if socket.send(&bytes).await? != bytes.len() {
            return Err(invalid());
        }
        let mut response = vec![0; 65536];
        let length = socket.recv(&mut response).await?;
        let response = Message::from_vec(&response[..length]).map_err(|_| invalid())?;
        validate_reply(question, &response)?;
        if response.truncated() {
            let mut socket = super::socket::connect_with(Some(egress), server, None).await?;
            socket
                .write_all(&(bytes.len() as u16).to_be_bytes())
                .await?;
            socket.write_all(&bytes).await?;
            let length = usize::from(socket.read_u16().await?);
            let mut response = vec![0; length];
            socket.read_exact(&mut response).await?;
            let response = Message::from_vec(&response).map_err(|_| invalid())?;
            validate_reply(question, &response)?;
            if response.truncated() {
                return Err(invalid());
            }
            return Ok(response);
        }
        Ok(response)
    };
    tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .map_err(|_| VpnFailure::new("timeout", "物理 DNS 地址族查询超时"))?
}
fn invalid() -> VpnFailure {
    VpnFailure::new("dns", "物理 DNS 查询或回答无效")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_upstream_is_explicit_normalized_and_cannot_be_a_local_stub() -> anyhow::Result<()> {
        assert!(explicit_upstream(None)?.is_none());
        assert!(explicit_upstream(Some("  "))?.is_none());
        for value in [
            "127.0.2.2",
            "::ffff:127.0.2.3",
            "fe80::1",
            "dns.example",
            "224.0.0.1",
        ] {
            assert!(explicit_upstream(Some(value)).is_err());
        }
        assert_eq!(
            explicit_upstream(Some(" ::ffff:192.0.2.53 "))?,
            Some(vec!["192.0.2.53".parse::<IpAddr>()?])
        );
        Ok(())
    }

    #[test]
    fn explicit_physical_resolvers_cannot_delegate_to_a_loopback_stub() -> anyhow::Result<()> {
        for server in [
            "127.0.0.53",
            "::1",
            "::ffff:127.0.2.2",
            "0.0.0.0",
            "224.0.0.1",
        ] {
            assert!(servers(Some(vec![server.parse()?])).is_err());
        }
        assert_eq!(
            servers(Some(vec!["192.0.2.53".parse()?]))?,
            ["192.0.2.53:53".parse::<SocketAddr>()?]
        );
        Ok(())
    }
}
