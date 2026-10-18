#[cfg(target_os = "windows")]
#[allow(dead_code)]
#[path = "../tests/support/gateway.rs"]
mod gateway;

#[cfg(target_os = "windows")]
#[path = "support/windows_full_observation.rs"]
mod full_observation;
#[cfg(target_os = "windows")]
#[path = "support/windows_full_observer.rs"]
mod full_observer;
#[cfg(target_os = "windows")]
#[path = "support/windows_full_process.rs"]
mod full_process;

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod contract {
    use anyhow::{ensure, Context};
    use ipnet::IpNet;
    use serde::Deserialize;
    use serde_json::Value;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    pub const CAMPUS4: &str = "192.0.2.77";
    pub const CAMPUS6: &str = "2001:db8:77::7";
    pub const SOURCE4: &str = "10.99.0.2";
    pub const SOURCE6: &str = "2001:db8::2";

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Peer {
        pub peer4: Ipv4Addr,
        pub peer6: Ipv6Addr,
        pub source4: Ipv4Addr,
        pub source6: Ipv6Addr,
        pub tcp_port: u16,
        pub udp_port: u16,
        pub blocked_port: u16,
        pub control_port: u16,
        pub dns_port: u16,
        pub token: String,
    }

    impl Peer {
        pub fn validate(&self) -> anyhow::Result<()> {
            ensure!(
                !self.peer4.is_unspecified()
                    && !self.peer4.is_loopback()
                    && !self.peer4.is_multicast()
                    && !self.peer4.is_broadcast()
                    && !self.peer4.is_link_local()
                    && !self.peer6.is_unspecified()
                    && !self.peer6.is_loopback()
                    && !self.peer6.is_multicast()
                    && !self.peer6.is_unicast_link_local()
                    && self.peer6.to_ipv4_mapped().is_none(),
                "peer must be a distinct routable dual-stack machine"
            );
            ensure!(
                self.peer4 != self.source4 && self.peer6 != self.source6,
                "independent peer cannot be a local source address"
            );
            let ports = [
                self.tcp_port,
                self.blocked_port,
                self.control_port,
                self.dns_port,
            ];
            ensure!(
                self.dns_port == 53
                    && self.udp_port != 0
                    && self.udp_port != 53
                    && self.udp_port != self.blocked_port
                    && ![443, 9001].contains(&self.blocked_port)
                    && ports.iter().all(|port| *port != 0)
                    && ports
                        .iter()
                        .enumerate()
                        .all(|(index, port)| !ports[..index].contains(port)),
                "distinct TCP service ports and DNS port 53 required"
            );
            ensure!(
                self.token.len() == 32 && self.token.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "peer observer token must be 32 hexadecimal characters"
            );
            for address in [IpAddr::V4(self.peer4), IpAddr::V6(self.peer6)] {
                ensure!(
                    address.to_string() != CAMPUS4
                        && address.to_string() != CAMPUS6
                        && address.to_string() != SOURCE4
                        && address.to_string() != SOURCE6,
                    "physical peer overlaps a campus fixture address"
                );
            }
            Ok(())
        }

        pub fn validate_plan(&self, plan: &Value) -> anyhow::Result<()> {
            ensure!(
                plan["capture_mode"] == "full" && plan["dns_capture"] == true,
                "formal API did not produce Full with DNS capture"
            );
            let routes: Vec<IpNet> = plan["routes"]
                .as_array()
                .context("formal capture routes absent")?
                .iter()
                .map(|route| {
                    route
                        .as_str()
                        .context("non-text route")?
                        .parse()
                        .map_err(Into::into)
                })
                .collect::<anyhow::Result<_>>()?;
            let protected = plan["protected_ips"]
                .as_array()
                .context("protected addresses absent")?;
            for address in [self.source4.to_string(), self.source6.to_string()] {
                ensure!(
                    protected.contains(&Value::String(address)),
                    "physical source not protected"
                );
            }
            for address in [
                IpAddr::V4(self.peer4),
                IpAddr::V6(self.peer6),
                CAMPUS4.parse()?,
                CAMPUS6.parse()?,
            ] {
                ensure!(
                    routes.iter().any(|route| route.contains(&address)),
                    "test destination {address} bypasses Full capture"
                );
            }
            ensure!(
                plan["physical_dns_servers"] == serde_json::json!([self.peer4]),
                "formal DNS is not the independent peer"
            );
            Ok(())
        }
    }

    pub fn validate_dns_audit(before: &Value, after: &Value) -> anyhow::Result<()> {
        let previous = before["dns"]
            .as_array()
            .context("peer DNS baseline absent")?;
        let current = after["dns"].as_array().context("peer DNS audit absent")?;
        ensure!(
            current.starts_with(previous),
            "peer DNS history was replaced"
        );
        let records = &current[previous.len()..];
        for (name, transport, kind, truncated) in [
            ("direct.fixture.test", "udp", 1, false),
            ("direct.fixture.test", "tcp", 28, false),
            ("truncated.fixture.test", "udp", 1, true),
            ("truncated.fixture.test", "tcp", 1, false),
        ] {
            ensure!(
                records.iter().any(|record| {
                    record["name"] == name
                        && record["transport"] == transport
                        && record["type"] == kind
                        && record["truncated"] == truncated
                        && record["query_id"].as_u64().is_some()
                        && record["query_id"] == record["response_id"]
                }),
                "independent peer did not witness {name} {transport} type {kind}"
            );
        }
        ensure!(
            records.iter().any(|record| {
                record["name"] == "badid.fixture.test"
                    && record["query_id"].as_u64().is_some()
                    && record["response_id"].as_u64().is_some()
                    && record["query_id"] != record["response_id"]
            }),
            "SERVFAIL lacks independent wrong-ID response evidence"
        );
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        fn peer() -> anyhow::Result<Peer> {
            Ok(serde_json::from_value(json!({
                "peer4":"198.51.100.2","peer6":"2001:db8:ffff::2",
                "source4":"198.51.100.1","source6":"2001:db8:ffff::1",
                "tcp_port":18001,"udp_port":18002,"blocked_port":18003,
                "control_port":18004,"dns_port":53,"token":"0123456789abcdef0123456789abcdef"
            }))?)
        }

        fn plan() -> Value {
            json!({"capture_mode":"full","dns_capture":true,
                "routes":["0.0.0.0/1","128.0.0.0/1","::/1","8000::/1"],
                "protected_ips":["198.51.100.1","2001:db8:ffff::1"],
                "physical_dns_servers":["198.51.100.2"]})
        }

        #[test]
        fn refuses_local_peers_and_mismatched_service_ports() -> anyhow::Result<()> {
            let mut input = peer()?;
            input.validate()?;
            input.peer4 = input.source4;
            assert!(input.validate().is_err());
            let mut input = peer()?;
            input.peer6 = "::1".parse()?;
            assert!(input.validate().is_err());
            let mut input = peer()?;
            input.blocked_port = input.tcp_port;
            assert!(input.validate().is_err());
            let mut input = peer()?;
            input.dns_port = 1053;
            assert!(input.validate().is_err());
            let mut input = peer()?;
            input.blocked_port = 443;
            assert!(input.validate().is_err());
            let mut input = peer()?;
            input.udp_port = input.blocked_port;
            assert!(input.validate().is_err());
            Ok(())
        }

        #[test]
        fn refuses_campus_mode_and_test_targets_outside_capture() -> anyhow::Result<()> {
            let peer = peer()?;
            peer.validate_plan(&plan())?;
            for field in [
                "capture_mode",
                "dns_capture",
                "protected_ips",
                "physical_dns_servers",
            ] {
                let mut document = plan();
                document[field] = Value::Null;
                assert!(peer.validate_plan(&document).is_err(), "{field}");
            }
            for routes in [
                json!(["192.0.2.77/32", "2001:db8:77::7/128"]),
                json!(["0.0.0.0/1", "128.0.0.0/1"]),
                json!(["::/1", "8000::/1"]),
            ] {
                let mut document = plan();
                document["routes"] = routes;
                assert!(peer.validate_plan(&document).is_err());
            }
            Ok(())
        }

        #[test]
        fn dns_failures_require_a_new_independent_wrong_id_witness() -> anyhow::Result<()> {
            let before = json!({"dns":[]});
            let mut records = vec![];
            for (name, transport, kind, truncated) in [
                ("direct.fixture.test", "udp", 1, false),
                ("direct.fixture.test", "tcp", 28, false),
                ("truncated.fixture.test", "udp", 1, true),
                ("truncated.fixture.test", "tcp", 1, false),
            ] {
                records.push(json!({"name":name,"transport":transport,"type":kind,
                    "truncated":truncated,"query_id":518,"response_id":518}));
            }
            assert!(validate_dns_audit(&before, &json!({"dns":records})).is_err());
            records.push(json!({"name":"badid.fixture.test","query_id":518,"response_id":519}));
            let complete = json!({"dns":records});
            validate_dns_audit(&before, &complete)?;
            assert!(validate_dns_audit(&complete, &complete).is_err());
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
mod probe {
    use super::full_observation::Observation;
    use super::{contract::*, gateway};
    use anyhow::{ensure, Context};
    use hickory_proto::{
        op::{Message, MessageType, Query, ResponseCode},
        rr::{Name, RData, RecordType},
    };
    use serde_json::{json, Value};
    use shuvpn_core::api::{
        desktop,
        routing::RoutingManager,
        tun,
        vpn::{CaptureMode, VpnSession, VpnState},
    };
    use std::{
        net::{IpAddr, SocketAddr},
        os::windows::io::AsRawSocket,
        sync::atomic::Ordering,
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpSocket, TcpStream, UdpSocket},
    };
    use windows_sys::Win32::Networking::WinSock::{setsockopt, IPPROTO_IP, IP_UNICAST_IF};

    const BUDGET: Duration = Duration::from_secs(8);

    pub async fn run() -> anyhow::Result<()> {
        let mode = std::env::args().nth(1).unwrap_or_default();
        let mut observation = Observation::connect(&mode).await?;
        let result = run_observed(&mut observation).await;
        if let Err(error) = &result {
            observation.abort(error).await;
        }
        result
    }

    async fn run_observed(observation: &mut Observation) -> anyhow::Result<()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        ensure!(
            args.len() == 3
                && ["--plan", "--run-privileged", "--run-local-elevated"].contains(&args[0].as_str()),
            "Usage: desktop_windows_full_smoke --plan|--run-privileged|--run-local-elevated <physical-alias> <peer.json>"
        );
        let alias = &args[1];
        let input = std::fs::read(&args[2])?;
        let peer: Peer =
            serde_json::from_slice(input.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&input))?;
        peer.validate()?;
        let interfaces = desktop::desktop_network_interfaces().await?;
        let index = interfaces
            .iter()
            .find(|interface| interface.name == *alias)
            .context("physical interface is not eligible")?
            .index;
        if args[0] != "--plan" {
            let driver: Value = serde_json::from_str(&tun::tun_driver_info().await?)?;
            ensure!(
                driver["elevated"] == (args[0] == "--run-local-elevated")
                    && driver["available"] == true
                    && driver["sessionDataPlaneReady"] == true,
                "execution mode or matching packaged driver/helper is not ready"
            );
        }
        let gateway = gateway::Gateway::start(gateway::Mode::L3Echo).await?;
        let mut fixture: Value = serde_json::from_str(&gateway.plan())?;
        fixture["dnsServers"] = json!([]);
        let resources = fixture["routes"]
            .as_array_mut()
            .context("fixture resources absent")?;
        resources.retain(|route| {
            matches!(
                route["host"].as_str(),
                Some("10.21.0.0/16" | "2001:db8:21::/64")
            )
        });
        for resource in resources {
            resource["host"] = json!(if resource["host"] == "10.21.0.0/16" {
                format!("{CAMPUS4}/32")
            } else {
                format!("{CAMPUS6}/128")
            });
        }
        let rounds = if args[0] == "--plan" { 1 } else { 2 };
        for round in 0..rounds {
            let session = VpnSession::connect(fixture.to_string(), 3000).await?;
            let tested = async {
                let formal: Value = serde_json::from_str(
                    &session
                        .desktop_tun_plan(
                            1500,
                            Some(peer.peer4.to_string()),
                            Some(CaptureMode::Full),
                            Some(alias.clone()),
                        )
                        .await?,
                )?;
                peer.validate_plan(&formal)?;
                println!(
                    "WINDOWS_FULL_FORMAL_PLAN {}",
                    json!({"round":round,"plan":formal})
                );
                if args[0] == "--plan" {
                    let status: Value = serde_json::from_str(&session.tun_status().await?)?;
                    ensure!(
                        status["state"] == "stopped" && status["cleanup_pending"] == false,
                        "preview unexpectedly activated TUN"
                    );
                    return Ok(());
                }
                exercise(&session, alias, index, &peer, &formal, round, observation).await
            }
            .await;
            if let Err(error) = &tested {
                eprintln!("WINDOWS_FULL_EXERCISE_FAILED {error:#}");
            }
            // Every failure after connect still crosses the real cleanup barrier.
            let cleanup = tokio::time::timeout(BUDGET, session.finish_stop())
                .await
                .context("Full session cleanup timeout")?;
            ensure!(
                cleanup?.is_none(),
                "Full session retained a cleanup diagnostic"
            );
            let status: Value = serde_json::from_str(&session.tun_status().await?)?;
            let egress: Value = serde_json::from_str(&session.egress_status()?)?;
            ensure!(
                status["state"] == "stopped"
                    && status["cleanup_pending"] == false
                    && egress["active"] == false
                    && egress["pending_cleanup"] == false,
                "Full session cleanup remains pending"
            );
            tested?;
            observation
                .step("cleaned", round, json!({"status":status}))
                .await?;
            println!(
                "WINDOWS_FULL_ROUND_CLEANED {}",
                json!({"round":round,"status":status})
            );
        }
        tokio::time::timeout(BUDGET, async {
            while gateway.connections.load(Ordering::Relaxed) != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .context("fixture connections did not drain")?;
        ensure!(
            gateway.errors.lock().await.is_empty(),
            "gateway errors: {:?}",
            gateway.errors.lock().await
        );
        observation.step("finish", 0, json!({})).await?;
        println!(
            "{}",
            if args[0] == "--plan" {
                "WINDOWS_FULL_FORMAL_PREVIEW_AND_NO_TUN_PASSED"
            } else {
                "WINDOWS_FULL_TWO_NORMAL_ROUNDS_DATA_AND_SESSION_CLEANUP_PASSED"
            }
        );
        Ok(())
    }

    fn policy(peer: &Peer, action: &str) -> String {
        format!("automatic_campus: false\nrules:\n  - 'DST-PORT,{},{}'\n  - 'IP-CIDR,{CAMPUS4}/32,SHU-VPN,no-resolve'\n  - 'IP-CIDR6,{CAMPUS6}/128,SHU-VPN,no-resolve'\n  - MATCH,DIRECT\n",
            peer.blocked_port, action)
    }

    async fn apply_policy(manager: &RoutingManager, profile: String) -> anyhow::Result<()> {
        let current: Value = serde_json::from_str(&manager.current()?)?;
        let version = current["version"]
            .as_u64()
            .context("routing version absent")?;
        manager.apply(profile, "{}".into(), version).await?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn repeated_probe_policy_changes_use_the_committed_version() -> anyhow::Result<()> {
            let directory = tempfile::tempdir()?;
            let manager =
                RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
            for action in ["DIRECT", "REJECT", "SHU-VPN"] {
                apply_policy(&manager, format!("rules:\n  - MATCH,{action}\n")).await?;
            }
            let current: Value = serde_json::from_str(&manager.current()?)?;
            ensure!(current["version"] == 3, "probe lost a policy revision");
            Ok(())
        }
    }

    async fn exercise(
        session: &VpnSession,
        alias: &str,
        index: u32,
        peer: &Peer,
        formal: &Value,
        round: usize,
        observation: &mut Observation,
    ) -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
        apply_policy(&manager, policy(peer, "REJECT")).await?;
        session.attach_routing(&manager)?;
        let before: Value = serde_json::from_str(&session.egress_status()?)?;
        let mut observer = observer(peer, index).await?;
        let prior_audit = audit(&mut observer, peer).await?;
        let installed: Value = serde_json::from_str(
            &session
                .start_desktop_tun(
                    1500,
                    alias.to_owned(),
                    Some(peer.peer4.to_string()),
                    Some(CaptureMode::Full),
                )
                .await?,
        )?;
        ensure!(
            installed["routes"] == formal["routes"] && installed["mtu"] == formal["mtu"],
            "installed Full differs from the approved formal plan"
        );
        let status: Value = serde_json::from_str(&session.tun_status().await?)?;
        ensure!(
            status["state"] == "running"
                && status["capture_mode"] == "full"
                && status["physical_interface"] == alias,
            "Full did not run on the selected interface"
        );
        let running: Value = serde_json::from_str(
            &session
                .desktop_tun_plan(
                    1,
                    Some("invalid".into()),
                    Some(CaptureMode::Campus),
                    Some("missing".into()),
                )
                .await?,
        )?;
        peer.validate_plan(&running)?;
        ensure!(
            running["routes"] == formal["routes"],
            "running preview lost Full configuration"
        );
        println!(
            "WINDOWS_FULL_RUNNING {}",
            json!({"round":round,"status":status})
        );
        observation
            .step("ready", round, json!({"status":status,"plan":formal}))
            .await?;
        for (source, destination) in [
            (SOURCE4, IpAddr::V4(peer.peer4)),
            (SOURCE6, IpAddr::V6(peer.peer6)),
        ] {
            udp(source, destination, peer.udp_port).await?;
            tcp(source, destination, peer.tcp_port, false).await?;
        }
        udp(SOURCE4, CAMPUS4.parse()?, 9001).await?;
        tcp(SOURCE4, CAMPUS4.parse()?, 443, true).await?;
        tcp(SOURCE6, CAMPUS6.parse()?, 443, true).await?;
        let decisions: Value = serde_json::from_str(&session.decisions()?)?;
        for (action, network, port) in [
            ("DIRECT", "tcp", peer.tcp_port),
            ("DIRECT", "udp", peer.udp_port),
            ("SHU-VPN", "tcp", 443),
            ("SHU-VPN", "udp", 9001),
        ] {
            ensure!(
                decisions
                    .as_array()
                    .context("decisions not an array")?
                    .iter()
                    .any(|event| {
                        event["flow"]["port"] == port
                            && event["flow"]["network"] == network
                            && event["flow"]["inbound"] == "tun"
                            && event["outbound"] == action
                            && event["decision"]["action"] == action
                    }),
                "missing successful native {action} {network} TUN decision"
            );
        }
        println!(
            "WINDOWS_FULL_SUCCESSFUL_DECISIONS {}",
            json!({"round":round,"decisions":decisions})
        );
        let checkpoint = audit(&mut observer, peer).await?;
        for action in ["REJECT", "SHU-VPN"] {
            apply_policy(&manager, policy(peer, action)).await?;
            for (source, destination) in [
                (SOURCE4, IpAddr::V4(peer.peer4)),
                (SOURCE6, IpAddr::V6(peer.peer6)),
            ] {
                let before: Value = serde_json::from_str(&session.decisions()?)?;
                let count = before.as_array().context("decisions not an array")?.len();
                let connection = connect(source, destination, peer.blocked_port).await;
                ensure!(connection.is_err(), "blocked flow reached the peer");
                let after: Value = serde_json::from_str(&session.decisions()?)?;
                ensure!(
                    after
                        .as_array()
                        .context("decisions not an array")?
                        .iter()
                        .skip(count)
                        .any(|event| {
                            event["flow"]["port"] == peer.blocked_port
                                && event["flow"]["inbound"] == "tun"
                                && event["decision"]["action"] == action
                                && event["outbound"].is_null()
                                && event["error"].as_str().is_some()
                        }),
                    "no native failure evidence for blocked {action} flow"
                );
            }
        }
        let rejected = audit(&mut observer, peer).await?;
        ensure!(
            rejected["tcp"] == checkpoint["tcp"],
            "blocked flows increased independent peer TCP accepts"
        );
        validate_sources(&prior_audit, &rejected, peer)?;
        // DNS mappings deliberately share peer addresses. Keep them after the
        // isolated REJECT/authorization cases so ambiguity is not their cause.
        dns(peer, RecordType::A, "direct.fixture.test", false).await?;
        dns(peer, RecordType::AAAA, "direct.fixture.test", true).await?;
        dns(peer, RecordType::A, "truncated.fixture.test", false).await?;
        dns(peer, RecordType::A, "badid.fixture.test", false).await?;
        let final_audit = audit(&mut observer, peer).await?;
        validate_sources(&prior_audit, &final_audit, peer)?;
        validate_dns_audit(&prior_audit, &final_audit)?;
        println!(
            "WINDOWS_FULL_PEER_AUDIT {}",
            json!({"round":round,"before":prior_audit,"before_blocked":checkpoint,"after_blocked":rejected,"after_dns":final_audit})
        );
        session.stop_tun().await?;
        ensure!(
            session.snapshot().state == VpnState::Connected,
            "stop TUN also stopped the VPN"
        );
        let restored: Value = serde_json::from_str(&session.egress_status()?)?;
        let controls = |value: &Value| -> anyhow::Result<Vec<String>> {
            let mut sockets: Vec<_> = value["sockets"]
                .as_array()
                .context("egress sockets absent")?
                .iter()
                .filter(|socket| socket["peer"] == "127.0.0.1")
                .map(Value::to_string)
                .collect();
            sockets.sort();
            Ok(sockets)
        };
        let originals = controls(&before)?;
        ensure!(
            !originals.is_empty()
                && controls(&restored)? == originals
                && restored["active"] == false
                && restored["pending_cleanup"] == false,
            "original control socket bindings were not restored"
        );
        let after_stop = audit(&mut observer, peer).await?;
        validate_sources(&prior_audit, &after_stop, peer)?;
        println!(
            "WINDOWS_FULL_PEER_AFTER_STOP {}",
            json!({"round":round,"audit":after_stop})
        );
        println!(
            "WINDOWS_FULL_CONTROL_BINDINGS_RESTORED {}",
            json!({"round":round,"before":before,"after":restored})
        );
        Ok(())
    }

    async fn connect(source: &str, destination: IpAddr, port: u16) -> anyhow::Result<TcpStream> {
        let socket = if destination.is_ipv4() {
            TcpSocket::new_v4()?
        } else {
            TcpSocket::new_v6()?
        };
        socket.bind(SocketAddr::new(source.parse()?, 0))?;
        Ok(
            tokio::time::timeout(BUDGET, socket.connect(SocketAddr::new(destination, port)))
                .await
                .context("captured TCP connect timeout")??,
        )
    }

    async fn tcp(source: &str, destination: IpAddr, port: u16, campus: bool) -> anyhow::Result<()> {
        let mut socket = connect(source, destination, port).await?;
        tokio::time::timeout(BUDGET, async {
            if campus {
                let mut prelude = [0; 7];
                socket.read_exact(&mut prelude).await?;
                ensure!(&prelude == b"prelude", "campus prelude differs");
            }
            let payload: Vec<_> = (0..65536).map(|index| (index % 251) as u8).collect();
            socket.write_all(&payload).await?;
            socket.shutdown().await?;
            let mut reply = Vec::new();
            socket.take(65537).read_to_end(&mut reply).await?;
            ensure!(reply == payload, "captured TCP bytes/FIN differ");
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("captured TCP data/FIN timeout")??;
        Ok(())
    }

    async fn udp(source: &str, destination: IpAddr, port: u16) -> anyhow::Result<()> {
        let socket = UdpSocket::bind(SocketAddr::new(source.parse()?, 0)).await?;
        let remote = SocketAddr::new(destination, port);
        socket.send_to(b"windows-full-udp", remote).await?;
        let mut response = [0; 1024];
        let (length, sender) = tokio::time::timeout(BUDGET, socket.recv_from(&mut response))
            .await
            .context("captured UDP timeout")??;
        ensure!(
            sender == remote && &response[..length] == b"windows-full-udp",
            "UDP peer/bytes differ"
        );
        Ok(())
    }

    async fn dns(peer: &Peer, kind: RecordType, name: &str, tcp: bool) -> anyhow::Result<()> {
        let mut message = Message::new();
        message
            .set_id(518)
            .set_recursion_desired(true)
            .add_query(Query::query(Name::from_ascii(name)?, kind));
        let query = message.to_vec()?;
        let response = tokio::time::timeout(BUDGET, async {
            if tcp {
                let mut socket = connect(SOURCE4, peer.peer4.into(), 53).await?;
                socket.write_u16(query.len() as u16).await?;
                socket.write_all(&query).await?;
                let length = socket.read_u16().await?;
                ensure!(length <= 4096, "DNS response exceeds budget");
                let mut bytes = vec![0; usize::from(length)];
                socket.read_exact(&mut bytes).await?;
                Ok::<_, anyhow::Error>(bytes)
            } else {
                let socket = UdpSocket::bind(format!("{SOURCE4}:0")).await?;
                socket.send_to(&query, (peer.peer4, 53)).await?;
                let mut bytes = [0; 4096];
                let (length, sender) = socket.recv_from(&mut bytes).await?;
                ensure!(
                    sender == SocketAddr::new(peer.peer4.into(), 53),
                    "DNS peer changed"
                );
                Ok(bytes[..length].to_vec())
            }
        })
        .await
        .context("captured DNS timeout")??;
        let response = Message::from_vec(&response)?;
        ensure!(
            response.id() == 518 && response.message_type() == MessageType::Response,
            "DNS response identity changed"
        );
        if name == "badid.fixture.test" {
            ensure!(
                response.response_code() == ResponseCode::ServFail,
                "bad upstream ID accepted"
            );
        } else {
            let expected = if kind == RecordType::A {
                IpAddr::V4(peer.peer4)
            } else {
                IpAddr::V6(peer.peer6)
            };
            ensure!(
                response.response_code() == ResponseCode::NoError
                    && response.answers().iter().any(|record| match record.data() {
                        RData::A(address) => IpAddr::V4(address.0) == expected,
                        RData::AAAA(address) => IpAddr::V6(address.0) == expected,
                        _ => false,
                    }),
                "DNS address/rcode differs"
            );
        }
        Ok(())
    }

    async fn observer(peer: &Peer, index: u32) -> anyhow::Result<TcpStream> {
        let socket = TcpSocket::new_v4()?;
        let binding = index.to_be();
        // SAFETY: the live socket accepts a four-byte network-order interface index.
        let code = unsafe {
            setsockopt(
                socket.as_raw_socket() as _,
                IPPROTO_IP,
                IP_UNICAST_IF,
                std::ptr::from_ref(&binding).cast(),
                std::mem::size_of::<u32>() as i32,
            )
        };
        ensure!(
            code == 0,
            "cannot bind independent observer to the physical interface"
        );
        Ok(tokio::time::timeout(
            BUDGET,
            socket.connect((peer.peer4, peer.control_port).into()),
        )
        .await
        .context("independent observer connect timeout")??)
    }

    async fn audit(observer: &mut TcpStream, peer: &Peer) -> anyhow::Result<Value> {
        tokio::time::timeout(BUDGET, async {
            observer
                .write_all(
                    format!("{}\n", json!({"token":peer.token,"command":"audit"})).as_bytes(),
                )
                .await?;
            let mut bytes = Vec::new();
            loop {
                let byte = observer.read_u8().await?;
                if byte == b'\n' {
                    break;
                }
                ensure!(bytes.len() < 65536, "peer audit exceeds budget");
                bytes.push(byte);
            }
            let result: Value = serde_json::from_slice(&bytes)?;
            ensure!(
                result["token"] == peer.token,
                "peer observer identity changed"
            );
            Ok(result)
        })
        .await
        .context("independent peer audit timeout")?
    }

    fn validate_sources(before: &Value, after: &Value, peer: &Peer) -> anyhow::Result<()> {
        for protocol in ["tcp", "udp"] {
            let previous = before[protocol]
                .as_array()
                .context("peer baseline absent")?;
            let current = after[protocol].as_array().context("peer audit absent")?;
            ensure!(
                current.starts_with(previous),
                "peer audit history was replaced"
            );
            let entries = &current[previous.len()..];
            ensure!(
                entries
                    .iter()
                    .all(|entry| entry[0] == "direct" || entry[0] == "dns"),
                "peer accepted a blocked or unknown service flow"
            );
            for source in [peer.source4.to_string(), peer.source6.to_string()] {
                ensure!(
                    entries
                        .iter()
                        .any(|entry| entry == &json!(["direct", source])),
                    "peer did not observe physical {protocol} from {source}"
                );
            }
            ensure!(
                entries
                    .iter()
                    .all(|entry| entry[1] == peer.source4.to_string()
                        || entry[1] == peer.source6.to_string()),
                "peer observed a nonphysical source"
            );
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    let (done, watch) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if watch
            .recv_timeout(std::time::Duration::from_secs(240))
            .is_err()
        {
            eprintln!("Windows Full probe watchdog expired; cleanup unproven");
            std::process::exit(124);
        }
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        let args: Vec<_> = std::env::args().skip(1).collect();
        let mode = args.first().map(String::as_str).unwrap_or("");
        if mode.starts_with("--observe-") {
            anyhow::ensure!(args.len() == 2, "observer requires mode and token");
            let mode = match mode {
                "--observe-plan" => "--plan",
                "--observe-helper" => "--run-privileged",
                "--observe-local" => "--run-local-elevated",
                _ => anyhow::bail!("unknown observer mode"),
            };
            full_observer::run(mode, &args[1]).await
        } else {
            probe::run().await
        }
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
    done.send(())?;
    watchdog
        .join()
        .map_err(|_| anyhow::anyhow!("watchdog panicked"))?;
    result
}

#[cfg(not(target_os = "windows"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Windows Full probe requires Windows")
}
