#[derive(Clone, Copy)]
pub struct Defaults {
    pub debug_logging: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    ThemeMode,
    WelcomeCompleted,
    SocksPort,
    SocksListen,
    SocksProxyEnabled,
    HttpProxyEnabled,
    SystemProxyEnabled,
    HttpListen,
    HttpPort,
    VpnEnabled,
    VpnMtu,
    VpnDns,
    VpnTcpWindowScaling,
    TimeoutSeconds,
    Server,
    LoginDomain,
    LogEnabled,
    LogLevel,
    AtrustEnabled,
    EasyConnectEnabled,
    OpenVpnEnabled,
    VpnCaptureMode,
}
impl SettingKey {
    pub(crate) const ALL: [Self; 22] = [
        Self::ThemeMode,
        Self::WelcomeCompleted,
        Self::SocksPort,
        Self::SocksListen,
        Self::SocksProxyEnabled,
        Self::HttpProxyEnabled,
        Self::SystemProxyEnabled,
        Self::HttpListen,
        Self::HttpPort,
        Self::VpnEnabled,
        Self::VpnMtu,
        Self::VpnDns,
        Self::VpnTcpWindowScaling,
        Self::TimeoutSeconds,
        Self::Server,
        Self::LoginDomain,
        Self::LogEnabled,
        Self::LogLevel,
        Self::AtrustEnabled,
        Self::EasyConnectEnabled,
        Self::OpenVpnEnabled,
        Self::VpnCaptureMode,
    ];
    pub(crate) fn storage_key(self) -> &'static str {
        match self {
            Self::ThemeMode => "settings.themeMode",
            Self::WelcomeCompleted => "settings.welcomeCompleted",
            Self::SocksPort => "settings.socksPort",
            Self::SocksListen => "settings.socksListen",
            Self::SocksProxyEnabled => "settings.autoStartProxy",
            Self::HttpProxyEnabled => "settings.httpProxyEnabled",
            Self::SystemProxyEnabled => "settings.systemProxyEnabled",
            Self::HttpListen => "settings.httpListen",
            Self::HttpPort => "settings.httpPort",
            Self::VpnEnabled => "settings.vpnEnabled",
            Self::VpnMtu => "settings.vpnMtu",
            Self::VpnDns => "settings.vpnDns",
            Self::VpnTcpWindowScaling => "settings.vpnTcpWindowScaling",
            Self::TimeoutSeconds => "settings.timeoutSeconds",
            Self::Server => "settings.server",
            Self::LoginDomain => "settings.loginDomain",
            Self::LogEnabled => "settings.logEnabled",
            Self::LogLevel => "settings.logLevel",
            Self::AtrustEnabled => "settings.protocol.atrust",
            Self::EasyConnectEnabled => "settings.protocol.easyconnect",
            Self::OpenVpnEnabled => "settings.protocol.openvpn",
            Self::VpnCaptureMode => "settings.vpnCaptureMode",
        }
    }
}
pub struct Edit {
    pub key: SettingKey,
    pub value_json: String,
}

pub struct SettingsSnapshot {
    pub revision: u32,
    pub schema_version: u32,
    pub theme_mode: String,
    pub welcome_completed: bool,
    pub socks_port: u16,
    pub socks_listen: String,
    pub socks_proxy_enabled: bool,
    pub http_proxy_enabled: bool,
    pub system_proxy_enabled: bool,
    pub http_listen: String,
    pub http_port: u16,
    pub vpn_enabled: bool,
    pub vpn_mtu: u16,
    pub vpn_dns: String,
    pub vpn_tcp_window_scaling: bool,
    pub timeout_seconds: u32,
    pub server: String,
    pub login_domain: String,
    pub log_enabled: bool,
    pub log_level: String,
    pub enabled_protocols: Vec<String>,
    pub vpn_capture_mode: crate::platform::model::CaptureMode,
}

#[derive(Debug)]
pub struct SettingsFailure {
    pub code: String,
    pub message: String,
}
impl std::fmt::Display for SettingsFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for SettingsFailure {}
pub(crate) fn failure(code: &str, message: &str) -> SettingsFailure {
    SettingsFailure {
        code: code.into(),
        message: message.into(),
    }
}
