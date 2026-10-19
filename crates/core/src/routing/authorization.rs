use super::model::Network;
use sangfor_core::route::Route;
use std::net::IpAddr;

pub(crate) fn domain_match(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim();
    let Ok(host) = super::model::normalize_domain(host) else {
        return false;
    };
    if let Some(suffix) = pattern
        .strip_prefix("*.")
        .or_else(|| pattern.strip_prefix('.'))
    {
        let Ok(suffix) = super::model::normalize_domain(suffix) else {
            return false;
        };
        return host.ends_with(&format!(".{suffix}"));
    }
    super::model::normalize_domain(pattern).is_ok_and(|pattern| pattern == host)
}

pub(crate) fn address_match(pattern: &str, address: IpAddr) -> bool {
    let pattern = pattern.trim();
    if let Ok(network) = pattern.parse::<ipnet::IpNet>() {
        return network.contains(&address);
    }
    if let Ok(literal) = pattern.parse::<IpAddr>() {
        return literal == address;
    }
    if let Some((low, high)) = address_range(pattern) {
        return address.is_ipv4() == low.is_ipv4() && low <= address && address <= high;
    }
    match address {
        IpAddr::V4(address) => sangfor_core::route::host_covers(pattern, u32::from(address)),
        IpAddr::V6(_) => false,
    }
}

pub(crate) fn address_range(pattern: &str) -> Option<(IpAddr, IpAddr)> {
    let (low, high) = pattern
        .split_once('~')
        .or_else(|| pattern.split_once('-'))?;
    let low = low.trim().parse::<IpAddr>().ok()?;
    let high = high.trim().parse::<IpAddr>().ok()?;
    (low.is_ipv4() == high.is_ipv4() && low <= high).then_some((low, high))
}

pub(crate) fn route_for<'a>(
    routes: &'a [Route],
    host: &str,
    port: u16,
    network: Network,
) -> Option<&'a Route> {
    let address = host.parse::<IpAddr>().ok();
    [false, true].into_iter().find_map(|preferred| {
        routes.iter().find(|route| {
            let protocol = match network {
                Network::Tcp => "tcp",
                Network::Udp => "udp",
            };
            (route.protocol == protocol || route.protocol == "all")
                && route.port_min <= port
                && port <= route.port_max
                && (preferred || network != Network::Tcp || !route.enable_tcp_pref_l3)
                && match address {
                    Some(address) => address_match(&route.host, address),
                    None => domain_match(&route.host, host),
                }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_address_ranges_preserve_both_delimiters_families_and_bounds() -> anyhow::Result<()> {
        for separator in ['-', '~'] {
            for (low, high, inside, outside) in [
                ("10.0.0.1", "10.0.3.254", "10.0.2.1", "10.0.3.255"),
                (
                    "2001:db8::1",
                    "2001:db8::ff",
                    "2001:db8::80",
                    "2001:db8::100",
                ),
            ] {
                let pattern = format!("{low}{separator}{high}");
                assert!(address_match(&pattern, low.parse()?));
                assert!(address_match(&pattern, high.parse()?));
                assert!(address_match(&pattern, inside.parse()?));
                assert!(!address_match(&pattern, outside.parse()?));
            }
        }
        assert!(!address_match("10.0.0.2-10.0.0.1", "10.0.0.1".parse()?));
        assert!(!address_match("10.0.0.1-2001:db8::1", "10.0.0.2".parse()?));
        assert!(address_range("host-name.example").is_none());
        Ok(())
    }
}
