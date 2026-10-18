use super::model::{failure, Defaults, SettingKey, SettingsFailure, SettingsSnapshot};
use serde_json::{Map, Value};
use std::net::{IpAddr, Ipv4Addr};

pub(super) const VERSION: &str = "settings.schema.version";
pub(super) type Values = Map<String, Value>;

pub(super) fn migrate(raw: &str) -> Result<Values, SettingsFailure> {
    if raw.len() > 64 * 1024 {
        return Err(failure("size", "旧设置超过大小限制"));
    }
    let input: Value =
        serde_json::from_str(raw).map_err(|_| failure("format", "旧设置格式无效"))?;
    let mut values: Values = input
        .as_object()
        .ok_or_else(|| failure("format", "旧设置必须是对象"))?
        .iter()
        .filter(|(key, _)| key.starts_with("settings."))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if values.len() > 256 {
        return Err(failure("size", "设置条目过多"));
    }
    let from = values
        .get(VERSION)
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| failure("schema", "设置版本无效"))
        })
        .transpose()?
        .unwrap_or(0);
    if from < 1 {
        values.remove("settings.lastServer");
    }
    if from < 2 {
        values.remove("settings.proxyDirectFallback");
    }
    if (1..3).contains(&from) {
        values.insert("settings.welcomeCompleted".into(), Value::Bool(true));
    }
    if from < 4 {
        values.remove("settings.vpnTcpOverL3");
        values.insert(VERSION.into(), Value::from(4));
    }
    // Match legacy read fallbacks without modifying unrelated or future keys.
    for key in SettingKey::ALL {
        if let Some(value) = values.get(key.storage_key()).cloned() {
            let normalized = match key {
                SettingKey::SocksListen | SettingKey::HttpListen => {
                    let raw = value
                        .as_str()
                        .ok_or_else(|| failure("type", "监听地址类型无效"))?;
                    Value::String(match raw {
                        "any" => "0.0.0.0".into(),
                        "loopback" => "127.0.0.1".into(),
                        _ => raw
                            .trim()
                            .parse::<IpAddr>()
                            .map(|ip| ip.to_string())
                            .unwrap_or_else(|_| "127.0.0.1".into()),
                    })
                }
                SettingKey::ThemeMode if value.as_str().is_some() => Value::String(
                    match value.as_str() {
                        Some("light") => "light",
                        Some("dark") => "dark",
                        _ => "system",
                    }
                    .into(),
                ),
                SettingKey::LogLevel if value.as_str().is_some() => Value::String(
                    match value.as_str().unwrap_or("").to_uppercase().as_str() {
                        "ERROR" => "ERROR",
                        "WARN" => "WARN",
                        "DEBUG" => "DEBUG",
                        _ => "INFO",
                    }
                    .into(),
                ),
                _ => normalize(key, value)?,
            };
            values.insert(key.storage_key().into(), normalized);
        }
    }
    if values
        .get("settings.systemProxyEnabled")
        .and_then(Value::as_bool)
        == Some(true)
    {
        values.insert("settings.httpProxyEnabled".into(), Value::Bool(true));
    }
    Ok(values)
}

pub(super) fn normalize(key: SettingKey, value: Value) -> Result<Value, SettingsFailure> {
    use SettingKey::*;
    let invalid = || failure("validation", "设置值无效");
    match key {
        SocksPort | HttpPort | VpnMtu | TimeoutSeconds => {
            let number = value.as_u64().ok_or_else(invalid)?;
            let valid = match key {
                SocksPort | HttpPort => number <= 65535,
                VpnMtu => (1280..=9000).contains(&number),
                TimeoutSeconds => (1..=3600).contains(&number),
                _ => false,
            };
            if !valid {
                return Err(invalid());
            }
            Ok(Value::from(number))
        }
        ThemeMode | SocksListen | HttpListen | VpnDns | Server | LoginDomain | LogLevel
        | VpnCaptureMode => {
            let raw = value.as_str().ok_or_else(invalid)?;
            if raw.len() > 1024 || raw.chars().any(char::is_control) {
                return Err(invalid());
            }
            let text = raw.trim();
            let normalized = match key {
                ThemeMode if matches!(text, "system" | "light" | "dark") => text.to_owned(),
                VpnCaptureMode if matches!(text, "campus" | "full") => text.to_owned(),
                LogLevel if matches!(text, "ERROR" | "WARN" | "INFO" | "DEBUG") => text.to_owned(),
                SocksListen | HttpListen => text
                    .parse::<IpAddr>()
                    .map_err(|_| failure("validation", "请填写 IP 监听地址"))?
                    .to_string(),
                VpnDns if text.is_empty() => String::new(),
                VpnDns => {
                    let octets = text
                        .split('.')
                        .map(str::parse::<u8>)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| failure("validation", "请填 IPv4 地址"))?;
                    let octets: [u8; 4] = octets
                        .try_into()
                        .map_err(|_| failure("validation", "请填 IPv4 地址"))?;
                    Ipv4Addr::from(octets).to_string()
                }
                Server if !text.is_empty() && !text.chars().any(char::is_whitespace) => {
                    text.to_owned()
                }
                LoginDomain if !text.is_empty() => text.to_owned(),
                _ => return Err(invalid()),
            };
            Ok(Value::String(normalized))
        }
        _ => {
            if value.is_boolean() {
                Ok(value)
            } else {
                Err(invalid())
            }
        }
    }
}

pub(super) fn snapshot(values: &Values, revision: u32, defaults: Defaults) -> SettingsSnapshot {
    let text = |key: &str, fallback: &str| {
        values
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_owned()
    };
    let boolean =
        |key: &str, fallback| values.get(key).and_then(Value::as_bool).unwrap_or(fallback);
    let integer = |key: &str, fallback| values.get(key).and_then(Value::as_u64).unwrap_or(fallback);
    SettingsSnapshot {
        revision,
        schema_version: integer(VERSION, 4) as u32,
        theme_mode: text("settings.themeMode", "system"),
        welcome_completed: boolean("settings.welcomeCompleted", false),
        socks_port: integer("settings.socksPort", 2233) as u16,
        socks_listen: text("settings.socksListen", "127.0.0.1"),
        socks_proxy_enabled: boolean("settings.autoStartProxy", false),
        http_proxy_enabled: boolean("settings.httpProxyEnabled", false),
        system_proxy_enabled: boolean("settings.systemProxyEnabled", false),
        http_listen: text("settings.httpListen", "127.0.0.1"),
        http_port: integer("settings.httpPort", 3322) as u16,
        vpn_enabled: boolean("settings.vpnEnabled", false),
        vpn_mtu: integer("settings.vpnMtu", 1400) as u16,
        vpn_dns: text("settings.vpnDns", ""),
        vpn_tcp_window_scaling: boolean("settings.vpnTcpWindowScaling", true),
        timeout_seconds: integer("settings.timeoutSeconds", 60) as u32,
        server: text("settings.server", "atrust.shu.edu.cn"),
        login_domain: text("settings.loginDomain", "customOAuth76881"),
        log_enabled: boolean("settings.logEnabled", defaults.debug_logging),
        log_level: text("settings.logLevel", "INFO"),
        enabled_protocols: if boolean("settings.protocol.atrust", true) {
            vec!["atrust".into()]
        } else {
            vec![]
        },
        vpn_capture_mode: if text("settings.vpnCaptureMode", "campus") == "full" {
            crate::platform::model::CaptureMode::Full
        } else {
            crate::platform::model::CaptureMode::Campus
        },
    }
}
