//! Complete unicast coverage without replacing any pre-existing route.
use super::model::PlatformFailure;
use ipnet::IpNet;
use std::net::IpAddr;

pub(crate) const FULL_ROUTE_LIMIT: usize = 1024;
const NODE_LIMIT: usize = 100_000;

fn reserved() -> Result<Vec<IpNet>, PlatformFailure> {
    [
        "0.0.0.0/8",
        "127.0.0.0/8",
        "169.254.0.0/16",
        "224.0.0.0/3",
        "::/128",
        "::1/128",
        "::ffff:0:0/96",
        "fe80::/10",
        "ff00::/8",
    ]
    .iter()
    .map(|value| value.parse().map_err(|_| error("保留地址范围无效")))
    .collect()
}

fn excluded(protected: &[IpAddr]) -> Result<Vec<IpNet>, PlatformFailure> {
    let mut result = reserved()?;
    for address in protected {
        result.push(
            IpNet::new(*address, if address.is_ipv4() { 32 } else { 128 })
                .map_err(|_| error("保护地址无效"))?,
        );
    }
    Ok(IpNet::aggregate(&result))
}

fn walk(
    network: IpNet,
    excluded: &[IpNet],
    existing: &[IpNet],
    output: &mut Vec<IpNet>,
    visited: &mut usize,
) -> Result<(), PlatformFailure> {
    *visited += 1;
    if *visited > NODE_LIMIT {
        return Err(error("完整捕获路由超过数量或计算预算"));
    }
    if excluded.iter().any(|item| item.contains(&network)) {
        return Ok(());
    }
    let partial_exclusion = excluded.iter().any(|item| network.contains(item));
    let shadowed = existing.iter().any(|item| network.contains(item));
    if partial_exclusion || shadowed {
        if network.prefix_len() == if network.addr().is_ipv4() { 32 } else { 128 } {
            return Err(error("现有主机路由无法被独占捕获，保持系统路由不变"));
        }
        for child in network
            .subnets(network.prefix_len() + 1)
            .map_err(|_| error("不能拆分捕获网段"))?
        {
            walk(child, excluded, existing, output, visited)?;
        }
    } else {
        output.push(network);
        if output.len() > FULL_ROUTE_LIMIT {
            return Err(error("完整捕获路由超过数量预算"));
        }
    }
    Ok(())
}

/// A more-specific TUN partition shadows even connected LAN routes. Existing
/// host routes that are not explicit exclusions make complete coverage impossible.
pub(crate) fn full_routes(
    protected: &[IpAddr],
    existing: &[IpNet],
) -> Result<Vec<IpNet>, PlatformFailure> {
    if protected.len() > 256 || existing.len() > 4096 {
        return Err(error("系统路由或保护地址超过预算"));
    }
    let excluded = excluded(protected)?;
    let mut result = Vec::new();
    let mut visited = 0;
    for network in ["0.0.0.0/0", "::/0"] {
        walk(
            network.parse().map_err(|_| error("地址族范围无效"))?,
            &excluded,
            existing,
            &mut result,
            &mut visited,
        )?;
    }
    Ok(result)
}

/// Verify the declared union, not just a sum of prefixes (overlaps must not
/// disguise holes). Physical route precedence is checked again by the creator.
pub(crate) fn validate_full(routes: &[IpNet], protected: &[IpAddr]) -> Result<(), PlatformFailure> {
    let expected = full_routes(protected, &[])?;
    let actual = routes.to_vec();
    for (index, network) in routes.iter().enumerate() {
        if routes[..index]
            .iter()
            .any(|previous| previous.contains(network) || network.contains(previous))
        {
            return Err(error("完整捕获分区不能重叠或重复"));
        }
    }
    if IpNet::aggregate(&actual) != IpNet::aggregate(&expected) {
        return Err(error("完整捕获必须准确覆盖保护范围以外的可路由单播地址"));
    }
    Ok(())
}

pub(crate) fn validate_precedence(
    routes: &[IpNet],
    existing: &[IpNet],
) -> Result<(), PlatformFailure> {
    if existing.len() > 4096 {
        return Err(error("系统路由超过预算"));
    }
    if routes
        .iter()
        .any(|route| existing.iter().any(|old| route.contains(old)))
    {
        return Err(error("现有路由会遮蔽完整捕获分区，保持系统路由不变"));
    }
    Ok(())
}

/// The local table is consulted before main, regardless of prefix length.
/// Only destinations already excluded from capture may live there.
#[cfg(target_os = "linux")]
pub(crate) fn validate_local_routes(
    routes: &[IpNet],
    local_addresses: &[IpAddr],
) -> Result<(), PlatformFailure> {
    let exclusions = excluded(local_addresses)?;
    if routes
        .iter()
        .any(|route| !exclusions.iter().any(|excluded| excluded.contains(route)))
    {
        return Err(error("local 表存在额外目的路由，无法证明完整捕获覆盖"));
    }
    Ok(())
}

pub(crate) fn validate_system(
    plan: &super::model::ValidatedTunPlan,
    routes: &[IpNet],
    local_addresses: &[IpAddr],
) -> Result<(), PlatformFailure> {
    validate_precedence(&plan.routes, routes)?;
    let reserved = reserved()?;
    if local_addresses.iter().any(|address| {
        !reserved.iter().any(|network| network.contains(address))
            && !plan.protected_ips.contains(address)
            && !plan
                .addresses
                .iter()
                .any(|network| network.addr() == *address)
    }) {
        return Err(error("完整捕获计划没有明确保留全部本机地址"));
    }
    Ok(())
}

pub(crate) fn validate_live(
    plan: &super::model::ValidatedTunPlan,
    index: u32,
    routes: &[(IpNet, u32)],
    local_addresses: &[IpAddr],
) -> Result<(), PlatformFailure> {
    let expected: std::collections::BTreeSet<_> = plan.routes.iter().copied().collect();
    let mut owned = std::collections::BTreeMap::<IpNet, usize>::new();
    let mut other = Vec::new();
    for (network, interface) in routes {
        if *interface == index && expected.contains(network) {
            *owned.entry(*network).or_default() += 1;
        } else {
            other.push(*network);
        }
    }
    if expected
        .iter()
        .any(|network| owned.get(network) != Some(&1))
    {
        return Err(error("完整捕获路由丢失、重复或被替换"));
    }
    validate_system(plan, &other, local_addresses)
}

fn error(message: &str) -> PlatformFailure {
    PlatformFailure::new("capture", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(target_os = "linux")]
    fn local_table_can_only_route_destinations_excluded_from_capture() -> anyhow::Result<()> {
        let addresses = ["192.0.2.1".parse()?, "192.0.2.255".parse()?];
        validate_local_routes(
            &[
                "127.0.0.0/8".parse()?,
                "::1/128".parse()?,
                "ff00::/8".parse()?,
                "192.0.2.1/32".parse()?,
                "192.0.2.255/32".parse()?,
            ],
            &addresses,
        )?;
        for route in ["198.51.100.7/32", "192.0.2.0/24", "2001:db8:21::/64"] {
            assert!(validate_local_routes(&[route.parse()?], &addresses).is_err());
        }
        Ok(())
    }

    #[test]
    fn live_guard_rejects_missing_replaced_and_new_shadowing_routes() -> anyhow::Result<()> {
        let excluded = vec!["192.0.2.1".parse()?, "10.99.0.2".parse()?];
        let physical: IpNet = "192.0.2.0/24".parse()?;
        let routes = full_routes(&excluded, &[physical])?;
        let document = serde_json::json!({
            "mtu":1400, "addresses":["10.99.0.2/32"], "protected_ips":["192.0.2.1"],
            "capture_mode":"full", "dns_capture":true, "physical_dns_servers":["192.0.2.53"],
            "routes":routes.iter().map(ToString::to_string).collect::<Vec<_>>(),
        })
        .to_string();
        let plan = super::super::model::validate(&document)?;
        let mut actual: Vec<_> = routes.iter().map(|route| (*route, 23)).collect();
        actual.push((physical, 2));
        validate_live(&plan, 23, &actual, &["192.0.2.1".parse()?])?;
        let mut missing = actual.clone();
        missing.remove(0);
        assert!(validate_live(&plan, 23, &missing, &[]).is_err());
        let mut replaced = actual.clone();
        replaced[0].1 = 99;
        assert!(validate_live(&plan, 23, &replaced, &[]).is_err());
        actual.push(("198.51.100.33/32".parse()?, 2));
        assert!(validate_live(&plan, 23, &actual, &[]).is_err());
        Ok(())
    }

    #[test]
    fn full_cover_preserves_nodes_and_local_traffic_but_shadows_connected_routes(
    ) -> anyhow::Result<()> {
        let protected = vec!["192.0.2.10".parse()?, "2001:db8::10".parse()?];
        let existing = vec![
            "0.0.0.0/0".parse()?,
            "192.0.2.0/24".parse()?,
            "::/0".parse()?,
            "2001:db8::/64".parse()?,
        ];
        let routes = full_routes(&protected, &existing)?;
        validate_full(&routes, &protected)?;
        for address in [
            "8.8.8.8",
            "192.0.2.11",
            "2001:db8::11",
            "2606:4700:4700::1111",
        ] {
            let address: IpAddr = address.parse()?;
            assert!(routes.iter().any(|network| network.contains(&address)));
        }
        for address in [
            "127.0.0.1",
            "169.254.1.1",
            "224.0.0.1",
            "::1",
            "fe80::1",
            "ff02::1",
            "192.0.2.10",
            "2001:db8::10",
        ] {
            let address: IpAddr = address.parse()?;
            assert!(!routes.iter().any(|network| network.contains(&address)));
        }
        let lan: IpAddr = "192.0.2.11".parse()?;
        assert!(routes
            .iter()
            .filter(|network| network.contains(&lan))
            .all(|network| network.prefix_len() > 24));
        validate_precedence(&routes, &existing)?;
        Ok(())
    }

    #[test]
    fn foreign_host_routes_holes_and_protected_overlaps_cannot_be_called_full() -> anyhow::Result<()>
    {
        assert!(full_routes(&[], &["198.51.100.7/32".parse()?]).is_err());
        assert!(full_routes(&["198.51.100.7".parse()?], &["198.51.100.7/32".parse()?]).is_ok());
        let protected = vec!["192.0.2.10".parse()?];
        let mut routes = full_routes(&protected, &[])?;
        routes.pop();
        assert!(validate_full(&routes, &protected).is_err());
        assert!(validate_full(&["0.0.0.0/0".parse()?, "::/0".parse()?], &protected).is_err());
        Ok(())
    }

    #[test]
    fn full_cover_rejects_parent_child_overlap_even_when_the_union_is_unchanged(
    ) -> anyhow::Result<()> {
        let routes = full_routes(&[], &[])?;
        for child in ["1.2.3.4/32", "2606:4700:4700::1111/128"] {
            let child: IpNet = child.parse()?;
            assert!(routes
                .iter()
                .any(|parent| parent != &child && parent.contains(&child)));
            let mut overlapping = routes.clone();
            overlapping.push(child);
            assert_eq!(IpNet::aggregate(&overlapping), IpNet::aggregate(&routes));
            let failure = validate_full(&overlapping, &[])
                .err()
                .ok_or_else(|| anyhow::anyhow!("overlap must be rejected"))?;
            assert_eq!(failure.code, "capture");
            assert_eq!(failure.message, "完整捕获分区不能重叠或重复");
        }
        Ok(())
    }
}
