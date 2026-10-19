#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use shuvpn_core::api::tun::TunInterface;
    use std::process::Command;
    use tokio::{
        net::UdpSocket,
        time::{timeout, Duration},
    };
    fn ip(args: &[&str]) -> anyhow::Result<serde_json::Value> {
        let result = Command::new("/usr/sbin/ip").args(args).output()?;
        anyhow::ensure!(result.status.success(), "ip inspection failed");
        Ok(serde_json::from_slice(&result.stdout)?)
    }
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode != "denied" {
        anyhow::ensure!(
            std::fs::read_link("/proc/self/ns/net")? != std::fs::read_link("/proc/1/ns/net")?,
            "Run this probe inside an isolated network namespace"
        );
    }
    let name = "shuvpntest";
    let plan = serde_json::json!({"name":name,"mtu":1400,"addresses":["10.250.0.2/32","2001:db8:250::2/128"],
        "routes":["10.21.0.0/16","2001:db8:21::/64"],"protected_ips":["192.0.2.10"]});
    if mode == "collision" || mode == "conflict" || mode == "denied" || mode == "partial" {
        let before_links = ip(&["-j", "address", "show"])?;
        let before_routes = ip(&["-j", "route", "show", "table", "all"])?;
        let failure = TunInterface::create(plan.to_string())
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("invalid platform situation unexpectedly succeeded"))?;
        if mode == "denied" {
            anyhow::ensure!(
                failure.code == "permission",
                "unexpected error: {failure:?}"
            );
        }
        if mode == "partial" {
            anyhow::ensure!(
                failure.code == "netlink",
                "expected partial Netlink failure: {failure:?}"
            );
        }
        anyhow::ensure!(
            before_links == ip(&["-j", "address", "show"])?,
            "existing interfaces changed"
        );
        anyhow::ensure!(
            before_routes == ip(&["-j", "route", "show", "table", "all"])?,
            "existing routes changed"
        );
        println!("TUN_FAILURE_ROLLBACK_PASSED mode={mode}");
        return Ok(());
    }
    let before = ip(&["-j", "route", "show", "default"])?;
    let device = TunInterface::create(plan.to_string()).await?;
    let info: serde_json::Value = serde_json::from_str(&device.info()?)?;
    anyhow::ensure!(info["name"] == name, "wrong interface");
    let links = ip(&["-j", "address", "show", "dev", name])?;
    anyhow::ensure!(links[0]["mtu"] == 1400, "wrong MTU");
    anyhow::ensure!(
        links[0]["addr_info"]
            .as_array()
            .is_some_and(|addresses| addresses
                .iter()
                .any(|address| address["local"] == "10.250.0.2")
                && addresses
                    .iter()
                    .any(|address| address["local"] == "2001:db8:250::2")),
        "addresses not configured"
    );
    if mode == "hold" {
        println!("TUN_HOLD_READY");
        tokio::task::spawn_blocking(|| {
            use std::io::Read;
            let mut signal = [0; 1];
            std::io::stdin().read(&mut signal)
        })
        .await??;
        device.close().await?;
        return Ok(());
    }
    let socket = UdpSocket::bind(("10.250.0.2", 0)).await?;
    let local = socket.local_addr()?;
    socket.send_to(b"tun-packet", ("10.21.0.5", 9001)).await?;
    let packet = timeout(Duration::from_secs(3), async {
        loop {
            if let Some(packet) = device.read_packet(500).await? {
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
    anyhow::ensure!(
        &packet[28..] == b"tun-packet",
        "kernel packet payload changed"
    );
    let payload = b"tun-reply";
    let mut reply = vec![0u8; 28 + payload.len()];
    reply[0] = 0x45;
    let length = reply.len() as u16;
    reply[2..4].copy_from_slice(&length.to_be_bytes());
    reply[8] = 64;
    reply[9] = 17;
    reply[12..16].copy_from_slice(&[10, 21, 0, 5]);
    reply[16..20].copy_from_slice(&[10, 250, 0, 2]);
    reply[20..22].copy_from_slice(&9001u16.to_be_bytes());
    reply[22..24].copy_from_slice(&local.port().to_be_bytes());
    reply[24..26].copy_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
    reply[28..].copy_from_slice(payload);
    let crc4 = checksum(&reply[..20]);
    reply[10..12].copy_from_slice(&crc4.to_be_bytes());
    device.write_packet(reply).await?;
    let mut received = [0u8; 64];
    let length = timeout(Duration::from_secs(3), socket.recv(&mut received)).await??;
    anyhow::ensure!(
        &received[..length] == payload,
        "TUN injection did not reach the socket"
    );
    let socket6 = UdpSocket::bind(("2001:db8:250::2", 0)).await?;
    let local6 = socket6.local_addr()?;
    socket6.send_to(b"tun-v6", ("2001:db8:21::5", 9002)).await?;
    timeout(Duration::from_secs(3), async {
        loop {
            if let Some(packet) = device.read_packet(500).await? {
                if packet.len() >= 48
                    && packet[0] >> 4 == 6
                    && packet[6] == 17
                    && packet[40..42] == local6.port().to_be_bytes()
                {
                    anyhow::ensure!(&packet[48..] == b"tun-v6", "IPv6 capture changed");
                    return Ok::<_, anyhow::Error>(());
                }
            }
        }
    })
    .await??;
    let source6: std::net::Ipv6Addr = "2001:db8:21::5".parse()?;
    let dest6: std::net::Ipv6Addr = "2001:db8:250::2".parse()?;
    let payload6 = b"reply-v6";
    let mut reply6 = vec![0u8; 48 + payload6.len()];
    reply6[0] = 0x60;
    let udp_length = (8 + payload6.len()) as u16;
    reply6[4..6].copy_from_slice(&udp_length.to_be_bytes());
    reply6[6] = 17;
    reply6[7] = 64;
    reply6[8..24].copy_from_slice(&source6.octets());
    reply6[24..40].copy_from_slice(&dest6.octets());
    reply6[40..42].copy_from_slice(&9002u16.to_be_bytes());
    reply6[42..44].copy_from_slice(&local6.port().to_be_bytes());
    reply6[44..46].copy_from_slice(&udp_length.to_be_bytes());
    reply6[48..].copy_from_slice(payload6);
    let mut pseudo = source6.octets().to_vec();
    pseudo.extend_from_slice(&dest6.octets());
    pseudo.extend_from_slice(&u32::from(udp_length).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0, 17]);
    pseudo.extend_from_slice(&reply6[40..]);
    let crc = checksum(&pseudo);
    reply6[46..48].copy_from_slice(&(if crc == 0 { u16::MAX } else { crc }).to_be_bytes());
    device.write_packet(reply6).await?;
    let length = timeout(Duration::from_secs(3), socket6.recv(&mut received)).await??;
    anyhow::ensure!(&received[..length] == payload6, "IPv6 injection failed");
    while device.read_packet(10).await?.is_some() {}
    let waiting = device.read_packet(5000);
    let closing = async {
        tokio::time::sleep(Duration::from_millis(30)).await;
        device.close().await
    };
    let (read, close) = tokio::join!(waiting, closing);
    close?;
    anyhow::ensure!(read.is_err(), "close did not wake packet read");
    device.close().await?;
    let result = Command::new("/usr/sbin/ip")
        .args(["link", "show", "dev", name])
        .output()?;
    anyhow::ensure!(
        !result.status.success(),
        "owned TUN interface leaked after close"
    );
    anyhow::ensure!(
        before == ip(&["-j", "route", "show", "default"])?,
        "default route changed"
    );
    println!("TUN_NATIVE_PACKET_AND_CLEANUP_PASSED");
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
fn main() {
    eprintln!("This isolated TUN probe requires Linux.");
    std::process::exit(1);
}
