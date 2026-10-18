#[allow(dead_code)]
#[path = "../tests/support/gateway.rs"]
mod gateway;

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use shuvpn_core::api::{
        routing::RoutingManager,
        vpn::{CaptureMode, VpnSession},
    };
    use std::{process::Command, sync::atomic::Ordering};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpSocket, UdpSocket},
        time::{timeout, Duration},
    };
    let arguments: Vec<_> = std::env::args().collect();
    let desktop = arguments
        .get(1)
        .is_some_and(|argument| argument == "--desktop-helper");
    let crash = desktop && std::env::var("SHUVPN_PROBE_HELPER_CRASH").as_deref() == Ok("1");
    let full = std::env::var("SHUVPN_PROBE_FULL_CAPTURE").as_deref() == Ok("1");
    let formal = desktop || full;
    if desktop {
        anyhow::ensure!(
            cfg!(all(feature = "tun-probe", feature = "egress-probe")),
            "Build with tun-probe and egress-probe for the ordinary-user desktop probe"
        );
        let baseline = std::env::var("SHUVPN_PROBE_HOST_NETNS")?;
        anyhow::ensure!(
            std::fs::read_link("/proc/self/ns/net")?.to_string_lossy() != baseline,
            "Run the desktop probe in a fresh isolated network namespace"
        );
        let status = std::fs::read_to_string("/proc/self/status")?;
        let capabilities = status
            .lines()
            .find_map(|line| line.strip_prefix("CapEff:"))
            .ok_or_else(|| anyhow::anyhow!("missing effective capabilities"))?;
        // SAFETY: geteuid only observes the caller identity.
        anyhow::ensure!(
            unsafe { libc::geteuid() } != 0 && u64::from_str_radix(capabilities.trim(), 16)? == 0,
            "desktop caller must be unprivileged"
        );
    } else {
        anyhow::ensure!(
            std::fs::read_link("/proc/self/ns/net")? != std::fs::read_link("/proc/1/ns/net")?,
            "Run inside an isolated network namespace"
        );
    }
    anyhow::ensure!(
        arguments.len() == if desktop { 8 } else { 5 },
        "Expected IPv4/IPv6 UDP/TCP ports, plus HTTPS port and certificate for --desktop-helper"
    );
    let offset = if desktop { 2 } else { 1 };
    let port4: u16 = arguments[offset].parse()?;
    let port6: u16 = arguments[offset + 1].parse()?;
    let tcp4: u16 = arguments[offset + 2].parse()?;
    let tcp6: u16 = arguments[offset + 3].parse()?;
    fn routes() -> anyhow::Result<Vec<u8>> {
        let v4 = Command::new("/usr/sbin/ip")
            .args(["-j", "route", "show", "default"])
            .output()?;
        let v6 = Command::new("/usr/sbin/ip")
            .args(["-j", "-6", "route", "show", "default"])
            .output()?;
        anyhow::ensure!(
            v4.status.success() && v6.status.success(),
            "route inspection failed"
        );
        Ok([v4.stdout, v6.stdout].concat())
    }
    let defaults = routes()?;
    let ipv4_gateway = full && std::env::var("SHUVPN_PROBE_IPV4_GATEWAY").as_deref() == Ok("1");
    let gateway = gateway::Gateway::start(if ipv4_gateway {
        gateway::Mode::L3Ipv4Echo
    } else {
        gateway::Mode::L3Echo
    })
    .await?;
    let mut authorization: serde_json::Value = serde_json::from_str(&gateway.plan())?;
    if formal {
        authorization["dnsServers"] = serde_json::json!([]);
        let resources = authorization["routes"]
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("fixture resources missing"))?;
        resources.retain(|route| {
            !route["host"]
                .as_str()
                .is_some_and(|host| host.starts_with("*."))
        });
        for resource in resources.iter_mut() {
            let port = if resource["protocol"] == "tcp" {
                443
            } else {
                9001
            };
            resource["portMin"] = port.into();
            resource["portMax"] = port.into();
        }
        // These ranges make the physical targets part of the kernel capture
        // plan. Explicit profile rules still select DIRECT for their traffic.
        for host in if full {
            Vec::new()
        } else {
            vec!["10.22.0.0/16", "2001:db8:22::/64", "10.99.0.53"]
        } {
            for protocol in ["tcp", "udp"] {
                resources.push(serde_json::json!({
                    "host":host, "protocol":protocol, "portMin":1,"portMax":65535,
                    "appId":"test-app", "nodeGroupId":"group", "addrPretend":false,"enableTcpPrefL3":false,
                }));
            }
        }
    }
    let mut session = VpnSession::connect(authorization.to_string(), 2000).await?;
    let before: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let policy = if formal {
        "automatic_campus: false\nrules:\n - DST-PORT,9003,REJECT\n - IP-CIDR,10.21.0.0/16,SHU-VPN,no-resolve\n - IP-CIDR6,2001:db8:21::/64,SHU-VPN,no-resolve\n - MATCH,DIRECT\n"
    } else {
        "rules:\n - DST-PORT,9003,REJECT\n - IP-CIDR,10.23.0.0/16,SHU-VPN,no-resolve\n - MATCH,DIRECT\n"
    };
    manager.apply(policy.to_owned(), "{}".to_owned(), 0).await?;
    session.attach_routing(&manager)?;
    if full && std::env::var("SHUVPN_PROBE_EXPECT_PLAN_FAILURE").as_deref() == Ok("1") {
        if std::env::var("SHUVPN_PROBE_REMOVE_LOCAL_RULE").as_deref() == Ok("1") {
            anyhow::ensure!(
                Command::new("/usr/sbin/ip")
                    .args(["-4", "rule", "del", "priority", "0"])
                    .status()?
                    .success(),
                "cannot remove isolated local rule"
            );
        }
        let failure = session
            .desktop_tun_plan(
                1400,
                Some("10.99.0.53".into()),
                Some(CaptureMode::Full),
                Some("shuphys".into()),
            )
            .await;
        let failure = failure.err().ok_or_else(|| {
            anyhow::anyhow!("unsafe full plan accepted despite an unreachable capture policy")
        })?;
        anyhow::ensure!(
            failure.code == "interface"
                && (failure.message.contains("local 表存在额外目的路由")
                    || failure.message.contains("缺少 local 或 main 策略规则")),
            "plan failed for an unrelated reason: {failure}"
        );
        let status: serde_json::Value = serde_json::from_str(&session.tun_status().await?)?;
        anyhow::ensure!(
            status["state"] == "stopped" && status["cleanup_pending"] == false,
            "failed preview changed system state"
        );
        session.stop().await?;
        println!("FULL_CAPTURE_UNREACHABLE_POLICY_REJECTED_BEFORE_OS_MUTATION");
        return Ok(());
    }
    let plan = serde_json::json!({
        "name":"shuvpndata", "mtu":1400,
        "addresses":["10.99.0.2/32","2001:db8::2/128"],
        "routes":["10.21.0.0/16","10.22.0.0/16","10.23.0.0/16","2001:db8:21::/64","2001:db8:22::/64"],
        "protected_ips":["192.0.2.2","127.0.0.1"]
    }).to_string();
    let initial = if formal {
        session
            .start_desktop_tun(
                1500,
                String::new(),
                Some("10.99.0.53".into()),
                full.then_some(CaptureMode::Full),
            )
            .await?
    } else {
        session
            .start_tun(plan.clone(), "shuphys".to_owned())
            .await?
    };
    let initial: serde_json::Value = serde_json::from_str(&initial)?;
    let name = initial["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("TUN name missing"))?
        .to_owned();
    if full {
        let preview: serde_json::Value = serde_json::from_str(
            &session
                .desktop_tun_plan(
                    1280,
                    Some("invalid".into()),
                    Some(CaptureMode::Campus),
                    Some("missing-interface".into()),
                )
                .await?,
        )?;
        let status: serde_json::Value = serde_json::from_str(&session.tun_status().await?)?;
        anyhow::ensure!(
            preview["capture_mode"] == "full"
                && status["capture_mode"] == "full"
                && preview["name"] == name
                && preview["routes"] == initial["routes"]
                && preview["mtu"] == initial["mtu"],
            "running full preview replanned or misreported the installed capture"
        );
        println!("FULL_CAPTURE_RUNNING_PREVIEW_AND_NATIVE_MODE_PASSED");
        let selected = Command::new("/usr/sbin/ip")
            .args(["-j", "route", "get", "198.51.100.7"])
            .output()?;
        let selected: serde_json::Value = serde_json::from_slice(&selected.stdout)?;
        anyhow::ensure!(
            selected[0]["dev"] == name,
            "an outside-resource destination escaped full capture"
        );
    }
    if desktop {
        let status: serde_json::Value = serde_json::from_str(&session.tun_status().await?)?;
        let egress: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
        anyhow::ensure!(
            status["state"] == "running"
                && status["physical_interface"] == "shuphys"
                && initial["mtu"] == 1400
                && egress["active"] == true,
            "formal desktop plan or state mismatch"
        );
        anyhow::ensure!(
            egress["sockets"].as_array().is_some_and(|sockets| sockets
                .iter()
                .any(|socket| socket["peer"] == "127.0.0.1" && socket["binding"] == "lo")),
            "existing control socket was not protected through the helper"
        );
        println!("DESKTOP_LINUX_ORDINARY_UID_FORMAL_PLAN_AND_CONTROL_PROTECTION_PASSED");
        println!("DESKTOP_LINUX_HELPER_READY");
        harness_confirmation("helper-observed").await?;
    }
    let socket = UdpSocket::bind(("10.99.0.2", 0)).await?;
    let source6 = initial["addresses"]
        .as_array()
        .and_then(|addresses| {
            addresses
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter_map(|value| value.parse::<ipnet::IpNet>().ok())
                .find(|network| network.addr().is_ipv6())
        })
        .ok_or_else(|| anyhow::anyhow!("TUN IPv6 ingress address missing"))?
        .addr();
    let socket6 = UdpSocket::bind((source6, 0)).await?;
    for (socket, target, payload) in [
        (
            &socket,
            "10.21.0.5:9001".parse::<std::net::SocketAddr>()?,
            b"campus-kernel".as_slice(),
        ),
        (
            &socket,
            format!("10.22.0.5:{port4}").parse()?,
            b"direct-kernel-v4".as_slice(),
        ),
        (
            &socket6,
            format!("[2001:db8:22::5]:{port6}").parse()?,
            b"direct-kernel-v6".as_slice(),
        ),
    ] {
        socket.send_to(payload, target).await?;
        let mut reply = [0; 64];
        let (length, peer) =
            timeout(Duration::from_secs(3), socket.recv_from(&mut reply)).await??;
        anyhow::ensure!(
            &reply[..length] == payload && peer == target,
            "wrong actual network path reply"
        );
    }
    if desktop {
        println!("DESKTOP_LINUX_ORDINARY_UID_CAMPUS_AND_DIRECT_UDP_PASSED");
    }
    if full {
        let bound = UdpSocket::bind("192.0.2.1:0").await?;
        bound
            .send_to(b"physical-source-campus", "10.21.0.5:9001")
            .await?;
        let mut reply = [0; 64];
        let (length, peer) = timeout(Duration::from_secs(3), bound.recv_from(&mut reply)).await??;
        anyhow::ensure!(
            &reply[..length] == b"physical-source-campus"
                && peer == "10.21.0.5:9001".parse::<std::net::SocketAddr>()?,
            "gateway source translation did not restore the local UDP endpoint"
        );
        println!("FULL_CAPTURE_LOCAL_SOURCE_CAMPUS_UDP_TRANSLATION_PASSED");
    }
    let source6_text = format!("[{source6}]:0");
    for (source, target, campus) in [
        ("10.99.0.2:0", "10.21.0.5:443".to_owned(), true),
        (
            source6_text.as_str(),
            "[2001:db8:21::5]:443".to_owned(),
            true,
        ),
        ("10.99.0.2:0", format!("10.22.0.5:{tcp4}"), false),
        (
            source6_text.as_str(),
            format!("[2001:db8:22::5]:{tcp6}"),
            false,
        ),
    ] {
        let source: std::net::SocketAddr = source.parse()?;
        let socket = if source.is_ipv4() {
            TcpSocket::new_v4()?
        } else {
            TcpSocket::new_v6()?
        };
        socket.bind(source)?;
        let mut stream = timeout(Duration::from_secs(3), socket.connect(target.parse()?)).await??;
        if campus {
            let mut prelude = [0; 7];
            stream.read_exact(&mut prelude).await?;
            anyhow::ensure!(&prelude == b"prelude", "missing actual campus TCP prelude");
        }
        let payload: Vec<_> = (0..256 * 1024).map(|index| (index % 251) as u8).collect();
        let (mut reader, mut writer) = stream.into_split();
        let sending = async {
            writer.write_all(&payload).await?;
            writer.shutdown().await
        };
        let receiving = async {
            let mut reply = vec![];
            reader.read_to_end(&mut reply).await?;
            Ok::<_, std::io::Error>(reply)
        };
        let (sent, reply) = timeout(Duration::from_secs(5), async {
            tokio::join!(sending, receiving)
        })
        .await?;
        sent?;
        anyhow::ensure!(
            reply? == payload,
            "kernel TCP bytes/FIN changed on selected outbound"
        );
    }
    if desktop {
        println!("DESKTOP_LINUX_ORDINARY_UID_DUAL_STACK_TCP_AND_FIN_PASSED");
    }
    if full {
        verify_full_dns(&session, &manager, tcp4, policy).await?;
    }
    #[cfg(feature = "egress-probe")]
    if desktop {
        let https_port: u16 = arguments[6].parse()?;
        let provider_directory = directory.path().join("providers");
        std::fs::create_dir(&provider_directory)?;
        shuvpn_core::egress_probe::verify(
            "direct.fixture.test".into(),
            format!("https://provider.fixture.test:{https_port}/rules"),
            std::fs::read(&arguments[7])?,
            provider_directory.to_string_lossy().into_owned(),
        )
        .await?;
        println!("DESKTOP_LINUX_ORDINARY_UID_PHYSICAL_DNS_FALLBACK_HTTPS_PASSED");
    }
    let uploaded = gateway.uploaded.load(Ordering::Relaxed);
    for (target, code) in [
        ("10.21.0.5:9003", "rejected"),
        (
            if formal {
                "10.21.0.5:9004"
            } else {
                "10.23.0.5:9001"
            },
            "unrouted",
        ),
    ] {
        socket.send_to(b"must-not-send", target).await?;
        timeout(Duration::from_secs(2), async {
            loop {
                let events: serde_json::Value = serde_json::from_str(&session.decisions()?)?;
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
        let mut reply = [0; 64];
        anyhow::ensure!(
            timeout(Duration::from_millis(100), socket.recv_from(&mut reply))
                .await
                .is_err(),
            "rejected target replied"
        );
        let tcp = TcpSocket::new_v4()?;
        tcp.bind("10.99.0.2:0".parse()?)?;
        anyhow::ensure!(
            timeout(Duration::from_secs(2), tcp.connect(target.parse()?))
                .await?
                .is_err(),
            "rejected TCP completed a kernel handshake"
        );
    }
    anyhow::ensure!(
        gateway.uploaded.load(Ordering::Relaxed) == uploaded,
        "rejected packets reached campus gateway"
    );
    #[cfg(feature = "egress-probe")]
    let mut retained = if desktop {
        Some(
            shuvpn_core::egress_probe::RetainedTcp::connect(format!("10.22.0.5:{tcp4}").parse()?)
                .await?,
        )
    } else {
        None
    };
    #[cfg(feature = "egress-probe")]
    let mut retained_http = if desktop {
        Some(
            shuvpn_core::egress_probe::RetainedHttp::open(
                format!("https://provider.fixture.test:{}/held", arguments[6]),
                std::fs::read(&arguments[7])?,
            )
            .await?,
        )
    } else {
        None
    };
    #[cfg(feature = "egress-probe")]
    if desktop {
        let active: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
        let sockets = active["sockets"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("active socket status missing"))?;
        let physical = sockets
            .iter()
            .filter(|socket| socket["peer"] == "10.22.0.5")
            .collect::<Vec<_>>();
        anyhow::ensure!(
            physical.len() >= 2 && physical.iter().all(|socket| socket["binding"] == "shuphys"),
            "native TCP/HTTPS did not hold the protected physical interface"
        );
    }
    if crash {
        println!("DESKTOP_LINUX_HELPER_KILL_READY");
        harness_confirmation("helper-killed").await?;
        timeout(Duration::from_secs(5), async {
            loop {
                let status: serde_json::Value = serde_json::from_str(&session.tun_status().await?)?;
                if status["state"] != "running" {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::task::yield_now().await;
            }
        })
        .await??;
        let diagnostic = timeout(Duration::from_secs(8), session.finish_stop()).await??;
        anyhow::ensure!(
            diagnostic.is_some(),
            "unexpected helper crash was hidden as a normal stop"
        );
        println!("DESKTOP_LINUX_HELPER_CRASH_DIAGNOSTIC {diagnostic:?}");
    } else if full && !desktop {
        let table = if std::env::var("SHUVPN_PROBE_LIVE_LOCAL_ROUTE").as_deref() == Ok("1") {
            "local"
        } else {
            "main"
        };
        let changed = Command::new("/usr/sbin/ip")
            .args([
                "route",
                "add",
                "198.51.100.33/32",
                "via",
                "192.0.2.2",
                "dev",
                "shuphys",
                "table",
                table,
            ])
            .status()?;
        anyhow::ensure!(changed.success(), "cannot inject isolated foreign route");
        timeout(Duration::from_secs(6), async {
            loop {
                if session.snapshot().state == shuvpn_core::api::vpn::VpnState::Failed {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await??;
        let diagnostic = session.finish_stop().await?;
        anyhow::ensure!(
            diagnostic.is_some(),
            "full capture coverage change was hidden"
        );
        let remaining = Command::new("/usr/sbin/ip")
            .args(["-j", "route", "show", "table", table, "198.51.100.33/32"])
            .output()?;
        let rows: serde_json::Value = serde_json::from_slice(&remaining.stdout)?;
        anyhow::ensure!(
            rows.as_array().is_some_and(|rows| rows.len() == 1),
            "recovery deleted the foreign route"
        );
        anyhow::ensure!(
            Command::new("/usr/sbin/ip")
                .args([
                    "route",
                    "del",
                    "198.51.100.33/32",
                    "via",
                    "192.0.2.2",
                    "dev",
                    "shuphys",
                    "table",
                    table,
                ])
                .status()?
                .success(),
            "cannot remove test-owned injected route"
        );
        println!("FULL_CAPTURE_COVERAGE_CHANGE_STOPS_AND_PRESERVES_FOREIGN_ROUTE_PASSED");
        if table == "local" {
            println!("FULL_CAPTURE_LOCAL_TABLE_CHANGE_STOPS_AND_PRESERVES_FOREIGN_ROUTE_PASSED");
        }
    } else {
        session.stop_tun().await?;
    }
    anyhow::ensure!(
        !Command::new("/usr/sbin/ip")
            .args(["link", "show", "dev", &name])
            .output()?
            .status
            .success(),
        "stop_tun left interface"
    );
    if desktop {
        #[cfg(feature = "egress-probe")]
        if let Some(connection) = retained.as_mut() {
            connection.verify_restored().await?;
            println!("DESKTOP_LINUX_RETAINED_NATIVE_TCP_BINDING_AND_BYTES_RESTORED");
        }
        #[cfg(feature = "egress-probe")]
        drop(retained);
        #[cfg(feature = "egress-probe")]
        if let Some(response) = retained_http.take() {
            let held: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
            let sockets = held["sockets"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("socket status missing"))?;
            let physical = sockets
                .iter()
                .filter(|socket| socket["peer"] == "10.22.0.5")
                .collect::<Vec<_>>();
            anyhow::ensure!(
                physical.len() == 1 && physical[0]["binding"].is_null(),
                "live HTTPS socket was not restored before its delayed body"
            );
            println!("DESKTOP_LINUX_RETAINED_HTTP_RELEASE_READY");
            harness_confirmation("http-released").await?;
            response.verify_restored_body().await?;
            println!("DESKTOP_LINUX_RETAINED_HTTPS_BINDING_AND_DELAYED_BODY_RESTORED");
        }
        timeout(Duration::from_secs(2), async {
            loop {
                let status: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
                let settled = if crash {
                    status["sockets"].as_array().is_some_and(Vec::is_empty)
                } else {
                    status["sockets"] == before["sockets"]
                };
                if settled {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::task::yield_now().await;
            }
        })
        .await??;
        let restored: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
        anyhow::ensure!(
            restored["active"] == false
                && restored["pending_cleanup"] == false
                && if crash {
                    restored["sockets"].as_array().is_some_and(Vec::is_empty)
                } else {
                    restored["sockets"] == before["sockets"]
                },
            "existing control bindings were not restored"
        );
    }
    if crash || (full && !desktop) {
        session = VpnSession::connect(authorization.to_string(), 2000).await?;
        session.attach_routing(&manager)?;
        if crash {
            println!("DESKTOP_LINUX_NEW_SESSION_AFTER_HELPER_CRASH_READY");
        } else {
            println!("FULL_CAPTURE_NEW_SESSION_AFTER_COVERAGE_RECOVERY_READY");
        }
    }
    // A stopped adapter releases its packet port and can be created again.
    let restarted = if formal {
        session
            .start_desktop_tun(
                1500,
                "shuphys".into(),
                Some("10.99.0.53".into()),
                full.then_some(CaptureMode::Full),
            )
            .await?
    } else {
        session.start_tun(plan, "shuphys".to_owned()).await?
    };
    let restarted: serde_json::Value = serde_json::from_str(&restarted)?;
    let restarted_name = restarted["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("restart name missing"))?;
    if desktop {
        println!("DESKTOP_LINUX_RESTART_READY");
        harness_confirmation("restart-observed").await?;
        let diagnostic = timeout(Duration::from_secs(8), session.finish_stop()).await??;
        anyhow::ensure!(
            diagnostic.is_none(),
            "formal session needed cleanup recovery: {diagnostic:?}"
        );
        let status: serde_json::Value = serde_json::from_str(&session.tun_status().await?)?;
        let egress: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
        anyhow::ensure!(
            status["state"] == "stopped"
                && status["cleanup_pending"] == false
                && egress["active"] == false
                && egress["pending_cleanup"] == false,
            "formal session cleanup remains pending"
        );
    } else {
        session.stop().await?;
    }
    anyhow::ensure!(
        !Command::new("/usr/sbin/ip")
            .args(["link", "show", "dev", restarted_name])
            .output()?
            .status
            .success(),
        "session stop left interface"
    );
    if desktop {
        println!("DESKTOP_LINUX_CLEANUP_READY");
        harness_confirmation("cleanup-confirmed").await?;
    }
    anyhow::ensure!(defaults == routes()?, "physical defaults changed");
    timeout(Duration::from_secs(2), async {
        while gateway.connections.load(Ordering::Relaxed) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    anyhow::ensure!(
        gateway.errors.lock().await.is_empty(),
        "gateway protocol errors"
    );
    println!("TUN_ROUTED_KERNEL_TCP_UDP_CAMPUS_DIRECT_V4_V6_REJECT_CLEANUP_PASSED");
    if full {
        println!("FULL_CAPTURE_KERNEL_OUTSIDE_RESOURCES_DIRECT_DUAL_STACK_AND_CLEANUP_PASSED");
        if ipv4_gateway {
            println!("FULL_CAPTURE_IPV4_ONLY_GATEWAY_LOCAL_IPV6_INGRESS_PASSED");
        }
    }
    if desktop {
        println!("DESKTOP_LINUX_FORMAL_HELPER_SESSION_AND_RECOVERY_PASSED");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn verify_full_dns(
    session: &shuvpn_core::api::vpn::VpnSession,
    manager: &shuvpn_core::api::routing::RoutingManager,
    tcp_port: u16,
    policy: &str,
) -> anyhow::Result<()> {
    use hickory_proto::{
        op::{Message, Query, ResponseCode},
        rr::{Name, RecordType},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpSocket, UdpSocket},
        time::{timeout, Duration},
    };
    fn question(host: &str) -> anyhow::Result<Message> {
        let mut query = Message::new();
        query
            .set_id(518)
            .set_recursion_desired(true)
            .add_query(Query::query(
                Name::from_ascii(format!("{host}."))?,
                RecordType::A,
            ));
        Ok(query)
    }
    async fn udp(host: &str) -> anyhow::Result<Message> {
        let socket = UdpSocket::bind("10.99.0.2:0").await?;
        socket
            .send_to(&question(host)?.to_vec()?, "8.8.8.8:53")
            .await?;
        let mut bytes = vec![0; 4096];
        let (length, peer) =
            timeout(Duration::from_secs(3), socket.recv_from(&mut bytes)).await??;
        anyhow::ensure!(
            peer == "8.8.8.8:53".parse::<std::net::SocketAddr>()?,
            "hijacked DNS reply peer changed"
        );
        Ok(Message::from_vec(&bytes[..length])?)
    }
    manager
        .apply(
            "automatic_campus: false\nrules: ['DOMAIN,direct.fixture.test,DIRECT','MATCH,REJECT']"
                .into(),
            "{}".into(),
            1,
        )
        .await?;
    let response = udp("direct.fixture.test").await?;
    anyhow::ensure!(
        response.id() == 518
            && response.response_code() == ResponseCode::NoError
            && !response.answers().is_empty(),
        "full UDP DNS failed"
    );
    let client = TcpSocket::new_v4()?;
    client.bind("10.99.0.2:0".parse()?)?;
    let mut client = timeout(
        Duration::from_secs(3),
        client.connect(format!("10.22.0.5:{tcp_port}").parse()?),
    )
    .await??;
    client.write_all(b"dns-policy").await?;
    let mut echo = [0; 10];
    timeout(Duration::from_secs(3), client.read_exact(&mut echo)).await??;
    anyhow::ensure!(
        &echo == b"dns-policy",
        "public DNS mapping did not drive a domain rule"
    );
    drop(client);
    let dns = TcpSocket::new_v4()?;
    dns.bind("10.99.0.2:0".parse()?)?;
    let mut dns = timeout(Duration::from_secs(3), dns.connect("8.8.4.4:53".parse()?)).await??;
    let query = question("truncated.fixture.test")?.to_vec()?;
    dns.write_u16(query.len() as u16).await?;
    dns.write_all(&query).await?;
    let length = timeout(Duration::from_secs(3), dns.read_u16()).await?? as usize;
    let mut bytes = vec![0; length];
    timeout(Duration::from_secs(3), dns.read_exact(&mut bytes)).await??;
    let response = Message::from_vec(&bytes)?;
    anyhow::ensure!(
        response.id() == 518
            && !response.truncated()
            && response.response_code() == ResponseCode::NoError,
        "full TCP DNS or protected UDP-to-TCP fallback failed"
    );
    drop(dns);
    let blocked = TcpSocket::new_v4()?;
    blocked.bind("10.99.0.2:0".parse()?)?;
    anyhow::ensure!(
        timeout(
            Duration::from_secs(3),
            blocked.connect(format!("10.22.0.5:{tcp_port}").parse()?)
        )
        .await?
        .is_err(),
        "ambiguous public DNS names bypassed a domain rule"
    );
    let decisions: serde_json::Value = serde_json::from_str(&session.decisions()?)?;
    anyhow::ensure!(
        decisions
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row["error"] == "dns_ambiguous")),
        "blocked socket had no explicit DNS ambiguity diagnosis"
    );
    let invalid = udp("badid.fixture.test").await?;
    anyhow::ensure!(
        invalid.id() == 518 && invalid.response_code() == ResponseCode::ServFail,
        "unvalidated DNS response escaped"
    );
    manager.apply(policy.into(), "{}".into(), 2).await?;
    println!("FULL_CAPTURE_DNS_UDP_TCP_PHYSICAL_UPSTREAM_DOMAIN_POLICY_AND_AMBIGUITY_PASSED");
    Ok(())
}

#[cfg(target_os = "linux")]
async fn harness_confirmation(expected: &'static str) -> anyhow::Result<()> {
    let acknowledgement = tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).map(|_| line)
    })
    .await??;
    anyhow::ensure!(
        acknowledgement.trim() == expected,
        "harness did not confirm helper state while the parent stayed alive"
    );
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This probe requires Linux network namespaces");
    std::process::exit(1);
}
