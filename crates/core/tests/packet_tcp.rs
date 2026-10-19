use anyhow::Context;
use serde_json::Value;
use shuvpn_core::api::{
    routing::RoutingManager,
    vpn::{ProxyKind, VpnSession},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::{timeout, Duration},
};

#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;
#[path = "support/tcp_client.rs"]
mod tcp_client;
use gateway::{Gateway, Mode};
use tcp_client::Client;

#[tokio::test]
async fn http_socks_and_tun_tcp_share_rule_decisions_and_the_actual_direct_endpoint(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager.apply("automatic_campus: false\nrules: ['IP-CIDR,127.0.0.0/8,DIRECT,no-resolve', 'MATCH,REJECT']".to_owned(), "{}".to_owned(), 0).await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    session.attach_routing(&manager)?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let target = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let mut received = vec![];
        for _ in 0..3 {
            let (mut socket, _) = listener.accept().await?;
            let mut payload = [0; 4];
            socket.read_exact(&mut payload).await?;
            socket.write_all(&payload).await?;
            socket.shutdown().await?;
            received.push(payload);
        }
        Ok::<_, std::io::Error>(received)
    });
    let http = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let mut client = tokio::net::TcpStream::connect(("127.0.0.1", http.port)).await?;
    client
        .write_all(format!("CONNECT {target} HTTP/1.1\r\n\r\n").as_bytes())
        .await?;
    let mut headers = vec![];
    while !headers.ends_with(b"\r\n\r\n") {
        headers.push(client.read_u8().await?);
    }
    assert!(headers.starts_with(b"HTTP/1.1 200"));
    client.write_all(b"http").await?;
    let mut reply = [0; 4];
    client.read_exact(&mut reply).await?;
    assert_eq!(&reply, b"http");
    drop(client);
    let socks = session
        .start_proxy(ProxyKind::Socks, "127.0.0.1".to_owned(), 0)
        .await?;
    let mut client = tokio::net::TcpStream::connect(("127.0.0.1", socks.port)).await?;
    client.write_all(&[5, 1, 0]).await?;
    let mut method = [0; 2];
    client.read_exact(&mut method).await?;
    assert_eq!(method, [5, 0]);
    let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
    request.extend_from_slice(&target.port().to_be_bytes());
    client.write_all(&request).await?;
    let mut connected = [0; 10];
    client.read_exact(&mut connected).await?;
    assert_eq!(&connected[..2], &[5, 0]);
    client.write_all(b"sock").await?;
    client.read_exact(&mut reply).await?;
    assert_eq!(&reply, b"sock");
    drop(client);
    let port = session.open_packet_port(None).await?;
    let mut client = Client::connect(&port, "10.99.0.2:52010".parse()?, target).await?;
    client.send(b"tun!").await?;
    assert_eq!(client.read(4).await?, b"tun!");
    client.wait_eof().await?;
    client.finish().await?;
    assert_eq!(
        timeout(Duration::from_secs(3), server).await???,
        [*b"http", *b"sock", *b"tun!"]
    );
    let decisions: Value = serde_json::from_str(&session.decisions()?)?;
    let decisions = decisions
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing decisions"))?;
    assert_eq!(decisions.len(), 3);
    assert_eq!(decisions[0]["decision"], decisions[1]["decision"]);
    assert_eq!(decisions[0]["decision"], decisions[2]["decision"]);
    for (event, inbound) in decisions.iter().zip(["http", "socks", "tun"]) {
        assert_eq!(event["flow"]["inbound"], inbound);
        assert_eq!(event["outbound"], "DIRECT");
    }
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn cancellation_joins_tcp_workers_when_the_packet_consumer_stops_reading(
) -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let port = session.open_packet_port(None).await?;
    for source_port in 53001..53081 {
        port.write_packet(tcp_client::packet(
            format!("10.99.0.2:{source_port}").parse()?,
            "10.21.0.5:443".parse()?,
            1000,
            0,
            2,
            32768,
            &[],
        ))
        .await?;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    timeout(Duration::from_secs(3), session.stop()).await??;
    assert_eq!(
        port.read_packet(1).await.err().map(|error| error.code),
        Some("closed".to_owned())
    );
    timeout(Duration::from_secs(3), async {
        while gateway
            .connections
            .load(std::sync::atomic::Ordering::Relaxed)
            != 0
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn campus_tcp_uses_real_signed_tls_for_both_ip_families() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let port = session.open_packet_port(None).await?;
    for (source, target) in [
        ("10.99.0.2:52000", "10.21.0.5:443"),
        ("[2001:db8::2]:52001", "[2001:db8:21::5]:443"),
    ] {
        let mut client = Client::connect(&port, source.parse()?, target.parse()?)
            .await
            .with_context(|| format!("campus TCP connect {target}"))?;
        assert_eq!(
            client
                .read(7)
                .await
                .with_context(|| format!("campus TCP prelude {target}"))?,
            b"prelude"
        );
        let payload: Vec<_> = (0..256 * 1024).map(|index| (index % 251) as u8).collect();
        client
            .send(&payload)
            .await
            .with_context(|| format!("campus TCP upload {target}"))?;
        assert_eq!(
            client
                .read(payload.len())
                .await
                .with_context(|| format!("campus TCP download {target}"))?,
            payload
        );
        client.finish().await?;
        client
            .wait_eof()
            .await
            .with_context(|| format!("campus TCP FIN {target}"))?;
    }
    assert_eq!(
        gateway.destinations.lock().await.as_slice(),
        ["10.21.0.5", "2001:db8:21::5"]
    );
    let decisions: Value = serde_json::from_str(&session.decisions()?)?;
    assert!(decisions
        .as_array()
        .is_some_and(
            |events| events.iter().all(|event| event["flow"]["network"] == "tcp"
                && event["flow"]["inbound"] == "tun"
                && event["outbound"] == "SHU-VPN")
        ));
    session.stop().await?;
    assert!(
        gateway.errors.lock().await.is_empty(),
        "{:?}",
        gateway.errors.lock().await
    );
    Ok(())
}

#[tokio::test]
async fn direct_tcp_has_real_dual_stack_endpoints_and_client_fin_keeps_reading_alive(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['MATCH,DIRECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    let payload: Vec<_> = (0..128 * 1024).map(|index| (index % 251) as u8).collect();
    for (listen, source) in [
        ("127.0.0.1:0", "10.99.0.2:52002"),
        ("[::1]:0", "[2001:db8::2]:52003"),
    ] {
        let listener = TcpListener::bind(listen).await?;
        let target = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            let mut bytes = vec![];
            socket.read_to_end(&mut bytes).await?;
            socket.write_all(&bytes).await?;
            socket.shutdown().await?;
            Ok::<_, std::io::Error>(bytes)
        });
        let mut client = Client::connect(&port, source.parse()?, target).await?;
        client.send(&payload).await?;
        client.finish().await?;
        assert_eq!(client.read(payload.len()).await?, payload);
        client.wait_eof().await?;
        assert_eq!(timeout(Duration::from_secs(3), server).await???, payload);
    }
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn remote_fin_preserves_the_clients_write_half_until_its_own_fin() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['MATCH,DIRECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let target = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        socket.write_all(b"bye").await?;
        socket.shutdown().await?;
        let mut bytes = vec![];
        socket.read_to_end(&mut bytes).await?;
        Ok::<_, std::io::Error>(bytes)
    });
    let mut client = Client::connect(&port, "10.99.0.2:52004".parse()?, target).await?;
    assert_eq!(client.read(3).await?, b"bye");
    client.wait_eof().await?;
    client.send(b"after-remote-fin").await?;
    client.finish().await?;
    assert_eq!(
        timeout(Duration::from_secs(3), server).await???,
        b"after-remote-fin"
    );
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn a_zero_window_resumes_without_losing_bytes_or_overflowing_the_tcp_buffers(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['MATCH,DIRECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let target = listener.local_addr()?;
    let payload: Vec<_> = (0..512 * 1024).map(|index| (index % 239) as u8).collect();
    let expected = payload.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        let mut trigger = [0; 1];
        socket.read_exact(&mut trigger).await?;
        socket.write_all(&payload).await?;
        socket.shutdown().await?;
        Ok::<_, std::io::Error>(())
    });
    let mut client = Client::connect(&port, "10.99.0.2:52005".parse()?, target).await?;
    client.window(0).await?;
    client.send(b"x").await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(session.snapshot().tunnel_failures, 0);
    client.window(32768).await?;
    assert_eq!(client.read(expected.len()).await?, expected);
    client.wait_eof().await?;
    client.finish().await?;
    timeout(Duration::from_secs(3), server).await???;
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn reject_and_unlicensed_tcp_reset_the_syn_without_opening_a_physical_target(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['MATCH,REJECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let target = listener.local_addr()?;
    for (version, source) in [(0, "10.99.0.2:52006"), (1, "10.99.0.2:52007")] {
        if version == 1 {
            manager
                .apply(
                    "automatic_campus: false\nrules: ['MATCH,SHU-VPN']".to_owned(),
                    "{}".to_owned(),
                    1,
                )
                .await?;
        }
        let result = Client::connect(&port, source.parse()?, target).await;
        assert!(result.is_err());
    }
    assert!(timeout(Duration::from_millis(100), listener.accept())
        .await
        .is_err());
    assert!(gateway.destinations.lock().await.is_empty());
    let decisions: Value = serde_json::from_str(&session.decisions()?)?;
    assert_eq!(decisions[0]["error"], "rejected");
    assert_eq!(decisions[1]["error"], "unrouted");
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn tcp_rule_updates_keep_existing_flows_and_cancellation_joins_blocked_readers(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply("rules: ['MATCH,SHU-VPN']".to_owned(), "{}".to_owned(), 0)
        .await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    let target = "10.21.0.5:443".parse()?;
    let mut client = Client::connect(&port, "10.99.0.2:52008".parse()?, target).await?;
    client.read(7).await?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['MATCH,REJECT']".to_owned(),
            "{}".to_owned(),
            1,
        )
        .await?;
    client.send(b"retained").await?;
    assert_eq!(client.read(8).await?, b"retained");
    assert!(Client::connect(&port, "10.99.0.2:52009".parse()?, target)
        .await
        .is_err());
    timeout(Duration::from_secs(2), session.stop()).await??;
    assert_eq!(
        port.read_packet(1).await.err().map(|error| error.code),
        Some("closed".to_owned())
    );
    Ok(())
}
