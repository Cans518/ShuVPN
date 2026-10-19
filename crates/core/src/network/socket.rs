use super::{Egress, Record};
use crate::connection::model::VpnFailure;
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpSocket, TcpStream, UdpSocket},
};

pub(crate) struct Tcp {
    inner: TcpStream,
    _record: Arc<Record>,
}
impl Tcp {
    #[cfg(all(target_os = "linux", feature = "egress-probe"))]
    pub(crate) fn probe_binding(&self) -> std::io::Result<Option<Vec<u8>>> {
        SockRef::from(&self.inner).device()
    }

    pub(crate) fn set_nodelay(&self, enabled: bool) -> std::io::Result<()> {
        self.inner.set_nodelay(enabled)
    }
    pub(crate) fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.peer_addr()
    }
    pub(crate) fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}
impl AsyncRead for Tcp {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buffer)
    }
}
impl AsyncWrite for Tcp {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buffer)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
pub(crate) struct Udp {
    inner: UdpSocket,
    _record: Arc<Record>,
}
impl Udp {
    pub(crate) async fn send(&self, bytes: &[u8]) -> std::io::Result<usize> {
        self.inner.send(bytes).await
    }
    pub(crate) async fn recv(&self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.inner.recv(bytes).await
    }
}

fn selected<'a>(
    egress: Option<&'a Egress>,
    interface: Option<&'a str>,
    peer: IpAddr,
) -> Result<Option<&'a str>, VpnFailure> {
    super::interface::validate(interface)?;
    #[cfg(target_os = "windows")]
    if (egress.is_some() || interface.is_some())
        && (peer.is_multicast() || matches!(peer, IpAddr::V4(address) if address.is_broadcast()))
    {
        return Err(VpnFailure::new(
            "protocol",
            "Windows 当前物理出口仅验证单播，不能将组播/广播作为已保护流发送",
        ));
    }
    if let Some(egress) = egress {
        if egress.retired.load(std::sync::atomic::Ordering::Acquire) {
            return Err(VpnFailure::new("closed", "TUN 物理出口保护已关闭"));
        }
        if interface.is_some_and(|interface| interface != egress.interface) {
            return Err(VpnFailure::new(
                "protected",
                "指定出口与当前 TUN 物理出口不一致",
            ));
        }
        return Ok(Some(egress.interface_for(peer)));
    }
    Ok(interface)
}
pub(super) async fn connect_with(
    egress: Option<&Egress>,
    address: SocketAddr,
    interface: Option<&str>,
) -> Result<Tcp, VpnFailure> {
    #[cfg(target_os = "windows")]
    let address = canonical_address(address)?;
    let original = interface;
    let interface = selected(egress, interface, address.ip())?;
    #[cfg(target_os = "linux")]
    let _operation = egress.map(Egress::operation).transpose()?;
    let socket = if address.is_ipv4() {
        TcpSocket::new_v4()?
    } else {
        TcpSocket::new_v6()?
    };

    #[cfg(target_os = "linux")]
    if let Some(interface) = interface {
        let socket = SockRef::from(&socket);
        match egress {
            Some(egress) => egress.bind(&socket, Some(interface.as_bytes())).await?,
            None => socket.bind_device(Some(interface.as_bytes()))?,
        }
    }
    #[cfg(target_os = "windows")]
    if let Some(interface) = interface {
        let adapter = super::windows::selected(interface, address.ip(), egress)?;
        super::windows::set_index(&SockRef::from(&socket), address.ip(), adapter.index)?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = interface;
    #[cfg(not(target_os = "linux"))]
    let stream = socket.connect(address).await?;
    #[cfg(target_os = "linux")]
    let stream = if let Some(egress) = egress {
        tokio::select! {
            biased;
            _ = egress.cancellation.cancelled() => return Err(VpnFailure::new("closed", "物理连接已取消")),
            result = socket.connect(address) => result?,
        }
    } else {
        socket.connect(address).await?
    };

    let record = super::register(
        SockRef::from(&stream).try_clone()?,
        address.ip(),
        original,
        egress,
    )
    .await?;
    Ok(Tcp {
        inner: stream,
        _record: record,
    })
}
pub(crate) async fn connect_address(
    address: SocketAddr,
    interface: Option<&str>,
) -> Result<Tcp, VpnFailure> {
    let _gate = super::read_gate().await;
    let egress = super::current()?;
    connect_with(egress.as_deref(), address, interface).await
}
pub(crate) async fn connect_host(host: &str, port: u16) -> Result<Tcp, VpnFailure> {
    let _gate = super::read_gate().await;
    let egress = super::current()?;
    let addresses =
        super::dns::resolve_with(egress.as_deref(), host, Some(Arc::clone(&_gate))).await?;
    let mut last = None;
    for address in addresses {
        match connect_with(egress.as_deref(), SocketAddr::new(address, port), None).await {
            Ok(socket) => return Ok(socket),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| VpnFailure::new("dns", "物理解析没有返回地址")))
}

pub(super) async fn udp_with(
    egress: Option<&Egress>,
    address: SocketAddr,
    interface: Option<&str>,
) -> Result<Udp, VpnFailure> {
    #[cfg(target_os = "windows")]
    let address = canonical_address(address)?;
    let original = interface;
    let interface = selected(egress, interface, address.ip())?;
    #[cfg(target_os = "linux")]
    let _operation = egress.map(Egress::operation).transpose()?;
    let socket = Socket::new(
        if address.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        },
        Type::DGRAM,
        Some(Protocol::UDP),
    )?;

    #[cfg(target_os = "linux")]
    if let Some(interface) = interface {
        match egress {
            Some(egress) => egress.bind(&socket, Some(interface.as_bytes())).await?,
            None => socket.bind_device(Some(interface.as_bytes()))?,
        }
    }
    #[cfg(target_os = "windows")]
    if let Some(interface) = interface {
        let adapter = super::windows::selected(interface, address.ip(), egress)?;
        super::windows::set_index(&socket, address.ip(), adapter.index)?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = interface;
    let any = if address.is_ipv4() {
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))
    } else {
        socket.set_only_v6(true)?;
        SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))
    };
    socket.bind(&any.into())?;
    socket.connect(&address.into())?;
    socket.set_nonblocking(true)?;
    let inner = UdpSocket::from_std(socket.into())?;
    let record = super::register(
        SockRef::from(&inner).try_clone()?,
        address.ip(),
        original,
        egress,
    )
    .await?;
    Ok(Udp {
        inner,
        _record: record,
    })
}
pub(crate) async fn udp_socket(
    address: SocketAddr,
    interface: Option<&str>,
) -> Result<Udp, VpnFailure> {
    let _gate = super::read_gate().await;
    udp_with(super::current()?.as_deref(), address, interface).await
}

#[cfg(target_os = "windows")]
fn canonical_address(address: SocketAddr) -> Result<SocketAddr, VpnFailure> {
    if let SocketAddr::V6(address6) = address {
        if let Some(address4) = address6.ip().to_ipv4_mapped() {
            if address6.scope_id() != 0 {
                return Err(VpnFailure::new("address", "IPv4 映射地址不能带作用域"));
            }
            // A mapped destination uses IPv4 routing; never protect only the
            // IPv6 option while leaving its IPv4 transport unbound.
            return Ok(SocketAddr::new(IpAddr::V4(address4), address6.port()));
        }
    }
    Ok(address)
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn mapped_ipv4_destinations_use_the_actual_protected_ipv4_socket() -> anyhow::Result<()> {
        let adapter = super::super::windows::Interface::loopback()?;
        let index = adapter.index;
        let egress = Egress {
            interface: "test".to_owned(),
            resolvers: vec![],
            epoch: 0,
            retired: std::sync::atomic::AtomicBool::new(false),
            adapter,
        };
        for peer in ["224.0.0.1:53", "[ff02::1]:53", "255.255.255.255:53"] {
            assert_eq!(
                udp_with(Some(&egress), peer.parse()?, None)
                    .await
                    .err()
                    .map(|error| error.code),
                Some("protocol".to_owned())
            );
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let peer = SocketAddr::new("::ffff:127.0.0.1".parse()?, port);
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            socket.write_all(b"real-ipv4").await
        });
        let mut tcp = connect_with(Some(&egress), peer, None).await?;
        assert!(tcp.peer_addr()?.is_ipv4());
        assert_eq!(
            super::super::windows::index(&SockRef::from(&tcp.inner), tcp.peer_addr()?.ip())?,
            index
        );
        let mut bytes = [0; 9];
        tcp.read_exact(&mut bytes).await?;
        assert_eq!(&bytes, b"real-ipv4");
        server.await??;
        let server = UdpSocket::bind("127.0.0.1:0").await?;
        let peer = SocketAddr::new("::ffff:127.0.0.1".parse()?, server.local_addr()?.port());
        let udp = udp_with(Some(&egress), peer, None).await?;
        assert!(udp.inner.peer_addr()?.is_ipv4());
        assert_eq!(
            super::super::windows::index(&SockRef::from(&udp.inner), udp.inner.peer_addr()?.ip())?,
            index
        );
        udp.send(b"ipv4").await?;
        let mut bytes = [0; 4];
        server.recv_from(&mut bytes).await?;
        assert_eq!(&bytes, b"ipv4");
        Ok(())
    }
}
