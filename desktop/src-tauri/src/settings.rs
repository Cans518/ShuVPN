use crate::error::{CommandError, CommandResult};
use serde::Deserialize;
use serde_json::{json, Value};
use shuvpn_core::api::{
    settings::{Edit, SettingKey, SettingsSnapshot},
    vpn::{CaptureMode, ProxyStartup, TunStartup},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingEdit {
    key: EditableSetting,
    value: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum EditableSetting {
    ThemeMode,
    Server,
    TimeoutSeconds,
    HttpPort,
    HttpListen,
    HttpProxyEnabled,
    SocksPort,
    SocksListen,
    SocksProxyEnabled,
    SystemProxyEnabled,
    VpnEnabled,
    VpnMtu,
    VpnCaptureMode,
}

impl SettingEdit {
    pub fn into_core(self) -> Edit {
        let key = match self.key {
            EditableSetting::ThemeMode => SettingKey::ThemeMode,
            EditableSetting::Server => SettingKey::Server,
            EditableSetting::TimeoutSeconds => SettingKey::TimeoutSeconds,
            EditableSetting::HttpPort => SettingKey::HttpPort,
            EditableSetting::HttpListen => SettingKey::HttpListen,
            EditableSetting::HttpProxyEnabled => SettingKey::HttpProxyEnabled,
            EditableSetting::SocksPort => SettingKey::SocksPort,
            EditableSetting::SocksListen => SettingKey::SocksListen,
            EditableSetting::SocksProxyEnabled => SettingKey::SocksProxyEnabled,
            EditableSetting::SystemProxyEnabled => SettingKey::SystemProxyEnabled,
            EditableSetting::VpnEnabled => SettingKey::VpnEnabled,
            EditableSetting::VpnMtu => SettingKey::VpnMtu,
            EditableSetting::VpnCaptureMode => SettingKey::VpnCaptureMode,
        };
        Edit {
            key,
            value_json: self.value.to_string(),
        }
    }
}

pub fn snapshot(settings: &SettingsSnapshot) -> Value {
    json!({
        "revision": settings.revision,
        "themeMode": settings.theme_mode,
        "server": settings.server,
        "timeoutSeconds": settings.timeout_seconds,
        "httpPort": settings.http_port,
        "httpListen": settings.http_listen,
        "httpProxyEnabled": settings.http_proxy_enabled,
        "socksPort": settings.socks_port,
        "socksListen": settings.socks_listen,
        "socksProxyEnabled": settings.socks_proxy_enabled,
        "systemProxyEnabled": settings.system_proxy_enabled,
        "vpnEnabled": settings.vpn_enabled,
        "vpnMtu": settings.vpn_mtu,
        "vpnCaptureMode": match settings.vpn_capture_mode {
            CaptureMode::Campus => "campus",
            CaptureMode::Full => "full",
        },
    })
}

pub fn proxies(settings: &SettingsSnapshot) -> ProxyStartup {
    ProxyStartup {
        http_enabled: settings.http_proxy_enabled,
        http_address: settings.http_listen.clone(),
        http_port: settings.http_port,
        socks_enabled: settings.socks_proxy_enabled,
        socks_address: settings.socks_listen.clone(),
        socks_port: settings.socks_port,
        system_proxy_enabled: settings.system_proxy_enabled,
    }
}

pub fn tun(settings: &SettingsSnapshot, interface: String, dns: Option<String>) -> TunStartup {
    TunStartup {
        enabled: settings.vpn_enabled,
        mtu: settings.vpn_mtu.into(),
        physical_interface: interface,
        physical_dns: dns,
        capture_mode: Some(settings.vpn_capture_mode),
    }
}

pub fn validate_batch(edits: &[SettingEdit]) -> CommandResult<()> {
    if edits.is_empty() || edits.len() > 32 {
        return Err(CommandError::new("size", "每次保存需要 1 至 32 项设置"));
    }
    Ok(())
}
