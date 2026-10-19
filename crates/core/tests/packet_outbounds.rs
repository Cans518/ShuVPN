use serde_json::Value;
use shuvpn_core::api::{
    routing::RoutingManager,
    vpn::{VpnPacketPort, VpnSession},
};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use tokio::{
    net::UdpSocket,
    time::{timeout, Duration},
};

#[path = "support/datagram.rs"]
mod datagram;
#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;
use gateway::{Gateway, Mode};

async fn response(port: &VpnPacketPort) -> anyhow::Result<Vec<u8>> {
    timeout(Duration::from_secs(3), async {
        loop {
            if let Some(packet) = port.read_packet(500).await? {
                return Ok(packet);
            }
        }
    })
    .await?
}

async fn decision_error(session: &VpnSession, code: &str) -> anyhow::Result<()> {
    timeout(Duration::from_secs(3), async {
        loop {
            let events: Value = serde_json::from_str(&session.decisions()?)?;
            if events
                .as_array()
                .is_some_and(|events| events.iter().any(|event| event["error"] == code))
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}

#[tokio::test]
async fn campus_udp_requires_signed_flow_authorization_and_returns_real_tls_packets(
) -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let port = session.open_packet_port(None).await?;
    let source = "10.99.0.2:51001".parse()?;
    let target = "10.21.0.5:9001".parse()?;
    let packet = datagram::packet(source, target, b"campus-udp");
    port.write_packet(packet.clone()).await?;
    assert_eq!(
        response(&port).await?,
        datagram::packet(target, source, b"campus-udp")
    );
    port.write_packet(packet.clone()).await?;
    assert_eq!(
        response(&port).await?,
        datagram::packet(target, source, b"campus-udp")
    );
    assert_eq!(gateway.uploaded.load(Ordering::Relaxed), packet.len() * 2);
    assert_eq!(gateway.destinations.lock().await.as_slice(), ["10.21.0.5"]);
    let decisions: Value = serde_json::from_str(&session.decisions()?)?;
    assert_eq!(decisions[0]["flow"]["inbound"], "tun");
    assert_eq!(decisions[0]["flow"]["network"], "udp");
    assert_eq!(decisions[0]["outbound"], "SHU-VPN");
    session.stop().await?;
    assert_eq!(
        port.read_packet(1).await.err().map(|error| error.code),
        Some("closed".to_owned())
    );
    port.close().await;
    timeout(Duration::from_secs(2), async {
        while gateway.connections.load(Ordering::Relaxed) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert!(
        gateway.errors.lock().await.is_empty(),
        "{:?}",
        gateway.errors.lock().await
    );
    Ok(())
}

#[tokio::test]
async fn direct_udp_has_real_dual_stack_sockets_and_preserves_virtual_reply_tuples(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(
            "name: direct\nautomatic_campus: false\nrules:\n - MATCH,DIRECT\n".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    for (address, virtual_source) in [
        ("127.0.0.1:0", "10.99.0.2:51002"),
        ("[::1]:0", "[2001:db8::2]:51003"),
    ] {
        let server = UdpSocket::bind(address).await?;
        let destination = server.local_addr()?;
        let source = virtual_source.parse()?;
        port.write_packet(datagram::packet(source, destination, b"physical-udp"))
            .await?;
        let mut buffer = [0; 64];
        let (length, peer) =
            timeout(Duration::from_secs(2), server.recv_from(&mut buffer)).await??;
        assert_eq!(&buffer[..length], b"physical-udp");
        assert_ne!(peer, source);
        server.send_to(&buffer[..length], peer).await?;
        let mut reply = response(&port).await?;
        let expected = datagram::packet(destination, source, b"physical-udp");
        if source.is_ipv4() {
            reply[26..28].fill(0);
        }
        assert_eq!(reply, expected);
    }
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn reject_and_unlicensed_campus_udp_never_fall_back_to_a_physical_socket(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let server = UdpSocket::bind("127.0.0.1:0").await?;
    let target = server.local_addr()?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    manager
        .apply("rules:\n - MATCH,REJECT\n".to_owned(), "{}".to_owned(), 0)
        .await?;
    let port = session.open_packet_port(None).await?;
    port.write_packet(datagram::packet(
        "10.99.0.2:51004".parse()?,
        target,
        b"blocked",
    ))
    .await?;
    decision_error(&session, "rejected").await?;
    manager
        .apply("rules:\n - MATCH,SHU-VPN\n".to_owned(), "{}".to_owned(), 1)
        .await?;
    port.write_packet(datagram::packet(
        "10.99.0.2:51005".parse()?,
        target,
        b"unauthorized",
    ))
    .await?;
    decision_error(&session, "unrouted").await?;
    let mut buffer = [0; 64];
    assert!(
        timeout(Duration::from_millis(100), server.recv_from(&mut buffer))
            .await
            .is_err()
    );
    assert_eq!(gateway.uploaded.load(Ordering::Relaxed), 0);
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn refused_or_mismatched_auth_cannot_release_queued_packets() -> anyhow::Result<()> {
    for (mode, code) in [(Mode::L3Refuse, "auth"), (Mode::L3BadHash, "protocol")] {
        let gateway = Gateway::start(mode).await?;
        let session = VpnSession::connect(gateway.plan(), 1000).await?;
        let port = session.open_packet_port(None).await?;
        port.write_packet(datagram::packet(
            "10.99.0.2:51006".parse()?,
            "10.21.0.5:9001".parse()?,
            b"must-not-send",
        ))
        .await?;
        decision_error(&session, code).await?;
        assert_eq!(gateway.uploaded.load(Ordering::Relaxed), 0);
        assert_eq!(port.read_packet(30).await?, None);
        session.stop().await?;
    }
    Ok(())
}

#[tokio::test]
async fn rule_updates_apply_to_new_udp_flows_and_keep_existing_flow_decisions() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply("rules:\n - MATCH,SHU-VPN\n".to_owned(), "{}".to_owned(), 0)
        .await?;
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    let target = "10.21.0.5:9001".parse()?;
    let original: SocketAddr = "10.99.0.2:51007".parse()?;
    port.write_packet(datagram::packet(original, target, b"before"))
        .await?;
    response(&port).await?;
    manager
        .apply(
            "automatic_campus: false\nrules:\n - MATCH,REJECT\n".to_owned(),
            "{}".to_owned(),
            1,
        )
        .await?;
    port.write_packet(datagram::packet(original, target, b"after"))
        .await?;
    assert_eq!(
        response(&port).await?,
        datagram::packet(target, original, b"after")
    );
    port.write_packet(datagram::packet("10.99.0.2:51008".parse()?, target, b"new"))
        .await?;
    decision_error(&session, "rejected").await?;
    assert_eq!(gateway.destinations.lock().await.len(), 1);
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn packet_port_validates_source_protocol_mtu_and_closes_pending_reads() -> anyhow::Result<()>
{
    let gateway = Gateway::start(Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let port = session.open_packet_port(None).await?;
    assert_eq!(
        session
            .open_packet_port(None)
            .await
            .err()
            .map(|error| error.code),
        Some("busy".to_owned())
    );
    let target = "10.21.0.5:9001".parse()?;
    let spoofed = datagram::packet("10.99.0.99:51009".parse()?, target, b"spoofed");
    assert_eq!(
        port.write_packet(spoofed)
            .await
            .err()
            .map(|error| error.code),
        Some("source".to_owned())
    );
    let too_large = datagram::packet("10.99.0.2:51009".parse()?, target, &vec![0; 1400]);
    assert_eq!(
        port.write_packet(too_large)
            .await
            .err()
            .map(|error| error.code),
        Some("packet".to_owned())
    );
    assert!(port.write_packet(vec![0x60; 40]).await.is_err());
    let read = port.read_packet(5000);
    let close = async {
        tokio::time::sleep(Duration::from_millis(30)).await;
        session.stop().await
    };
    let (read, close) =
        timeout(Duration::from_secs(2), async { tokio::join!(read, close) }).await?;
    close?;
    assert_eq!(
        read.err().map(|error| error.code),
        Some("closed".to_owned())
    );
    assert_eq!(
        session
            .open_packet_port(None)
            .await
            .err()
            .map(|error| error.code),
        Some("closed".to_owned())
    );
    Ok(())
}

#[tokio::test]
async fn unverified_campus_ipv6_and_unrelated_inbound_packets_do_not_reach_the_stack(
) -> anyhow::Result<()> {
    let gateway = Gateway::start(Mode::L3WrongPacket).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let port = session.open_packet_port(None).await?;
    port.write_packet(datagram::packet(
        "[2001:db8::2]:51010".parse()?,
        "[2001:db8:21::5]:9001".parse()?,
        b"v6",
    ))
    .await?;
    decision_error(&session, "capability").await?;
    assert_eq!(gateway.destinations.lock().await.len(), 0);
    port.write_packet(datagram::packet(
        "10.99.0.2:51011".parse()?,
        "10.21.0.5:9001".parse()?,
        b"v4",
    ))
    .await?;
    decision_error(&session, "packet").await?;
    assert_eq!(port.read_packet(30).await?, None);
    assert!(session.snapshot().tunnel_failures > 0);
    session.stop().await?;
    Ok(())
}
