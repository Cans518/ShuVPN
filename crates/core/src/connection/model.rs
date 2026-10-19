use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProxyKind {
    Http,
    Socks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnState {
    Connected,
    Disconnected,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnEventKind {
    State,
    Traffic,
}

#[derive(Clone)]
pub struct VpnEvent {
    pub kind: VpnEventKind,
    pub state: VpnState,
    pub virtual_address: Option<String>,
    pub dns_servers: Vec<String>,
    pub message: Option<String>,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub latency_ms: Option<f64>,
    /// Cumulative successful dials, matching the UI's lifetime connection count.
    pub connections: u64,
    pub unrouted: u64,
    pub tunnel_failures: u64,
}

#[derive(Clone)]
pub struct ProxyBinding {
    pub address: String,
    pub port: u16,
    pub display_endpoint: String,
    pub display_warning: Option<String>,
}

#[derive(Clone, Debug)]
pub struct VpnFailure {
    pub code: String,
    pub message: String,
}

impl VpnFailure {
    pub(crate) fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

impl fmt::Display for VpnFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for VpnFailure {}

impl From<std::io::Error> for VpnFailure {
    fn from(error: std::io::Error) -> Self {
        Self::new("network", &format!("网络通道失败：{error}"))
    }
}

impl From<sangfor_core::Error> for VpnFailure {
    fn from(_: sangfor_core::Error) -> Self {
        Self::new("protocol", "aTrust 通道返回了无效报文")
    }
}
