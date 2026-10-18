use anyhow::{ensure, Context};
use smoltcp::wire::{IpAddress, IpProtocol, Ipv4Packet, Ipv6Packet, UdpPacket};
use std::net::{IpAddr, SocketAddr};

pub(super) fn reply(
    bytes: &[u8],
    local: SocketAddr,
    peer: SocketAddr,
    payload: &[u8],
) -> anyhow::Result<Option<Vec<u8>>> {
    let (source, destination, header) = match bytes.first().map(|byte| byte >> 4) {
        Some(4) => {
            let ip = Ipv4Packet::new_checked(bytes).context("invalid kernel IPv4 packet")?;
            if ip.next_header() != IpProtocol::Udp {
                return Ok(None);
            }
            ensure!(
                !ip.more_frags() && ip.frag_offset() == 0,
                "unexpected fragmented UDP fixture"
            );
            ensure!(
                usize::from(ip.total_len()) == bytes.len(),
                "IPv4 length mismatch"
            );
            (
                IpAddr::V4(ip.src_addr()),
                IpAddr::V4(ip.dst_addr()),
                usize::from(ip.header_len()),
            )
        }
        Some(6) => {
            let ip = Ipv6Packet::new_checked(bytes).context("invalid kernel IPv6 packet")?;
            if ip.next_header() != IpProtocol::Udp {
                return Ok(None);
            }
            ensure!(
                usize::from(ip.payload_len()) + 40 == bytes.len(),
                "IPv6 length mismatch"
            );
            (IpAddr::V6(ip.src_addr()), IpAddr::V6(ip.dst_addr()), 40)
        }
        _ => anyhow::bail!("unexpected kernel packet version"),
    };
    let udp = UdpPacket::new_checked(&bytes[header..]).context("invalid kernel UDP packet")?;
    ensure!(
        usize::from(udp.len()) == bytes.len() - header,
        "UDP length mismatch"
    );
    if SocketAddr::new(source, udp.src_port()) != local
        || SocketAddr::new(destination, udp.dst_port()) != peer
        || udp.payload() != payload
    {
        return Ok(None);
    }
    // Kernel transmit checksums may be offloaded. Always calculate fresh
    // checksums on the injected response, whose bytes the kernel validates.
    let mut reply = bytes.to_vec();
    if source.is_ipv4() {
        let mut ip = Ipv4Packet::new_unchecked(reply.as_mut_slice());
        if let (IpAddr::V4(source), IpAddr::V4(destination)) = (source, destination) {
            ip.set_src_addr(destination);
            ip.set_dst_addr(source);
        }
        ip.fill_checksum();
    } else {
        let mut ip = Ipv6Packet::new_unchecked(reply.as_mut_slice());
        if let (IpAddr::V6(source), IpAddr::V6(destination)) = (source, destination) {
            ip.set_src_addr(destination);
            ip.set_dst_addr(source);
        }
    }
    let mut udp = UdpPacket::new_unchecked(&mut reply[header..]);
    udp.set_src_port(peer.port());
    udp.set_dst_port(local.port());
    udp.fill_checksum(&IpAddress::from(destination), &IpAddress::from(source));
    Ok(Some(reply))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(local: SocketAddr, peer: SocketAddr, payload: &[u8]) -> Vec<u8> {
        let header = if local.is_ipv4() { 24 } else { 40 };
        let mut bytes = vec![0; header + 8 + payload.len()];
        match (local.ip(), peer.ip()) {
            (IpAddr::V4(source), IpAddr::V4(destination)) => {
                bytes[0] = 0x46; // Exercise an IPv4 header with options.
                let length = bytes.len() as u16;
                bytes[2..4].copy_from_slice(&length.to_be_bytes());
                bytes[8] = 64;
                bytes[9] = 17;
                bytes[12..16].copy_from_slice(&source.octets());
                bytes[16..20].copy_from_slice(&destination.octets());
                Ipv4Packet::new_unchecked(bytes.as_mut_slice()).fill_checksum();
            }
            (IpAddr::V6(source), IpAddr::V6(destination)) => {
                bytes[0] = 0x60;
                bytes[4..6].copy_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
                bytes[6] = 17;
                bytes[7] = 64;
                bytes[8..24].copy_from_slice(&source.octets());
                bytes[24..40].copy_from_slice(&destination.octets());
            }
            _ => panic!("fixture families must match"),
        }
        let mut udp = UdpPacket::new_unchecked(&mut bytes[header..]);
        udp.set_src_port(local.port());
        udp.set_dst_port(peer.port());
        udp.set_len((8 + payload.len()) as u16);
        udp.payload_mut().copy_from_slice(payload);
        // Simulate checksum offload, including IPv6.
        udp.set_checksum(0);
        bytes
    }

    #[test]
    fn ring_reply_reverses_dual_stack_tuples_and_calculates_injection_checksums(
    ) -> anyhow::Result<()> {
        for (local, peer) in [
            ("192.0.2.2:49152", "192.0.2.3:19518"),
            ("[2001:db8:518::2]:49152", "[2001:db8:518::3]:19518"),
        ] {
            let local: SocketAddr = local.parse()?;
            let peer: SocketAddr = peer.parse()?;
            let bytes = fixture(local, peer, b"ring-payload");
            let response = reply(&bytes, local, peer, b"ring-payload")?.context("missing reply")?;
            let header = if local.is_ipv4() {
                let ip = Ipv4Packet::new_checked(response.as_slice())?;
                ensure!(ip.verify_checksum(), "invalid IPv4 injection checksum");
                ensure!(IpAddr::V4(ip.src_addr()) == peer.ip(), "wrong source");
                ensure!(IpAddr::V4(ip.dst_addr()) == local.ip(), "wrong destination");
                24
            } else {
                let ip = Ipv6Packet::new_checked(response.as_slice())?;
                ensure!(IpAddr::V6(ip.src_addr()) == peer.ip(), "wrong source");
                ensure!(IpAddr::V6(ip.dst_addr()) == local.ip(), "wrong destination");
                40
            };
            let udp = UdpPacket::new_checked(&response[header..])?;
            ensure!(
                udp.src_port() == peer.port()
                    && udp.dst_port() == local.port()
                    && udp.payload() == b"ring-payload"
                    && udp.checksum() != 0
                    && udp.verify_checksum(&peer.ip().into(), &local.ip().into()),
                "invalid injected UDP reply"
            );
            assert!(reply(&bytes, local, peer, b"different")?.is_none());
            assert!(reply(&bytes, peer, local, b"ring-payload")?.is_none());
            assert!(reply(&bytes[..bytes.len() - 1], local, peer, b"ring-payload").is_err());
        }
        Ok(())
    }
}
