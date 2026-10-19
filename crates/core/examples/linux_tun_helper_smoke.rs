#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use shuvpn_core::tun_helper::Client;
    use socket2::{Domain, Protocol, Socket, Type};
    use std::{path::PathBuf, process::Command, time::Duration};
    use tokio::{net::UdpSocket, time::timeout};

    // Ordinary UIDs cannot read /proc/1/ns/net on the target system. The root
    // harness records its namespace before unshare and passes that baseline.
    let baseline =
        std::env::var("SHUVPN_PROBE_HOST_NETNS").context("namespace baseline missing")?;
    ensure!(
        std::fs::read_link("/proc/self/ns/net")?.to_string_lossy() != baseline,
        "helper probe must use a fresh isolated network namespace"
    );
    let status = std::fs::read_to_string("/proc/self/status")?;
    let cap = status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:"))
        .context("effective capabilities missing")?;
    ensure!(
        u64::from_str_radix(cap.trim(), 16)? == 0,
        "probe must be unprivileged"
    );
    // SAFETY: geteuid only observes the process identity.
    ensure!(
        unsafe { libc::geteuid() } != 0,
        "probe must use a normal UID"
    );
    let path = PathBuf::from(std::env::args().nth(1).context("helper path missing")?);
    let mode = std::env::args().nth(2).unwrap_or_default();
    if mode == "denied" {
        ensure!(
            Client::launch_probe(path).await.is_err(),
            "helper without capabilities was accepted"
        );
        println!("LINUX_TUN_HELPER_PERMISSION_DENIED_PASSED");
        return Ok(());
    }
    let client = Client::launch_probe(path).await?;
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.bind(&"127.0.0.1:0".parse::<std::net::SocketAddr>()?.into())?;
    client.bind_socket(&socket, Some("lo".into())).await?;
    ensure!(
        socket.device()? == Some(b"lo".to_vec()),
        "helper did not bind the real socket"
    );
    ensure!(
        socket.bind_device(None).is_err(),
        "normal UID unexpectedly restored protected socket"
    );
    client.bind_socket(&socket, None).await?;
    ensure!(
        socket.device()?.is_none(),
        "helper did not restore the real socket"
    );
    let local = socket
        .local_addr()?
        .as_socket()
        .context("UDP local address missing")?;
    socket.set_nonblocking(true)?;
    let socket = UdpSocket::from_std(socket.into())?;
    let peer = UdpSocket::bind("127.0.0.1:0").await?;
    peer.send_to(b"after restore", local).await?;
    let mut bytes = [0; 32];
    let length = timeout(Duration::from_secs(2), socket.recv(&mut bytes)).await??;
    ensure!(
        &bytes[..length] == b"after restore",
        "restored socket lost bytes"
    );
    println!("LINUX_TUN_HELPER_UNPRIVILEGED_SOCKET_RESTORE_PASSED");

    let name = "shuvpnhlp";
    let plan = serde_json::json!({
        "name": name, "mtu":1400, "addresses":["10.250.0.2/32","2001:db8:250::2/128"],
        "routes":["10.21.0.0/16","2001:db8:21::/64"],"protected_ips":["192.0.2.10"]
    })
    .to_string();
    if mode == "abandoned" {
        client.abandon_create_probe(plan).await?;
        client
            .bind_socket(
                &Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?,
                Some("lo".into()),
            )
            .await?;
        ensure!(
            !Command::new("/usr/sbin/ip")
                .args(["link", "show", "dev", name])
                .output()?
                .status
                .success(),
            "unaccepted Create left a helper-owned TUN"
        );
        println!("LINUX_TUN_HELPER_UNACCEPTED_CREATE_CLEANUP_PASSED");
        return Ok(());
    }
    if mode == "generated" {
        let mut plan: serde_json::Value = serde_json::from_str(&plan)?;
        plan.as_object_mut()
            .context("plan object missing")?
            .remove("name");
        let device = client.create(plan.to_string()).await?;
        let info: serde_json::Value = serde_json::from_str(&device.info()?)?;
        let generated = info["name"]
            .as_str()
            .context("generated TUN name missing")?;
        ensure!(
            generated.starts_with("shuvpn") && generated != name,
            "TUN name was not generated"
        );
        device.close().await?;
        ensure!(
            !Command::new("/usr/sbin/ip")
                .args(["link", "show", "dev", generated])
                .output()?
                .status
                .success(),
            "generated TUN survived close"
        );
        println!("LINUX_TUN_HELPER_GENERATED_IDENTITY_AND_CLEANUP_PASSED");
        return Ok(());
    }
    if mode == "transfer-failure" {
        let failure = client
            .fail_transfer_probe(plan)
            .await
            .err()
            .context("injection succeeded")?;
        ensure!(
            failure.code == "fixture",
            "transfer failure was hidden: {failure:?}"
        );
        ensure!(
            !Command::new("/usr/sbin/ip")
                .args(["link", "show", "dev", name])
                .output()?
                .status
                .success(),
            "failed descriptor transfer left a helper-owned TUN"
        );
        client.close_tun().await?;
        println!("LINUX_TUN_HELPER_FAILED_TRANSFER_ROLLBACK_PASSED");
        return Ok(());
    }
    if mode == "collision" || mode == "partial" {
        let failure = client
            .create(plan)
            .await
            .err()
            .context("invalid create succeeded")?;
        ensure!(
            failure.code
                == if mode == "collision" {
                    "system"
                } else {
                    "netlink"
                },
            "unexpected failure: {failure:?}"
        );
        println!("LINUX_TUN_HELPER_CREATE_ROLLBACK_PASSED mode={mode}");
        return Ok(());
    }
    let device = client.create(plan.clone()).await?;
    ensure!(
        client.create(plan).await.is_err(),
        "helper accepted a second TUN"
    );
    if mode == "hold" {
        println!(
            "LINUX_TUN_HELPER_HOLD_READY helper_pid={}",
            client.process_id()
        );
        std::future::pending::<()>().await;
    }
    if mode == "helper-crash" {
        // SAFETY: the PID is the live child retained by this Client; the probe
        // intentionally terminates only its own helper inside this namespace.
        ensure!(
            unsafe { libc::kill(client.process_id() as i32, libc::SIGKILL) } == 0,
            "could not kill owned helper"
        );
        timeout(Duration::from_secs(3), async {
            // Initial IPv6 control packets can already be queued when the
            // helper exits; consume them until the owned fd is actually revoked.
            while device.read_packet(5000).await.is_ok() {}
        })
        .await
        .context("helper crash did not wake local packet read")?;
        let snapshot: serde_json::Value = serde_json::from_str(&device.info()?)?;
        ensure!(
            snapshot["closed"] == true,
            "helper crash did not revoke the local fd owner"
        );
        ensure!(
            !Command::new("/usr/sbin/ip")
                .args(["link", "show", "dev", name])
                .output()?
                .status
                .success(),
            "local TUN fd survived helper exit"
        );
        println!("LINUX_TUN_HELPER_CRASH_REVOKES_LOCAL_FD_PASSED");
        return Ok(());
    }
    if mode == "drop" {
        drop(device);
        timeout(Duration::from_secs(3), async {
            loop {
                if !Command::new("/usr/sbin/ip")
                    .args(["link", "show", "dev", name])
                    .output()?
                    .status
                    .success()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        client
            .bind_socket(
                &Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?,
                Some("lo".into()),
            )
            .await?;
        println!("LINUX_TUN_HELPER_DROPPED_LEASE_WITH_RETAINED_BROKER_PASSED");
        return Ok(());
    }
    let dynamic: std::net::IpAddr = "198.51.100.77".parse()?;
    device.sync_routes(vec![dynamic]).await?;
    let routes = Command::new("/usr/sbin/ip")
        .args(["-j", "route", "show", "dev", name])
        .output()?;
    ensure!(routes.status.success(), "cannot inspect dynamic route");
    let routes: serde_json::Value = serde_json::from_slice(&routes.stdout)?;
    ensure!(
        routes
            .as_array()
            .context("routes missing")?
            .iter()
            .any(|route| route["dst"] == "198.51.100.77"),
        "dynamic route was not installed"
    );
    ensure!(
        device
            .sync_routes(vec!["192.0.2.10".parse()?])
            .await
            .is_err(),
        "protected host was captured"
    );
    device.sync_routes(vec![]).await?;
    let socket = UdpSocket::bind("10.250.0.2:0").await?;
    let local = socket.local_addr()?;
    socket.send_to(b"helper packet", "10.21.0.5:9001").await?;
    let mut packet = timeout(Duration::from_secs(2), async {
        loop {
            if let Some(packet) = device.read_packet(200).await? {
                if packet.len() >= 28
                    && packet[0] >> 4 == 4
                    && packet[9] == 17
                    && packet[16..20] == [10, 21, 0, 5]
                {
                    return Ok::<_, anyhow::Error>(packet);
                }
            }
        }
    })
    .await??;
    ensure!(
        &packet[28..] == b"helper packet",
        "transferred TUN fd changed packet bytes"
    );
    packet[12..16].copy_from_slice(&[10, 21, 0, 5]);
    packet[16..20].copy_from_slice(&[10, 250, 0, 2]);
    packet[20..22].copy_from_slice(&9001u16.to_be_bytes());
    packet[22..24].copy_from_slice(&local.port().to_be_bytes());
    packet[10..12].fill(0);
    packet[26..28].fill(0); // An IPv4 UDP checksum of zero is valid.
    let sum = checksum(&packet[..20]);
    packet[10..12].copy_from_slice(&sum.to_be_bytes());
    device.write_packet(packet).await?;
    let length = timeout(Duration::from_secs(2), socket.recv(&mut bytes)).await??;
    ensure!(
        &bytes[..length] == b"helper packet",
        "TUN injection did not reach normal UID socket"
    );
    while device.read_packet(1).await?.is_some() {}
    let (pending, close) = tokio::join!(device.read_packet(5000), async {
        tokio::task::yield_now().await;
        device.close().await
    });
    close?;
    ensure!(pending.is_err(), "close failed to wake local packet read");
    device.close().await?;
    ensure!(
        !Command::new("/usr/sbin/ip")
            .args(["link", "show", "dev", name])
            .output()?
            .status
            .success(),
        "helper-owned TUN leaked after acknowledged close"
    );
    client
        .bind_socket(
            &Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?,
            Some("lo".into()),
        )
        .await?;
    println!("LINUX_TUN_HELPER_UNPRIVILEGED_PACKET_ROUTES_AND_CLEANUP_PASSED");
    client.shutdown().await?;
    ensure!(
        !std::path::Path::new(&format!("/proc/{}", client.process_id())).exists(),
        "acknowledged shutdown left the helper process alive"
    );
    println!("LINUX_TUN_HELPER_PROCESS_SHUTDOWN_CONFIRMED");
    Ok(())
}

#[cfg(target_os = "linux")]
fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = bytes
        .chunks(2)
        .map(|pair| u32::from(u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)])))
        .sum::<u32>();
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(not(target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Linux helper probe requires Linux")
}
