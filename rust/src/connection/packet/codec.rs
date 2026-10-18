use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::connection::model::VpnFailure;
use crate::routing::model::{Flow, Inbound, Network};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Tuple {
    pub source: SocketAddr,
    pub destination: SocketAddr,
}

pub(super) struct Datagram<'a> {
    pub tuple: Tuple,
    pub payload: &'a [u8],
}

pub(super) struct IpPacket<'a> {
    pub source: IpAddr,
    pub destination: IpAddr,
    pub protocol: u8,
    pub payload: &'a [u8],
}

impl Tuple {
    pub(super) fn flow(self) -> Flow {
        Flow {
            host: self.destination.ip().to_string(),
            ips: vec![self.destination.ip()],
            port: self.destination.port(),
            network: Network::Udp,
            inbound: Inbound::Tun,
            process: None,
            domain_candidates: Vec::new(),
        }
    }

    pub(super) fn reverse(self) -> Self {
        Self {
            source: self.destination,
            destination: self.source,
        }
    }

    pub(super) fn packet(self, payload: &[u8]) -> Result<Vec<u8>, VpnFailure> {
        let header = if self.source.is_ipv4() { 20 } else { 40 };
        if payload.len() + header + 8 > 9000 || self.source.is_ipv4() != self.destination.is_ipv4()
        {
            return Err(invalid());
        }
        let length = header + 8 + payload.len();
        let mut packet = vec![0; length];
        match (self.source.ip(), self.destination.ip()) {
            (IpAddr::V4(source), IpAddr::V4(destination)) => {
                packet[0] = 0x45;
                packet[2..4].copy_from_slice(&(length as u16).to_be_bytes());
                packet[8] = 64;
                packet[9] = 17;
                packet[12..16].copy_from_slice(&source.octets());
                packet[16..20].copy_from_slice(&destination.octets());
                let checksum = checksum(&packet[..20]);
                packet[10..12].copy_from_slice(&checksum.to_be_bytes());
            }
            (IpAddr::V6(source), IpAddr::V6(destination)) => {
                packet[0] = 0x60;
                packet[4..6].copy_from_slice(&((length - 40) as u16).to_be_bytes());
                packet[6] = 17;
                packet[7] = 64;
                packet[8..24].copy_from_slice(&source.octets());
                packet[24..40].copy_from_slice(&destination.octets());
            }
            _ => return Err(invalid()),
        }
        packet[header..header + 2].copy_from_slice(&self.source.port().to_be_bytes());
        packet[header + 2..header + 4].copy_from_slice(&self.destination.port().to_be_bytes());
        packet[header + 4..header + 6].copy_from_slice(&((length - header) as u16).to_be_bytes());
        packet[header + 8..].copy_from_slice(payload);
        let value = udp_checksum(self, &packet[header..]);
        packet[header + 6..header + 8]
            .copy_from_slice(&if value == 0 { u16::MAX } else { value }.to_be_bytes());
        Ok(packet)
    }
}

pub(super) fn parse(packet: &[u8]) -> Result<Datagram<'_>, VpnFailure> {
    let ip = parse_ip(packet)?;
    if ip.protocol != 17 {
        return Err(unsupported());
    }
    let (source, destination, udp) = (ip.source, ip.destination, ip.payload);
    if udp.len() < 8 {
        return Err(invalid());
    }
    let tuple = Tuple {
        source: SocketAddr::new(source, u16::from_be_bytes([udp[0], udp[1]])),
        destination: SocketAddr::new(destination, u16::from_be_bytes([udp[2], udp[3]])),
    };
    let received_checksum = u16::from_be_bytes([udp[6], udp[7]]);
    if usize::from(u16::from_be_bytes([udp[4], udp[5]])) != udp.len()
        || tuple.source.port() == 0
        || tuple.destination.port() == 0
        || (received_checksum == 0 && source.is_ipv6())
        || (received_checksum != 0 && udp_checksum(tuple, udp) != 0)
    {
        return Err(invalid());
    }
    Ok(Datagram {
        tuple,
        payload: &udp[8..],
    })
}

pub(super) fn parse_ip(packet: &[u8]) -> Result<IpPacket<'_>, VpnFailure> {
    if packet.len() > 9000 {
        return Err(invalid());
    }
    let (source, destination, header, protocol) = match packet.first().map(|byte| byte >> 4) {
        Some(4) if packet.len() >= 20 => {
            let header = usize::from(packet[0] & 15) * 4;
            if header < 20
                || header > packet.len()
                || usize::from(u16::from_be_bytes([packet[2], packet[3]])) != packet.len()
                || checksum(&packet[..header]) != 0
                || u16::from_be_bytes([packet[6], packet[7]]) & 0xbfff != 0
            {
                return Err(invalid());
            }
            (
                IpAddr::V4(Ipv4Addr::new(
                    packet[12], packet[13], packet[14], packet[15],
                )),
                IpAddr::V4(Ipv4Addr::new(
                    packet[16], packet[17], packet[18], packet[19],
                )),
                header,
                packet[9],
            )
        }
        Some(6) if packet.len() >= 40 => {
            if usize::from(u16::from_be_bytes([packet[4], packet[5]])) + 40 != packet.len() {
                return Err(invalid());
            }
            let mut source = [0; 16];
            let mut destination = [0; 16];
            source.copy_from_slice(&packet[8..24]);
            destination.copy_from_slice(&packet[24..40]);
            (
                IpAddr::V6(Ipv6Addr::from(source)),
                IpAddr::V6(Ipv6Addr::from(destination)),
                40,
                packet[6],
            )
        }
        _ => return Err(invalid()),
    };
    if [source, destination].iter().any(|address| match address {
        IpAddr::V4(address) => address.is_broadcast(),
        IpAddr::V6(address) => {
            address.is_unicast_link_local() || address.to_ipv4_mapped().is_some()
        }
    }) {
        return Err(unsupported());
    }
    if source.is_unspecified()
        || source.is_multicast()
        || destination.is_unspecified()
        || destination.is_multicast()
    {
        return Err(invalid());
    }
    Ok(IpPacket {
        source,
        destination,
        protocol,
        payload: &packet[header..],
    })
}

fn udp_checksum(tuple: Tuple, udp: &[u8]) -> u16 {
    transport_checksum(tuple, 17, udp)
}

pub(super) fn transport_checksum(tuple: Tuple, protocol: u8, transport: &[u8]) -> u16 {
    let mut pseudo = Vec::with_capacity(40 + transport.len());
    match (tuple.source.ip(), tuple.destination.ip()) {
        (IpAddr::V4(source), IpAddr::V4(destination)) => {
            pseudo.extend_from_slice(&source.octets());
            pseudo.extend_from_slice(&destination.octets());
            pseudo.extend_from_slice(&[0, protocol]);
            pseudo.extend_from_slice(&(transport.len() as u16).to_be_bytes());
        }
        (IpAddr::V6(source), IpAddr::V6(destination)) => {
            pseudo.extend_from_slice(&source.octets());
            pseudo.extend_from_slice(&destination.octets());
            pseudo.extend_from_slice(&(transport.len() as u32).to_be_bytes());
            pseudo.extend_from_slice(&[0, 0, 0, protocol]);
        }
        _ => return 1,
    }
    pseudo.extend_from_slice(transport);
    checksum(&pseudo)
}

pub(super) fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = bytes.chunks(2).fold(0u32, |sum, pair| {
        sum + u32::from(u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)]))
    });
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn invalid() -> VpnFailure {
    VpnFailure::new("packet", "UDP IP 包格式、校验和、分片或长度无效")
}
fn unsupported() -> VpnFailure {
    VpnFailure::new(
        "packet_protocol",
        "此包端口当前支持未分片 UDP；TCP、扩展头与其他协议尚未接入",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_stack_packets_preserve_tuples_payloads_and_checksums() -> anyhow::Result<()> {
        for (source, destination) in [
            ("10.99.0.2:50123", "10.21.0.5:9001"),
            ("[2001:db8::2]:50123", "[2001:db8::5]:9001"),
        ] {
            let tuple = Tuple {
                source: source.parse()?,
                destination: destination.parse()?,
            };
            for payload in [b"".as_slice(), b"odd", b"even"] {
                let packet = tuple.packet(payload)?;
                let parsed = parse(&packet)?;
                assert_eq!(parsed.tuple, tuple);
                assert_eq!(parsed.payload, payload);
                assert_eq!(
                    parse(&tuple.reverse().packet(payload)?)?.tuple,
                    tuple.reverse()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn malformed_checksums_lengths_fragments_and_unknown_protocols_fail() -> anyhow::Result<()> {
        let tuple = Tuple {
            source: "10.99.0.2:50123".parse()?,
            destination: "10.21.0.5:9001".parse()?,
        };
        let packet = tuple.packet(b"test")?;
        for index in [0, 2, 6, 9, 10, 20, 24, 26, 28] {
            let mut changed = packet.clone();
            changed[index] ^= 1;
            assert!(parse(&changed).is_err(), "index {index}");
        }
        for length in 0..packet.len() {
            assert!(parse(&packet[..length]).is_err());
        }
        Ok(())
    }
}
