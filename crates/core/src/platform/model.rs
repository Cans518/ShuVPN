use std::collections::BTreeSet;
use std::net::IpAddr;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureMode {
    #[default]
    Campus,
    Full,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TunPlan {
    pub name: Option<String>,
    pub mtu: u32,
    pub addresses: Vec<String>,
    pub routes: Vec<String>,
    pub protected_ips: Vec<IpAddr>,
    #[serde(default)]
    pub physical_dns_servers: Option<Vec<IpAddr>>,
    #[serde(default)]
    pub dns_capture: bool,
    #[serde(default)]
    pub capture_mode: CaptureMode,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone)]
pub(crate) struct ValidatedTunPlan {
    pub name: String,
    pub mtu: u32,
    pub addresses: Vec<ipnet::IpNet>,
    pub routes: Vec<ipnet::IpNet>,
    pub physical_dns_servers: Option<Vec<IpAddr>>,
    pub protected_ips: Vec<IpAddr>,
    pub dns_capture: bool,
    pub capture_mode: CaptureMode,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) struct TunInfo {
    pub name: String,
    pub index: u32,
    pub mtu: u32,
    pub addresses: Vec<String>,
    pub routes: Vec<String>,
    pub closed: bool,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn validate_packet(packet: &[u8], mtu: u32) -> Result<(), PlatformFailure> {
    let valid = match packet.first().map(|byte| byte >> 4) {
        Some(4) => {
            packet.len() >= 20
                && usize::from(u16::from_be_bytes([packet[2], packet[3]])) == packet.len()
                && usize::from(packet[0] & 15) * 4 >= 20
                && usize::from(packet[0] & 15) * 4 <= packet.len()
        }
        Some(6) => {
            packet.len() >= 40
                && usize::from(u16::from_be_bytes([packet[4], packet[5]])) + 40 == packet.len()
        }
        _ => false,
    };
    if !valid || packet.len() > mtu as usize {
        return Err(PlatformFailure::new("packet", "IP 包格式或 MTU 无效"));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn validate_dynamic(
    info: &TunInfo,
    protected: &[IpAddr],
    wanted: &mut BTreeSet<IpAddr>,
) -> Result<(), PlatformFailure> {
    if wanted.len() > 1024
        || wanted.iter().any(|ip| {
            ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_loopback()
                || matches!(ip, IpAddr::V4(ip) if ip.is_broadcast())
                || matches!(ip, IpAddr::V6(ip) if ip.to_ipv4_mapped().is_some())
                || protected.contains(ip)
                || info
                    .addresses
                    .iter()
                    .filter_map(|address| address.parse::<ipnet::IpNet>().ok())
                    .any(|vip| vip.addr() == *ip)
        })
    {
        return Err(PlatformFailure::new(
            "protected",
            "动态捕获地址无效、超限或属于保护范围",
        ));
    }
    wanted.retain(|ip| {
        !info
            .routes
            .iter()
            .filter_map(|route| route.parse::<ipnet::IpNet>().ok())
            .any(|route| route.contains(ip))
    });
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[allow(dead_code)]
pub(crate) fn dynamic_route_set(
    info: &TunInfo,
    protected: &[IpAddr],
    mut wanted: BTreeSet<IpAddr>,
) -> Result<Vec<String>, PlatformFailure> {
    validate_dynamic(info, protected, &mut wanted)?;
    let mut routes = info.routes.clone();
    routes.extend(
        wanted
            .iter()
            .map(|ip| format!("{ip}/{}", if ip.is_ipv4() { 32 } else { 128 })),
    );
    if routes.len() > 1280 {
        return Err(PlatformFailure::new(
            "capture_limit",
            "动态路由总数超过预算",
        ));
    }
    Ok(routes)
}

#[derive(Debug)]
pub struct PlatformFailure {
    pub code: String,
    pub message: String,
}
impl PlatformFailure {
    pub(crate) fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}
impl std::fmt::Display for PlatformFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}
impl std::error::Error for PlatformFailure {}
impl From<std::io::Error> for PlatformFailure {
    fn from(error: std::io::Error) -> Self {
        let code = if error.kind() == std::io::ErrorKind::PermissionDenied {
            "permission"
        } else {
            "system"
        };
        Self::new(code, &format!("平台操作失败：{error}"))
    }
}

pub(crate) fn validate(document: &str) -> Result<ValidatedTunPlan, PlatformFailure> {
    if document.len() > 64 * 1024 {
        return Err(PlatformFailure::new("size", "TUN 计划过大"));
    }
    let plan: TunPlan = serde_json::from_str(document)
        .map_err(|_| PlatformFailure::new("plan", "TUN 计划格式无效"))?;
    if !(1280..=9000).contains(&plan.mtu)
        || plan.addresses.is_empty()
        || plan.addresses.len() > 4
        || plan.routes.is_empty()
        || plan.routes.len()
            > if plan.capture_mode == CaptureMode::Full {
                super::capture::FULL_ROUTE_LIMIT
            } else {
                256
            }
        || plan.protected_ips.is_empty()
        || plan.protected_ips.len()
            > if plan.capture_mode == CaptureMode::Full {
                256
            } else {
                128
            }
    {
        return Err(PlatformFailure::new(
            "plan",
            "TUN 地址、路由、保护地址或 MTU 无效",
        ));
    }
    let name = match plan.name {
        Some(name) => name,
        None => {
            let mut random = [0; 4];
            getrandom::getrandom(&mut random)
                .map_err(|_| PlatformFailure::new("random", "无法生成 TUN 名称"))?;
            format!("shuvpn{:08x}", u32::from_be_bytes(random))
        }
    };
    if plan.physical_dns_servers.as_ref().is_some_and(|servers| {
        servers.len() > 16
            || servers.iter().any(|server| {
                server.is_unspecified() || server.is_multicast() || server.is_loopback()
            })
    }) {
        return Err(PlatformFailure::new(
            "dns",
            "物理 DNS 必须是明确的非 loopback 上游地址",
        ));
    }
    if !name.starts_with("shuvpn")
        || name.len() > 15
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(PlatformFailure::new(
            "name",
            "TUN 名称必须使用 shuvpn 前缀且不超过 15 个 ASCII 字符",
        ));
    }
    let addresses = plan
        .addresses
        .iter()
        .map(|address| {
            address
                .parse::<ipnet::IpNet>()
                .map_err(|_| PlatformFailure::new("address", "虚拟地址必须包含有效前缀"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen = BTreeSet::new();
    for address in &addresses {
        let host = address.addr();
        if address.prefix_len() != if host.is_ipv4() { 32 } else { 128 }
            || host.is_unspecified()
            || host.is_multicast()
            || host.is_loopback()
            || plan.protected_ips.contains(&host)
            || !seen.insert(host)
        {
            return Err(PlatformFailure::new(
                "address",
                "虚拟地址必须是唯一的 /32 或 /128 单播地址",
            ));
        }
    }
    let routes = plan
        .routes
        .iter()
        .map(|route| {
            route
                .parse::<ipnet::IpNet>()
                .map(|route| route.trunc())
                .map_err(|_| PlatformFailure::new("route", "捕获网段无效"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen = BTreeSet::new();
    let mut ipv4_space = 0u64;
    for route in &routes {
        let min_prefix = if route.addr().is_ipv4() { 8 } else { 32 };
        if (plan.capture_mode == CaptureMode::Campus && route.prefix_len() < min_prefix)
            || route.addr().is_loopback()
            || route.addr().is_multicast()
            || plan.protected_ips.iter().any(|host| route.contains(host))
            || !seen.insert(*route)
        {
            return Err(PlatformFailure::new(
                "capture",
                "捕获范围过宽、重复或包含控制面/物理出站保护地址",
            ));
        }
        if route.addr().is_ipv4() {
            ipv4_space += 1u64 << (32 - route.prefix_len());
        }
    }
    if plan.capture_mode == CaptureMode::Campus && ipv4_space >= (1u64 << 31) {
        return Err(PlatformFailure::new("capture", "尚未开放完整 IPv4 捕获"));
    }
    if plan.capture_mode == CaptureMode::Full {
        if !plan.dns_capture || plan.physical_dns_servers.as_ref().is_none_or(Vec::is_empty) {
            return Err(PlatformFailure::new(
                "dns",
                "完整捕获需要 DNS 接管及明确的物理 DNS 上游",
            ));
        }
        let mut exclusions = plan.protected_ips.clone();
        exclusions.extend(addresses.iter().map(ipnet::IpNet::addr));
        super::capture::validate_full(&routes, &exclusions)?;
    }
    Ok(ValidatedTunPlan {
        name,
        mtu: plan.mtu,
        addresses,
        routes,
        physical_dns_servers: plan.physical_dns_servers,
        protected_ips: plan.protected_ips,
        dns_capture: plan.dns_capture,
        capture_mode: plan.capture_mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> serde_json::Value {
        serde_json::json!({"name":"shuvpntest","mtu":1400,"addresses":["10.250.0.2/32"],
            "routes":["10.21.0.0/16"],"protected_ips":["192.0.2.10"]})
    }
    #[test]
    fn full_mode_requires_complete_nonoverlapping_coverage_and_dns() -> anyhow::Result<()> {
        let mut value = plan();
        value["capture_mode"] = serde_json::json!("full");
        value["dns_capture"] = serde_json::json!(true);
        value["physical_dns_servers"] = serde_json::json!(["192.0.2.53"]);
        let excluded = ["192.0.2.10".parse()?, "10.250.0.2".parse()?];
        let routes = super::super::capture::full_routes(&excluded, &[])?;
        value["routes"] =
            serde_json::json!(routes.iter().map(ToString::to_string).collect::<Vec<_>>());
        assert_eq!(
            validate(&value.to_string())?.capture_mode,
            CaptureMode::Full
        );
        let mut hole = value.clone();
        hole["routes"]
            .as_array_mut()
            .ok_or_else(|| PlatformFailure::new("test", "routes missing"))?
            .pop();
        assert!(validate(&hole.to_string()).is_err());
        let mut without_capture = value.clone();
        without_capture["dns_capture"] = serde_json::json!(false);
        assert!(validate(&without_capture.to_string()).is_err());
        for physical_dns in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!([])),
        ] {
            let mut invalid = value.clone();
            if let Some(servers) = physical_dns {
                invalid["physical_dns_servers"] = servers;
            } else {
                invalid
                    .as_object_mut()
                    .ok_or_else(|| PlatformFailure::new("test", "plan missing"))?
                    .remove("physical_dns_servers");
            }
            let failure = validate(&invalid.to_string())
                .err()
                .ok_or_else(|| anyhow::anyhow!("physical DNS is required"))?;
            assert_eq!(failure.code, "dns");
            assert_eq!(
                failure.message,
                "完整捕获需要 DNS 接管及明确的物理 DNS 上游"
            );
        }
        Ok(())
    }

    #[test]
    fn split_capture_cannot_install_defaults_or_capture_a_protected_endpoint() {
        for route in ["0.0.0.0/0", "0.0.0.0/1", "::/0", "192.0.2.0/24"] {
            let mut value = plan();
            value["routes"] = serde_json::json!([route]);
            assert!(validate(&value.to_string()).is_err());
        }
    }
    #[test]
    fn names_addresses_and_mtu_are_validated_before_any_platform_mutation() {
        for (key, value) in [
            ("name", serde_json::json!("eth0")),
            ("name", serde_json::json!("shuvpn;ip")),
            ("mtu", serde_json::json!(100000)),
            ("addresses", serde_json::json!(["0.0.0.0/32"])),
            ("addresses", serde_json::json!(["10.250.0.2/24"])),
            ("addresses", serde_json::json!(["192.0.2.10/32"])),
            ("protected_ips", serde_json::json!([])),
            ("physical_dns_servers", serde_json::json!(["127.0.0.53"])),
        ] {
            let mut document = plan();
            document[key] = value;
            assert!(validate(&document.to_string()).is_err());
        }
    }
    #[test]
    fn capture_networks_are_canonical_and_duplicate_or_unknown_options_fail() -> anyhow::Result<()>
    {
        let mut value = plan();
        value["routes"] = serde_json::json!(["10.21.1.3/16"]);
        assert_eq!(
            validate(&value.to_string())?.routes[0].to_string(),
            "10.21.0.0/16"
        );
        value["routes"] = serde_json::json!(["10.21.1.3/16", "10.21.0.0/16"]);
        assert!(validate(&value.to_string()).is_err());
        value = plan();
        value["full_capture"] = serde_json::json!(true);
        assert!(validate(&value.to_string()).is_err());
        Ok(())
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn dynamic_hosts_cannot_capture_control_addresses_or_mapped_and_broadcast_targets(
    ) -> anyhow::Result<()> {
        let info = TunInfo {
            name: "shuvpntest".to_owned(),
            index: 10,
            mtu: 1400,
            addresses: vec!["10.250.0.2/32".to_owned()],
            routes: vec!["10.21.0.0/16".to_owned()],
            closed: false,
        };
        let protected = ["192.0.2.10".parse()?];
        for ip in [
            "127.0.0.1",
            "::1",
            "::ffff:192.0.2.10",
            "::ffff:127.0.0.1",
            "255.255.255.255",
            "224.0.0.1",
            "::",
            "10.250.0.2",
            "192.0.2.10",
        ] {
            let mut hosts = BTreeSet::from([ip.parse()?]);
            assert!(validate_dynamic(&info, &protected, &mut hosts).is_err());
        }
        let mut hosts = BTreeSet::from([
            "10.21.0.7".parse()?,
            "10.99.0.7".parse()?,
            "2001:db8::7".parse()?,
        ]);
        validate_dynamic(&info, &protected, &mut hosts)?;
        assert_eq!(
            hosts,
            BTreeSet::from(["10.99.0.7".parse()?, "2001:db8::7".parse()?])
        );
        let mut hosts = (0..1025)
            .map(|value| IpAddr::V4(std::net::Ipv4Addr::from(0x0a000000 + value)))
            .collect();
        assert!(validate_dynamic(&info, &protected, &mut hosts).is_err());
        Ok(())
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn dynamic_routes_preserve_base_coverage_and_protect_control_addresses() -> anyhow::Result<()> {
        let info = TunInfo {
            name: "shuvpntest".to_owned(),
            index: 10,
            mtu: 1400,
            addresses: vec!["10.250.0.2/32".to_owned()],
            routes: vec!["10.21.0.0/16".to_owned(), "2001:db8:21::/64".to_owned()],
            closed: false,
        };
        let protected = ["10.21.0.8".parse()?];
        assert_eq!(
            dynamic_route_set(
                &info,
                &protected,
                BTreeSet::from(["10.21.0.7".parse()?, "2001:db8:21::7".parse()?]),
            )?,
            info.routes
        );
        assert_eq!(
            dynamic_route_set(
                &info,
                &protected,
                BTreeSet::from(["10.99.0.7".parse()?, "2001:db8::7".parse()?])
            )?,
            vec![
                "10.21.0.0/16",
                "2001:db8:21::/64",
                "10.99.0.7/32",
                "2001:db8::7/128"
            ]
        );
        assert_eq!(
            dynamic_route_set(&info, &protected, BTreeSet::from(["10.21.0.8".parse()?]))
                .err()
                .map(|error| error.code),
            Some("protected".to_owned())
        );
        Ok(())
    }
}
