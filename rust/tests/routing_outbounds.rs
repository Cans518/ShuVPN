use anyhow::Context;
use serde_json::Value;
use shuvpn_core::api::{
    routing::{edit_routing, RoutingManager},
    vpn::{ProxyKind, VpnSession},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{timeout, Duration},
};

#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;
use gateway::{Gateway, Mode};
#[allow(dead_code)]
#[path = "support/geodata.rs"]
mod geo_fixture;

async fn http_tunnel(
    port: u16,
    host: &str,
    target_port: u16,
) -> anyhow::Result<(TcpStream, Vec<u8>)> {
    let mut client = TcpStream::connect(("127.0.0.1", port)).await?;
    client
        .write_all(format!("CONNECT {host}:{target_port} HTTP/1.1\r\n\r\n").as_bytes())
        .await?;
    let mut response = vec![];
    while !response.ends_with(b"\r\n\r\n") {
        response.push(timeout(Duration::from_secs(3), client.read_u8()).await??);
    }
    Ok((client, response))
}

#[tokio::test]
async fn direct_reject_and_shu_vpn_have_independently_proven_network_paths() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    let physical = TcpListener::bind("127.0.0.1:0").await?;
    let port = physical.local_addr()?.port();
    let seen = Arc::new(AtomicUsize::new(0));
    let physical_seen = Arc::clone(&seen);
    let physical_task = tokio::spawn(async move {
        let (mut socket, _) = physical.accept().await?;
        physical_seen.fetch_add(1, Ordering::Relaxed);
        let mut bytes = [0; 4];
        socket.read_exact(&mut bytes).await?;
        socket.write_all(&bytes).await?;
        Ok::<_, std::io::Error>(())
    });
    let ip_cidr = [
        geo_fixture::bytes(1, &[127, 0, 0, 0]),
        geo_fixture::number(2, 8),
    ]
    .concat();
    let geoip = geo_fixture::bytes(
        1,
        &[
            geo_fixture::bytes(1, b"LOCAL"),
            geo_fixture::bytes(2, &ip_cidr),
        ]
        .concat(),
    );
    let geosite = geo_fixture::bytes(
        1,
        &[
            geo_fixture::bytes(1, b"CAMPUS"),
            geo_fixture::site_domain(1, "^blocked[.]internal[.]invalid$", &[("blocked", 1)]),
            geo_fixture::site_domain(3, "allowed.internal.invalid", &[("allowed", 1)]),
        ]
        .concat(),
    );
    std::fs::write(directory.path().join("ip.dat"), &geoip)?;
    std::fs::write(directory.path().join("site.dat"), &geosite)?;
    let dataset = |behavior: &str, code: &str, path: &str, bytes: &[u8]| {
        serde_json::json!({
            "type":"file","behavior":behavior,"format":"dat","path":path,
            "data":{"code":code,"version":"outbound-v1",
                "sha256":sangfor_core::crypto::hex_upper(&sangfor_core::crypto::sha256(bytes))}
        })
    };
    manager.apply(serde_json::json!({
        "name": "advanced actual paths", "automatic_campus": false,
        "rule-providers": {
            "ips": dataset("geoip", "LOCAL", "ip.dat", &geoip),
            "blocked": dataset("geosite", "campus@blocked", "site.dat", &geosite),
            "allowed": dataset("geosite", "campus@allowed", "site.dat", &geosite),
        },
        "rules": [
            "AND,((GEOSITE,campus@blocked),(NETWORK,tcp)),REJECT",
            format!("AND,((GEOIP,LOCAL,no-resolve),(DST-PORT,{port}),(IN-TYPE,http)),DIRECT"),
            "OR,((GEOSITE,campus@allowed),(DOMAIN,extra.internal.invalid)),SHU-VPN",
            "MATCH,REJECT"
        ]
    }).to_string(), "{}".to_owned(), 0).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let (mut direct, reply) = http_tunnel(binding.port, "127.0.0.1", port).await?;
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    direct.write_all(b"real").await?;
    let mut received = [0; 4];
    direct.read_exact(&mut received).await?;
    assert_eq!(&received, b"real");
    physical_task.await??;
    assert_eq!(seen.load(Ordering::Relaxed), 1);
    assert!(gateway.destinations.lock().await.is_empty());
    let (_, reply) = http_tunnel(binding.port, "blocked.internal.invalid", 443).await?;
    assert!(reply.starts_with(b"HTTP/1.1 403"));
    assert!(gateway.destinations.lock().await.is_empty());
    let (mut via_gateway, reply) =
        http_tunnel(binding.port, "ALLOWED.internal.invalid.", 443).await?;
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    let mut greeting = [0; 7];
    via_gateway.read_exact(&mut greeting).await?;
    assert_eq!(&greeting, b"prelude");
    assert_eq!(
        gateway.destinations.lock().await.as_slice(),
        ["allowed.internal.invalid"]
    );
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    assert!(events
        .as_array()
        .is_some_and(|events| events.iter().any(|event| event["outbound"] == "DIRECT")));
    assert!(events.as_array().is_some_and(|events| events
        .iter()
        .any(|event| event["decision"]["action"] == "REJECT" && event["outbound"].is_null())));
    assert!(events
        .as_array()
        .is_some_and(|events| events.iter().any(|event| event["outbound"] == "SHU-VPN")));
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn selecting_shu_vpn_cannot_expand_authorization_or_fall_back_to_direct() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['NOT,((DOMAIN,never.example)),SHU-VPN', 'MATCH,REJECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let physical = TcpListener::bind("127.0.0.1:0").await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let (_, reply) = http_tunnel(binding.port, "127.0.0.1", physical.local_addr()?.port())
        .await
        .with_context(|| {
            let state = session.snapshot();
            format!(
                "proxy connect failed; session={:?}, message={:?}",
                state.state, state.message
            )
        })?;
    assert!(reply.starts_with(b"HTTP/1.1 403"));
    assert!(timeout(Duration::from_millis(100), physical.accept())
        .await
        .is_err());
    assert!(gateway.destinations.lock().await.is_empty());
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    assert_eq!(events[0]["decision"]["action"], "SHU-VPN");
    assert!(events[0]["outbound"].is_null());
    assert_eq!(events[0]["error"], "unrouted");
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn updates_change_new_flows_while_existing_tunnels_keep_their_original_path(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let (mut established, reply) = http_tunnel(binding.port, "keep.internal.invalid", 443).await?;
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    let mut greeting = [0; 7];
    established.read_exact(&mut greeting).await?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['MATCH,REJECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let (_, reply) = http_tunnel(binding.port, "keep.internal.invalid", 443).await?;
    assert!(reply.starts_with(b"HTTP/1.1 403"));
    established.write_all(b"kept").await?;
    let mut data = [0; 4];
    established.read_exact(&mut data).await?;
    assert_eq!(&data, b"kept");
    manager.rollback(1).await?;
    let (_, reply) = http_tunnel(binding.port, "again.internal.invalid", 443).await?;
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn direct_cannot_loop_back_into_the_engines_own_proxy_listener() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['MATCH,DIRECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let (_, reply) = http_tunnel(binding.port, "127.0.0.1", binding.port).await?;
    assert!(reply.starts_with(b"HTTP/1.1 403"));
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn disabling_a_reject_rule_requires_commit_and_keeps_existing_gateway_tunnels(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    let physical = TcpListener::bind("127.0.0.1:0").await?;
    let physical_port = physical.local_addr()?.port();
    let reject = "IP-CIDR,127.0.0.1/32,REJECT,no-resolve";
    let profile = serde_json::json!({
        "automatic_campus":false,
        "rules":[reject,"IP-CIDR,127.0.0.1/32,DIRECT,no-resolve","MATCH,SHU-VPN"]
    })
    .to_string();
    manager.apply(profile.clone(), "{}".to_owned(), 0).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let (mut established, reply) = http_tunnel(binding.port, "keep.internal.invalid", 443).await?;
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    let mut greeting = [0; 7];
    timeout(
        Duration::from_secs(3),
        established.read_exact(&mut greeting),
    )
    .await??;
    assert_eq!(&greeting, b"prelude");
    let (_, reply) = http_tunnel(binding.port, "127.0.0.1", physical_port).await?;
    assert!(reply.starts_with(b"HTTP/1.1 403"));

    let draft: Value = serde_json::from_str(&edit_routing(
        profile,
        "{}".to_owned(),
        serde_json::json!({
            "type":"update","source":"profile","index":0,
            "expression":reject,"enabled":false
        })
        .to_string(),
    )?)?;
    let preview = manager
        .prepare(
            draft["profile"]
                .as_str()
                .context("Missing edited profile")?
                .to_owned(),
            draft["overrides"]
                .as_str()
                .context("Missing edited overrides")?
                .to_owned(),
            1,
        )
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        1
    );
    let (_, reply) = http_tunnel(binding.port, "127.0.0.1", physical_port).await?;
    assert!(reply.starts_with(b"HTTP/1.1 403"));
    assert!(timeout(Duration::from_millis(100), physical.accept())
        .await
        .is_err());
    preview.commit().await?;
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        2
    );

    let physical_task = tokio::spawn(async move {
        let (mut socket, _) = timeout(Duration::from_secs(3), physical.accept()).await??;
        let mut data = [0; 4];
        timeout(Duration::from_secs(3), socket.read_exact(&mut data)).await??;
        assert_eq!(&data, b"real");
        socket.write_all(&data).await?;
        Ok::<_, anyhow::Error>(())
    });
    let (mut direct, reply) = http_tunnel(binding.port, "127.0.0.1", physical_port).await?;
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    direct.write_all(b"real").await?;
    let mut data = [0; 4];
    timeout(Duration::from_secs(3), direct.read_exact(&mut data)).await??;
    assert_eq!(&data, b"real");
    physical_task.await??;
    established.write_all(b"kept").await?;
    timeout(Duration::from_secs(3), established.read_exact(&mut data)).await??;
    assert_eq!(&data, b"kept");
    assert_eq!(
        gateway.destinations.lock().await.as_slice(),
        ["keep.internal.invalid"]
    );
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    let events = events.as_array().context("Missing decision history")?;
    assert!(events
        .iter()
        .any(|event| event["flow"]["host"] == "127.0.0.1"
            && event["decision"]["version"] == 2
            && event["outbound"] == "DIRECT"));
    assert!(events
        .iter()
        .any(|event| event["flow"]["host"] == "keep.internal.invalid"
            && event["decision"]["version"] == 1
            && event["outbound"] == "SHU-VPN"));
    assert_eq!(session.finish_stop().await?, None);
    Ok(())
}
