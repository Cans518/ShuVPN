use super::codec::{self, Tuple};
use crate::connection::model::VpnFailure;
use std::net::{IpAddr, SocketAddr};

pub(super) const FIN: u8 = 1;
pub(super) const SYN: u8 = 2;
pub(super) const RST: u8 = 4;
#[cfg(test)]
pub(super) const PSH: u8 = 8;
pub(super) const ACK: u8 = 16;

pub(super) struct Segment<'a> {
    pub tuple: Tuple,
    pub sequence: u32,
    pub acknowledgment: u32,
    pub flags: u8,
    pub payload: &'a [u8],
}

impl Segment<'_> {
    pub(super) fn is_opening(&self) -> bool {
        self.flags & (SYN | ACK | RST | FIN) == SYN && self.payload.is_empty()
    }

    pub(super) fn reset(&self) -> Result<Option<Vec<u8>>, VpnFailure> {
        if self.flags & RST != 0 {
            return Ok(None);
        }
        let (sequence, acknowledgment, flags) = if self.flags & ACK != 0 {
            (self.acknowledgment, 0, RST)
        } else {
            let consumed = self.payload.len() as u32
                + u32::from(self.flags & SYN != 0)
                + u32::from(self.flags & FIN != 0);
            (0, self.sequence.wrapping_add(consumed), RST | ACK)
        };
        Ok(Some(build(
            self.tuple.reverse(),
            sequence,
            acknowledgment,
            flags,
            0,
            &[],
            None,
        )?))
    }
}

pub(super) fn parse(packet: &[u8]) -> Result<Segment<'_>, VpnFailure> {
    let ip = codec::parse_ip(packet)?;
    if ip.protocol != 6 || ip.payload.len() < 20 {
        return Err(invalid());
    }
    let tcp = ip.payload;
    let header = usize::from(tcp[12] >> 4) * 4;
    let tuple = Tuple {
        source: SocketAddr::new(ip.source, u16::from_be_bytes([tcp[0], tcp[1]])),
        destination: SocketAddr::new(ip.destination, u16::from_be_bytes([tcp[2], tcp[3]])),
    };
    if header < 20
        || header > tcp.len()
        || tuple.source.port() == 0
        || tuple.destination.port() == 0
        || codec::transport_checksum(tuple, 6, tcp) != 0
    {
        return Err(invalid());
    }
    if tcp[13] & SYN != 0 && tcp.len() != header {
        return Err(VpnFailure::new("packet_protocol", "TCP Fast Open 尚未接入"));
    }
    Ok(Segment {
        tuple,
        sequence: u32::from_be_bytes([tcp[4], tcp[5], tcp[6], tcp[7]]),
        acknowledgment: u32::from_be_bytes([tcp[8], tcp[9], tcp[10], tcp[11]]),
        flags: tcp[13],
        payload: &tcp[header..],
    })
}

pub(super) fn separate_fin(mut packet: Vec<u8>) -> Result<(Vec<u8>, Option<Vec<u8>>), VpnFailure> {
    let segment = parse(&packet)?;
    if segment.flags & (FIN | SYN | RST) != FIN || segment.payload.is_empty() {
        return Ok((packet, None));
    }
    let tuple = segment.tuple;
    let sequence = segment.sequence.wrapping_add(segment.payload.len() as u32);
    let mut fin = packet[..packet.len() - segment.payload.len()].to_vec();
    let ip_header = packet.len() - codec::parse_ip(&packet)?.payload.len();
    packet[ip_header + 13] &= !FIN;
    fin[ip_header + 4..ip_header + 8].copy_from_slice(&sequence.to_be_bytes());
    let length = fin.len();
    if tuple.source.is_ipv4() {
        fin[2..4].copy_from_slice(&(length as u16).to_be_bytes());
        fin[10..12].fill(0);
        let checksum = codec::checksum(&fin[..ip_header]);
        fin[10..12].copy_from_slice(&checksum.to_be_bytes());
    } else {
        fin[4..6].copy_from_slice(&((length - ip_header) as u16).to_be_bytes());
    }
    for bytes in [&mut packet[..], &mut fin[..]] {
        let tcp = &mut bytes[ip_header..];
        tcp[16..18].fill(0);
        let checksum = codec::transport_checksum(tuple, 6, tcp);
        tcp[16..18].copy_from_slice(&checksum.to_be_bytes());
    }
    Ok((packet, Some(fin)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    tuple: Tuple,
    sequence: u32,
    acknowledgment: u32,
    flags: u8,
    window: u16,
    payload: &[u8],
    mss: Option<u16>,
) -> Result<Vec<u8>, VpnFailure> {
    let header = if tuple.source.is_ipv4() { 20 } else { 40 };
    let tcp_header = if mss.is_some() { 24 } else { 20 };
    let length = header + tcp_header + payload.len();
    if length > 9000 {
        return Err(invalid());
    }
    let mut bytes = vec![0; length];
    match (tuple.source.ip(), tuple.destination.ip()) {
        (IpAddr::V4(source), IpAddr::V4(destination)) => {
            bytes[0] = 0x45;
            bytes[2..4].copy_from_slice(&(length as u16).to_be_bytes());
            bytes[8] = 64;
            bytes[9] = 6;
            bytes[12..16].copy_from_slice(&source.octets());
            bytes[16..20].copy_from_slice(&destination.octets());
            let checksum = codec::checksum(&bytes[..header]);
            bytes[10..12].copy_from_slice(&checksum.to_be_bytes());
        }
        (IpAddr::V6(source), IpAddr::V6(destination)) => {
            bytes[0] = 0x60;
            bytes[4..6].copy_from_slice(&((length - header) as u16).to_be_bytes());
            bytes[6] = 6;
            bytes[7] = 64;
            bytes[8..24].copy_from_slice(&source.octets());
            bytes[24..40].copy_from_slice(&destination.octets());
        }
        _ => return Err(invalid()),
    }
    let tcp = &mut bytes[header..];
    tcp[..2].copy_from_slice(&tuple.source.port().to_be_bytes());
    tcp[2..4].copy_from_slice(&tuple.destination.port().to_be_bytes());
    tcp[4..8].copy_from_slice(&sequence.to_be_bytes());
    tcp[8..12].copy_from_slice(&acknowledgment.to_be_bytes());
    tcp[12] = (tcp_header as u8 / 4) << 4;
    tcp[13] = flags;
    tcp[14..16].copy_from_slice(&window.to_be_bytes());
    if let Some(mss) = mss {
        tcp[20..24].copy_from_slice(&[2, 4, (mss >> 8) as u8, mss as u8]);
    }
    tcp[tcp_header..].copy_from_slice(payload);
    let checksum = codec::transport_checksum(tuple, 6, tcp);
    tcp[16..18].copy_from_slice(&checksum.to_be_bytes());
    Ok(bytes)
}

fn invalid() -> VpnFailure {
    VpnFailure::new("packet", "TCP 包头、校验和或长度无效")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn separated_fin_preserves_payload_options_checksums_and_sequence_wraparound(
    ) -> anyhow::Result<()> {
        for (source, destination) in [
            ("10.99.0.2:51000", "10.21.0.5:443"),
            ("[2001:db8::2]:51000", "[2001:db8:21::5]:443"),
        ] {
            let tuple = Tuple {
                source: source.parse()?,
                destination: destination.parse()?,
            };
            let original = build(
                tuple,
                u32::MAX - 3,
                9,
                ACK | FIN,
                32768,
                b"abcdef",
                Some(1200),
            )?;
            let (data, fin) = separate_fin(original)?;
            let fin = fin.ok_or_else(|| anyhow::anyhow!("missing separate FIN"))?;
            let payload = parse(&data)?;
            let finished = parse(&fin)?;
            assert_eq!(payload.flags, ACK);
            assert_eq!(payload.sequence, u32::MAX - 3);
            assert_eq!(payload.payload, b"abcdef");
            assert_eq!(finished.flags, ACK | FIN);
            assert_eq!(finished.sequence, 2);
            assert_eq!(finished.acknowledgment, 9);
            assert!(finished.payload.is_empty());
            let header = if tuple.source.is_ipv4() { 20 } else { 40 };
            assert_eq!(
                &data[header + 20..header + 24],
                &fin[header + 20..header + 24]
            );
        }
        Ok(())
    }

    #[test]
    fn dual_stack_checksums_headers_options_and_reset_sequences_are_valid() -> anyhow::Result<()> {
        for (source, destination) in [
            ("10.99.0.2:51000", "10.21.0.5:443"),
            ("[2001:db8::2]:51000", "[2001:db8:21::5]:443"),
        ] {
            let tuple = Tuple {
                source: source.parse()?,
                destination: destination.parse()?,
            };
            let packet = build(tuple, u32::MAX, 0, SYN, 32768, &[], Some(1200))?;
            let segment = parse(&packet)?;
            assert!(segment.is_opening());
            let reset = segment
                .reset()?
                .ok_or_else(|| anyhow::anyhow!("missing reset"))?;
            let reset = parse(&reset)?;
            assert_eq!(reset.acknowledgment, 0);
            assert_eq!(reset.flags, RST | ACK);
            assert_eq!(reset.tuple, tuple.reverse());
            let payload = build(tuple, 1, 9, ACK | PSH, 32768, b"payload", None)?;
            assert_eq!(parse(&payload)?.payload, b"payload");
            let mut corrupt = payload;
            *corrupt
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("empty packet"))? ^= 1;
            assert!(parse(&corrupt).is_err());
        }
        Ok(())
    }
}
