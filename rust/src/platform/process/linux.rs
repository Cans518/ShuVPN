use super::{deadline, identity, normalize_address, Failure, Query, MAX_ROWS};
use crate::routing::model::{Network, ProcessIdentity};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Take},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    os::fd::AsRawFd,
    time::Instant,
};

fn read(path: &str) -> Result<Option<String>, Failure> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut input: Take<File> = file.take(16 * 1024 * 1024 + 1);
    let mut result = String::new();
    input.read_to_string(&mut result)?;
    if result.len() > 16 * 1024 * 1024 {
        return Err(Failure("套接字表超过大小预算".into()));
    }
    Ok(Some(result))
}

fn address(value: &str) -> Option<SocketAddr> {
    let (host, port) = value.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let ip = match host.len() {
        8 => IpAddr::V4(Ipv4Addr::from(
            u32::from_str_radix(host, 16).ok()?.to_ne_bytes(),
        )),
        32 => {
            let mut bytes = [0u8; 16];
            for (index, chunk) in host.as_bytes().chunks_exact(8).enumerate() {
                bytes[index * 4..index * 4 + 4].copy_from_slice(
                    &u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16)
                        .ok()?
                        .to_ne_bytes(),
                );
            }
            IpAddr::V6(Ipv6Addr::from(bytes))
        }
        _ => return None,
    };
    Some(normalize_address(SocketAddr::new(ip, port)))
}

fn inodes(query: Query, end: Instant) -> Result<BTreeSet<u64>, Failure> {
    let files: &[&str] = match (query.network, query.source.is_ipv6()) {
        (Network::Tcp, false) => &["tcp", "tcp6"],
        (Network::Tcp, true) => &["tcp6"],
        (Network::Udp, false) => &["udp", "udp6"],
        (Network::Udp, true) => &["udp6"],
    };
    let mut found = BTreeSet::new();
    let mut uncertain_family = false;
    for file in files {
        let Some(table) = read(&format!("/proc/net/{file}"))? else {
            continue;
        };
        for (index, line) in table.lines().skip(1).enumerate() {
            deadline(end)?;
            if index >= MAX_ROWS {
                return Err(Failure("套接字表超过条目预算".into()));
            }
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 10 {
                return Err(Failure("套接字表格式无法识别".into()));
            }
            let local = address(fields[1]).ok_or_else(|| Failure("套接字地址无法识别".into()))?;
            let remote = address(fields[2]).ok_or_else(|| Failure("套接字地址无法识别".into()))?;
            let matches = match query.network {
                Network::Tcp => local == query.source && remote == query.destination,
                Network::Udp => {
                    local.port() == query.source.port()
                        && (local.ip() == query.source.ip() || local.ip().is_unspecified())
                        && (remote == query.destination
                            || (remote.ip().is_unspecified() && remote.port() == 0))
                }
            };
            if matches {
                if query.source.is_ipv4() && local.is_ipv6() && local.ip().is_unspecified() {
                    // udp6 does not expose IPV6_V6ONLY. This endpoint might
                    // not be capable of producing IPv4 packets at all.
                    uncertain_family = true;
                    continue;
                }
                let inode = fields[9]
                    .parse()
                    .map_err(|_| Failure("套接字 inode 无效".into()))?;
                if inode != 0 {
                    found.insert(inode);
                }
            }
        }
    }
    if uncertain_family {
        return Err(Failure("IPv6 通配绑定无法确认 IPv4 UDP 归属".into()));
    }
    if found.len() != 1 {
        return Err(Failure("套接字归属不存在或有歧义".into()));
    }
    Ok(found)
}

pub(super) fn lookup(query: Query, end: Instant) -> Result<ProcessIdentity, Failure> {
    // proc socket tables carry no IPv6 zone ID.
    if [query.source.ip(), query.destination.ip()]
        .iter()
        .any(|ip| matches!(ip, IpAddr::V6(ip) if ip.is_unicast_link_local()))
    {
        return Err(Failure("当前进程查询无法区分 IPv6 链路本地区域".into()));
    }
    let wanted = inodes(query, end)?;
    let mut found = None;
    let mut files = 0;
    for (index, entry) in fs::read_dir("/proc")?.enumerate() {
        deadline(end)?;
        if index >= MAX_ROWS {
            return Err(Failure("进程列表超过数量预算".into()));
        }
        let entry = entry?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(directory) = File::open(entry.path()) else {
            continue;
        };
        // Pin the proc directory before inspecting fd/exe; PID reuse cannot
        // redirect these paths to a later process.
        let base = format!("/proc/self/fd/{}", directory.as_raw_fd());
        let Ok(descriptors) = fs::read_dir(format!("{base}/fd")) else {
            continue;
        };
        let mut owned = None;
        for descriptor in descriptors {
            deadline(end)?;
            files += 1;
            if files > MAX_ROWS {
                return Err(Failure("进程描述符扫描超过数量预算".into()));
            }
            let Ok(descriptor) = descriptor else {
                continue;
            };
            let Ok(link) = fs::read_link(descriptor.path()) else {
                continue;
            };
            let value = link
                .to_str()
                .and_then(|value| value.strip_prefix("socket:["))
                .and_then(|value| value.strip_suffix(']'))
                .and_then(|value| value.parse::<u64>().ok());
            if value.is_some_and(|inode| wanted.contains(&inode)) {
                owned = Some((descriptor.path(), link));
                break;
            }
        }
        let Some(owned) = owned else {
            continue;
        };
        if found.is_some() {
            return Err(Failure("套接字由多个可见进程共享，无法唯一归属".into()));
        }
        let image = fs::read_link(format!("{base}/exe"))?;
        let image_path = image.clone();
        let image = image
            .to_str()
            .ok_or_else(|| Failure("可执行文件路径不是 UTF-8".into()))?;
        if image.ends_with(" (deleted)") {
            return Err(Failure("可执行文件已删除，无法确认路径".into()));
        }
        let identity = identity(pid, image.to_owned(), "linux-proc-socket-snapshot")?;
        // A holder may move network namespaces while retaining the socket.
        found = Some((directory, owned, image_path, identity));
    }
    if inodes(query, end)? != wanted {
        return Err(Failure("查询期间套接字归属变化".into()));
    }
    let (directory, (descriptor, link), image, identity) = found.ok_or_else(|| {
        Failure("没有可见的本机套接字进程；可能是远端客户端、权限不足或进程已退出".into())
    })?;
    if fs::read_link(descriptor).ok().as_ref() != Some(&link)
        || fs::read_link(format!("/proc/self/fd/{}/exe", directory.as_raw_fd()))
            .ok()
            .as_ref()
            != Some(&image)
    {
        return Err(Failure("查询期间可执行文件或描述符归属变化".into()));
    }
    Ok(identity)
}
