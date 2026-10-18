use super::{deadline, identity, normalize_address, Failure, Query, MAX_ROWS};
use crate::routing::model::{Network, ProcessIdentity};
use std::{
    collections::BTreeSet,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NOT_SUPPORTED, FILETIME, HANDLE, WAIT_TIMEOUT,
    },
    NetworkManagement::IpHelper::*,
    Networking::WinSock::{AF_INET, AF_INET6},
    System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    },
};

struct Process(HANDLE);
impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns the process handle.
        unsafe { CloseHandle(self.0) };
    }
}

struct Table {
    words: Vec<u64>,
    bytes: usize,
}
impl Table {
    fn rows<T: Copy>(&self, offset: usize) -> Result<Vec<T>, Failure> {
        if self.bytes < 4 {
            return Err(Failure("套接字表头不完整".into()));
        }
        let pointer = self.words.as_ptr().cast::<u8>();
        // SAFETY: the checked buffer contains a u32 entry count.
        let count = unsafe { pointer.cast::<u32>().read_unaligned() } as usize;
        if count > MAX_ROWS
            || count
                .checked_mul(std::mem::size_of::<T>())
                .and_then(|length| offset.checked_add(length))
                .is_none_or(|length| length > self.bytes)
        {
            return Err(Failure("套接字表大小或条目数量无效".into()));
        }
        Ok((0..count)
            .map(|index| {
                // SAFETY: row bounds were checked; these MIB rows consist only of
                // integers and byte arrays, and unaligned reads avoid layout assumptions.
                unsafe {
                    pointer
                        .add(offset + index * std::mem::size_of::<T>())
                        .cast::<T>()
                        .read_unaligned()
                }
            })
            .collect())
    }
}

fn table(network: Network, ipv6: bool, end: Instant) -> Result<Option<Table>, Failure> {
    let family = if ipv6 { AF_INET6 } else { AF_INET } as u32;
    let mut size = 0;
    let call = |buffer, size: &mut u32| {
        // SAFETY: either a null size query or a writable size-checked buffer;
        // Windows receives only valid address-family/table-class constants.
        unsafe {
            match network {
                Network::Tcp => {
                    GetExtendedTcpTable(buffer, size, 0, family, TCP_TABLE_OWNER_PID_ALL, 0)
                }
                Network::Udp => {
                    GetExtendedUdpTable(buffer, size, 0, family, UDP_TABLE_OWNER_PID, 0)
                }
            }
        }
    };
    let first = call(std::ptr::null_mut(), &mut size);
    if first == ERROR_NOT_SUPPORTED && ipv6 {
        return Ok(None);
    }
    if first != ERROR_INSUFFICIENT_BUFFER && first != 0 {
        return Err(Failure(format!("套接字表查询失败：{first}")));
    }
    for _ in 0..3 {
        deadline(end)?;
        if !(4..=16 * 1024 * 1024).contains(&size) {
            return Err(Failure("套接字表超过大小预算".into()));
        }
        let mut words = vec![0u64; (size as usize).div_ceil(8)];
        let result = call(words.as_mut_ptr().cast(), &mut size);
        if result == ERROR_NOT_SUPPORTED && ipv6 {
            return Ok(None);
        }
        if result == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if result != 0 {
            return Err(Failure(format!("套接字表查询失败：{result}")));
        }
        if size as usize > words.len() * 8 {
            return Err(Failure("套接字表返回大小无效".into()));
        }
        return Ok(Some(Table {
            words,
            bytes: size as usize,
        }));
    }
    Err(Failure("套接字表持续变化".into()))
}

fn ipv4(ip: u32, port: u32) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::from(ip.to_ne_bytes()), u16::from_be(port as u16)))
}
fn ipv6(ip: [u8; 16], port: u32, scope: u32) -> SocketAddr {
    normalize_address(SocketAddr::V6(SocketAddrV6::new(
        Ipv6Addr::from(ip),
        u16::from_be(port as u16),
        0,
        scope,
    )))
}
fn udp_matches(local: SocketAddr, source: SocketAddr) -> bool {
    local.port() == source.port() && (local == source || local.ip().is_unspecified())
}

fn owner(query: Query, end: Instant) -> Result<u32, Failure> {
    let mut owners = BTreeSet::new();
    let mut uncertain = BTreeSet::new();
    for ipv6_table in [false, true] {
        if query.source.is_ipv6() && !ipv6_table {
            continue;
        }
        let Some(table) = table(query.network, ipv6_table, end)? else {
            continue;
        };
        match (query.network, ipv6_table) {
            (Network::Tcp, false) => {
                for row in table.rows::<MIB_TCPROW_OWNER_PID>(std::mem::offset_of!(
                    MIB_TCPTABLE_OWNER_PID,
                    table
                ))? {
                    if ipv4(row.dwLocalAddr, row.dwLocalPort) == query.source
                        && ipv4(row.dwRemoteAddr, row.dwRemotePort) == query.destination
                    {
                        owners.insert(row.dwOwningPid);
                    }
                }
            }
            (Network::Tcp, true) => {
                for row in table.rows::<MIB_TCP6ROW_OWNER_PID>(std::mem::offset_of!(
                    MIB_TCP6TABLE_OWNER_PID,
                    table
                ))? {
                    if ipv6(row.ucLocalAddr, row.dwLocalPort, row.dwLocalScopeId) == query.source
                        && ipv6(row.ucRemoteAddr, row.dwRemotePort, row.dwRemoteScopeId)
                            == query.destination
                    {
                        owners.insert(row.dwOwningPid);
                    }
                }
            }
            (Network::Udp, false) => {
                for row in table.rows::<MIB_UDPROW_OWNER_PID>(std::mem::offset_of!(
                    MIB_UDPTABLE_OWNER_PID,
                    table
                ))? {
                    if udp_matches(ipv4(row.dwLocalAddr, row.dwLocalPort), query.source) {
                        owners.insert(row.dwOwningPid);
                    }
                }
            }
            (Network::Udp, true) => {
                for row in table.rows::<MIB_UDP6ROW_OWNER_PID>(std::mem::offset_of!(
                    MIB_UDP6TABLE_OWNER_PID,
                    table
                ))? {
                    let local = ipv6(row.ucLocalAddr, row.dwLocalPort, row.dwLocalScopeId);
                    if udp_matches(local, query.source) {
                        if query.source.is_ipv4() && local.is_ipv6() && local.ip().is_unspecified()
                        {
                            uncertain.insert(row.dwOwningPid);
                        } else {
                            owners.insert(row.dwOwningPid);
                        }
                    }
                }
            }
        }
    }
    if !uncertain.is_empty()
        && (owners.len() != 1 || uncertain.iter().any(|pid| !owners.contains(pid)))
    {
        return Err(Failure("IPv6 通配绑定无法确认 IPv4 UDP 归属".into()));
    }
    if owners.len() != 1 || owners.contains(&0) {
        return Err(Failure("没有唯一的本机套接字进程".into()));
    }
    owners
        .into_iter()
        .next()
        .ok_or_else(|| Failure("套接字进程缺失".into()))
}

pub(super) fn lookup(query: Query, end: Instant) -> Result<ProcessIdentity, Failure> {
    let begun = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure("系统时钟不可用".into()))?
        .as_nanos()
        / 100
        + 116_444_736_000_000_000;
    let pid = owner(query, end)?;
    // SAFETY: non-inheritable read/wait handle, retained until all checks finish.
    let process = Process(unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    });
    if process.0.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: handle and all four writable FILETIME outputs are live.
    if unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let created = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    if u128::from(created) > begun {
        return Err(Failure("查询期间进程 PID 已复用".into()));
    }
    let mut image = vec![0u16; 32768];
    let mut length = image.len() as u32;
    // SAFETY: Win32-format image path, writable UTF-16 buffer measured in characters.
    if unsafe { QueryFullProcessImageNameW(process.0, 0, image.as_mut_ptr(), &mut length) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if length as usize >= image.len() {
        return Err(Failure("可执行文件路径过长".into()));
    }
    let image = String::from_utf16(&image[..length as usize])
        .map_err(|_| Failure("可执行文件路径不是有效 Unicode".into()))?;
    deadline(end)?;
    // SAFETY: the retained handle has SYNCHRONIZE rights; timeout zero never blocks.
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT || owner(query, end)? != pid {
        return Err(Failure("查询期间进程或套接字归属变化".into()));
    }
    identity(pid, image, "windows-ip-helper-socket-snapshot")
}
