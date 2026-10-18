//! Windows interface identity and per-socket unicast egress.
use crate::connection::model::VpnFailure;
use socket2::Socket;
use std::net::IpAddr;
use std::os::windows::io::AsRawSocket;
use windows_sys::Win32::{
    Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR},
    NetworkManagement::{
        IpHelper::*,
        Ndis::{IfOperStatusUp, NET_LUID_LH},
    },
    Networking::WinSock::*,
};

struct MibTable<T>(*mut T);
impl<T> Drop for MibTable<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: only tables allocated by IP Helper are stored here.
            unsafe { FreeMibTable(self.0.cast()) };
        }
    }
}

pub(super) fn capture_snapshot() -> Result<super::interface::CaptureSnapshot, VpnFailure> {
    let mut table = MibTable(std::ptr::null_mut::<MIB_IPFORWARD_TABLE2>());
    // SAFETY: IP Helper owns the output allocation guarded above.
    check(unsafe { GetIpForwardTable2(AF_UNSPEC, &mut table.0) })?;
    if table.0.is_null() {
        return Err(failure("系统没有返回路由表"));
    }
    // SAFETY: count is in the live IP Helper allocation.
    let count = unsafe { (*table.0).NumEntries } as usize;
    if count > 100_000 {
        return Err(failure("系统路由表超过预算"));
    }
    // SAFETY: this allocation contains count contiguous rows.
    let rows = unsafe { std::slice::from_raw_parts((*table.0).Table.as_ptr(), count) };
    let mut networks = Vec::new();
    let mut route_interfaces = Vec::new();
    for row in rows {
        if row.ValidLifetime == 0 {
            continue;
        }
        // SAFETY: the family selects the initialized union member.
        let address = match unsafe { row.DestinationPrefix.Prefix.si_family } {
            AF_INET => std::net::IpAddr::V4(std::net::Ipv4Addr::from(
                unsafe { row.DestinationPrefix.Prefix.Ipv4.sin_addr.S_un.S_addr }.to_ne_bytes(),
            )),
            AF_INET6 => std::net::IpAddr::V6(std::net::Ipv6Addr::from(unsafe {
                row.DestinationPrefix.Prefix.Ipv6.sin6_addr.u.Byte
            })),
            _ => continue,
        };
        let network = ipnet::IpNet::new(address, row.DestinationPrefix.PrefixLength)
            .map_err(|cause| failure(&format!("系统路由前缀无效：{cause}")))?
            .trunc();
        networks.push(network);
        route_interfaces.push((network, row.InterfaceIndex));
        if networks.len() > 4096 {
            return Err(failure("完整捕获所需系统路由超过预算"));
        }
    }
    let mut addresses = MibTable(std::ptr::null_mut::<MIB_UNICASTIPADDRESS_TABLE>());
    // SAFETY: live output pointer and an RAII-guarded IP Helper allocation.
    check(unsafe { GetUnicastIpAddressTable(AF_UNSPEC, &mut addresses.0) })?;
    if addresses.0.is_null() {
        return Err(failure("系统没有返回本机地址表"));
    }
    // SAFETY: count and contiguous rows belong to this live allocation.
    let count = unsafe { (*addresses.0).NumEntries } as usize;
    if count > 100_000 {
        return Err(failure("本机地址表超过预算"));
    }
    let rows = unsafe { std::slice::from_raw_parts((*addresses.0).Table.as_ptr(), count) };
    let mut local_addresses = Vec::new();
    let mut source_addresses = Vec::new();
    for row in rows {
        // Include deprecated/tentative local addresses too: they must not be
        // advertised as externally captured destinations.
        let address = match unsafe { row.Address.si_family } {
            AF_INET => IpAddr::V4(std::net::Ipv4Addr::from(
                unsafe { row.Address.Ipv4.sin_addr.S_un.S_addr }.to_ne_bytes(),
            )),
            AF_INET6 => IpAddr::V6(std::net::Ipv6Addr::from(unsafe {
                row.Address.Ipv6.sin6_addr.u.Byte
            })),
            _ => continue,
        };
        local_addresses.push(address);
        source_addresses.push(address);
        if let IpAddr::V4(ip) = address {
            if row.OnLinkPrefixLength < 31 {
                let network = ipnet::Ipv4Net::new(ip, row.OnLinkPrefixLength)
                    .map_err(|cause| failure(&format!("本机地址前缀无效：{cause}")))?;
                local_addresses.extend([
                    IpAddr::V4(network.network()),
                    IpAddr::V4(network.broadcast()),
                ]);
            }
        }
    }
    Ok(super::interface::CaptureSnapshot {
        routes: networks,
        local_addresses,
        source_addresses,
        route_interfaces,
    })
}

pub(super) fn lan_addresses() -> Result<Vec<(u32, IpAddr)>, VpnFailure> {
    let mut table = MibTable(std::ptr::null_mut::<MIB_UNICASTIPADDRESS_TABLE>());
    // SAFETY: the output pointer is writable; the returned allocation is guarded.
    check(unsafe { GetUnicastIpAddressTable(AF_UNSPEC, &mut table.0) })?;
    if table.0.is_null() {
        return Err(failure("系统没有返回单播地址表"));
    }
    // SAFETY: successful IP Helper query returned a live table.
    let count = unsafe { (*table.0).NumEntries } as usize;
    if count > 100_000 {
        return Err(failure("系统单播地址表超过预算"));
    }
    // SAFETY: the guarded allocation owns count contiguous rows.
    let rows = unsafe { std::slice::from_raw_parts((*table.0).Table.as_ptr(), count) };
    let mut addresses = Vec::new();
    for row in rows {
        if row.DadState != IpDadStatePreferred || row.PreferredLifetime == 0 {
            continue;
        }
        // SAFETY: family is the common initial member; it selects the union below.
        let address = match unsafe { row.Address.si_family } {
            AF_INET => IpAddr::V4(std::net::Ipv4Addr::from(
                unsafe { row.Address.Ipv4.sin_addr.S_un.S_addr }.to_ne_bytes(),
            )),
            AF_INET6 => {
                // SAFETY: AF_INET6 identifies the initialized IPv6 member.
                let address = unsafe { row.Address.Ipv6 };
                if unsafe { address.Anonymous.sin6_scope_id } != 0 {
                    continue;
                }
                IpAddr::V6(std::net::Ipv6Addr::from(unsafe {
                    address.sin6_addr.u.Byte
                }))
            }
            _ => continue,
        };
        addresses.push((row.InterfaceIndex, address));
    }
    Ok(addresses)
}

pub(super) fn available() -> Result<Vec<super::interface::NetworkInterface>, VpnFailure> {
    let mut table = MibTable(std::ptr::null_mut::<MIB_IF_TABLE2>());
    // SAFETY: the output pointer is writable and its allocation is guarded.
    check(unsafe { GetIfTable2(&mut table.0) })?;
    if table.0.is_null() {
        return Err(failure("系统没有返回接口表"));
    }
    // SAFETY: the successful query returned a live table.
    let count = unsafe { (*table.0).NumEntries } as usize;
    if count > 100_000 {
        return Err(failure("系统接口表超过预算"));
    }
    // SAFETY: the table owns count contiguous rows until its guard is dropped.
    let rows = unsafe { std::slice::from_raw_parts((*table.0).Table.as_ptr(), count) };
    let mut choices = vec![];
    for row in rows {
        if row.Type == IF_TYPE_SOFTWARE_LOOPBACK || validate_row(row).is_err() {
            continue;
        }
        let decode = |units: &[u16]| {
            let length = units
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(units.len());
            String::from_utf16(&units[..length]).map_err(|_| failure("网络接口名称编码无效"))
        };
        choices.push(super::interface::NetworkInterface {
            name: decode(&row.Alias)?,
            description: decode(&row.Description)?,
            index: row.InterfaceIndex,
            recommended: false,
        });
    }
    let mut routes = MibTable(std::ptr::null_mut::<MIB_IPFORWARD_TABLE2>());
    // SAFETY: the output pointer is writable and its allocation is guarded.
    check(unsafe { GetIpForwardTable2(AF_UNSPEC, &mut routes.0) })?;
    if routes.0.is_null() {
        return Err(failure("系统没有返回路由表"));
    }
    // SAFETY: the successful query returned a live table.
    let count = unsafe { (*routes.0).NumEntries } as usize;
    if count > 100_000 {
        return Err(failure("系统路由表超过预算"));
    }
    // SAFETY: the table owns count contiguous rows until its guard is dropped.
    let rows = unsafe { std::slice::from_raw_parts((*routes.0).Table.as_ptr(), count) };
    let mut defaults = vec![];
    for row in rows {
        if row.DestinationPrefix.PrefixLength != 0 || row.ValidLifetime == 0 {
            continue;
        }
        // SAFETY: ADDRESS_FAMILY is the common initial member of SOCKADDR_INET.
        let family = unsafe { row.DestinationPrefix.Prefix.si_family };
        if ![AF_INET, AF_INET6].contains(&family) {
            continue;
        }
        let mut interface = MIB_IPINTERFACE_ROW {
            Family: family,
            InterfaceIndex: row.InterfaceIndex,
            ..Default::default()
        };
        // SAFETY: the initialized selector and writable result remain live.
        if unsafe { GetIpInterfaceEntry(&mut interface) } != NO_ERROR {
            continue;
        }
        defaults.push(super::interface::DefaultRoute {
            ipv4: family == AF_INET,
            index: row.InterfaceIndex,
            metric: u64::from(row.Metric) + u64::from(interface.Metric),
        });
    }
    super::interface::recommend(&mut choices, defaults)?;
    Ok(choices)
}

#[derive(Clone)]
pub(super) struct Interface {
    luid: u64,
    pub index: u32,
}

impl Interface {
    pub(super) fn resolve(alias: &str) -> Result<Self, VpnFailure> {
        if alias.is_empty() || alias.encode_utf16().count() > 256 || alias.contains('\0') {
            return Err(failure("物理接口名称无效"));
        }
        let alias: Vec<u16> = alias.encode_utf16().chain(Some(0)).collect();
        let mut luid = NET_LUID_LH::default();
        check(unsafe { ConvertInterfaceAliasToLuid(alias.as_ptr(), &mut luid) })?;
        Self::from_luid(luid)
    }

    fn from_luid(luid: NET_LUID_LH) -> Result<Self, VpnFailure> {
        let mut row = MIB_IF_ROW2 {
            InterfaceLuid: luid,
            ..Default::default()
        };
        check(unsafe { GetIfEntry2(&mut row) })?;
        validate_row(&row)?;
        Ok(Self {
            luid: unsafe { luid.Value },
            index: row.InterfaceIndex,
        })
    }

    pub(super) fn check_identity(&self) -> Result<(), VpnFailure> {
        let mut row = MIB_IF_ROW2 {
            InterfaceIndex: self.index,
            ..Default::default()
        };
        check(unsafe { GetIfEntry2(&mut row) })?;
        if unsafe { row.InterfaceLuid.Value } != self.luid {
            return Err(failure("物理接口身份已改变"));
        }
        validate_row(&row)
    }

    pub(super) fn loopback() -> Result<Self, VpnFailure> {
        let mut table = std::ptr::null_mut();
        check(unsafe { GetIfTable2(&mut table) })?;
        if table.is_null() {
            return Err(failure("系统没有返回接口表"));
        }
        let _table = MibTable(table);
        let count = unsafe { (*table).NumEntries } as usize;
        if count > 100_000 {
            return Err(failure("系统接口表超过预算"));
        }
        // SAFETY: GetIfTable2 owns a contiguous array until _table is dropped.
        let rows = unsafe { std::slice::from_raw_parts((*table).Table.as_ptr(), count) };
        let row = rows
            .iter()
            .find(|row| row.Type == IF_TYPE_SOFTWARE_LOOPBACK)
            .ok_or_else(|| failure("系统没有可用回环接口"))?;
        Self::from_luid(row.InterfaceLuid)
    }

    pub(super) fn dns_servers(&self) -> Result<Vec<IpAddr>, VpnFailure> {
        self.check_identity()?;
        let mut size = 15 * 1024u32;
        for _ in 0..3 {
            if size as usize > 4 * 1024 * 1024 {
                return Err(failure("系统 DNS 接口表超过预算"));
            }
            // u64 storage provides the alignment required by the native linked structs.
            let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
            let adapters = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            let code = unsafe {
                GetAdaptersAddresses(
                    AF_UNSPEC as u32,
                    GAA_FLAG_SKIP_UNICAST | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST,
                    std::ptr::null(),
                    adapters,
                    &mut size,
                )
            };
            if code == ERROR_BUFFER_OVERFLOW {
                continue;
            }
            check(code)?;
            let mut adapter = adapters;
            let mut visited = 0;
            while !adapter.is_null() {
                visited += 1;
                if visited > 100_000 {
                    return Err(failure("系统 DNS 接口表超过预算"));
                }
                // SAFETY: successful GetAdaptersAddresses initialized this linked list.
                let entry = unsafe { &*adapter };
                if unsafe { entry.Luid.Value } == self.luid {
                    let mut dns = entry.FirstDnsServerAddress;
                    let mut addresses = vec![];
                    let mut visited_dns = 0;
                    while !dns.is_null() {
                        visited_dns += 1;
                        if visited_dns > 64 || addresses.len() >= 16 {
                            return Err(failure("物理 DNS 数量超过预算"));
                        }
                        let entry = unsafe { &*dns };
                        if let Some(address) = dns_address(&entry.Address)? {
                            if !addresses.contains(&address) {
                                addresses.push(address);
                            }
                        }
                        dns = entry.Next;
                    }
                    return Ok(addresses);
                }
                adapter = entry.Next;
            }
            return Err(failure("物理接口的 DNS 配置不存在"));
        }
        Err(failure("系统 DNS 接口表持续变化"))
    }
}

fn validate_row(row: &MIB_IF_ROW2) -> Result<(), VpnFailure> {
    let physical = matches!(row.Type, IF_TYPE_ETHERNET_CSMACD | IF_TYPE_IEEE80211)
        && row.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
    if row.InterfaceIndex == 0
        || unsafe { row.InterfaceLuid.Value } == 0
        || row.OperStatus != IfOperStatusUp
        || !(physical || row.Type == IF_TYPE_SOFTWARE_LOOPBACK)
    {
        return Err(failure(
            "出口须为启用的物理以太网、无线网或回环接口，不能绑定 TUN",
        ));
    }
    Ok(())
}

fn dns_address(address: &SOCKET_ADDRESS) -> Result<Option<IpAddr>, VpnFailure> {
    if address.lpSockaddr.is_null() || address.iSockaddrLength < std::mem::size_of::<u16>() as i32 {
        return Err(failure("系统 DNS 地址为空"));
    }
    let family = unsafe { (*address.lpSockaddr).sa_family };
    let ip = match family {
        AF_INET if address.iSockaddrLength as usize >= std::mem::size_of::<SOCKADDR_IN>() => {
            let row = unsafe { &*address.lpSockaddr.cast::<SOCKADDR_IN>() };
            IpAddr::V4(std::net::Ipv4Addr::from(
                unsafe { row.sin_addr.S_un.S_addr }.to_ne_bytes(),
            ))
        }
        AF_INET6 if address.iSockaddrLength as usize >= std::mem::size_of::<SOCKADDR_IN6>() => {
            let row = unsafe { &*address.lpSockaddr.cast::<SOCKADDR_IN6>() };
            let ip = std::net::Ipv6Addr::from(unsafe { row.sin6_addr.u.Byte });
            if unsafe { row.Anonymous.sin6_scope_id } != 0 || ip.is_unicast_link_local() {
                return Err(failure("带作用域的物理 DNS 尚未接入"));
            }
            ip.to_ipv4_mapped()
                .map(IpAddr::V4)
                .unwrap_or(IpAddr::V6(ip))
        }
        _ => return Err(failure("系统 DNS 地址族或长度无效")),
    };
    // Ignore stubs rather than selecting another adapter's DNS.
    Ok((!ip.is_loopback() && !ip.is_unspecified() && !ip.is_multicast()).then_some(ip))
}

pub(super) fn selected(
    interface: &str,
    peer: IpAddr,
    egress: Option<&super::Egress>,
) -> Result<Interface, VpnFailure> {
    let interface = if peer.is_loopback() {
        Interface::loopback()?
    } else if let Some(egress) = egress {
        egress.adapter.clone()
    } else {
        Interface::resolve(interface)?
    };
    interface.check_identity()?;
    Ok(interface)
}

pub(super) fn set_index(socket: &Socket, peer: IpAddr, index: u32) -> std::io::Result<()> {
    let (level, option, value) = if peer.is_ipv4() {
        (IPPROTO_IP, IP_UNICAST_IF, index.to_be())
    } else {
        (IPPROTO_IPV6, IPV6_UNICAST_IF, index)
    };
    // SAFETY: socket is live; option takes an initialized DWORD of this length.
    let code = unsafe {
        setsockopt(
            socket.as_raw_socket() as SOCKET,
            level,
            option,
            (&value as *const u32).cast(),
            std::mem::size_of::<u32>() as i32,
        )
    };
    socket_check(code)
}

pub(super) fn index(socket: &Socket, peer: IpAddr) -> std::io::Result<u32> {
    let (level, option) = if peer.is_ipv4() {
        (IPPROTO_IP, IP_UNICAST_IF)
    } else {
        (IPPROTO_IPV6, IPV6_UNICAST_IF)
    };
    let mut value = 0u32;
    let mut length = std::mem::size_of::<u32>() as i32;
    // Windows returns the interface index in host byte order for both families.
    let code = unsafe {
        getsockopt(
            socket.as_raw_socket() as SOCKET,
            level,
            option,
            (&mut value as *mut u32).cast(),
            &mut length,
        )
    };
    socket_check(code)?;
    if length != std::mem::size_of::<u32>() as i32 {
        return Err(std::io::Error::other("接口索引大小无效"));
    }
    Ok(value)
}

fn socket_check(code: i32) -> std::io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(unsafe {
            WSAGetLastError()
        }))
    }
}
fn check(code: u32) -> Result<(), VpnFailure> {
    if code == NO_ERROR {
        Ok(())
    } else {
        Err(failure(&format!(
            "Windows 物理接口查询失败：{}",
            std::io::Error::from_raw_os_error(code as i32)
        )))
    }
}
fn failure(message: &str) -> VpnFailure {
    VpnFailure::new("interface", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn actual_socket_options_preserve_family_byte_order_and_restore_the_original_binding(
    ) -> anyhow::Result<()> {
        let adapter = Interface::loopback()?;
        adapter.check_identity()?;
        let mut changed = adapter.clone();
        changed.luid ^= 1;
        assert!(changed.check_identity().is_err());
        for peer in ["127.0.0.1", "::1"] {
            let peer: IpAddr = peer.parse()?;
            let socket = Socket::new(
                if peer.is_ipv4() {
                    socket2::Domain::IPV4
                } else {
                    socket2::Domain::IPV6
                },
                socket2::Type::STREAM,
                Some(socket2::Protocol::TCP),
            )?;
            let original = index(&socket, peer)?;
            let record = super::super::Record {
                socket,
                peer,
                previous: std::sync::Mutex::new(None),
            };
            let egress = super::super::Egress {
                interface: "test".to_owned(),
                resolvers: vec![],
                epoch: 9,
                retired: std::sync::atomic::AtomicBool::new(false),
                adapter: adapter.clone(),
            };
            record.protect(&egress).await?;
            assert_eq!(index(&record.socket, peer)?, adapter.index);
            record.restore(10)?;
            assert_eq!(index(&record.socket, peer)?, adapter.index);
            record.restore(9)?;
            assert_eq!(index(&record.socket, peer)?, original);
            record.restore(9)?;
            assert_eq!(index(&record.socket, peer)?, original);
        }
        Ok(())
    }

    #[test]
    fn virtual_tunnel_down_and_unidentified_interfaces_cannot_be_physical_egress() {
        let mut row = MIB_IF_ROW2 {
            InterfaceLuid: NET_LUID_LH { Value: 10 },
            InterfaceIndex: 2,
            Type: IF_TYPE_IEEE80211,
            OperStatus: IfOperStatusUp,
            InterfaceAndOperStatusFlags: MIB_IF_ROW2_0 { _bitfield: 1 },
            ..Default::default()
        };
        assert!(validate_row(&row).is_ok());
        row.InterfaceAndOperStatusFlags._bitfield = 0;
        assert!(validate_row(&row).is_err());
        row.InterfaceAndOperStatusFlags._bitfield = 1;
        row.Type = IF_TYPE_TUNNEL;
        assert!(validate_row(&row).is_err());
        row.Type = IF_TYPE_IEEE80211;
        row.OperStatus = windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusDown;
        assert!(validate_row(&row).is_err());
        assert!(Interface::resolve("missing-shuvpn-interface").is_err());
        assert!(Interface::resolve("bad\0interface").is_err());
    }

    #[tokio::test]
    async fn retired_egress_cannot_rebind_a_record_after_cleanup() -> anyhow::Result<()> {
        let peer: IpAddr = "127.0.0.1".parse()?;
        let record = super::super::Record {
            socket: Socket::new(
                socket2::Domain::IPV4,
                socket2::Type::STREAM,
                Some(socket2::Protocol::TCP),
            )?,
            peer,
            previous: std::sync::Mutex::new(None),
        };
        let original = index(&record.socket, peer)?;
        let egress = super::super::Egress {
            interface: "test".to_owned(),
            resolvers: vec![],
            epoch: 10,
            retired: std::sync::atomic::AtomicBool::new(true),
            adapter: Interface::loopback()?,
        };
        assert_eq!(
            record.protect(&egress).await.err().map(|error| error.code),
            Some("closed".into())
        );
        assert_eq!(index(&record.socket, peer)?, original);
        assert!(record
            .previous
            .lock()
            .map_err(|_| anyhow::anyhow!("previous lock"))?
            .is_none());
        Ok(())
    }
}
