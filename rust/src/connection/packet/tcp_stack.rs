use super::{codec::Tuple, tcp_codec};
use crate::connection::model::VpnFailure;
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    socket::tcp,
    time::{Duration, Instant},
    wire::{HardwareAddress, IpAddress, IpCidr},
};
use std::collections::VecDeque;
use std::net::IpAddr;

const PACKET_QUEUE: usize = 64;
pub(super) const BUFFER_SIZE: usize = 64 * 1024;

pub(super) struct Stack {
    interface: Interface,
    device: PacketDevice,
    sockets: SocketSet<'static>,
    handle: SocketHandle,
    started: std::time::Instant,
}

impl Stack {
    pub(super) fn new(tuple: Tuple, mtu: u16) -> Result<Self, VpnFailure> {
        let mut seed = [0; 8];
        getrandom::getrandom(&mut seed)
            .map_err(|_| VpnFailure::new("random", "TCP 序列种子生成失败"))?;
        let mut config = Config::new(HardwareAddress::Ip);
        config.random_seed = u64::from_ne_bytes(seed);
        let mut device = PacketDevice {
            incoming: VecDeque::new(),
            outgoing: VecDeque::new(),
            mtu,
        };
        let mut interface = Interface::new(config, &mut device, Instant::from_millis(0));
        let address = match tuple.destination.ip() {
            IpAddr::V4(address) => IpAddress::Ipv4(address),
            IpAddr::V6(address) => IpAddress::Ipv6(address),
        };
        interface.update_ip_addrs(|addresses| {
            // One virtual server per flow; IP medium needs no ARP or NDP.
            // /0 makes the actual client VIP reachable without an OS route.
            let _ = addresses.push(IpCidr::new(address, 0));
        });
        let mut socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; BUFFER_SIZE]),
            tcp::SocketBuffer::new(vec![0; BUFFER_SIZE]),
        );
        socket.set_ack_delay(None);
        socket.set_timeout(Some(Duration::from_secs(300)));
        socket
            .listen((address, tuple.destination.port()))
            .map_err(|_| VpnFailure::new("packet", "本地 TCP 监听状态无效"))?;
        let mut sockets = SocketSet::new(vec![]);
        let handle = sockets.add(socket);
        Ok(Self {
            interface,
            device,
            sockets,
            handle,
            started: std::time::Instant::now(),
        })
    }

    pub(super) fn accept(&mut self, packet: Vec<u8>) -> bool {
        let Ok((packet, fin)) = tcp_codec::separate_fin(packet) else {
            return false;
        };
        if self.device.incoming.len() + 1 + usize::from(fin.is_some()) > PACKET_QUEUE {
            return false;
        }
        self.device.incoming.push_back(packet);
        // smoltcp 0.14 can accept FIN after clipping a segment's payload at the
        // receive-window edge. A separate FIN retains its original sequence;
        // the stack then accepts it only after all preceding bytes arrive.
        if let Some(fin) = fin {
            self.device.incoming.push_back(fin);
        }
        true
    }

    pub(super) fn poll(&mut self) {
        let now =
            Instant::from_millis(self.started.elapsed().as_millis().min(i64::MAX as u128) as i64);
        self.interface
            .poll(now, &mut self.device, &mut self.sockets);
    }

    pub(super) fn socket(&mut self) -> &mut tcp::Socket<'static> {
        self.sockets.get_mut(self.handle)
    }

    pub(super) fn packet(&mut self) -> Option<Vec<u8>> {
        self.device.outgoing.pop_front()
    }
    pub(super) fn discard_output(&mut self) {
        self.device.outgoing.clear();
    }
}

struct PacketDevice {
    incoming: VecDeque<Vec<u8>>,
    outgoing: VecDeque<Vec<u8>>,
    mtu: u16,
}

struct Incoming(Vec<u8>);
struct Outgoing<'a>(&'a mut VecDeque<Vec<u8>>);
impl RxToken for Incoming {
    fn consume<R, F>(self, consume: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        consume(&self.0)
    }
}
impl TxToken for Outgoing<'_> {
    fn consume<R, F>(self, length: usize, consume: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut packet = vec![0; length];
        let result = consume(&mut packet);
        self.0.push_back(packet);
        result
    }
}
impl Device for PacketDevice {
    type RxToken<'a> = Incoming;
    type TxToken<'a> = Outgoing<'a>;
    fn receive(&mut self, _: Instant) -> Option<(Incoming, Outgoing<'_>)> {
        if self.outgoing.len() >= PACKET_QUEUE {
            return None;
        }
        let packet = self.incoming.pop_front()?;
        Some((Incoming(packet), Outgoing(&mut self.outgoing)))
    }
    fn transmit(&mut self, _: Instant) -> Option<Outgoing<'_>> {
        if self.outgoing.len() >= PACKET_QUEUE {
            return None;
        }
        Some(Outgoing(&mut self.outgoing))
    }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ip;
        capabilities.max_transmission_unit = usize::from(self.mtu);
        capabilities.max_burst_size = Some(PACKET_QUEUE);
        capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::packet::tcp_codec;

    #[test]
    fn a_fin_beyond_the_receive_window_cannot_discard_the_remaining_payload() -> anyhow::Result<()>
    {
        for (source, destination) in [
            ("10.99.0.2:53001", "10.21.0.5:443"),
            ("[2001:db8::2]:53001", "[2001:db8:21::5]:443"),
        ] {
            let tuple = Tuple {
                source: source.parse()?,
                destination: destination.parse()?,
            };
            let mut stack = Stack::new(tuple, 1400)?;
            let mut sequence = 1000;
            assert!(stack.accept(tcp_codec::build(
                tuple,
                sequence,
                0,
                tcp_codec::SYN,
                65535,
                &[],
                None,
            )?));
            stack.poll();
            let syn_ack = stack
                .packet()
                .ok_or_else(|| anyhow::anyhow!("missing SYN-ACK"))?;
            let acknowledgment = tcp_codec::parse(&syn_ack)?.sequence.wrapping_add(1);
            sequence += 1;
            assert!(stack.accept(tcp_codec::build(
                tuple,
                sequence,
                acknowledgment,
                tcp_codec::ACK,
                65535,
                &[],
                None,
            )?));
            stack.poll();
            while stack.packet().is_some() {}

            let prefix = vec![7; BUFFER_SIZE - 16];
            for chunk in prefix.chunks(1024) {
                assert!(stack.accept(tcp_codec::build(
                    tuple,
                    sequence,
                    acknowledgment,
                    tcp_codec::ACK,
                    65535,
                    chunk,
                    None,
                )?));
                sequence = sequence.wrapping_add(chunk.len() as u32);
                stack.poll();
                while stack.packet().is_some() {}
            }
            assert_eq!(stack.socket().recv_queue(), BUFFER_SIZE - 16);
            let suffix = [9; 32];
            assert!(stack.accept(tcp_codec::build(
                tuple,
                sequence,
                acknowledgment,
                tcp_codec::ACK | tcp_codec::FIN,
                65535,
                &suffix,
                None,
            )?));
            stack.poll();
            assert_eq!(
                stack.socket().state(),
                tcp::State::Established,
                "a clipped payload accepted its out-of-window FIN"
            );
            let mut received = vec![0; BUFFER_SIZE];
            assert_eq!(stack.socket().recv_slice(&mut received)?, BUFFER_SIZE);
            assert_eq!(&received[..prefix.len()], prefix);
            assert_eq!(&received[prefix.len()..], &suffix[..16]);
            stack.poll();
            while stack.packet().is_some() {}
            assert!(stack.accept(tcp_codec::build(
                tuple,
                sequence.wrapping_add(16),
                acknowledgment,
                tcp_codec::ACK | tcp_codec::FIN,
                65535,
                &suffix[16..],
                None,
            )?));
            stack.poll();
            let mut remaining = [0; 16];
            assert_eq!(stack.socket().recv_slice(&mut remaining)?, 16);
            assert_eq!(remaining, suffix[16..]);
            assert!(
                !stack.socket().may_recv(),
                "the final FIN was not delivered"
            );
        }
        Ok(())
    }

    #[test]
    fn blocked_packet_egress_bounds_retransmissions_while_dial_is_pending() -> anyhow::Result<()> {
        let tuple = Tuple {
            source: "10.99.0.2:53000".parse()?,
            destination: "10.21.0.5:443".parse()?,
        };
        let packet = tcp_codec::build(tuple, 1000, 0, tcp_codec::SYN, 32768, &[], None)?;
        let mut stack = Stack::new(tuple, 1400)?;
        // Simulate held SYN-ACK output during an unfinished authorization.
        for _ in 0..PACKET_QUEUE {
            stack.device.outgoing.push_back(vec![0; 40]);
        }
        let mut dropped = 0;
        for _ in 0..10_000 {
            if !stack.accept(packet.clone()) {
                dropped += 1;
            }
            stack.poll();
        }
        assert!(dropped > 0);
        assert_eq!(stack.device.incoming.len(), PACKET_QUEUE);
        assert_eq!(stack.device.outgoing.len(), PACKET_QUEUE);
        Ok(())
    }
}
