#[cfg(target_os = "windows")]
#[allow(dead_code)]
#[path = "../tests/support/gateway.rs"]
mod gateway;

#[cfg(target_os = "windows")]
mod probe {
    use super::gateway;
    use anyhow::{ensure, Context};
    use hickory_proto::{
        op::{Message, MessageType, Query},
        rr::{Name, RecordType},
    };
    use serde_json::{json, Value};
    use shuvpn_core::api::{desktop, routing::RoutingManager, tun, vpn::VpnSession};
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::atomic::Ordering,
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpSocket, TcpStream, UdpSocket},
    };
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NOT_FOUND},
        NetworkManagement::{
            IpHelper::{
                ConvertInterfaceIndexToLuid, GetBestRoute2, GetIfEntry2, MIB_IF_ROW2,
                MIB_IPFORWARD_ROW2,
            },
            Ndis::NET_LUID_LH,
        },
        Networking::WinSock::{AF_INET, SOCKADDR_IN, SOCKADDR_INET},
    };

    const TARGET4: &str = "192.0.2.77";
    const TARGET6: &str = "2001:db8:77::7";
    const SOURCE4: &str = "10.99.0.2";
    const SOURCE6: &str = "2001:db8::2";

    pub async fn run() -> anyhow::Result<()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        ensure!(args.len() == 3 && ["--plan", "--run-privileged", "--run-local-elevated"].contains(&args[0].as_str()),
            "Usage: desktop_windows_tun_smoke --plan|--run-privileged|--run-local-elevated <interface-alias> <explicit-ipv4-dns-upstream>");
        let alias = &args[1];
        let dns: Ipv4Addr = args[2]
            .parse()
            .context("DNS must be an explicitly chosen IPv4 upstream")?;
        ensure!(
            !dns.is_unspecified()
                && !dns.is_loopback()
                && !dns.is_multicast()
                && !dns.is_broadcast(),
            "DNS must be a unicast non-loopback resolver"
        );
        ensure!(
            dns.to_string() != TARGET4,
            "DNS cannot equal the documentation fixture target"
        );
        let choices = desktop::desktop_network_interfaces().await?;
        ensure!(
            choices.iter().any(|choice| choice.name == *alias),
            "physical interface is not currently eligible"
        );
        let configuration = json!({
            "mtu":1400, "addresses":[format!("{SOURCE4}/32"), format!("{SOURCE6}/128")],
            "routes":[format!("{TARGET4}/32"), format!("{TARGET6}/128"), format!("{dns}/32")],
            "protected_ips":["127.0.0.1"], "physical_dns_servers":[dns], "dns_capture":false,
        });
        println!(
            "DESKTOP_WINDOWS_TUN_PROBE_PLAN {}",
            json!({
                "interface":alias, "tun":configuration,
                "gateway":"local pinned TLS fixture; no campus credentials",
                "traffic":"signed IPv4 UDP and dual-stack TCP; DIRECT UDP/TCP DNS to the explicitly supplied upstream",
            })
        );
        if args[0] == "--plan" {
            return Ok(());
        }
        let driver: Value = serde_json::from_str(&tun::tun_driver_info().await?)?;
        let local = args[0] == "--run-local-elevated";
        ensure!(
            driver["elevated"] == local,
            "helper mode requires an ordinary process; local control requires an elevated process"
        );
        ensure!(
            driver["available"] == true && driver["sessionDataPlaneReady"] == true,
            "packaged driver/helper did not report the production capability"
        );
        println!(
            "DESKTOP_WINDOWS_TUN_EXECUTION_MODE {}",
            if local {
                "elevated-local"
            } else {
                "ordinary-helper"
            }
        );
        let directory = std::env::current_exe()?
            .parent()
            .context("probe directory")?
            .to_owned();
        ensure!(
            directory.join("shuvpn_tun_helper.exe").is_file(),
            "place the matching helper beside the probe"
        );
        let gateway = gateway::Gateway::start(gateway::Mode::L3Echo).await?;
        let mut plan: Value = serde_json::from_str(&gateway.plan())?;
        plan["dnsServers"] = json!([]);
        for route in plan["routes"].as_array_mut().context("fixture routes")? {
            match route["host"].as_str() {
                Some("10.21.0.0/16") => route["host"] = json!(format!("{TARGET4}/32")),
                Some("2001:db8:21::/64") => route["host"] = json!(format!("{TARGET6}/128")),
                _ => {}
            }
        }
        let resources = plan["routes"].as_array_mut().context("fixture routes")?;
        resources.retain(|route| {
            !route["host"]
                .as_str()
                .is_some_and(|host| host.starts_with("*."))
        });
        resources.push(json!({
            "host":format!("{dns}/32"), "protocol":"tcp", "portMin":53, "portMax":53,
            "appId":"test-app","nodeGroupId":"group","addrPretend":false,"enableTcpPrefL3":false,
        }));
        let session = VpnSession::connect(plan.to_string(), 3000).await?;
        let formal: Value = serde_json::from_str(
            &session
                .desktop_tun_plan(1500, Some(dns.to_string()), None, None)
                .await?,
        )?;
        ensure!(
            route_set(&formal)? == route_set(&configuration)?
                && formal["addresses"] == configuration["addresses"]
                && formal["mtu"] == 1400
                && formal["dns_capture"] == false
                && formal["physical_dns_servers"] == configuration["physical_dns_servers"],
            "formal desktop plan changed the approved capture scope"
        );
        println!("DESKTOP_WINDOWS_FORMAL_PLAN_VALIDATED {formal}");
        let policy_directory = tempfile::tempdir()?;
        let manager =
            RoutingManager::open(policy_directory.path().to_string_lossy().into_owned()).await?;
        let policy = format!("automatic_campus: false\nrules:\n  - 'IP-CIDR,{dns}/32,DIRECT,no-resolve'\n  - MATCH,SHU-VPN\n");
        manager.apply(policy, "{}".into(), 0).await?;
        session.attach_routing(&manager)?;
        let tested = exercise(&session, &configuration, alias, dns).await;
        let before_cleanup = session
            .tun_status()
            .await
            .ok()
            .and_then(|status| serde_json::from_str::<Value>(&status).ok());
        let identity = before_cleanup
            .as_ref()
            .and_then(|snapshot| snapshot["interface"]["index"].as_u64())
            .map(|index| {
                let mut luid = NET_LUID_LH::default();
                // SAFETY: native index is bounded by u32 and the output is writable.
                let code = unsafe { ConvertInterfaceIndexToLuid(index as u32, &mut luid) };
                (code, luid)
            });
        if let Some(status) = &before_cleanup {
            println!("DESKTOP_WINDOWS_TUN_BEFORE_CLEANUP {status}");
        }
        if let Ok(decisions) = session.decisions() {
            println!("DESKTOP_WINDOWS_TUN_DECISIONS {decisions}");
        }
        let totals = session.snapshot();
        println!(
            "DESKTOP_WINDOWS_TUN_TOTALS {}",
            json!({
                "uploadBytes": totals.upload_bytes, "downloadBytes": totals.download_bytes,
                "connections": totals.connections, "unrouted": totals.unrouted,
                "failures": totals.tunnel_failures, "message": totals.message,
            })
        );
        if let Err(error) = &tested {
            eprintln!("DESKTOP_WINDOWS_DATA_FAILURE {error:#}");
        }
        let stopped = tokio::time::timeout(Duration::from_secs(8), session.finish_stop()).await;
        let diagnostic = stopped.context("session stop exceeded eight seconds")??;
        ensure!(
            diagnostic.is_none(),
            "session required recovery: {diagnostic:?}"
        );
        let status: Value = serde_json::from_str(&session.tun_status().await?)?;
        ensure!(
            status["state"] == "stopped" && status["cleanup_pending"] == false,
            "native cleanup remains pending"
        );
        let egress: Value = serde_json::from_str(&session.egress_status()?)?;
        ensure!(egress["active"] == false, "physical egress remains active");
        if let Some((code, luid)) = identity {
            ensure!(
                code == 0,
                "installed interface LUID was not retained: {code}"
            );
            let mut row = MIB_IF_ROW2 {
                InterfaceLuid: luid,
                ..Default::default()
            };
            // SAFETY: query the exact LUID captured before cleanup, not an alias lookup.
            let absent = unsafe { GetIfEntry2(&mut row) };
            ensure!(
                [ERROR_FILE_NOT_FOUND, ERROR_NOT_FOUND].contains(&absent),
                "interface removal not proven: {absent}"
            );
        }
        println!("DESKTOP_WINDOWS_CLEANUP_CONFIRMED {status}");
        tested?;
        tokio::time::timeout(Duration::from_secs(3), async {
            while gateway.connections.load(Ordering::Relaxed) != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .context("fixture connections did not drain")?;
        ensure!(
            gateway.errors.lock().await.is_empty(),
            "fixture recorded errors: {:?}",
            gateway.errors.lock().await
        );
        println!(
            "{}",
            if local {
                "DESKTOP_WINDOWS_LOCAL_KERNEL_CAMPUS_DIRECT_DNS_AND_CLEANUP_PASSED"
            } else {
                "DESKTOP_WINDOWS_SESSION_KERNEL_CAMPUS_DIRECT_DNS_AND_HELPER_CLEANUP_PASSED"
            }
        );
        Ok(())
    }

    async fn exercise(
        session: &VpnSession,
        plan: &Value,
        alias: &str,
        dns: Ipv4Addr,
    ) -> anyhow::Result<()> {
        eprintln!("Starting only the printed host routes; an ordinary process requests UAC for the matching helper.");
        let installed: Value = serde_json::from_str(
            &session
                .start_desktop_tun(1500, alias.to_owned(), Some(dns.to_string()), None)
                .await?,
        )?;
        ensure!(
            installed["mtu"] == 1400 && route_set(&installed)? == route_set(plan)?,
            "formal desktop entry did not install the expected routes/MTU"
        );
        let name = installed["name"]
            .as_str()
            .context("installed interface name")?
            .to_owned();
        println!("DESKTOP_WINDOWS_TUN_INSTALLED {installed}");
        let initial: Value = serde_json::from_str(&session.tun_status().await?)?;
        ensure!(
            initial["state"] == "running" && initial["physical_interface"] == *alias,
            "TUN running state or physical interface mismatch"
        );
        let udp = UdpSocket::bind(format!("{SOURCE4}:0")).await?;
        println!(
            "DESKTOP_WINDOWS_UDP_ROUTE {}",
            json!({
                "socket": udp.local_addr()?.to_string(),
                "automatic": route_choice(None, TARGET4.parse()?),
                "boundSource": route_choice(Some(SOURCE4.parse()?), TARGET4.parse()?),
            })
        );
        udp.send_to(b"desktop-campus-udp", format!("{TARGET4}:9001"))
            .await?;
        let mut buffer = [0; 2048];
        let (length, peer) =
            tokio::time::timeout(Duration::from_secs(5), udp.recv_from(&mut buffer))
                .await
                .context("campus IPv4 UDP echo timed out")??;
        ensure!(
            &buffer[..length] == b"desktop-campus-udp" && peer.ip() == TARGET4.parse::<IpAddr>()?,
            "signed campus UDP changed"
        );
        drop(udp);
        println!("DESKTOP_WINDOWS_CAMPUS_UDP_PASSED");
        for (source, target) in [(SOURCE4, TARGET4), (SOURCE6, TARGET6)] {
            let mut socket = connect(source, target.parse()?, 443).await?;
            let mut prelude = [0; 7];
            tokio::time::timeout(Duration::from_secs(5), socket.read_exact(&mut prelude))
                .await
                .with_context(|| format!("campus TCP {target} prelude timed out"))??;
            ensure!(&prelude == b"prelude", "campus TCP fixture prelude changed");
            let payload: Vec<_> = (0..128 * 1024).map(|index| (index % 251) as u8).collect();
            tokio::time::timeout(Duration::from_secs(8), async {
                socket.write_all(&payload).await?;
                socket.shutdown().await?;
                let mut reply = vec![];
                socket.read_to_end(&mut reply).await?;
                ensure!(reply == payload, "campus TCP payload/FIN changed");
                Ok::<_, anyhow::Error>(())
            })
            .await
            .with_context(|| format!("campus TCP {target} payload/FIN timed out"))??;
            println!("DESKTOP_WINDOWS_CAMPUS_TCP_PASSED {target}");
        }
        let mut request = Message::new();
        request
            .set_id(0x739a)
            .set_recursion_desired(true)
            .add_query(Query::query(
                Name::from_ascii("shuvpn-probe.invalid")?,
                RecordType::A,
            ));
        let query = request.to_vec()?;
        let socket = UdpSocket::bind(format!("{SOURCE4}:0")).await?;
        socket.send_to(&query, SocketAddr::from((dns, 53))).await?;
        let (length, peer) =
            tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut buffer))
                .await
                .context("DIRECT DNS UDP response timed out")??;
        ensure!(
            peer == SocketAddr::from((dns, 53)),
            "DIRECT DNS UDP peer changed"
        );
        check_dns(&buffer[..length], request.id())?;
        drop(socket);
        println!("DESKTOP_WINDOWS_DIRECT_DNS_UDP_PASSED");
        let mut socket = connect(SOURCE4, IpAddr::V4(dns), 53).await?;
        tokio::time::timeout(Duration::from_secs(5), async {
            socket
                .write_all(&(query.len() as u16).to_be_bytes())
                .await?;
            socket.write_all(&query).await?;
            let length = socket.read_u16().await?;
            ensure!(length <= 4096, "DNS TCP response exceeds probe budget");
            let mut response = vec![0; usize::from(length)];
            socket.read_exact(&mut response).await?;
            check_dns(&response, request.id())?;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("DIRECT DNS TCP response timed out")??;
        drop(socket);
        println!("DESKTOP_WINDOWS_DIRECT_DNS_TCP_PASSED {name}");
        Ok(())
    }

    async fn connect(source: &str, target: IpAddr, port: u16) -> anyhow::Result<TcpStream> {
        let socket = if target.is_ipv4() {
            TcpSocket::new_v4()?
        } else {
            TcpSocket::new_v6()?
        };
        socket.bind(SocketAddr::new(source.parse()?, 0))?;
        Ok(tokio::time::timeout(
            Duration::from_secs(5),
            socket.connect(SocketAddr::new(target, port)),
        )
        .await
        .with_context(|| format!("kernel TCP connect timed out: {target}:{port}"))??)
    }

    fn route_choice(source: Option<Ipv4Addr>, destination: Ipv4Addr) -> Value {
        fn address(ip: Ipv4Addr) -> SOCKADDR_INET {
            let mut socket = SOCKADDR_IN {
                sin_family: AF_INET,
                ..Default::default()
            };
            socket.sin_addr.S_un.S_addr = u32::from_ne_bytes(ip.octets());
            SOCKADDR_INET { Ipv4: socket }
        }
        let source = source.map(address);
        let destination = address(destination);
        let mut route = MIB_IPFORWARD_ROW2::default();
        let mut selected = SOCKADDR_INET::default();
        // SAFETY: this query only reads routing state. All input/output
        // structures are live; a null source leaves source selection automatic.
        let code = unsafe {
            GetBestRoute2(
                std::ptr::null(),
                0,
                source.as_ref().map_or(std::ptr::null(), std::ptr::from_ref),
                &destination,
                0,
                &mut route,
                &mut selected,
            )
        };
        let selected = if code == 0 && unsafe { selected.si_family } == AF_INET {
            Some(
                Ipv4Addr::from(unsafe { selected.Ipv4.sin_addr.S_un.S_addr }.to_ne_bytes())
                    .to_string(),
            )
        } else {
            None
        };
        json!({
            "code": code, "interfaceIndex": route.InterfaceIndex,
            "source": selected, "prefixLength": route.DestinationPrefix.PrefixLength,
        })
    }

    fn route_set(plan: &Value) -> anyhow::Result<std::collections::BTreeSet<String>> {
        plan["routes"]
            .as_array()
            .context("capture routes missing")?
            .iter()
            .map(|route| {
                route
                    .as_str()
                    .map(str::to_owned)
                    .context("non-text capture route")
            })
            .collect()
    }

    fn check_dns(bytes: &[u8], id: u16) -> anyhow::Result<()> {
        let reply = Message::from_vec(bytes)?;
        ensure!(
            reply.id() == id && reply.message_type() == MessageType::Response,
            "DIRECT DNS response identity changed"
        );
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    let (done, watch) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if watch
            .recv_timeout(std::time::Duration::from_secs(180))
            .is_err()
        {
            eprintln!("Desktop TUN probe exceeded watchdog; cleanup unproven");
            std::process::exit(124);
        }
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(probe::run());
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
    done.send(())?;
    watchdog
        .join()
        .map_err(|_| anyhow::anyhow!("watchdog panicked"))?;
    result
}

#[cfg(not(target_os = "windows"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("This probe requires Windows")
}
