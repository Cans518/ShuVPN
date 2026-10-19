use std::{collections::BTreeSet, net::IpAddr};

use ipnet::{IpNet, Ipv4Subnets, Ipv6Subnets};

use super::{
    desktop_services::{TunOperation, TunStartup},
    session::Engine,
};
use super::{model::VpnFailure, session::Shared};
use crate::platform::model::CaptureMode;
use crate::routing::{authorization, model::Network};

pub(crate) async fn start(
    shared: std::sync::Arc<Shared>,
    mtu: u32,
    physical_interface: String,
    physical_dns: Option<String>,
    capture_mode: Option<CaptureMode>,
) -> Result<String, VpnFailure> {
    let choices = tokio::select! {
        biased;
        _ = shared.cancellation.cancelled() => return Err(failure("closed", "会话已断开")),
        choices = crate::network::interface::available() => choices?,
    };
    let physical_interface = crate::network::interface::select(&choices, &physical_interface)?;
    let mut document: serde_json::Value = serde_json::from_str(
        &plan_for_mode(
            &shared,
            mtu,
            physical_dns.as_deref(),
            capture_mode.unwrap_or_default(),
            &physical_interface,
        )
        .await?,
    )
    .map_err(|_| failure("plan", "桌面 TUN 计划编码失败"))?;
    if document["physical_dns_servers"].is_null() {
        document["physical_dns_servers"] = serde_json::to_value(
            crate::network::interface::physical_dns(&physical_interface)?,
        )
        .map_err(|_| failure("dns", "物理 DNS 编码失败"))?;
    }
    super::packet::tun::start(shared, document.to_string(), physical_interface).await
}

pub(crate) async fn start_service(
    engine: &Engine,
    request: TunStartup,
) -> Result<TunOperation, VpnFailure> {
    let _operation = engine.desktop_operations.lock().await;
    start_service_unlocked(engine, request).await
}

pub(crate) async fn start_service_unlocked(
    engine: &Engine,
    request: TunStartup,
) -> Result<TunOperation, VpnFailure> {
    let status_json = super::packet::tun::status(&engine.shared).await?;
    let status: serde_json::Value = serde_json::from_str(&status_json)
        .map_err(|_| failure("state", "桌面 TUN 状态编码失败"))?;
    if !request.enabled || status["state"] == "running" {
        return Ok(TunOperation {
            status_json,
            diagnostic: None,
        });
    }
    if status["cleanup_pending"] == true {
        return Ok(TunOperation {
            status_json,
            diagnostic: Some("VPN 清理尚未完成，请先停止后重试".into()),
        });
    }
    let result = async {
        let driver: serde_json::Value = serde_json::from_str(
            &crate::platform::tun_driver_info()
                .await
                .map_err(|error| failure(&error.code, &error.message))?,
        )
        .map_err(|_| failure("state", "VPN 权限状态编码失败"))?;
        if driver["available"] != true {
            return Err(failure(
                "permission",
                driver["unavailableReason"]
                    .as_str()
                    .unwrap_or("当前不能启动 VPN"),
            ));
        }
        start(
            std::sync::Arc::clone(&engine.shared),
            request.mtu,
            request.physical_interface,
            request.physical_dns,
            request.capture_mode,
        )
        .await
    }
    .await;
    let mut diagnostic = result.err().map(|error| error.message);
    if diagnostic.is_some() {
        if let Err(error) = super::packet::tun::stop(&engine.shared).await {
            if let Some(message) = diagnostic.as_mut() {
                message.push_str(&format!("；VPN 清理失败：{}", error.message));
            }
        }
    }
    Ok(TunOperation {
        status_json: super::packet::tun::status(&engine.shared).await?,
        diagnostic,
    })
}

pub(crate) async fn stop_service(engine: &Engine) -> Result<TunOperation, VpnFailure> {
    let _operation = engine.desktop_operations.lock().await;
    let diagnostic = super::packet::tun::stop(&engine.shared)
        .await
        .err()
        .map(|error| error.message);
    Ok(TunOperation {
        status_json: super::packet::tun::status(&engine.shared).await?,
        diagnostic,
    })
}

pub(crate) async fn plan(
    shared: &Shared,
    mtu: u32,
    physical_dns: Option<&str>,
) -> Result<String, VpnFailure> {
    if !(1280..=9000).contains(&mtu) {
        return Err(failure("mtu", "MTU 必须在1280到9000之间"));
    }
    let physical_dns = crate::network::dns::explicit_upstream(physical_dns)?;
    let protected = tokio::select! {
        biased;
        _ = shared.cancellation.cancelled() => return Err(failure("closed", "会话已断开")),
        result = tokio::time::timeout(shared.dial_timeout, protected_nodes(shared)) =>
            result.map_err(|_| failure("timeout", "准备网关保护地址超时"))??,
    };
    let addresses: Vec<_> = shared
        .virtual_addresses
        .read()
        .map_err(|_| failure("address", "无法读取网关虚拟地址"))?
        .iter()
        .map(|ip| format!("{ip}/{}", if ip.is_ipv4() { 32 } else { 128 }))
        .collect();
    let mut networks = BTreeSet::new();
    let mut domains = false;
    for route in &shared.plan.routes {
        match resource_networks(&route.host)? {
            Some(routes) => networks.extend(routes),
            None => domains = true,
        }
        if networks.len() > 4096 {
            return Err(failure("capture_limit", "资源网段超过数量限制"));
        }
    }
    let dns: Vec<_> = shared
        .plan
        .dns_servers
        .iter()
        .filter_map(|server| {
            let ip = server.parse::<IpAddr>().ok()?;
            authorization::route_for(&shared.plan.routes, server, 53, Network::Tcp).map(|_| ip)
        })
        .take(16)
        .collect();
    if domains && dns.is_empty() {
        return Err(failure("dns", "域名型资源需要获授权的校园 DNS"));
    }
    for ip in &dns {
        if ip.is_unspecified()
            || ip.is_loopback()
            || ip.is_multicast()
            || protected.contains(ip)
            || addresses
                .iter()
                .any(|address| address.parse::<IpNet>().is_ok_and(|vip| vip.addr() == *ip))
        {
            return Err(failure("protected", "校园 DNS 地址无效或属于网关保护范围"));
        }
        networks.insert(
            IpNet::new(*ip, if ip.is_ipv4() { 32 } else { 128 })
                .map_err(|_| failure("dns", "校园 DNS 地址无效"))?,
        );
    }
    let mut collapsed = vec![];
    let mut sorted: Vec<_> = networks.into_iter().collect();
    sorted.sort_by_key(|network| {
        (
            network.addr().is_ipv6(),
            network.prefix_len(),
            network.addr(),
        )
    });
    for network in sorted {
        if !collapsed
            .iter()
            .any(|parent: &IpNet| parent.contains(&network.addr()))
        {
            collapsed.push(network);
        }
    }
    let mut routes = vec![];
    for network in collapsed {
        exclude_nodes(network, &protected, &mut routes)?;
    }
    let mtu = mtu.min(u32::from(shared.plan.mtu));
    let document = serde_json::json!({
        "mtu": mtu, "addresses": addresses, "routes": routes,
        "protected_ips": protected, "dns_capture": !dns.is_empty(),
        "physical_dns_servers": physical_dns,
    })
    .to_string();
    // Preserve the platform capture limits; a broad authorization is not
    // evidence that full capture and system DNS have been implemented.
    crate::platform::model::validate(&document)
        .map_err(|error| failure(&error.code, &error.message))?;
    Ok(document)
}

pub(crate) async fn plan_for_mode(
    shared: &Shared,
    mtu: u32,
    physical_dns: Option<&str>,
    mode: CaptureMode,
    physical_interface: &str,
) -> Result<String, VpnFailure> {
    if let Some(plan) = super::packet::tun::current_plan(shared).await? {
        return Ok(plan);
    }
    if mode == CaptureMode::Campus {
        return plan(shared, mtu, physical_dns).await;
    }
    if !(1280..=9000).contains(&mtu) {
        return Err(failure("mtu", "MTU 必须在1280到9000之间"));
    }
    let choices = crate::network::interface::available().await?;
    let physical_interface = crate::network::interface::select(&choices, physical_interface)?;
    let dns = match crate::network::dns::explicit_upstream(physical_dns)? {
        Some(addresses) => addresses,
        None => crate::network::interface::physical_dns(&physical_interface)?,
    };
    let snapshot = crate::network::interface::capture_snapshot().await?;
    let mut protected: BTreeSet<IpAddr> = protected_nodes(shared).await?.into_iter().collect();
    protected.extend(snapshot.local_addresses.iter().copied());
    let mut addresses = shared
        .virtual_addresses
        .read()
        .map_err(|_| failure("address", "无法读取网关虚拟地址"))?
        .clone();
    if addresses.iter().any(|address| protected.contains(address)) {
        return Err(failure("conflict", "网关虚拟地址与本机地址或节点冲突"));
    }
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).map_err(|_| failure("random", "无法生成本地 TUN 地址"))?;
    if !addresses.iter().any(IpAddr::is_ipv4) {
        addresses.push(
            std::net::Ipv4Addr::new(100, 64 | (random[0] & 63), random[1], 1 + random[2] % 254)
                .into(),
        );
    }
    if !addresses.iter().any(IpAddr::is_ipv6) {
        random[0] = 0xfd;
        addresses.push(std::net::Ipv6Addr::from(random).into());
    }
    let addresses_text: Vec<_> = addresses
        .iter()
        .map(|ip| format!("{ip}/{}", if ip.is_ipv4() { 32 } else { 128 }))
        .collect();
    let mut exclusions: Vec<_> = protected.iter().copied().collect();
    exclusions.extend(addresses);
    let routes = crate::platform::capture::full_routes(&exclusions, &snapshot.routes)
        .map_err(|error| failure(&error.code, &error.message))?;
    if dns
        .iter()
        .any(|ip| !routes.iter().any(|network| network.contains(ip)))
    {
        return Err(failure(
            "dns",
            "物理 DNS 位于完整捕获排除范围，请选择明确的可捕获上游",
        ));
    }
    let document = serde_json::json!({
        "mtu":mtu.min(u32::from(shared.plan.mtu)), "addresses":addresses_text,
        "routes":routes.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "protected_ips":protected, "capture_mode":mode,
        "dns_capture":true, "physical_dns_servers":dns,
    })
    .to_string();
    crate::platform::model::validate(&document)
        .map_err(|error| failure(&error.code, &error.message))?;
    Ok(document)
}

async fn protected_nodes(shared: &Shared) -> Result<Vec<IpAddr>, VpnFailure> {
    let mut protected = BTreeSet::new();
    for nodes in shared.plan.nodes.values() {
        for node in nodes {
            let (host, _) = sangfor_core::plan::parse_endpoint(node)
                .ok_or_else(|| failure("nodes", "网关节点地址无效"))?;
            protected.extend(crate::network::dns::resolve(&host).await?);
            if protected.len() > 128 {
                return Err(failure("protected", "网关保护地址超过数量限制"));
            }
        }
    }
    if protected.is_empty() {
        return Err(failure("nodes", "网关没有可保护的节点地址"));
    }
    Ok(protected.into_iter().collect())
}

fn resource_networks(host: &str) -> Result<Option<Vec<IpNet>>, VpnFailure> {
    let host = host.trim();
    if let Ok(network) = host.parse::<IpNet>() {
        return Ok(Some(vec![network.trunc()]));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(Some(vec![IpNet::new(
            ip,
            if ip.is_ipv4() { 32 } else { 128 },
        )
        .map_err(|_| failure("route", "资源地址无效"))?]));
    }
    if let Some((low, high)) = authorization::address_range(host) {
        return Ok(Some(match (low, high) {
            (IpAddr::V4(low), IpAddr::V4(high)) => {
                Ipv4Subnets::new(low, high, 0).map(IpNet::V4).collect()
            }
            (IpAddr::V6(low), IpAddr::V6(high)) => {
                Ipv6Subnets::new(low, high, 0).map(IpNet::V6).collect()
            }
            _ => return Err(failure("route", "资源区间地址族不一致")),
        }));
    }
    let invalid_range = host.split_once('-').is_some_and(|(low, high)| {
        low.trim().parse::<IpAddr>().is_ok() && high.trim().parse::<IpAddr>().is_ok()
    });
    if invalid_range || host.contains('~') || host.contains('/') {
        return Err(failure("route", "资源包含无效的地址区间或网段"));
    }
    let domain = host
        .strip_prefix("*.")
        .or_else(|| host.strip_prefix('.'))
        .unwrap_or(host);
    crate::routing::model::normalize_domain(domain)
        .map_err(|_| failure("route", "资源包含无效的地址区间、网段或域名"))?;
    Ok(None)
}

fn exclude_nodes(
    network: IpNet,
    protected: &[IpAddr],
    routes: &mut Vec<String>,
) -> Result<(), VpnFailure> {
    if !protected.iter().any(|ip| network.contains(ip)) {
        routes.push(network.to_string());
        if routes.len() > 256 {
            return Err(failure("capture_limit", "排除网关后的捕获路由超过数量限制"));
        }
        return Ok(());
    }
    let maximum = if network.addr().is_ipv4() { 32 } else { 128 };
    if network.prefix_len() == maximum {
        return Ok(());
    }
    for child in network
        .subnets(network.prefix_len() + 1)
        .map_err(|_| failure("route", "不能拆分资源网段"))?
    {
        exclude_nodes(child, protected, routes)?;
    }
    Ok(())
}

fn failure(code: &str, message: &str) -> VpnFailure {
    VpnFailure::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_ranges_and_control_exclusions_do_not_capture_or_drop_other_addresses(
    ) -> anyhow::Result<()> {
        for separator in ['-', '~'] {
            let networks = resource_networks(&format!("10.0.0.1{separator}10.0.0.254"))?
                .ok_or_else(|| anyhow::anyhow!("range mistaken for domain"))?;
            let protected = ["10.0.0.42".parse()?];
            let mut routes = vec![];
            for network in networks {
                exclude_nodes(network, &protected, &mut routes)?;
            }
            let routes: Vec<IpNet> = routes
                .iter()
                .map(|route| route.parse())
                .collect::<Result<_, _>>()?;
            for last in 0..=255 {
                let ip = std::net::Ipv4Addr::new(10, 0, 0, last);
                let captured = routes.iter().any(|route| route.contains(&IpAddr::V4(ip)));
                assert_eq!(captured, (1..=254).contains(&last) && last != 42);
            }
        }
        assert!(resource_networks("host-name.shu.edu.cn")?.is_none());
        assert!(resource_networks("10.0.0.2-10.0.0.1").is_err());
        Ok(())
    }
}
