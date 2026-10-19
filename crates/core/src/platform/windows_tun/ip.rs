use super::api::{error, wide};
use crate::platform::model::{PlatformFailure, ValidatedTunPlan};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_NETWORK_UNREACHABLE, ERROR_NOT_FOUND, NO_ERROR,
};
use windows_sys::Win32::NetworkManagement::{
    IpHelper::*,
    Ndis::{IfOperStatusUp, MediaConnectStateConnected, NET_LUID_LH},
};
use windows_sys::Win32::Networking::WinSock::*;

struct Table(*mut std::ffi::c_void);
impl Drop for Table {
    fn drop(&mut self) {
        unsafe {
            FreeMibTable(self.0);
        }
    }
}

pub(super) fn check_plan(plan: &ValidatedTunPlan) -> Result<(), PlatformFailure> {
    if plan.capture_mode == crate::platform::model::CaptureMode::Full {
        let snapshot = crate::network::windows_capture_snapshot()
            .map_err(|cause| error("capture", &cause.message))?;
        crate::platform::capture::validate_system(
            plan,
            &snapshot.routes,
            &snapshot.local_addresses,
        )?;
    }
    let mut luid = NET_LUID_LH::default();
    if unsafe { ConvertInterfaceAliasToLuid(wide(&plan.name).as_ptr(), &mut luid) } == NO_ERROR {
        return Err(error("conflict", "TUN 名称已被其他接口使用"));
    }
    let mut table: *mut MIB_UNICASTIPADDRESS_TABLE = null_mut();
    check(
        unsafe { GetUnicastIpAddressTable(AF_UNSPEC, &mut table) },
        "读取现有接口地址",
    )?;
    if table.is_null() {
        return Err(error("system", "系统没有返回接口地址表"));
    }
    let _table = Table(table.cast());
    let count = unsafe { (*table).NumEntries } as usize;
    if count > 100_000 {
        return Err(error("size", "系统地址表超过预算"));
    }
    for row in unsafe { std::slice::from_raw_parts((*table).Table.as_ptr(), count) } {
        if address(&row.Address)
            .is_some_and(|ip| plan.addresses.iter().any(|planned| planned.addr() == ip))
        {
            return Err(error("conflict", "虚拟地址已被其他接口使用"));
        }
    }
    let routes = route_table()?;
    if routes.iter().any(|row| {
        plan.routes.iter().any(|network| {
            row.DestinationPrefix.PrefixLength == network.prefix_len()
                && address(&row.DestinationPrefix.Prefix) == Some(network.network())
        })
    }) {
        return Err(error("conflict", "现有路由与捕获网段相同"));
    }
    Ok(())
}

pub(super) struct Configuration {
    addresses: Vec<MIB_UNICASTIPADDRESS_ROW>,
    routes: Vec<MIB_IPFORWARD_ROW2>,
    pub(super) dynamic: super::dynamic::Routes,
}

impl Configuration {
    pub(super) fn new() -> Self {
        Self {
            addresses: vec![],
            routes: vec![],
            dynamic: super::dynamic::Routes::default(),
        }
    }

    pub(super) fn install(
        &mut self,
        luid: NET_LUID_LH,
        index: u32,
        plan: &ValidatedTunPlan,
    ) -> Result<(), PlatformFailure> {
        for family in [AF_INET, AF_INET6] {
            if !plan
                .addresses
                .iter()
                .any(|address| address.addr().is_ipv4() == (family == AF_INET))
            {
                continue;
            }
            let mut row = MIB_IPINTERFACE_ROW::default();
            unsafe {
                InitializeIpInterfaceEntry(&mut row);
            }
            row.Family = family;
            row.InterfaceLuid = luid;
            check(
                unsafe { GetIpInterfaceEntry(&mut row) },
                "读取本次 TUN 的 IP 接口",
            )?;
            row.NlMtu = plan.mtu;
            row.DadTransmits = 0;
            row.RouterDiscoveryBehavior = RouterDiscoveryDisabled;
            row.DisableDefaultRoutes = true;
            if family == AF_INET {
                row.SitePrefixLength = 0;
            }
            check(
                unsafe { SetIpInterfaceEntry(&mut row) },
                "设置本次 TUN 的 MTU",
            )?;
        }
        let interface = MIB_IFROW {
            dwIndex: index,
            dwAdminStatus: MIB_IF_ADMIN_STATUS_UP,
            ..Default::default()
        };
        check(unsafe { SetIfEntry(&interface) }, "启动本次 TUN 接口")?;
        for network in &plan.addresses {
            let row = address_row(luid, *network);
            check(
                unsafe { CreateUnicastIpAddressEntry(&row) },
                "添加本次 TUN 的虚拟地址",
            )?;
            self.addresses.push(row);
        }
        for network in &plan.routes {
            let row = route_row(luid, index, *network);
            check(
                unsafe { CreateIpForwardEntry2(&row) },
                "添加本次 TUN 的捕获路由",
            )?;
            self.routes.push(row);
        }
        Ok(())
    }

    pub(super) fn close(&mut self) -> Result<(), PlatformFailure> {
        let dynamic = self.dynamic.close(&mut NativeRoutes);
        let configured = self.close_with(
            |row| unsafe { DeleteIpForwardEntry2(row) },
            |row| unsafe { DeleteUnicastIpAddressEntry(row) },
        );
        dynamic.and(configured)
    }

    pub(super) fn ready(&self, luid: NET_LUID_LH, index: u32) -> Result<bool, PlatformFailure> {
        let mut interface = MIB_IF_ROW2 {
            InterfaceLuid: luid,
            ..Default::default()
        };
        // SAFETY: live writable selector; IP Helper fills the same interface row.
        check(
            unsafe { GetIfEntry2(&mut interface) },
            "读取本次 TUN 就绪状态",
        )?;
        if interface.InterfaceIndex != index
            || unsafe { interface.InterfaceLuid.Value } != unsafe { luid.Value }
        {
            return Err(error("identity", "TUN 就绪检查的接口身份改变"));
        }
        if interface.OperStatus != IfOperStatusUp
            || interface.MediaConnectState != MediaConnectStateConnected
        {
            return Ok(false);
        }
        for expected in &self.addresses {
            let mut actual = *expected;
            // SAFETY: the stored LUID/address select a row owned by this configuration.
            let code = unsafe { GetUnicastIpAddressEntry(&mut actual) };
            if [ERROR_NOT_FOUND, ERROR_FILE_NOT_FOUND].contains(&code) {
                return Ok(false);
            }
            check(code, "读取本次 TUN 地址状态")?;
            if actual.DadState == IpDadStateDuplicate {
                return Err(error("address", "TUN 虚拟地址发生冲突"));
            }
            if actual.DadState != IpDadStatePreferred {
                return Ok(false);
            }
        }
        for expected in &self.routes {
            let mut actual = *expected;
            // SAFETY: all route-key fields came from a successfully installed owned row.
            let code = unsafe { GetIpForwardEntry2(&mut actual) };
            if [ERROR_NOT_FOUND, ERROR_FILE_NOT_FOUND].contains(&code) {
                return Ok(false);
            }
            check(code, "读取本次 TUN 路由状态")?;
            // SAFETY: family is the common initial member of SOCKADDR_INET.
            let family = unsafe { expected.DestinationPrefix.Prefix.si_family };
            let host =
                expected.DestinationPrefix.PrefixLength == if family == AF_INET { 32 } else { 128 };
            if host {
                let mut best = MIB_IPFORWARD_ROW2::default();
                let mut source = SOCKADDR_INET::default();
                // SAFETY: null/zero leave the query unconstrained; input and outputs are live.
                let code = unsafe {
                    GetBestRoute2(
                        std::ptr::null(),
                        0,
                        std::ptr::null(),
                        &expected.DestinationPrefix.Prefix,
                        0,
                        &mut best,
                        &mut source,
                    )
                };
                if [ERROR_NOT_FOUND, ERROR_NETWORK_UNREACHABLE].contains(&code) {
                    return Ok(false);
                }
                check(code, "核验本次 TUN 主机选路")?;
                if best.InterfaceIndex != index
                    || unsafe { best.InterfaceLuid.Value } != unsafe { luid.Value }
                {
                    return Ok(false);
                }
                if !self
                    .addresses
                    .iter()
                    .any(|row| address(&row.Address) == address(&source))
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn close_with(
        &mut self,
        delete_route: impl FnMut(&MIB_IPFORWARD_ROW2) -> u32,
        delete_address: impl FnMut(&MIB_UNICASTIPADDRESS_ROW) -> u32,
    ) -> Result<(), PlatformFailure> {
        let mut failures = delete_rows(&mut self.routes, delete_route);
        failures.extend(delete_rows(&mut self.addresses, delete_address));
        if !failures.is_empty() {
            return Err(error(
                "cleanup",
                &format!("清理本次 TUN 地址/路由失败：{failures:?}"),
            ));
        }
        Ok(())
    }
}

fn delete_rows<T>(rows: &mut Vec<T>, mut delete: impl FnMut(&T) -> u32) -> Vec<u32> {
    let mut failures = vec![];
    for row in std::mem::take(rows).into_iter().rev() {
        let code = delete(&row);
        if code != NO_ERROR && code != ERROR_NOT_FOUND {
            failures.push(code);
            rows.push(row);
        }
    }
    // Failed rows retain their installation order for a later close attempt.
    rows.reverse();
    failures
}

pub(super) fn address_row(luid: NET_LUID_LH, network: ipnet::IpNet) -> MIB_UNICASTIPADDRESS_ROW {
    let mut row = MIB_UNICASTIPADDRESS_ROW::default();
    unsafe {
        InitializeUnicastIpAddressEntry(&mut row);
    }
    row.InterfaceLuid = luid;
    row.Address = sockaddr(network.addr());
    row.OnLinkPrefixLength = network.prefix_len();
    row.PrefixOrigin = IpPrefixOriginManual;
    row.SuffixOrigin = IpSuffixOriginManual;
    row.ValidLifetime = u32::MAX;
    row.PreferredLifetime = u32::MAX;
    row.SkipAsSource = false;
    row.DadState = IpDadStatePreferred;
    row
}

pub(super) fn route_row(
    luid: NET_LUID_LH,
    index: u32,
    network: ipnet::IpNet,
) -> MIB_IPFORWARD_ROW2 {
    let mut row = MIB_IPFORWARD_ROW2::default();
    unsafe {
        InitializeIpForwardEntry(&mut row);
    }
    row.InterfaceLuid = luid;
    row.InterfaceIndex = index;
    row.DestinationPrefix.Prefix = sockaddr(network.network());
    row.DestinationPrefix.PrefixLength = network.prefix_len();
    row.NextHop = sockaddr(if network.addr().is_ipv4() {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V6(Ipv6Addr::UNSPECIFIED)
    });
    row.SitePrefixLength = 0;
    row.ValidLifetime = u32::MAX;
    row.PreferredLifetime = u32::MAX;
    row.Metric = 32765;
    row.Protocol = MIB_IPPROTO_NETMGMT;
    row
}

fn sockaddr(ip: IpAddr) -> SOCKADDR_INET {
    match ip {
        IpAddr::V4(ip) => {
            let mut value = SOCKADDR_IN {
                sin_family: AF_INET,
                ..Default::default()
            };
            value.sin_addr.S_un.S_addr = u32::from_ne_bytes(ip.octets());
            SOCKADDR_INET { Ipv4: value }
        }
        IpAddr::V6(ip) => {
            let mut value = SOCKADDR_IN6 {
                sin6_family: AF_INET6,
                ..Default::default()
            };
            value.sin6_addr.u.Byte = ip.octets();
            SOCKADDR_INET { Ipv6: value }
        }
    }
}

pub(super) fn address(value: &SOCKADDR_INET) -> Option<IpAddr> {
    match unsafe { value.si_family } {
        AF_INET => Some(IpAddr::V4(Ipv4Addr::from(
            unsafe { value.Ipv4.sin_addr.S_un.S_addr }.to_ne_bytes(),
        ))),
        AF_INET6 => Some(IpAddr::V6(Ipv6Addr::from(unsafe {
            value.Ipv6.sin6_addr.u.Byte
        }))),
        _ => None,
    }
}

fn route_table() -> Result<Vec<MIB_IPFORWARD_ROW2>, PlatformFailure> {
    let mut table: *mut MIB_IPFORWARD_TABLE2 = null_mut();
    check(
        unsafe { GetIpForwardTable2(AF_UNSPEC, &mut table) },
        "读取现有系统路由",
    )?;
    if table.is_null() {
        return Err(error("system", "系统没有返回路由表"));
    }
    let _table = Table(table.cast());
    let count = unsafe { (*table).NumEntries } as usize;
    if count > 100_000 {
        return Err(error("size", "系统路由表超过预算"));
    }
    Ok(unsafe { std::slice::from_raw_parts((*table).Table.as_ptr(), count) }.to_vec())
}

pub(super) trait RouteIo {
    fn entries(&mut self) -> Result<Vec<MIB_IPFORWARD_ROW2>, PlatformFailure>;
    fn add(&mut self, row: &MIB_IPFORWARD_ROW2) -> Result<(), PlatformFailure>;
    fn remove(&mut self, row: &MIB_IPFORWARD_ROW2) -> Result<(), PlatformFailure>;
}
pub(super) struct NativeRoutes;
impl RouteIo for NativeRoutes {
    fn entries(&mut self) -> Result<Vec<MIB_IPFORWARD_ROW2>, PlatformFailure> {
        route_table()
    }
    fn add(&mut self, row: &MIB_IPFORWARD_ROW2) -> Result<(), PlatformFailure> {
        check(unsafe { CreateIpForwardEntry2(row) }, "添加动态捕获路由")
    }
    fn remove(&mut self, row: &MIB_IPFORWARD_ROW2) -> Result<(), PlatformFailure> {
        let mut actual = *row;
        let code = unsafe { GetIpForwardEntry2(&mut actual) };
        if code == ERROR_NOT_FOUND {
            return Ok(());
        }
        check(code, "核对动态捕获路由")?;
        if !same_owned_route(&actual, row) {
            return Err(error("conflict", "路由已被外部修改，保留外部状态"));
        }
        let code = unsafe { DeleteIpForwardEntry2(&actual) };
        if code == ERROR_NOT_FOUND {
            return Ok(());
        }
        check(code, "撤销动态捕获路由")
    }
}

pub(super) fn same_prefix(a: &MIB_IPFORWARD_ROW2, b: &MIB_IPFORWARD_ROW2) -> bool {
    a.DestinationPrefix.PrefixLength == b.DestinationPrefix.PrefixLength
        && address(&a.DestinationPrefix.Prefix) == address(&b.DestinationPrefix.Prefix)
}
pub(super) fn same_owned_route(a: &MIB_IPFORWARD_ROW2, b: &MIB_IPFORWARD_ROW2) -> bool {
    same_prefix(a, b)
        && same_sockaddr(&a.DestinationPrefix.Prefix, &b.DestinationPrefix.Prefix)
        && unsafe { a.InterfaceLuid.Value == b.InterfaceLuid.Value }
        && a.InterfaceIndex == b.InterfaceIndex
        && same_sockaddr(&a.NextHop, &b.NextHop)
        && a.Metric == b.Metric
        && a.Protocol == b.Protocol
        && a.SitePrefixLength == b.SitePrefixLength
        && a.ValidLifetime == b.ValidLifetime
        && a.PreferredLifetime == b.PreferredLifetime
        && a.Loopback == b.Loopback
        && a.AutoconfigureAddress == b.AutoconfigureAddress
        && a.Publish == b.Publish
        && a.Immortal == b.Immortal
}

fn same_sockaddr(a: &SOCKADDR_INET, b: &SOCKADDR_INET) -> bool {
    if address(a) != address(b) {
        return false;
    }
    // Scope and flow information are part of an IPv6 route identity even when
    // its address bytes match. Age/Origin are assigned by the network stack.
    unsafe {
        match (a.si_family, b.si_family) {
            (AF_INET, AF_INET) => a.Ipv4.sin_port == b.Ipv4.sin_port,
            (AF_INET6, AF_INET6) => {
                a.Ipv6.sin6_port == b.Ipv6.sin6_port
                    && a.Ipv6.sin6_flowinfo == b.Ipv6.sin6_flowinfo
                    && a.Ipv6.Anonymous.sin6_scope_id == b.Ipv6.Anonymous.sin6_scope_id
            }
            _ => false,
        }
    }
}

pub(super) fn check_device(
    luid: NET_LUID_LH,
    index: u32,
    name: &str,
) -> Result<(), PlatformFailure> {
    let mut row = MIB_IF_ROW2 {
        InterfaceLuid: luid,
        ..Default::default()
    };
    check(unsafe { GetIfEntry2(&mut row) }, "核对本次 TUN 接口")?;
    let length = row
        .Alias
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| error("identity", "TUN 名称未结束"))?;
    if row.InterfaceIndex != index
        || unsafe { row.InterfaceLuid.Value } != unsafe { luid.Value }
        || String::from_utf16(&row.Alias[..length]).ok().as_deref() != Some(name)
    {
        return Err(error("identity", "TUN 接口身份已变化"));
    }
    Ok(())
}

pub(super) fn check(code: u32, operation: &str) -> Result<(), PlatformFailure> {
    if code == NO_ERROR {
        return Ok(());
    }
    let cause = std::io::Error::from_raw_os_error(code as i32);
    let kind = if code == windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED {
        "permission"
    } else {
        "system"
    };
    Err(error(kind, &format!("{operation}失败：{cause}")))
}

pub(super) fn removed(luid: NET_LUID_LH) -> Result<(), PlatformFailure> {
    let mut row = MIB_IF_ROW2 {
        InterfaceLuid: luid,
        ..Default::default()
    };
    let code = unsafe { GetIfEntry2(&mut row) };
    removal_status(code)
}

#[cfg(feature = "tun-probe")]
pub(super) fn find_device(name: &str) -> Result<Option<NET_LUID_LH>, PlatformFailure> {
    let mut table: *mut MIB_IF_TABLE2 = null_mut();
    // SAFETY: Windows allocates the table; Table frees it on every return path.
    check(unsafe { GetIfTable2(&mut table) }, "读取接口撤销快照")?;
    if table.is_null() {
        return Err(error("system", "接口快照为空"));
    }
    let _table = Table(table.cast());
    // SAFETY: successful GetIfTable2 returned a valid table allocation.
    let count = unsafe { (*table).NumEntries } as usize;
    if count > 100_000 {
        return Err(error("size", "接口快照超过预算"));
    }
    // SAFETY: table header supplies the number of contiguous SDK rows.
    for row in unsafe { std::slice::from_raw_parts((*table).Table.as_ptr(), count) } {
        let length = row
            .Alias
            .iter()
            .position(|unit| *unit == 0)
            .ok_or_else(|| error("identity", "接口快照名称未结束"))?;
        let alias = String::from_utf16(&row.Alias[..length])
            .map_err(|_| error("identity", "接口快照名称编码无效"))?;
        if alias == name {
            return Ok(Some(row.InterfaceLuid));
        }
    }
    Ok(None)
}

fn removal_status(code: u32) -> Result<(), PlatformFailure> {
    // GetIfEntry2 uses FILE_NOT_FOUND for an absent interface; other IP Helper
    // lookups also use NOT_FOUND. Neither means a cleanup failure here.
    if code == windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND || code == ERROR_NOT_FOUND {
        return Ok(());
    }
    if code != NO_ERROR {
        return check(code, "验证 TUN 接口撤销");
    }
    Err(error(
        "cleanup",
        "Wintun 关闭后接口仍存在，不能确认清理完成",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;

    #[test]
    fn absent_interface_is_confirmed_removed_using_the_real_kernel_error() -> anyhow::Result<()> {
        let mut row = MIB_IF_ROW2 {
            InterfaceIndex: u32::MAX,
            ..Default::default()
        };
        // SAFETY: query-only operation, with a live row and no system mutation.
        let code = unsafe { GetIfEntry2(&mut row) };
        assert_eq!(code, windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND);
        removal_status(code)?;
        assert!(removal_status(NO_ERROR).is_err());
        assert!(removal_status(ERROR_ACCESS_DENIED).is_err());
        Ok(())
    }

    #[test]
    fn route_ownership_includes_ipv6_scope_and_stable_attributes_but_not_age() -> anyhow::Result<()>
    {
        let row = route_row(NET_LUID_LH { Value: 123 }, 10, "2001:db8::7/128".parse()?);
        let mut actual = row;
        actual.Age = 99;
        actual.Origin = row.Origin.wrapping_add(1);
        assert!(same_owned_route(&actual, &row));
        actual.NextHop.Ipv6.Anonymous.sin6_scope_id = 10;
        assert!(!same_owned_route(&actual, &row));
        actual = row;
        actual.DestinationPrefix.Prefix.Ipv6.Anonymous.sin6_scope_id = 10;
        assert!(!same_owned_route(&actual, &row));
        actual = row;
        actual.Metric += 1;
        assert!(!same_owned_route(&actual, &row));
        Ok(())
    }

    #[test]
    fn cleanup_retains_only_failed_rows_and_retries_in_reverse_installation_order(
    ) -> anyhow::Result<()> {
        let luid = NET_LUID_LH { Value: 123456 };
        let mut config = Configuration::new();
        config
            .routes
            .push(route_row(luid, 1, "10.21.0.0/16".parse()?));
        config
            .routes
            .push(route_row(luid, 2, "10.22.0.0/16".parse()?));
        config
            .addresses
            .push(address_row(luid, "10.253.254.2/32".parse()?));
        let mut visited = vec![];
        let failed = config.close_with(
            |row| {
                visited.push(row.InterfaceIndex);
                if row.InterfaceIndex == 1 {
                    ERROR_ACCESS_DENIED
                } else {
                    NO_ERROR
                }
            },
            |_| ERROR_ACCESS_DENIED,
        );
        assert_eq!(
            failed.err().map(|failure| failure.code),
            Some("cleanup".to_owned())
        );
        assert_eq!(visited, [2, 1]);
        assert_eq!(config.routes.len(), 1);
        assert_eq!(config.addresses.len(), 1);
        config.close_with(
            |row| {
                assert_eq!(row.InterfaceIndex, 1);
                assert_eq!(unsafe { row.InterfaceLuid.Value }, 123456);
                ERROR_NOT_FOUND
            },
            |_| NO_ERROR,
        )?;
        assert!(config.routes.is_empty());
        assert!(config.addresses.is_empty());
        config.close_with(
            |_| panic!("deleted route retried"),
            |_| panic!("deleted address retried"),
        )?;
        Ok(())
    }
}
