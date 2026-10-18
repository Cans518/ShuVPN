use super::model::{PlatformFailure, TunInfo, ValidatedTunPlan};
use futures_util::TryStreamExt;
use netlink_packet_route::{
    address::{AddressAttribute, AddressFlags},
    route::{RouteAttribute, RouteMessage, RouteScope},
};
use rtnetlink::{LinkMessageBuilder, LinkUnspec, RouteMessageBuilder};
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::unix::AsyncFd;
use tokio_util::sync::CancellationToken;

pub(crate) mod helper;
pub(crate) mod managed;

pub(crate) struct Device {
    io: super::packet_fd::PacketFd,
    info: TunInfo,
    protected: Vec<IpAddr>,
    dynamic: tokio::sync::Mutex<BTreeSet<IpAddr>>,
}

struct ConnectionTask(tokio::task::JoinHandle<()>);
impl Drop for ConnectionTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) fn privileges_ready() -> Result<bool, PlatformFailure> {
    let status = std::fs::read_to_string("/proc/thread-self/status")?;
    let capabilities = status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:"))
        .ok_or_else(|| PlatformFailure::new("permission", "内核没有返回网络权限状态"))?;
    let effective = u64::from_str_radix(capabilities.trim(), 16)
        .map_err(|_| PlatformFailure::new("permission", "内核网络权限状态无效"))?;
    const REQUIRED: u64 = (1 << 12) | (1 << 13);
    Ok(effective & REQUIRED == REQUIRED)
}

impl Device {
    pub(crate) async fn create(plan: ValidatedTunPlan) -> Result<Arc<Self>, PlatformFailure> {
        if plan.capture_mode == super::model::CaptureMode::Full {
            let snapshot = crate::network::interface::capture_snapshot()
                .await
                .map_err(|error| PlatformFailure::new(&error.code, &error.message))?;
            super::capture::validate_system(&plan, &snapshot.routes, &snapshot.local_addresses)?;
        }
        // The nonblocking device open and single setup ioctl complete before
        // yielding. No detached blocking job may create an FD after stop.
        let fd = open_exclusive(&plan.name)?;
        let descriptor = AsyncFd::new(fd)?;
        let (connection, handle, _) = rtnetlink::new_connection()?;
        let _connection = ConnectionTask(tokio::spawn(connection));
        let mut links = handle.link().get().match_name(plan.name.clone()).execute();
        let index = links
            .try_next()
            .await
            .map_err(netlink_error)?
            .ok_or_else(|| PlatformFailure::new("interface", "内核没有返回新建 TUN 接口"))?
            .header
            .index;
        verify_name(&descriptor, &plan.name)?;
        let mut existing_addresses = handle.address().get().execute();
        while let Some(address) = existing_addresses.try_next().await.map_err(netlink_error)? {
            if address.header.index != index
                && address.attributes.iter().any(|attribute| match attribute {
                    AddressAttribute::Address(host) | AddressAttribute::Local(host) => {
                        plan.addresses.iter().any(|planned| planned.addr() == *host)
                    }
                    _ => false,
                })
            {
                return Err(PlatformFailure::new(
                    "conflict",
                    "虚拟地址已被其他接口使用，保持原接口不变",
                ));
            }
        }
        for family in [false, true] {
            let request = if family {
                RouteMessageBuilder::<Ipv6Addr>::new().build()
            } else {
                RouteMessageBuilder::<Ipv4Addr>::new().build()
            };
            let mut existing = handle.route().get(request).execute();
            while let Some(route) = existing.try_next().await.map_err(netlink_error)? {
                if plan.routes.iter().any(|network| conflict(&route, network)) {
                    return Err(PlatformFailure::new(
                        "conflict",
                        "现有路由与捕获网段相同，保持原路由不变",
                    ));
                }
            }
        }
        handle
            .link()
            .change(
                LinkMessageBuilder::<LinkUnspec>::new()
                    .index(index)
                    .mtu(plan.mtu)
                    .up()
                    .build(),
            )
            .execute()
            .await
            .map_err(netlink_error)?;
        for address in &plan.addresses {
            let mut request = handle
                .address()
                .add(index, address.addr(), address.prefix_len());
            request
                .message_mut()
                .attributes
                .push(AddressAttribute::Flags(
                    AddressFlags::Noprefixroute | AddressFlags::Nodad,
                ));
            request.execute().await.map_err(netlink_error)?;
        }
        for network in &plan.routes {
            let request = match network {
                ipnet::IpNet::V4(network) => RouteMessageBuilder::<Ipv4Addr>::new()
                    .destination_prefix(network.network(), network.prefix_len())
                    .output_interface(index)
                    .scope(RouteScope::Link)
                    .priority(32765)
                    .build(),
                ipnet::IpNet::V6(network) => RouteMessageBuilder::<Ipv6Addr>::new()
                    .destination_prefix(network.network(), network.prefix_len())
                    .output_interface(index)
                    .scope(RouteScope::Link)
                    .priority(32765)
                    .build(),
            };
            handle
                .route()
                .add(request)
                .execute()
                .await
                .map_err(netlink_error)?;
        }
        Ok(Arc::new(Self {
            io: super::packet_fd::PacketFd::registered(descriptor, plan.mtu),
            info: TunInfo {
                name: plan.name,
                index,
                mtu: plan.mtu,
                addresses: plan.addresses.iter().map(ToString::to_string).collect(),
                routes: plan.routes.iter().map(ToString::to_string).collect(),
                closed: false,
            },
            protected: plan.protected_ips,
            dynamic: tokio::sync::Mutex::new(BTreeSet::new()),
        }))
    }

    /// Changes only host routes owned by this nonpersistent interface.
    pub(crate) async fn sync_dynamic(
        &self,
        mut wanted: BTreeSet<IpAddr>,
        cancellation: &CancellationToken,
    ) -> Result<(), PlatformFailure> {
        super::model::validate_dynamic(&self.info, &self.protected, &mut wanted)?;
        let mut installed = self.dynamic.lock().await;
        let operation = self.io.operation()?;
        verify_name(operation.fd.as_deref().ok_or_else(closed)?, &self.info.name)?;
        let (connection, handle, _) = rtnetlink::new_connection()?;
        let _connection = ConnectionTask(tokio::spawn(connection));
        let before = installed.clone();
        let operation = async {
            for ip in wanted.difference(&before) {
                let network = ipnet::IpNet::new(*ip, if ip.is_ipv4() { 32 } else { 128 })
                    .map_err(|_| PlatformFailure::new("route", "动态地址无效"))?;
                let request = if ip.is_ipv4() {
                    RouteMessageBuilder::<Ipv4Addr>::new().build()
                } else {
                    RouteMessageBuilder::<Ipv6Addr>::new().build()
                };
                let mut existing = handle.route().get(request).execute();
                while let Some(route) = existing.try_next().await.map_err(netlink_error)? {
                    if conflict(&route, &network) {
                        return Err(PlatformFailure::new(
                            "conflict",
                            "动态捕获不能覆盖已有主机路由",
                        ));
                    }
                }
                handle
                    .route()
                    .add(host_route(*ip, self.info.index))
                    .execute()
                    .await
                    .map_err(netlink_error)?;
                installed.insert(*ip);
            }
            for ip in before.difference(&wanted) {
                handle
                    .route()
                    .del(host_route(*ip, self.info.index))
                    .execute()
                    .await
                    .map_err(netlink_error)?;
                installed.remove(ip);
            }
            Ok::<_, PlatformFailure>(())
        };
        let operation = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(PlatformFailure::new("cancelled", "动态路由操作已取消，撤销 TUN")),
            result = tokio::time::timeout(Duration::from_secs(2), operation) => {
                match result {
                    Ok(result) => result,
                    Err(_) => return Err(PlatformFailure::new("route_rollback", "动态路由操作超时，无法证明内核提交状态，必须撤销 TUN")),
                }
            }
        };
        if let Err(error) = operation {
            // Preserve the last valid set. Failure to restore is fatal to the
            // caller, which closes this interface and all its remaining routes.
            let changed = installed.clone();
            let rollback = async {
                for ip in before.difference(&changed) {
                    if handle
                        .route()
                        .add(host_route(*ip, self.info.index))
                        .execute()
                        .await
                        .is_err()
                    {
                        return Err(PlatformFailure::new(
                            "route_rollback",
                            "动态路由恢复失败，必须撤销 TUN",
                        ));
                    }
                    installed.insert(*ip);
                }
                for ip in changed.difference(&before) {
                    if handle
                        .route()
                        .del(host_route(*ip, self.info.index))
                        .execute()
                        .await
                        .is_err()
                    {
                        return Err(PlatformFailure::new(
                            "route_rollback",
                            "动态路由回滚失败，必须撤销 TUN",
                        ));
                    }
                    installed.remove(ip);
                }
                Ok::<_, PlatformFailure>(())
            };
            tokio::time::timeout(Duration::from_secs(2), rollback)
                .await
                .map_err(|_| {
                    PlatformFailure::new("route_rollback", "动态路由回滚超时，必须撤销 TUN")
                })??;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn info(&self) -> TunInfo {
        let mut info = self.info.clone();
        info.closed = self.io.is_closed();
        info
    }

    pub(super) fn duplicate_fd(&self) -> Result<OwnedFd, PlatformFailure> {
        let operation = self.io.operation()?;
        Ok(operation
            .fd
            .as_deref()
            .ok_or_else(closed)?
            .get_ref()
            .try_clone()?)
    }

    pub(crate) async fn read(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        self.io.read(wait_ms).await
    }
    pub(crate) async fn write(&self, packet: &[u8]) -> Result<(), PlatformFailure> {
        self.io.write(packet).await
    }
    pub(crate) async fn close(&self) -> Result<(), PlatformFailure> {
        self.io.close().await
    }
}

fn open_exclusive(name: &str) -> Result<OwnedFd, PlatformFailure> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open("/dev/net/tun")?;
    // SAFETY: zero is valid for the C ifreq union; libc supplies the correct ABI size.
    let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
    for (slot, byte) in request.ifr_name.iter_mut().zip(name.bytes()) {
        *slot = byte as libc::c_char;
    }
    request.ifr_ifru.ifru_flags =
        (libc::IFF_TUN | libc::IFF_NO_PI | libc::IFF_TUN_EXCL) as libc::c_short;
    // SAFETY: file is owned and live; request has native ifreq layout and remains
    // writable for the ioctl. EXCL forbids attaching to another owner's interface.
    if unsafe { libc::ioctl(file.as_raw_fd(), libc::TUNSETIFF as _, &mut request) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(file.into())
}

fn verify_name(descriptor: &AsyncFd<OwnedFd>, expected: &str) -> Result<(), PlatformFailure> {
    // SAFETY: libc defines the complete native ifreq representation.
    let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
    // SAFETY: the owned descriptor and writable native ifreq remain live for
    // TUNGETIFF; this asks the FD's actual device, rather than a name lookup.
    if unsafe { libc::ioctl(descriptor.as_raw_fd(), libc::TUNGETIFF as _, &mut request) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let bytes: Vec<u8> = request
        .ifr_name
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect();
    if bytes != expected.as_bytes() {
        return Err(PlatformFailure::new(
            "identity",
            "TUN 接口身份在配置期间发生变化",
        ));
    }
    Ok(())
}

fn conflict(route: &RouteMessage, network: &ipnet::IpNet) -> bool {
    let table = route
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            RouteAttribute::Table(table) => Some(*table),
            _ => None,
        })
        .unwrap_or(u32::from(route.header.table));
    table == 254
        && route.header.destination_prefix_length == network.prefix_len()
        && route.attributes.iter().any(|attribute| match attribute {
            RouteAttribute::Destination(address) => {
                address == &netlink_packet_route::route::RouteAddress::from(network.network())
            }
            _ => false,
        })
}
fn netlink_error(error: rtnetlink::Error) -> PlatformFailure {
    PlatformFailure::new("netlink", &format!("配置 TUN 失败，撤销本次接口：{error}"))
}
fn host_route(ip: IpAddr, index: u32) -> RouteMessage {
    match ip {
        IpAddr::V4(ip) => RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(ip, 32)
            .output_interface(index)
            .scope(RouteScope::Link)
            .priority(32765)
            .build(),
        IpAddr::V6(ip) => RouteMessageBuilder::<Ipv6Addr>::new()
            .destination_prefix(ip, 128)
            .output_interface(index)
            .scope(RouteScope::Link)
            .priority(32765)
            .build(),
    }
}
fn closed() -> PlatformFailure {
    PlatformFailure::new("closed", "TUN 已关闭")
}
