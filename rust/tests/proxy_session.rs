use serde_json::json;
use shuvpn_core::api::vpn::{ProxyKind, VpnSession, VpnState};
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
// This suite uses only the non-DNS modes of the shared gateway fixture.
#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;
use gateway::{Gateway, Mode};

async fn connect_http(port: u16, target: &str) -> anyhow::Result<TcpStream> {
    let mut client = TcpStream::connect(("127.0.0.1", port)).await?;
    client
        .write_all(
            format!("CONNECT {target}:443 HTTP/1.1\r\nHost: {target}:443\r\n\r\n").as_bytes(),
        )
        .await?;
    let mut response = vec![];
    while !response.ends_with(b"\r\n\r\n") {
        response.push(client.read_u8().await?);
        anyhow::ensure!(response.len() < 4096, "unexpected HTTP reply");
    }
    anyhow::ensure!(
        response.starts_with(b"HTTP/1.1 200"),
        "HTTP proxy rejected: {}",
        String::from_utf8_lossy(&response)
    );
    Ok(client)
}

#[tokio::test]
async fn http_and_socks_use_real_pinned_tls_and_gateway_dns() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    assert_eq!(
        session.snapshot().virtual_address.as_deref(),
        Some("10.99.0.2")
    );
    let http = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let socks = session
        .start_proxy(ProxyKind::Socks, "127.0.0.1".to_owned(), 0)
        .await?;
    let mut http_client = connect_http(http.port, "echo.internal.invalid").await?;
    let mut prelude = [0; 7];
    http_client.read_exact(&mut prelude).await?;
    assert_eq!(&prelude, b"prelude");
    let payload = vec![42; 160 * 1024];
    http_client.write_all(&payload).await?;
    let mut received = vec![0; payload.len()];
    timeout(
        Duration::from_secs(3),
        http_client.read_exact(&mut received),
    )
    .await??;
    assert_eq!(received, payload);
    let mut socks_client = TcpStream::connect(("127.0.0.1", socks.port)).await?;
    socks_client.write_all(&[5, 1, 0]).await?;
    let mut method = [0; 2];
    socks_client.read_exact(&mut method).await?;
    assert_eq!(method, [5, 0]);
    let host = b"socks.internal.invalid";
    socks_client
        .write_all(
            &[
                vec![5, 1, 0, 3, host.len() as u8],
                host.to_vec(),
                vec![1, 0xbb],
            ]
            .concat(),
        )
        .await?;
    let mut reply = [0; 10];
    socks_client.read_exact(&mut reply).await?;
    assert_eq!(reply[1], 0);
    socks_client.read_exact(&mut prelude).await?;
    socks_client.write_all(b"socks-roundtrip").await?;
    let mut echoed = [0; 15];
    timeout(Duration::from_secs(2), socks_client.read_exact(&mut echoed)).await??;
    assert_eq!(&echoed, b"socks-roundtrip");
    assert_eq!(
        gateway.destinations.lock().await.as_slice(),
        ["echo.internal.invalid", "socks.internal.invalid"]
    );
    let snapshot = session.snapshot();
    assert!(snapshot.upload_bytes >= payload.len() as u64);
    assert!(snapshot.download_bytes >= payload.len() as u64);
    assert_eq!(snapshot.connections, 2);
    assert!(snapshot.latency_ms.is_some());
    timeout(Duration::from_secs(2), session.stop()).await??;
    assert_eq!(session.snapshot().state, VpnState::Disconnected);
    assert!(TcpStream::connect(("127.0.0.1", http.port)).await.is_err());
    assert!(TcpStream::connect(("127.0.0.1", socks.port)).await.is_err());
    Ok(())
}

#[tokio::test]
async fn mismatching_certificate_pin_fails_before_a_session_is_connected() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::Echo).await?;
    let mut plan: serde_json::Value = serde_json::from_str(&gateway.plan())?;
    plan["certificateDigests"] = json!(["00".repeat(32)]);
    assert!(VpnSession::connect(plan.to_string(), 1000).await.is_err());
    Ok(())
}

#[tokio::test]
async fn missing_heartbeats_fail_and_close_proxy_listeners() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::NoHeartbeat).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    timeout(Duration::from_secs(2), async {
        while session.snapshot().state != VpnState::Failed {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    session.stop().await?;
    assert!(TcpStream::connect(("127.0.0.1", binding.port))
        .await
        .is_err());
    Ok(())
}

#[tokio::test]
async fn concurrent_stops_and_port_rebinding_release_all_clients() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = Arc::new(VpnSession::connect(gateway.plan(), 1000).await?);
    let occupied = TcpListener::bind("127.0.0.1:0").await?;
    let requested = occupied.local_addr()?.port();
    let first = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), requested)
        .await?;
    assert_ne!(first.port, requested);
    let stalled = TcpStream::connect(("127.0.0.1", first.port)).await?;
    timeout(Duration::from_secs(1), session.stop_proxy(ProxyKind::Http)).await??;
    drop(stalled);
    let rebound = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), first.port)
        .await?;
    assert_eq!(rebound.port, first.port);
    let (left, right) = tokio::join!(session.stop(), session.stop());
    left?;
    right?;
    assert!(TcpStream::connect(("127.0.0.1", rebound.port))
        .await
        .is_err());
    timeout(Duration::from_secs(1), async {
        while gateway.connections.load(Ordering::Relaxed) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn wildcard_proxy_bindings_advertise_reachable_same_family_endpoints() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    for (kind, wildcard, loopback) in [
        (ProxyKind::Http, "0.0.0.0", "127.0.0.1"),
        (ProxyKind::Socks, "::", "::1"),
    ] {
        let binding = session.start_proxy(kind, wildcard.into(), 0).await?;
        let endpoint: std::net::SocketAddr = binding.display_endpoint.parse()?;
        assert_eq!(endpoint.is_ipv6(), wildcard == "::");
        assert_eq!(endpoint.port(), binding.port);
        let target = if endpoint.ip().is_unspecified() {
            assert!(binding.display_warning.is_some());
            std::net::SocketAddr::new(loopback.parse()?, binding.port)
        } else {
            assert!(binding.display_warning.is_none());
            assert!(!endpoint.ip().is_loopback());
            endpoint
        };
        eprintln!(
            "native listener {}:{} advertises {} warning={:?}",
            binding.address, binding.port, binding.display_endpoint, binding.display_warning
        );
        let mut client = timeout(Duration::from_secs(2), TcpStream::connect(target)).await??;
        if kind == ProxyKind::Socks {
            client.write_all(&[5, 1, 0]).await?;
            let mut method = [0; 2];
            timeout(Duration::from_secs(2), client.read_exact(&mut method)).await??;
            assert_eq!(method, [5, 0]);
        } else {
            client.write_all(b"INVALID\r\n\r\n").await?;
            let mut reply = [0; 12];
            timeout(Duration::from_secs(2), client.read_exact(&mut reply)).await??;
            assert_eq!(&reply, b"HTTP/1.1 400");
        }
        session.stop_proxy(kind).await?;
        assert!(TcpStream::connect(target).await.is_err());
    }
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn coalesced_second_vip_is_not_lost_after_the_handshake() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::SecondVip).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    timeout(Duration::from_secs(1), async {
        while session.snapshot().virtual_address.as_deref() != Some("10.99.0.3") {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn remote_half_close_does_not_truncate_the_clients_upload() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::HalfClose).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let mut client = connect_http(binding.port, "half.internal.invalid").await?;
    let mut greeting = [0; 7];
    client.read_exact(&mut greeting).await?;
    let mut byte = [0; 1];
    assert_eq!(
        timeout(Duration::from_secs(1), client.read(&mut byte)).await??,
        0
    );
    client.write_all(&vec![1; 64 * 1024]).await?;
    client.shutdown().await?;
    timeout(Duration::from_secs(2), async {
        while gateway.uploaded.load(Ordering::Relaxed) != 64 * 1024 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn absolute_http_rewrites_the_target_and_strips_proxy_credentials() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    let mut client = TcpStream::connect(("127.0.0.1", binding.port)).await?;
    client.write_all(b"POST http://post.internal.invalid/item?q=1 HTTP/1.1\r\nHost: wrong.invalid\r\nProxy-Authorization: Basic do-not-forward\r\nProxy-Connection: keep-alive\r\nContent-Length: 4\r\n\r\nbody").await?;
    let mut greeting = [0; 7];
    client.read_exact(&mut greeting).await?;
    let mut head = vec![];
    while !head.ends_with(b"\r\n\r\n") {
        head.push(client.read_u8().await?);
    }
    let head = String::from_utf8(head)?;
    assert!(head.starts_with("POST /item?q=1 HTTP/1.1\r\n"));
    assert!(head.contains("Host: post.internal.invalid\r\n"));
    assert!(!head.contains("do-not-forward"));
    assert!(!head.to_ascii_lowercase().contains("proxy-connection"));
    let mut body = [0; 4];
    client.read_exact(&mut body).await?;
    assert_eq!(&body, b"body");
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn malformed_http_headers_are_rejected_before_dialing() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    for headers in [
        "Host : bad\r\n",
        "Content-Length: 1\r\nContent-Length: 2\r\n",
        "Content-Length: 1\r\nTransfer-Encoding: chunked\r\n",
    ] {
        let mut client = TcpStream::connect(("127.0.0.1", binding.port)).await?;
        client
            .write_all(
                format!("GET http://valid.internal.invalid/ HTTP/1.1\r\n{headers}\r\n").as_bytes(),
            )
            .await?;
        let mut reply = [0; 12];
        client.read_exact(&mut reply).await?;
        assert_eq!(&reply, b"HTTP/1.1 400");
    }
    let mut malformed = TcpStream::connect(("127.0.0.1", binding.port)).await?;
    malformed.write_all(b"GARBAGE\r\n\r\n").await?;
    let mut reply = [0; 12];
    malformed.read_exact(&mut reply).await?;
    assert_eq!(&reply, b"HTTP/1.1 400");
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn unrouted_destinations_and_gateway_rejections_fail_explicitly() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::RefuseTcp).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    for (host, expected) in [
        ("127.0.0.1", b"HTTP/1.1 403"),
        ("refused.internal.invalid", b"HTTP/1.1 502"),
    ] {
        let mut client = TcpStream::connect(("127.0.0.1", binding.port)).await?;
        client
            .write_all(format!("CONNECT {host}:443 HTTP/1.1\r\n\r\n").as_bytes())
            .await?;
        let mut reply = [0; 12];
        client.read_exact(&mut reply).await?;
        assert_eq!(&reply, expected);
    }
    assert_eq!(session.snapshot().unrouted, 1);
    assert_eq!(session.snapshot().tunnel_failures, 1);
    assert_eq!(session.snapshot().connections, 0);
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn connect_attempt_can_cancel_a_stalled_tls_control_handshake() -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::HoldHandshake).await?;
    let attempt = Arc::new(shuvpn_core::api::vpn::VpnConnectAttempt::new());
    let worker = Arc::clone(&attempt);
    let plan = gateway.plan();
    let operation = tokio::spawn(async move { worker.connect(plan, 10_000).await });
    timeout(Duration::from_secs(2), async {
        while gateway.connections.load(Ordering::Relaxed) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    attempt.cancel();
    let failure = timeout(Duration::from_secs(1), operation)
        .await??
        .err()
        .ok_or_else(|| anyhow::anyhow!("cancelled connect succeeded"))?;
    assert_eq!(failure.code, "cancelled");
    Ok(())
}
