use std::net::{IpAddr, SocketAddr};

pub fn packet(source: SocketAddr, destination: SocketAddr, payload: &[u8]) -> Vec<u8> {
    let header = if source.is_ipv4() { 20 } else { 40 };
    let mut bytes = vec![0; header + 8 + payload.len()];
    let length = bytes.len();
    match (source.ip(), destination.ip()) {
        (IpAddr::V4(source), IpAddr::V4(destination)) => {
            bytes[0] = 0x45;
            bytes[2..4].copy_from_slice(&(length as u16).to_be_bytes());
            bytes[8] = 64;
            bytes[9] = 17;
            bytes[12..16].copy_from_slice(&source.octets());
            bytes[16..20].copy_from_slice(&destination.octets());
            let checksum = checksum(&bytes[..20]);
            bytes[10..12].copy_from_slice(&checksum.to_be_bytes());
        }
        (IpAddr::V6(source), IpAddr::V6(destination)) => {
            bytes[0] = 0x60;
            bytes[4..6].copy_from_slice(&((length - 40) as u16).to_be_bytes());
            bytes[6] = 17;
            bytes[7] = 64;
            bytes[8..24].copy_from_slice(&source.octets());
            bytes[24..40].copy_from_slice(&destination.octets());
        }
        _ => panic!("test fixture needs one address family"),
    }
    bytes[header..header + 2].copy_from_slice(&source.port().to_be_bytes());
    bytes[header + 2..header + 4].copy_from_slice(&destination.port().to_be_bytes());
    bytes[header + 4..header + 6].copy_from_slice(&((length - header) as u16).to_be_bytes());
    bytes[header + 8..].copy_from_slice(payload);
    if source.is_ipv6() {
        let mut pseudo = bytes[8..40].to_vec();
        pseudo.extend_from_slice(&((length - header) as u32).to_be_bytes());
        pseudo.extend_from_slice(&[0, 0, 0, 17]);
        pseudo.extend_from_slice(&bytes[header..]);
        let checksum = checksum(&pseudo);
        bytes[header + 6..header + 8]
            .copy_from_slice(&if checksum == 0 { u16::MAX } else { checksum }.to_be_bytes());
    }
    bytes
}

fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    for pair in bytes.chunks(2) {
        sum += u32::from(pair[0]) << 8;
        sum += u32::from(*pair.get(1).unwrap_or(&0));
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}
