use anyhow::Context;
use shuvpn_core::api::vpn::VpnPacketPort;
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use tokio::time::{timeout, Duration};

pub struct Client<'a> {
    pub port: &'a VpnPacketPort,
    pub source: SocketAddr,
    pub destination: SocketAddr,
    sequence: u32,
    acknowledged: u32,
    peer_sequence: u32,
    window: u16,
    received: VecDeque<u8>,
    pub eof: bool,
    pub reset: bool,
}

impl<'a> Client<'a> {
    pub async fn connect(
        port: &'a VpnPacketPort,
        source: SocketAddr,
        destination: SocketAddr,
    ) -> anyhow::Result<Self> {
        let mut client = Self {
            port,
            source,
            destination,
            sequence: 1000,
            acknowledged: 1000,
            peer_sequence: 0,
            window: 32768,
            received: VecDeque::new(),
            eof: false,
            reset: false,
        };
        client
            .port
            .write_packet(packet(source, destination, 1000, 0, 2, client.window, &[]))
            .await?;
        let reply = client.next_packet().await?;
        let header = if reply[0] >> 4 == 4 {
            usize::from(reply[0] & 15) * 4
        } else {
            40
        };
        anyhow::ensure!(reply[header + 13] & 4 == 0, "opening SYN was reset");
        anyhow::ensure!(reply[header + 13] & 0x12 == 0x12, "missing SYN-ACK");
        client.peer_sequence =
            u32::from_be_bytes(reply[header + 4..header + 8].try_into()?).wrapping_add(1);
        client.sequence += 1;
        client.acknowledged += 1;
        client.ack().await?;
        Ok(client)
    }

    async fn next_packet(&self) -> anyhow::Result<Vec<u8>> {
        timeout(Duration::from_secs(3), async {
            loop {
                if let Some(packet) = self.port.read_packet(500).await? {
                    return Ok(packet);
                }
            }
        })
        .await
        .context(format!(
            "raw TCP wait: {} -> {}, seq={}, acked={}, peer_seq={}, buffered={}, eof={}, window={}",
            self.source,
            self.destination,
            self.sequence,
            self.acknowledged,
            self.peer_sequence,
            self.received.len(),
            self.eof,
            self.window,
        ))?
    }

    pub async fn next(&mut self) -> anyhow::Result<()> {
        let packet = self.next_packet().await?;
        let header = if packet[0] >> 4 == 4 {
            usize::from(packet[0] & 15) * 4
        } else {
            40
        };
        anyhow::ensure!(packet.len() >= header + 20, "truncated TCP reply");
        let tcp = &packet[header..];
        anyhow::ensure!(
            u16::from_be_bytes(tcp[..2].try_into()?) == self.destination.port(),
            "wrong source port"
        );
        anyhow::ensure!(
            u16::from_be_bytes(tcp[2..4].try_into()?) == self.source.port(),
            "wrong destination port"
        );
        let flags = tcp[13];
        let sequence = u32::from_be_bytes(tcp[4..8].try_into()?);
        let acknowledgment = u32::from_be_bytes(tcp[8..12].try_into()?);
        let offset = usize::from(tcp[12] >> 4) * 4;
        let payload = &tcp[offset..];
        if flags & 4 != 0 {
            self.reset = true;
            return Ok(());
        }
        if flags & 16 != 0 && acknowledgment.wrapping_sub(self.acknowledged) < 0x8000_0000 {
            self.acknowledged = acknowledgment;
        }
        let overlap = self.peer_sequence.wrapping_sub(sequence);
        if overlap < 0x8000_0000 && overlap as usize <= payload.len() {
            let fresh = &payload[overlap as usize..];
            self.received.extend(fresh);
            self.peer_sequence = self.peer_sequence.wrapping_add(fresh.len() as u32);
            if flags & 1 != 0 && !self.eof {
                self.peer_sequence = self.peer_sequence.wrapping_add(1);
                self.eof = true;
            }
        }
        if !payload.is_empty() || flags & 3 != 0 {
            self.ack().await?;
        }
        Ok(())
    }

    async fn ack(&self) -> anyhow::Result<()> {
        self.port
            .write_packet(packet(
                self.source,
                self.destination,
                self.sequence,
                self.peer_sequence,
                16,
                self.window,
                &[],
            ))
            .await?;
        Ok(())
    }

    pub async fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        for chunk in bytes.chunks(1000) {
            let segment = packet(
                self.source,
                self.destination,
                self.sequence,
                self.peer_sequence,
                24,
                self.window,
                chunk,
            );
            self.port.write_packet(segment.clone()).await?;
            self.sequence = self.sequence.wrapping_add(chunk.len() as u32);
            timeout(Duration::from_secs(5), async {
                while self.acknowledged != self.sequence && !self.reset {
                    match timeout(Duration::from_millis(200), self.next()).await {
                        Ok(result) => result?,
                        Err(_) => self.port.write_packet(segment.clone()).await?,
                    }
                }
                Ok::<_, anyhow::Error>(())
            })
            .await??;
            anyhow::ensure!(!self.reset, "client send was reset");
        }
        Ok(())
    }

    pub async fn read(&mut self, length: usize) -> anyhow::Result<Vec<u8>> {
        while self.received.len() < length && !self.eof && !self.reset {
            self.next().await?;
        }
        anyhow::ensure!(self.received.len() >= length, "TCP response ended early");
        Ok(self.received.drain(..length).collect())
    }

    pub async fn finish(&mut self) -> anyhow::Result<()> {
        self.port
            .write_packet(packet(
                self.source,
                self.destination,
                self.sequence,
                self.peer_sequence,
                17,
                self.window,
                &[],
            ))
            .await?;
        self.sequence = self.sequence.wrapping_add(1);
        Ok(())
    }

    pub async fn window(&mut self, window: u16) -> anyhow::Result<()> {
        self.window = window;
        self.ack().await
    }

    pub async fn wait_eof(&mut self) -> anyhow::Result<()> {
        while !self.eof && !self.reset {
            self.next().await?;
        }
        anyhow::ensure!(!self.reset, "received reset instead of FIN");
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn packet(
    source: SocketAddr,
    destination: SocketAddr,
    sequence: u32,
    acknowledgment: u32,
    flags: u8,
    window: u16,
    payload: &[u8],
) -> Vec<u8> {
    let header = if source.is_ipv4() { 20 } else { 40 };
    let mut bytes = vec![0; header + 20 + payload.len()];
    let length = bytes.len();
    let mut pseudo = Vec::new();
    match (source.ip(), destination.ip()) {
        (IpAddr::V4(source), IpAddr::V4(destination)) => {
            bytes[0] = 0x45;
            bytes[2..4].copy_from_slice(&(length as u16).to_be_bytes());
            bytes[8] = 64;
            bytes[9] = 6;
            bytes[12..16].copy_from_slice(&source.octets());
            bytes[16..20].copy_from_slice(&destination.octets());
            let checksum = checksum(&bytes[..20]);
            bytes[10..12].copy_from_slice(&checksum.to_be_bytes());
            pseudo.extend_from_slice(&bytes[12..20]);
            pseudo.extend_from_slice(&[0, 6]);
            pseudo.extend_from_slice(&((length - header) as u16).to_be_bytes());
        }
        (IpAddr::V6(source), IpAddr::V6(destination)) => {
            bytes[0] = 0x60;
            bytes[4..6].copy_from_slice(&((length - header) as u16).to_be_bytes());
            bytes[6] = 6;
            bytes[7] = 64;
            bytes[8..24].copy_from_slice(&source.octets());
            bytes[24..40].copy_from_slice(&destination.octets());
            pseudo.extend_from_slice(&bytes[8..40]);
            pseudo.extend_from_slice(&((length - header) as u32).to_be_bytes());
            pseudo.extend_from_slice(&[0, 0, 0, 6]);
        }
        _ => panic!("test TCP family mismatch"),
    }
    let tcp = &mut bytes[header..];
    tcp[..2].copy_from_slice(&source.port().to_be_bytes());
    tcp[2..4].copy_from_slice(&destination.port().to_be_bytes());
    tcp[4..8].copy_from_slice(&sequence.to_be_bytes());
    tcp[8..12].copy_from_slice(&acknowledgment.to_be_bytes());
    tcp[12] = 0x50;
    tcp[13] = flags;
    tcp[14..16].copy_from_slice(&window.to_be_bytes());
    tcp[20..].copy_from_slice(payload);
    pseudo.extend_from_slice(tcp);
    tcp[16..18].copy_from_slice(&checksum(&pseudo).to_be_bytes());
    bytes
}

fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    for pair in bytes.chunks(2) {
        sum += (u32::from(pair[0]) << 8) + u32::from(*pair.get(1).unwrap_or(&0));
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}
