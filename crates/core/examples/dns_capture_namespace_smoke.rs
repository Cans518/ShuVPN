#[allow(dead_code)]
#[path = "../tests/support/gateway.rs"]
mod gateway;

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use hickory_proto::{
        op::{Message, Query, ResponseCode},
        rr::{Name, RecordType},
    };
    use shuvpn_core::api::{routing::RoutingManager, vpn::VpnSession};
    use std::{net::SocketAddr, process::Command};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpSocket, UdpSocket},
        time::{timeout, Duration},
    };
    anyhow::ensure!(
        std::fs::read_link("/proc/self/ns/net")? != std::fs::read_link("/proc/1/ns/net")?,
        "isolated namespace required"
    );
    fn routes() -> anyhow::Result<serde_json::Value> {
        let v4 = Command::new("/usr/sbin/ip")
            .args(["-j", "route", "show", "table", "main"])
            .output()?;
        let v6 = Command::new("/usr/sbin/ip")
            .args(["-j", "-6", "route", "show", "table", "main"])
            .output()?;
        Ok(
            serde_json::json!({"v4":serde_json::from_slice::<serde_json::Value>(&v4.stdout)?,
            "v6":serde_json::from_slice::<serde_json::Value>(&v6.stdout)?}),
        )
    }
    fn host_route(state: &serde_json::Value, address: &str) -> bool {
        ["v4", "v6"].iter().any(|family| {
            state[*family].as_array().is_some_and(|routes| {
                routes.iter().any(|route| {
                    route["dev"] == "shuvpndns"
                        && route["dst"].as_str().is_some_and(|destination| {
                            destination.split('/').next() == Some(address)
                        })
                })
            })
        })
    }
    async fn query(socket: &UdpSocket, host: &str) -> anyhow::Result<Message> {
        let mut message = Message::new();
        message
            .set_id(741)
            .set_recursion_desired(true)
            .add_query(Query::query(Name::from_ascii(host)?, RecordType::A));
        socket.send_to(&message.to_vec()?, "10.99.0.53:53").await?;
        let mut reply = [0; 4096];
        let (length, peer) =
            timeout(Duration::from_secs(3), socket.recv_from(&mut reply)).await??;
        anyhow::ensure!(
            peer == "10.99.0.53:53".parse::<SocketAddr>()?,
            "wrong DNS source"
        );
        let reply = Message::from_vec(&reply[..length])?;
        anyhow::ensure!(
            reply.id() == 741 && reply.queries() == message.queries(),
            "wrong DNS correlation"
        );
        Ok(reply)
    }
    let plan = serde_json::json!({"name":"shuvpndns","mtu":1400,
        "addresses":["10.99.0.2/32","2001:db8::2/128"],"routes":["10.21.0.0/16"],
        "protected_ips":["127.0.0.1"],"dns_capture":true})
    .to_string();
    let gateway =
        gateway::Gateway::start(gateway::Mode::DomainDns(gateway::DnsMode::Mapping)).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    session.start_tun(plan.clone(), "lo".to_owned()).await?;
    anyhow::ensure!(
        !host_route(&routes()?, "10.99.0.7"),
        "answer was statically captured"
    );
    let dns = UdpSocket::bind("10.99.0.2:0").await?;
    let answer = query(&dns, "fixture-dns.shu.edu.cn.").await?;
    anyhow::ensure!(
        answer.response_code() == ResponseCode::NoError && !answer.answers().is_empty(),
        "DNS proxy failed"
    );
    let installed = routes()?;
    anyhow::ensure!(
        host_route(&installed, "10.99.0.7") && host_route(&installed, "2001:db8::7"),
        "DNS reply preceded dual-stack routes"
    );
    let source4 = TcpSocket::new_v4()?;
    source4.bind("10.99.0.2:0".parse()?)?;
    let mut stream4 = timeout(
        Duration::from_secs(3),
        source4.connect("10.99.0.7:443".parse()?),
    )
    .await??;
    let mut prelude = [0; 7];
    stream4.read_exact(&mut prelude).await?;
    anyhow::ensure!(
        &prelude == b"prelude",
        "domain resource did not use campus TCP"
    );
    let source6 = TcpSocket::new_v6()?;
    source6.bind("[2001:db8::2]:0".parse()?)?;
    let mut stream6 = timeout(
        Duration::from_secs(3),
        source6.connect("[2001:db8::7]:443".parse()?),
    )
    .await??;
    stream6.read_exact(&mut prelude).await?;
    stream6.write_all(b"ipv6-name").await?;
    let mut reply6 = [0; 9];
    stream6.read_exact(&mut reply6).await?;
    anyhow::ensure!(&reply6 == b"ipv6-name", "mapped IPv6 data changed");
    stream6.shutdown().await?;
    let mut trailing = vec![];
    timeout(Duration::from_secs(2), stream6.read_to_end(&mut trailing)).await??;
    tokio::time::sleep(Duration::from_millis(3300)).await;
    let expired = routes()?;
    anyhow::ensure!(
        host_route(&expired, "10.99.0.7"),
        "TTL expiry broke an established flow"
    );
    anyhow::ensure!(
        !host_route(&expired, "2001:db8::7"),
        "unused IPv6 host route outlived TTL"
    );
    stream4.write_all(b"retained").await?;
    let mut reply4 = [0; 8];
    stream4.read_exact(&mut reply4).await?;
    anyhow::ensure!(
        &reply4 == b"retained",
        "leased mapping lost existing traffic"
    );
    stream4.shutdown().await?;
    timeout(Duration::from_secs(2), stream4.read_to_end(&mut trailing)).await??;
    timeout(Duration::from_secs(2), async {
        while host_route(&routes()?, "10.99.0.7") {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    let targets = gateway.destinations.lock().await.clone();
    anyhow::ensure!(
        targets
            .iter()
            .filter(|target| *target == "fixture-dns.shu.edu.cn")
            .count()
            == 2
            && !targets
                .iter()
                .any(|target| target == "10.99.0.7" || target == "2001:db8::7"),
        "domain-only resource identity was not recovered"
    );
    session
        .resolve_campus_dns("fixture-dns.shu.edu.cn".to_owned())
        .await?;
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    session.attach_routing(&manager)?;
    timeout(Duration::from_secs(2), async {
        while host_route(&routes()?, "10.99.0.7") {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    let mapped = query(&dns, "fixture-dns.shu.edu.cn.").await?;
    anyhow::ensure!(
        mapped.response_code() == ResponseCode::NoError,
        "UDP mapping refresh failed"
    );
    let udp = UdpSocket::bind("10.99.0.2:0").await?;
    udp.send_to(b"named-udp", "10.99.0.7:9001").await?;
    let mut udp_reply = [0; 32];
    let (length, source) = timeout(Duration::from_secs(3), udp.recv_from(&mut udp_reply)).await??;
    anyhow::ensure!(
        &udp_reply[..length] == b"named-udp" && source == "10.99.0.7:9001".parse::<SocketAddr>()?,
        "domain-only UDP flow did not use authenticated L3"
    );
    session.stop().await?;
    for mode in [gateway::DnsMode::BadId, gateway::DnsMode::ZeroTtl] {
        let gateway = gateway::Gateway::start(gateway::Mode::DomainDns(mode)).await?;
        let session = VpnSession::connect(gateway.plan(), 2000).await?;
        session.start_tun(plan.clone(), "lo".to_owned()).await?;
        let dns = UdpSocket::bind("10.99.0.2:0").await?;
        anyhow::ensure!(
            query(&dns, "fixture-dns.shu.edu.cn.")
                .await?
                .response_code()
                == ResponseCode::ServFail,
            "unsafe DNS result was published"
        );
        anyhow::ensure!(
            !host_route(&routes()?, "10.99.0.7"),
            "unsafe DNS result created a route"
        );
        session.stop().await?;
    }
    let gateway =
        gateway::Gateway::start(gateway::Mode::DomainDns(gateway::DnsMode::Mapping)).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    anyhow::ensure!(
        Command::new("/usr/sbin/ip")
            .args(["route", "add", "10.99.0.7/32", "dev", "lo"])
            .status()?
            .success(),
        "conflict setup failed"
    );
    session.start_tun(plan, "lo".to_owned()).await?;
    let dns = UdpSocket::bind("10.99.0.2:0").await?;
    anyhow::ensure!(
        query(&dns, "fixture-dns.shu.edu.cn.")
            .await?
            .response_code()
            == ResponseCode::ServFail,
        "foreign route overwritten"
    );
    anyhow::ensure!(
        !host_route(&routes()?, "10.99.0.7"),
        "conflicting route was captured"
    );
    session.stop().await?;
    println!("TUN_DNS_DOMAIN_ONLY_V4_V6_TTL_LEASE_VERSION_INVALID_AND_CONFLICT_PASSED");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Linux namespaces required");
    std::process::exit(1);
}
