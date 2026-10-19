use super::{
    authorization,
    compiler::{Compiled, Predicate, Rule, Term},
    model::*,
};
use sangfor_core::route::Route;

#[derive(Clone, Debug)]
pub enum Evaluation {
    Decided(RouteDecision),
    NeedAddress,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Match {
    Yes,
    No,
    NeedAddress,
    NeedProcess,
    NeedDomain,
}

pub fn evaluate(
    compiled: Option<&Compiled>,
    routes: &[Route],
    version: u64,
    flow: &Flow,
) -> Result<Evaluation, RoutingFailure> {
    if flow.port == 0 || flow.ips.len() > 64 || flow.domain_candidates.len() > 32 {
        return Err(RoutingFailure::new("flow", "流元数据无效"));
    }
    if let Some(process) = &flow.process {
        if process.name.len() > 1024
            || process.path.len() > 32768
            || process.source.len() > 64
            || [&process.name, &process.path, &process.source]
                .iter()
                .any(|value| value.chars().any(char::is_control))
        {
            return Err(RoutingFailure::new("process", "进程元数据无效"));
        }
    }
    let mut flow = flow.clone();
    for name in &mut flow.domain_candidates {
        *name = normalize_domain(name)?;
    }
    if cfg!(target_os = "windows") {
        if let Some(process) = &mut flow.process {
            process.name = process.name.to_lowercase();
            process.path = process.path.replace('/', "\\").to_lowercase();
        }
    }
    if let Ok(address) = flow.host.parse::<std::net::IpAddr>() {
        if !flow.ips.contains(&address) {
            flow.ips.insert(0, address);
        }
    } else {
        flow.host = normalize_domain(&flow.host)?;
    }
    let Some(compiled) = compiled else {
        return Ok(Evaluation::Decided(RouteDecision {
            version,
            rule_id: "legacy".to_owned(),
            source: "legacy".to_owned(),
            policy: "SHU-VPN".to_owned(),
            action: Action::ShuVpn,
            reason: "保持已有严格校园代理语义".to_owned(),
        }));
    };
    for (index, rule) in compiled.rules.iter().enumerate() {
        if index == compiled.campus_offset && compiled.automatic_campus {
            if !flow.domain_candidates.is_empty()
                && flow.domain_candidates.iter().any(|name| {
                    authorization::route_for(routes, name, flow.port, flow.network).is_some()
                })
                && !flow.ips.iter().any(|ip| {
                    authorization::route_for(routes, &ip.to_string(), flow.port, flow.network)
                        .is_some()
                })
            {
                return Err(RoutingFailure::new(
                    "dns_ambiguous",
                    "DNS 候选包含校园授权名称，不能伪造实际目标域名",
                ));
            }
            if let Some(decision) = auto_campus(routes, version, &flow) {
                return Ok(decision);
            }
        }
        match match_term(&rule.term, &flow, false) {
            Match::Yes => return Ok(Evaluation::Decided(decision(rule, version))),
            Match::NeedAddress => return Ok(Evaluation::NeedAddress),
            Match::NeedProcess => {
                return Err(RoutingFailure::new(
                    "process",
                    "规则需要本机进程身份，但当前入站、权限或套接字快照无法提供唯一身份",
                ))
            }
            Match::NeedDomain => {
                return Err(RoutingFailure::new(
                    "dns_ambiguous",
                    "多个有效 DNS 名称对规则产生不同结果，不能猜测域名",
                ))
            }
            Match::No => {}
        }
    }
    Err(RoutingFailure::new("match", "配置没有最终决策"))
}

fn decision(rule: &Rule, version: u64) -> RouteDecision {
    RouteDecision {
        version,
        rule_id: rule.id.clone(),
        source: rule.source.clone(),
        policy: rule.policy.clone(),
        action: rule.action,
        reason: "首条命中规则".to_owned(),
    }
}

fn auto_campus(routes: &[Route], version: u64, flow: &Flow) -> Option<Evaluation> {
    let matched =
        authorization::route_for(routes, &flow.host, flow.port, flow.network).or_else(|| {
            flow.ips.iter().find_map(|address| {
                authorization::route_for(routes, &address.to_string(), flow.port, flow.network)
            })
        });
    if let Some(route) = matched {
        return Some(Evaluation::Decided(RouteDecision {
            version,
            rule_id: format!("campus:{}", route.app_id),
            source: "campus".to_owned(),
            policy: "SHU-VPN".to_owned(),
            action: Action::ShuVpn,
            reason: "网关自动校园规则".to_owned(),
        }));
    }
    if flow.ips.is_empty()
        && routes.iter().any(|route| {
            (route.protocol == "all"
                || route.protocol
                    == match flow.network {
                        Network::Tcp => "tcp",
                        Network::Udp => "udp",
                    })
                && route.port_min <= flow.port
                && flow.port <= route.port_max
                && (route.host.contains('/')
                    || route.host.contains('~')
                    || route.host.parse::<std::net::IpAddr>().is_ok())
        })
    {
        return Some(Evaluation::NeedAddress);
    }
    None
}

fn match_term(term: &Term, flow: &Flow, parent_no_resolve: bool) -> Match {
    let no_resolve = term.no_resolve || parent_no_resolve;
    let domain = flow
        .host
        .parse::<std::net::IpAddr>()
        .is_err()
        .then_some(flow.host.as_str());
    let yes = match &term.predicate {
        Predicate::Domain(value) => return domain_condition(domain, flow, |host| host == value),
        Predicate::Suffix(value) => {
            return domain_condition(domain, flow, |host| {
                host == value || host.ends_with(&format!(".{value}"))
            })
        }
        Predicate::Keyword(value) => {
            return domain_condition(domain, flow, |host| host.contains(value))
        }
        Predicate::Wildcard(pattern) => {
            return domain_condition(domain, flow, |host| wildcard(pattern, host))
        }
        Predicate::Regex(regex) => {
            return domain_condition(domain, flow, |host| regex.is_match(host))
        }
        Predicate::GeoSite(index) => {
            return domain_condition(domain, flow, |host| index.matches(host))
        }
        Predicate::GeoIp(index) => {
            if flow.ips.is_empty() {
                return if no_resolve {
                    Match::No
                } else {
                    Match::NeedAddress
                };
            }
            index.matches(&flow.ips)
        }
        Predicate::Ip(network) => {
            if flow.ips.is_empty() {
                return if no_resolve {
                    Match::No
                } else {
                    Match::NeedAddress
                };
            }
            flow.ips.iter().any(|ip| network.contains(ip))
        }
        Predicate::Ports(ports) => ports
            .iter()
            .any(|(min, max)| *min <= flow.port && flow.port <= *max),
        Predicate::Network(value) => flow.network == *value,
        Predicate::Inbound(values) => values.contains(&flow.inbound),
        Predicate::ProcessName(expected) | Predicate::ProcessPath(expected) => {
            let value = flow.process.as_ref().and_then(|process| {
                let value = if matches!(&term.predicate, Predicate::ProcessName(_)) {
                    if process.name.is_empty() {
                        std::path::Path::new(&process.path).file_name()?.to_str()?
                    } else {
                        &process.name
                    }
                } else {
                    &process.path
                };
                (!value.is_empty()).then_some(value)
            });
            let Some(value) = value else {
                return Match::NeedProcess;
            };
            expected == value
        }
        Predicate::Match => true,
        Predicate::Not(child) => {
            return match match_term(child, flow, no_resolve) {
                Match::Yes => Match::No,
                Match::No => Match::Yes,
                Match::NeedAddress => Match::NeedAddress,
                Match::NeedProcess => Match::NeedProcess,
                Match::NeedDomain => Match::NeedDomain,
            }
        }
        Predicate::And(terms) => {
            let mut pending = false;
            let mut process_pending = false;
            let mut domain_pending = false;
            for term in terms {
                match match_term(term, flow, no_resolve) {
                    Match::No => return Match::No,
                    Match::NeedAddress => pending = true,
                    Match::NeedProcess => process_pending = true,
                    Match::NeedDomain => domain_pending = true,
                    Match::Yes => {}
                }
            }
            return if pending {
                Match::NeedAddress
            } else if process_pending {
                Match::NeedProcess
            } else if domain_pending {
                Match::NeedDomain
            } else {
                Match::Yes
            };
        }
        Predicate::Or(terms) => {
            let mut pending = false;
            let mut process_pending = false;
            let mut domain_pending = false;
            for term in terms {
                match match_term(term, flow, no_resolve) {
                    Match::Yes => return Match::Yes,
                    Match::NeedAddress => pending = true,
                    Match::NeedProcess => process_pending = true,
                    Match::NeedDomain => domain_pending = true,
                    Match::No => {}
                }
            }
            return if pending {
                Match::NeedAddress
            } else if process_pending {
                Match::NeedProcess
            } else if domain_pending {
                Match::NeedDomain
            } else {
                Match::No
            };
        }
        Predicate::Set(terms) => {
            let mut pending = false;
            let mut process_pending = false;
            let mut domain_pending = false;
            for term in terms.iter() {
                match match_term(term, flow, no_resolve) {
                    Match::Yes => return Match::Yes,
                    Match::NeedAddress => pending = true,
                    Match::NeedProcess => process_pending = true,
                    Match::NeedDomain => domain_pending = true,
                    Match::No => {}
                }
            }
            return if pending {
                Match::NeedAddress
            } else if process_pending {
                Match::NeedProcess
            } else if domain_pending {
                Match::NeedDomain
            } else {
                Match::No
            };
        }
    };
    if yes {
        Match::Yes
    } else {
        Match::No
    }
}

fn domain_condition(domain: Option<&str>, flow: &Flow, predicate: impl Fn(&str) -> bool) -> Match {
    if let Some(domain) = domain {
        return if predicate(domain) {
            Match::Yes
        } else {
            Match::No
        };
    }
    let mut values = flow.domain_candidates.iter().map(|name| predicate(name));
    let Some(first) = values.next() else {
        return Match::No;
    };
    if values.any(|value| value != first) {
        return Match::NeedDomain;
    }
    if first {
        Match::Yes
    } else {
        Match::No
    }
}

fn wildcard(pattern: &str, host: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("**.") {
        return host.ends_with(&format!(".{suffix}"));
    }
    let pattern: Vec<_> = pattern.split('.').collect();
    let host: Vec<_> = host.split('.').collect();
    pattern.len() == host.len()
        && pattern
            .iter()
            .zip(host)
            .all(|(pattern, host)| *pattern == "*" || *pattern == host)
}
