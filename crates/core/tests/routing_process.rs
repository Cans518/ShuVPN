use anyhow::Context;
use serde_json::{json, Value};
use shuvpn_core::api::{
    routing::RoutingManager,
    vpn::{ProxyKind, VpnSession},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    time::{timeout, Duration},
};

#[path = "support/datagram.rs"]
mod datagram;
#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;

async fn connect(
    kind: ProxyKind,
    proxy: u16,
    target: std::net::SocketAddr,
) -> anyhow::Result<(TcpStream, bool)> {
    let mut client = TcpStream::connect(("127.0.0.1", proxy)).await?;
    if kind == ProxyKind::Http {
        client
            .write_all(format!("CONNECT {target} HTTP/1.1\r\n\r\n").as_bytes())
            .await?;
        let mut reply = Vec::new();
        while !reply.ends_with(b"\r\n\r\n") {
            reply.push(timeout(Duration::from_secs(3), client.read_u8()).await??);
        }
        Ok((client, reply.starts_with(b"HTTP/1.1 200")))
    } else {
        client.write_all(&[5, 1, 0]).await?;
        let mut method = [0; 2];
        client.read_exact(&mut method).await?;
        assert_eq!(method, [5, 0]);
        let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
        request.extend_from_slice(&target.port().to_be_bytes());
        client.write_all(&request).await?;
        let mut reply = [0; 10];
        timeout(Duration::from_secs(3), client.read_exact(&mut reply)).await??;
        Ok((client, reply[1] == 0))
    }
}

#[tokio::test]
async fn actual_http_socks_process_name_and_path_control_the_real_physical_outbound(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let image = std::env::current_exe()?;
    let name = image
        .file_name()
        .and_then(|value| value.to_str())
        .context("test image name")?;
    manager
        .apply(
            json!({"automatic_campus":false,
        "rules":[format!("PROCESS-NAME,{name},DIRECT"),"MATCH,REJECT"]})
            .to_string(),
            "{}".into(),
            0,
        )
        .await?;
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 3000).await?;
    session.attach_routing(&manager)?;
    let target = TcpListener::bind("127.0.0.1:0").await?;
    let mut bindings = Vec::new();
    for kind in [ProxyKind::Http, ProxyKind::Socks] {
        let proxy = session.start_proxy(kind, "127.0.0.1".into(), 0).await?;
        let (mut client, success) = connect(kind, proxy.port, target.local_addr()?).await?;
        assert!(success, "{kind:?}: {}", session.decisions()?);
        let (mut server, _) = timeout(Duration::from_secs(3), target.accept()).await??;
        client.write_all(b"process").await?;
        let mut bytes = [0; 7];
        timeout(Duration::from_secs(3), server.read_exact(&mut bytes)).await??;
        assert_eq!(&bytes, b"process");
        server.write_all(&bytes).await?;
        timeout(Duration::from_secs(3), client.read_exact(&mut bytes)).await??;
        assert_eq!(&bytes, b"process");
        bindings.push((kind, proxy.port));
    }
    manager
        .apply(
            json!({"automatic_campus":false,
        "rules":[format!("PROCESS-PATH,{},REJECT", image.display()),"MATCH,DIRECT"]})
            .to_string(),
            "{}".into(),
            1,
        )
        .await?;
    for &(kind, proxy) in &bindings {
        let (_, success) = connect(kind, proxy, target.local_addr()?).await?;
        assert!(!success);
    }
    assert!(timeout(Duration::from_millis(100), target.accept())
        .await
        .is_err());
    manager
        .apply(
            json!({"automatic_campus":false,
        "rules":[format!("PROCESS-NAME,{name},SHU-VPN"),"MATCH,DIRECT"]})
            .to_string(),
            "{}".into(),
            2,
        )
        .await?;
    let (_, proxy) = bindings.first().copied().context("proxy missing")?;
    let (_, success) = connect(ProxyKind::Http, proxy, target.local_addr()?).await?;
    assert!(
        !success,
        "process rule must not expand gateway authorization"
    );
    assert!(timeout(Duration::from_millis(100), target.accept())
        .await
        .is_err());
    let events: Value = serde_json::from_str(&session.decisions()?)?;
    let events = events.as_array().context("events")?;
    assert_eq!(events.len(), 5);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["flow"]["process"]["pid"], std::process::id());
        assert_eq!(event["flow"]["process"]["name"], name);
        assert!(event["flow"]["process"]["source"]
            .as_str()
            .is_some_and(|value| value.ends_with("socket-snapshot")));
        assert_eq!(
            event["decision"]["action"],
            if index < 2 {
                "DIRECT"
            } else if index < 4 {
                "REJECT"
            } else {
                "SHU-VPN"
            }
        );
        if index >= 2 {
            assert!(event["outbound"].is_null());
        }
        if index == 4 {
            assert_eq!(event["error"], "unrouted");
        }
    }
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn an_unattributable_tun_packet_cannot_skip_a_process_rule_into_direct() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(
            json!({"automatic_campus":false,
        "rules":["PROCESS-NAME,unrelated,REJECT","MATCH,DIRECT"]})
            .to_string(),
            "{}".into(),
            0,
        )
        .await?;
    let gateway = gateway::Gateway::start(gateway::Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 3000).await?;
    session.attach_routing(&manager)?;
    let port = session.open_packet_port(None).await?;
    let target = UdpSocket::bind("127.0.0.1:0").await?;
    port.write_packet(datagram::packet(
        "10.99.0.2:51899".parse()?,
        target.local_addr()?,
        b"must-not-leak",
    ))
    .await?;
    timeout(Duration::from_secs(3), async {
        loop {
            let events: Value = serde_json::from_str(&session.decisions()?)?;
            if events
                .as_array()
                .is_some_and(|events| events.iter().any(|event| event["error"] == "process"))
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    assert!(
        timeout(Duration::from_millis(100), target.recv_from(&mut [0; 64]))
            .await
            .is_err()
    );
    assert!(gateway.destinations.lock().await.is_empty());
    session.stop().await?;
    Ok(())
}
