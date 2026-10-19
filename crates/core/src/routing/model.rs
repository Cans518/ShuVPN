use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    #[serde(rename = "DIRECT")]
    Direct,
    #[serde(rename = "REJECT")]
    Reject,
    #[serde(rename = "SHU-VPN")]
    ShuVpn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Inbound {
    Http,
    Socks,
    Tun,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub host: String,
    #[serde(default)]
    pub ips: Vec<IpAddr>,
    pub port: u16,
    pub network: Network,
    pub inbound: Inbound,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<ProcessIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domain_candidates: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessIdentity {
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default = "preview_source")]
    pub source: String,
    #[serde(default)]
    pub confidence: ProcessConfidence,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessConfidence {
    #[default]
    Preview,
    SocketSnapshot,
}

fn preview_source() -> String {
    "preview".to_owned()
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RouteDecision {
    pub version: u64,
    pub rule_id: String,
    pub source: String,
    pub policy: String,
    pub action: Action,
    pub reason: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(default = "default_name")]
    pub name: String,
    pub rules: Vec<RuleEntry>,
    #[serde(default = "enabled")]
    pub automatic_campus: bool,
    #[serde(default, rename = "proxy-groups")]
    pub groups: Vec<PolicyGroup>,
    #[serde(default, rename = "rule-providers")]
    pub providers: BTreeMap<String, Provider>,
}

fn default_name() -> String {
    "用户配置".to_owned()
}
fn enabled() -> bool {
    true
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum RuleEntry {
    Expression(String),
    Configured(RuleSetting),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSetting {
    pub rule: String,
    pub enabled: bool,
}

impl RuleEntry {
    pub fn new(expression: String, enabled: bool) -> Self {
        if enabled {
            Self::Expression(expression)
        } else {
            Self::Configured(RuleSetting {
                rule: expression,
                enabled,
            })
        }
    }

    pub fn expression(&self) -> &str {
        match self {
            Self::Expression(expression) => expression,
            Self::Configured(setting) => &setting.rule,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Self::Expression(_) => true,
            Self::Configured(setting) => setting.enabled,
        }
    }
}

impl From<String> for RuleEntry {
    fn from(value: String) -> Self {
        Self::Expression(value)
    }
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    #[serde(default)]
    pub prepend: Vec<RuleEntry>,
    #[serde(default)]
    pub append: Vec<RuleEntry>,
    #[serde(default)]
    pub selections: BTreeMap<String, String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyGroup {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub proxies: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    #[serde(rename = "type")]
    pub kind: String,
    pub behavior: String,
    #[serde(default = "yaml")]
    pub format: String,
    #[serde(default)]
    pub payload: Vec<String>,
    pub path: Option<String>,
    pub url: Option<String>,
    pub interval: Option<u64>,
    #[serde(default)]
    pub data: Option<GeoDataSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoDataSpec {
    pub code: String,
    pub version: String,
    pub sha256: String,
}
fn yaml() -> String {
    "yaml".to_owned()
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigurationInfo {
    pub version: u64,
    pub name: String,
    pub profile_document: String,
    pub overrides_document: String,
    pub rule_count: usize,
    pub notices: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RoutingStatus {
    pub configuration: ConfigurationInfo,
    pub refreshing: bool,
    pub refresh_error: Option<String>,
    pub provider_errors: BTreeMap<String, String>,
    pub last_checked_unix_ms: Option<u64>,
}

#[derive(Debug)]
pub struct RoutingFailure {
    pub code: String,
    pub message: String,
}
impl RoutingFailure {
    pub(crate) fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}
impl fmt::Display for RoutingFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for RoutingFailure {}

pub fn normalize_domain(host: &str) -> Result<String, RoutingFailure> {
    let host = host.trim_end_matches('.');
    if host.is_empty()
        || host.len() > 1024
        || host
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || host.contains(['/', ':', '@', '*'])
    {
        return Err(RoutingFailure::new("domain", "域名格式无效"));
    }
    let ascii = idna::domain_to_ascii(host)
        .map_err(|_| RoutingFailure::new("domain", "IDNA 域名无效"))?
        .to_ascii_lowercase();
    if ascii.len() > 253
        || ascii
            .split('.')
            .any(|label| label.is_empty() || label.len() > 63)
    {
        return Err(RoutingFailure::new("domain", "域名长度无效"));
    }
    Ok(ascii)
}
