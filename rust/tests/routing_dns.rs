use serde_json::Value;
use shuvpn_core::api::{
    routing::RoutingManager,
    vpn::{ProxyKind, VpnSession},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{timeout, Duration},
};

#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;
use gateway::{DnsMode, Gateway, Mode};

#[tokio::test]
async fn explicit_campus_dns_preserves_source_version_and_cache_without_enabling_capture(
) -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::DomainDns(DnsMode::Mapping)).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let first: Value = serde_json::from_str(
        &session
            .resolve_campus_dns("fixture-dns.shu.edu.cn".to_owned())
            .await?,
    )?;
    assert_eq!(first["source"]["kind"], "campus");
    assert_eq!(first["source"]["server"], "10.99.0.53");
    assert_eq!(first["ttl_seconds"], 3);
    assert_eq!(
        first["addresses"],
        serde_json::json!(["10.99.0.7", "2001:db8::7"])
    );
    let second: Value = serde_json::from_str(
        &session
            .resolve_campus_dns("fixture-dns.shu.edu.cn".to_owned())
            .await?,
    )?;
    assert_eq!(second["cached"], true);
    assert_eq!(
        serde_json::from_str::<Value>(&session.capture_status().await?)?["active"],
        false
    );
    assert_eq!(
        serde_json::from_str::<Value>(&session.dns_stats()?)?["system_lookups"],
        0
    );
    session.stop().await?;
    Ok(())
}

async fn connect(proxy: u16, host: &str, port: u16) -> anyhow::Result<Vec<u8>> {
    let mut socket = TcpStream::connect(("127.0.0.1", proxy)).await?;
    socket
        .write_all(format!("CONNECT {host}:{port} HTTP/1.1\r\n\r\n").as_bytes())
        .await?;
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        header.push(timeout(Duration::from_secs(5), socket.read_u8()).await??);
    }
    if header.starts_with(b"HTTP/1.1 200") {
        let mut prelude = [0; 7];
        timeout(Duration::from_secs(2), socket.read_exact(&mut prelude)).await??;
        assert_eq!(&prelude, b"prelude");
        socket.write_all(b"dns-path").await?;
        let mut echo = [0; 8];
        timeout(Duration::from_secs(2), socket.read_exact(&mut echo)).await??;
        assert_eq!(&echo, b"dns-path");
    }
    Ok(header)
}

const PROFILE: &str =
    "automatic_campus: false\nrules: ['IP-CIDR,10.99.0.0/24,SHU-VPN', 'MATCH,REJECT']";

#[tokio::test]
async fn an_ipv6_only_answer_is_authorized_and_sent_as_an_ipv6_tunnel_destination(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(
            "automatic_campus: false\nrules: ['IP-CIDR6,2001:db8::/32,SHU-VPN', 'MATCH,REJECT']"
                .to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let gateway = Gateway::start(Mode::Dns(DnsMode::FailA)).await?;
    let session = VpnSession::connect(gateway.plan(), 4000).await?;
    session.attach_routing(&manager)?;
    let proxy = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    assert!(connect(proxy.port, "fixture-dns.shu.edu.cn", 443)
        .await?
        .starts_with(b"HTTP/1.1 200"));
    assert!(gateway
        .destinations
        .lock()
        .await
        .iter()
        .any(|target| target == "2001:db8::7"));
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    assert_eq!(
        events[0]["dns"][0]["addresses"],
        serde_json::json!(["2001:db8::7"])
    );
    assert_eq!(
        serde_json::from_str::<Value>(&session.dns_stats()?)?["system_lookups"],
        0
    );
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn campus_dns_uses_the_authorized_tls_path_and_cache_expires_and_changes_version(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(PROFILE.to_owned(), "{}".to_owned(), 0)
        .await?;
    let gateway = Gateway::start(Mode::Dns(DnsMode::Valid)).await?;
    let session = VpnSession::connect(gateway.plan(), 4000).await?;
    session.attach_routing(&manager)?;
    let proxy = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    for _ in 0..2 {
        let response = connect(proxy.port, "fixture-dns.shu.edu.cn", 443).await?;
        assert!(
            response.starts_with(b"HTTP/1.1 200"),
            "response={}, gateway_errors={:?}, decisions={}",
            String::from_utf8_lossy(&response),
            gateway.errors.lock().await,
            session.decisions()?
        );
    }
    assert_eq!(gateway.dns_queries.lock().await.len(), 2);
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    let first = &events[0]["dns"][0];
    assert_eq!(first["source"]["kind"], "campus");
    assert_eq!(first["source"]["server"], "10.99.0.53");
    assert_eq!(first["ttl_seconds"], 1);
    assert_eq!(first["version"], 1);
    assert_eq!(first["cached"], false);
    assert_eq!(events[1]["dns"][0]["cached"], true);
    assert_eq!(events[1]["outbound"], "SHU-VPN");
    let stats: Value = serde_json::from_str(&session.dns_stats()?)?;
    assert_eq!(stats["system_lookups"], 0);
    assert_eq!(stats["cache_hits"], 1);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    connect(proxy.port, "fixture-dns.shu.edu.cn", 443).await?;
    assert_eq!(gateway.dns_queries.lock().await.len(), 4);
    manager
        .apply(PROFILE.to_owned(), "{}".to_owned(), 1)
        .await?;
    connect(proxy.port, "fixture-dns.shu.edu.cn", 443).await?;
    assert_eq!(gateway.dns_queries.lock().await.len(), 6);
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    assert_eq!(events[3]["dns"][0]["version"], 2);
    session.stop().await?;
    assert_eq!(
        serde_json::from_str::<Value>(&session.dns_stats()?)?["cached_entries"],
        0
    );
    Ok(())
}

#[tokio::test]
async fn zero_ttl_and_partial_family_failure_are_not_cached_or_sent_to_public_dns(
) -> anyhow::Result<()> {
    for mode in [DnsMode::ZeroTtl, DnsMode::FailAaaa, DnsMode::HangAaaa] {
        let directory = tempfile::tempdir()?;
        let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
        manager
            .apply(PROFILE.to_owned(), "{}".to_owned(), 0)
            .await?;
        let gateway = Gateway::start(Mode::Dns(mode)).await?;
        let session = VpnSession::connect(gateway.plan(), 4000).await?;
        session.attach_routing(&manager)?;
        let proxy = session
            .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
            .await?;
        for _ in 0..2 {
            assert!(connect(proxy.port, "fixture-dns.shu.edu.cn", 443)
                .await?
                .starts_with(b"HTTP/1.1 200"));
        }
        let expected = if matches!(mode, DnsMode::HangAaaa) {
            6
        } else {
            4
        };
        assert_eq!(gateway.dns_queries.lock().await.len(), expected);
        let stats: Value = serde_json::from_str(&session.dns_stats()?)?;
        assert_eq!(stats["system_lookups"], 0);
        assert_eq!(stats["cached_entries"], 0);
        let events: Value = serde_json::from_str(&session.decisions()?)?;
        assert_eq!(events[1]["dns"][0]["cached"], false);
        match mode {
            DnsMode::ZeroTtl => assert_eq!(events[0]["dns"][0]["ttl_seconds"], 0),
            _ => assert_eq!(
                events[0]["dns"][0]["warnings"].as_array().map(Vec::len),
                Some(1)
            ),
        }
        session.stop().await?;
    }
    Ok(())
}

#[tokio::test]
async fn unrelated_answers_and_bad_ids_fail_without_a_public_lookup_or_target_dial(
) -> anyhow::Result<()> {
    for mode in [DnsMode::Unrelated, DnsMode::BadId] {
        let directory = tempfile::tempdir()?;
        let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
        manager
            .apply(PROFILE.to_owned(), "{}".to_owned(), 0)
            .await?;
        let gateway = Gateway::start(Mode::Dns(mode)).await?;
        let session = VpnSession::connect(gateway.plan(), 4000).await?;
        session.attach_routing(&manager)?;
        let proxy = session
            .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
            .await?;
        assert!(connect(proxy.port, "fixture-dns.shu.edu.cn", 443)
            .await?
            .starts_with(b"HTTP/1.1 502"));
        assert_eq!(
            serde_json::from_str::<Value>(&session.dns_stats()?)?["system_lookups"],
            0
        );
        assert!(gateway
            .destinations
            .lock()
            .await
            .iter()
            .all(|host| host == "10.99.0.53"));
        let events: Value = serde_json::from_str(&session.decisions()?)?;
        assert_eq!(events[0]["error"], "dns");
        session.stop().await?;
    }
    Ok(())
}

#[tokio::test]
async fn legacy_campus_name_without_an_authorized_dns_channel_never_uses_system_dns(
) -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 4000).await?;
    let proxy = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    assert!(connect(proxy.port, "fixture-dns.shu.edu.cn", 443)
        .await?
        .starts_with(b"HTTP/1.1 502"));
    assert_eq!(
        serde_json::from_str::<Value>(&session.dns_stats()?)?["system_lookups"],
        0
    );
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn direct_uses_system_resolution_with_unknown_ttl_and_no_resolve_rejects_before_dns(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager.apply("automatic_campus: false\nrules: ['IP-CIDR,10.99.0.0/24,SHU-VPN,no-resolve', 'MATCH,REJECT']".to_owned(), "{}".to_owned(), 0).await?;
    let gateway = Gateway::start(Mode::Dns(DnsMode::Valid)).await?;
    let session = VpnSession::connect(gateway.plan(), 4000).await?;
    session.attach_routing(&manager)?;
    let proxy = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    assert!(connect(proxy.port, "fixture-dns.shu.edu.cn", 443)
        .await?
        .starts_with(b"HTTP/1.1 403"));
    assert!(gateway.dns_queries.lock().await.is_empty());
    manager
        .apply("rules: ['MATCH,DIRECT']".to_owned(), "{}".to_owned(), 1)
        .await?;
    let endpoint = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = endpoint.local_addr()?.port();
    let server = tokio::spawn(async move {
        let (mut socket, _) = endpoint.accept().await?;
        socket.write_all(b"prelude").await?;
        let mut echo = [0; 8];
        socket.read_exact(&mut echo).await?;
        socket.write_all(&echo).await?;
        Ok::<_, std::io::Error>(())
    });
    assert!(connect(proxy.port, "localhost", port)
        .await?
        .starts_with(b"HTTP/1.1 200"));
    server.await??;
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    assert_eq!(events[1]["dns"][0]["source"]["kind"], "system");
    assert!(events[1]["dns"][0]["ttl_seconds"].is_null());
    assert_eq!(events[1]["outbound"], "DIRECT");
    session.stop().await?;
    Ok(())
}
