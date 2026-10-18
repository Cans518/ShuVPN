use futures_util::TryStreamExt;
use netlink_packet_route::{
    address::{AddressAttribute, AddressFlags, AddressHeaderFlags},
    link::{InfoKind, LinkAttribute, LinkFlags, LinkInfo, LinkLayerType},
    route::{RouteAttribute, RouteHeader, RouteMessage, RouteType},
    rule::{RuleAction, RuleAttribute},
    AddressFamily,
};

pub(super) async fn capture_snapshot() -> Result<super::CaptureSnapshot, VpnFailure> {
    let (connection, handle, _) = rtnetlink::new_connection().map_err(error)?;
    let _worker = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(connection));
    for version in [rtnetlink::IpVersion::V4, rtnetlink::IpVersion::V6] {
        let mut rules = handle.rule().get(version).execute();
        let mut seen = 0;
        let mut standard = std::collections::BTreeSet::new();
        while let Some(rule) = rules.try_next().await.map_err(error)? {
            seen += 1;
            if seen > 32 {
                return Err(error("完整捕获不覆盖自定义策略路由"));
            }
            let table = rule
                .attributes
                .iter()
                .find_map(|item| match item {
                    RuleAttribute::Table(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or(u32::from(rule.header.table));
            let priority = rule
                .attributes
                .iter()
                .find_map(|item| match item {
                    RuleAttribute::Priority(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or(0);
            if rule.header.action != RuleAction::ToTable
                || rule.header.src_len != 0
                || rule.header.dst_len != 0
                || rule.header.tos != 0
                || !rule.header.flags.is_empty()
                || !matches!((priority, table), (0, 255) | (32766, 254) | (32767, 253))
                || rule.attributes.iter().any(|item| {
                    !matches!(
                        item,
                        RuleAttribute::Table(_)
                            | RuleAttribute::Priority(_)
                            | RuleAttribute::Protocol(_)
                            | RuleAttribute::SuppressPrefixLen(u32::MAX)
                    )
                })
                || !standard.insert((priority, table))
            {
                return Err(error(
                    "系统存在自定义策略规则，无法证明完整捕获覆盖，保持网络不变",
                ));
            }
        }
        if !standard.contains(&(0, 255)) || !standard.contains(&(32766, 254)) {
            return Err(error("缺少 local 或 main 策略规则，无法证明完整捕获覆盖"));
        }
    }
    let mut networks = Vec::new();
    let mut route_interfaces = Vec::new();
    let mut local_routes = Vec::new();
    for family in [AddressFamily::Inet, AddressFamily::Inet6] {
        let mut request = RouteMessage::default();
        request.header.address_family = family;
        let mut routes = handle.route().get(request).execute();
        let mut count = 0;
        while let Some(route) = routes.try_next().await.map_err(error)? {
            count += 1;
            if count > 100_000 {
                return Err(error("路由表超过预算"));
            }
            let table = route
                .attributes
                .iter()
                .find_map(|item| match item {
                    RouteAttribute::Table(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or(u32::from(route.header.table));
            if table == 253 {
                return Err(error("default 表中存在额外路由，无法证明完整捕获覆盖"));
            }
            if table != 254 && table != 255 {
                continue;
            }
            if table == 254
                && (route.header.kind != RouteType::Unicast
                    || route.header.source_prefix_length != 0
                    || route.header.tos != 0)
            {
                return Err(error("main 表包含特殊路由策略，保持网络不变"));
            }
            let address = route
                .attributes
                .iter()
                .find_map(|item| match item {
                    RouteAttribute::Destination(address) => match address {
                        netlink_packet_route::route::RouteAddress::Inet(ip) => {
                            Some(std::net::IpAddr::V4(*ip))
                        }
                        netlink_packet_route::route::RouteAddress::Inet6(ip) => {
                            Some(std::net::IpAddr::V6(*ip))
                        }
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or(if family == AddressFamily::Inet {
                    std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
                } else {
                    std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED)
                });
            let network = ipnet::IpNet::new(address, route.header.destination_prefix_length)
                .map_err(error)?
                .trunc();
            if table == 255 {
                local_routes.push(network);
                continue;
            }
            let index = route
                .attributes
                .iter()
                .find_map(|item| match item {
                    RouteAttribute::Oif(index) => Some(*index),
                    _ => None,
                })
                .ok_or_else(|| error("完整捕获不支持无法确定出口的多路径路由"))?;
            networks.push(network);
            route_interfaces.push((network, index));
            if networks.len() > 4096 {
                return Err(error("完整捕获所需系统路由超过预算"));
            }
        }
    }
    let mut addresses = Vec::new();
    let mut sources = Vec::new();
    // Broadcast destinations are local-table routes and must stay local.
    let mut query = handle.address().get().execute();
    while let Some(address) = query.try_next().await.map_err(error)? {
        let local = address
            .attributes
            .iter()
            .find_map(|attribute| match attribute {
                AddressAttribute::Local(ip) => Some(*ip),
                _ => None,
            })
            .or_else(|| {
                address
                    .attributes
                    .iter()
                    .find_map(|attribute| match attribute {
                        AddressAttribute::Address(ip) => Some(*ip),
                        _ => None,
                    })
            });
        if let Some(ip) = local {
            addresses.push(ip);
            sources.push(ip);
            if let std::net::IpAddr::V4(ip) = ip {
                if address.header.prefix_len < 31 {
                    let network =
                        ipnet::Ipv4Net::new(ip, address.header.prefix_len).map_err(error)?;
                    addresses.extend([
                        std::net::IpAddr::V4(network.network()),
                        std::net::IpAddr::V4(network.broadcast()),
                    ]);
                }
            }
        }
        for attribute in address.attributes {
            if let AddressAttribute::Broadcast(ip) = attribute {
                addresses.push(ip.into());
            }
        }
    }
    addresses.sort();
    addresses.dedup();
    crate::platform::capture::validate_local_routes(&local_routes, &addresses).map_err(error)?;
    Ok(super::CaptureSnapshot {
        routes: networks,
        local_addresses: addresses,
        source_addresses: sources,
        route_interfaces,
    })
}

use super::{recommend, DefaultRoute, NetworkInterface};
use crate::connection::model::VpnFailure;

pub(super) async fn lan_addresses() -> Result<Vec<(u32, std::net::IpAddr)>, VpnFailure> {
    let (connection, handle, _) = rtnetlink::new_connection().map_err(error)?;
    let _worker = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(connection));
    let mut query = handle.address().get().execute();
    let mut addresses = Vec::new();
    let mut visited = 0;
    while let Some(message) = query.try_next().await.map_err(error)? {
        visited += 1;
        if visited > 100_000 {
            return Err(error("单播地址表超过数量限制"));
        }
        if message.header.flags.intersects(
            AddressHeaderFlags::Tentative
                | AddressHeaderFlags::Dadfailed
                | AddressHeaderFlags::Deprecated,
        ) || message.attributes.iter().any(|attribute| match attribute {
            AddressAttribute::Flags(flags) => flags.intersects(
                AddressFlags::Tentative | AddressFlags::Dadfailed | AddressFlags::Deprecated,
            ),
            AddressAttribute::CacheInfo(info) => info.ifa_preferred == 0 || info.ifa_valid == 0,
            _ => false,
        }) {
            continue;
        }
        // IPv4 point-to-point peers are in Address; Local is the owned endpoint.
        let address = message
            .attributes
            .iter()
            .find_map(|attribute| match attribute {
                AddressAttribute::Local(address) => Some(*address),
                _ => None,
            })
            .or_else(|| {
                message
                    .attributes
                    .iter()
                    .find_map(|attribute| match attribute {
                        AddressAttribute::Address(address) => Some(*address),
                        _ => None,
                    })
            });
        if let Some(address) = address {
            addresses.push((message.header.index, address));
        }
    }
    Ok(addresses)
}

pub(super) async fn available() -> Result<Vec<NetworkInterface>, VpnFailure> {
    let (connection, handle, _) = rtnetlink::new_connection().map_err(error)?;
    let _worker = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(connection));
    let mut links = handle.link().get().execute();
    let mut choices = vec![];
    let mut visited = 0;
    while let Some(link) = links.try_next().await.map_err(error)? {
        visited += 1;
        if visited > 100_000 {
            return Err(error("接口表超过数量限制"));
        }
        if link.header.link_layer_type != LinkLayerType::Ether
            || !link.header.flags.contains(LinkFlags::Up)
            || link.attributes.iter().any(|attribute| {
                matches!(
                    attribute,
                    LinkAttribute::LinkInfo(values) if values.iter().any(|value|
                        matches!(value, LinkInfo::Kind(InfoKind::Tun | InfoKind::Wireguard)))
                )
            })
        {
            continue;
        }
        let name = link
            .attributes
            .iter()
            .find_map(|attribute| match attribute {
                LinkAttribute::IfName(name) => Some(name),
                _ => None,
            });
        let Some(name) = name else {
            continue;
        };
        if name.starts_with("shuvpn") {
            continue;
        }
        choices.push(NetworkInterface {
            name: name.clone(),
            description: name.clone(),
            index: link.header.index,
            recommended: false,
        });
    }
    let mut defaults = vec![];
    for family in [AddressFamily::Inet, AddressFamily::Inet6] {
        let mut query = RouteMessage::default();
        query.header.address_family = family;
        let mut routes = handle.route().get(query).execute();
        let mut visited = 0;
        while let Some(route) = routes.try_next().await.map_err(error)? {
            visited += 1;
            if visited > 100_000 {
                return Err(error("路由表超过数量限制"));
            }
            let table = route
                .attributes
                .iter()
                .find_map(|attribute| match attribute {
                    RouteAttribute::Table(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or(u32::from(route.header.table));
            if table != u32::from(RouteHeader::RT_TABLE_MAIN)
                || route.header.destination_prefix_length != 0
                || route.header.source_prefix_length != 0
                || route.header.tos != 0
                || route.header.kind != RouteType::Unicast
            {
                continue;
            }
            let index = route
                .attributes
                .iter()
                .find_map(|attribute| match attribute {
                    RouteAttribute::Oif(index) => Some(*index),
                    _ => None,
                });
            let metric = route
                .attributes
                .iter()
                .find_map(|attribute| match attribute {
                    RouteAttribute::Priority(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or(0);
            if let Some(index) = index {
                defaults.push(DefaultRoute {
                    ipv4: family == AddressFamily::Inet,
                    index,
                    metric: u64::from(metric),
                });
            }
        }
    }
    recommend(&mut choices, defaults)?;
    Ok(choices)
}

fn error(cause: impl std::fmt::Display) -> VpnFailure {
    VpnFailure::new("interface", &format!("读取系统网络接口失败：{cause}"))
}
