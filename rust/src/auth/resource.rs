use super::model::AuthFailure;
use sangfor_core::route::Route;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) struct Resource {
    pub routes: Vec<Route>,
    pub nodes: BTreeMap<String, Vec<String>>,
    pub major: String,
    pub dns: Vec<String>,
    pub warnings: Vec<String>,
}

pub(crate) fn parse(envelope: &Value, server_host: &str) -> Result<Resource, AuthFailure> {
    let data = &envelope["data"];
    let apps = &data["appList"]["data"];
    let config = &apps["config"]["nodeGroupConf"];
    let mut routes = vec![];
    let mut warnings = vec![];
    for info in array(&apps["appInfo"]) {
        for app in array(&info["apps"]) {
            if app["accessModel"].as_str() != Some("L3VPN") {
                continue;
            }
            let app_id = text(&app["id"]);
            let group = text(&app["nodeGroupId"]);
            if app_id.is_empty() {
                warnings.push("跳过缺少资源标识的 L3VPN 应用".to_owned());
                continue;
            }
            for address in array(&app["addressList"]) {
                let protocol = text(&address["protocol"]).to_ascii_lowercase();
                let host = text(&address["host"]);
                let Some((minimum, maximum)) = ports(text(&address["port"])) else {
                    warnings.push("跳过端口无效的资源地址".to_owned());
                    continue;
                };
                if host.is_empty()
                    || host.len() > 512
                    || !matches!(protocol.as_str(), "tcp" | "udp" | "all")
                {
                    warnings.push("跳过协议或目标无效的资源地址".to_owned());
                    continue;
                }
                routes.push(Route {
                    host: host.to_owned(),
                    protocol,
                    port_min: minimum,
                    port_max: maximum,
                    app_id: app_id.to_owned(),
                    node_group_id: group.to_owned(),
                    addr_pretend: app["addrPretend"].as_bool().unwrap_or(true),
                    enable_tcp_pref_l3: app["enableTCPPrefL3"].as_bool().unwrap_or(false),
                });
                if routes.len() > 4096 {
                    return Err(bad("资源地址超过数量上限"));
                }
            }
        }
    }
    let mut nodes = BTreeMap::new();
    let mut first_group = None;
    for group in array(&config["nodeGroupList"]) {
        let id = text(&group["id"]);
        if id.is_empty() || id.len() > 128 {
            continue;
        }
        if first_group.is_none() {
            first_group = Some(id.to_owned());
        }
        let mut wan = vec![];
        let mut lan = vec![];
        for address in array(&group["addressInfo"]) {
            let Some(endpoint) = node(text(&address["address"]), server_host) else {
                warnings.push("跳过节点地址无效的条目".to_owned());
                continue;
            };
            match text(&address["type"]).to_ascii_lowercase().as_str() {
                "wan" => wan.push(endpoint),
                "lan" => lan.push(endpoint),
                _ => {}
            }
        }
        wan.extend(lan);
        if wan.len() > 256 || nodes.len() >= 256 {
            return Err(bad("节点数量超过上限"));
        }
        nodes.insert(id.to_owned(), wan);
    }
    let raw_major = config["majorNodeGroup"]
        .as_str()
        .or_else(|| config["majorNodeGroup"]["id"].as_str())
        .unwrap_or("");
    let major = if nodes.contains_key(raw_major) {
        raw_major.to_owned()
    } else {
        first_group.unwrap_or_default()
    };
    if nodes.get(&major).is_none_or(Vec::is_empty) {
        return Err(bad("网关没有提供可用的主节点组"));
    }
    let options = &data["sdpPolicy"]["data"]["clientOption"];
    let mut dns = vec![];
    for field in ["firstDNS", "secondDNS"] {
        let value = options["dnsOption"][field]
            .as_str()
            .or_else(|| options["dnsOptionV2"][field].as_str())
            .unwrap_or("");
        if !value.is_empty() {
            if value.parse::<std::net::IpAddr>().is_err() {
                warnings.push("跳过无效的校园 DNS 地址".to_owned());
            } else if !dns.contains(&value.to_owned()) {
                dns.push(value.to_owned());
            }
        }
    }
    warnings.truncate(64);
    Ok(Resource {
        routes,
        nodes,
        major,
        dns,
        warnings,
    })
}

fn node(raw: &str, server_host: &str) -> Option<String> {
    let raw = if raw == "{{sdpcHost}}" {
        server_host
    } else {
        raw
    };
    if raw.is_empty()
        || raw.len() > 512
        || raw
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return None;
    }
    let endpoint = if raw.starts_with('[') {
        if raw.contains("]:") {
            raw.to_owned()
        } else {
            format!("{raw}:441")
        }
    } else {
        match raw.matches(':').count() {
            0 => format!("{raw}:441"),
            1 => raw.to_owned(),
            _ => format!("[{raw}]:441"),
        }
    };
    let (host, port) = sangfor_core::plan::parse_endpoint(&endpoint)?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some(endpoint)
}
fn ports(raw: &str) -> Option<(u16, u16)> {
    let mut parts = raw.split('-');
    let low = parts.next()?.parse().ok()?;
    let high = match parts.next() {
        Some(high) => high.parse().ok()?,
        None => low,
    };
    if parts.next().is_some() || high < low {
        None
    } else {
        Some((low, high))
    }
}
fn array(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn bad(message: &str) -> AuthFailure {
    AuthFailure::new("atrustResource", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_fields_ports_dns_fallback_and_ordered_node_groups_match_the_wire_contract(
    ) -> anyhow::Result<()> {
        let envelope = serde_json::json!({"data":{
            "appList":{"data":{"appInfo":[{"apps":[{"accessModel":"L3VPN","id":"app-1","nodeGroupId":"z",
                "enableTCPPrefL3":true,"addressList":[{"host":"10.0.0.1","protocol":"TCP","port":"443-445"}]}]}],
                "config":{"nodeGroupConf":{"majorNodeGroup":"unknown","nodeGroupList":[
                    {"id":"z","addressInfo":[{"address":"{{sdpcHost}}","type":"WAN"},{"address":"2001:db8::2","type":"lan"}]},
                    {"id":"a","addressInfo":[{"address":"10.0.0.3:442","type":"lan"}]}]}}}},
            "sdpPolicy":{"data":{"clientOption":{"dnsOption":{"firstDNS":"10.0.0.53"},
                "dnsOptionV2":{"secondDNS":"2001:db8::53"}}}}}});
        let resource = parse(&envelope, "vpn.example.test")?;
        assert_eq!(resource.major, "z");
        assert_eq!(
            resource.nodes["z"],
            ["vpn.example.test:441", "[2001:db8::2]:441"]
        );
        assert_eq!(resource.routes[0].port_max, 445);
        assert!(resource.routes[0].enable_tcp_pref_l3);
        assert_eq!(resource.dns, ["10.0.0.53", "2001:db8::53"]);
        for group in ["missing", ""] {
            let mut fallback = envelope.clone();
            fallback["data"]["appList"]["data"]["appInfo"][0]["apps"][0]["nodeGroupId"] =
                Value::from(group);
            let resource = parse(&fallback, "vpn.example.test")?;
            assert_eq!(resource.routes.len(), 1);
            assert_eq!(resource.routes[0].node_group_id, group);
            assert_eq!(resource.major, "z");
        }
        Ok(())
    }
}
