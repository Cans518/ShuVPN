//! Privileged namespace harness, excluded from normal desktop libraries.
use crate::{
    connection::model::VpnFailure,
    network,
    routing::{model::Provider, providers::ProviderLoader},
};

/// Holds a real registered physical connection across TUN retirement.
#[cfg(target_os = "linux")]
pub struct RetainedTcp {
    socket: network::Tcp,
}

#[cfg(target_os = "linux")]
impl RetainedTcp {
    pub async fn connect(address: std::net::SocketAddr) -> Result<Self, VpnFailure> {
        let mut connection = Self {
            socket: network::connect_address(address, None).await?,
        };
        connection.expect_binding(Some(b"shuphys"))?;
        connection.exchange(b"retained-before-helper-close").await?;
        Ok(connection)
    }

    pub async fn verify_restored(&mut self) -> Result<(), VpnFailure> {
        self.expect_binding(None)?;
        self.exchange(b"retained-after-helper-close").await
    }

    fn expect_binding(&self, expected: Option<&[u8]>) -> Result<(), VpnFailure> {
        if self.socket.probe_binding()?.as_deref() != expected {
            return Err(VpnFailure::new(
                "probe",
                "独立 TCP 连接的物理绑定未符合预期",
            ));
        }
        Ok(())
    }

    async fn exchange(&mut self, payload: &[u8]) -> Result<(), VpnFailure> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut reply = vec![0; payload.len()];
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            self.socket.write_all(payload).await?;
            self.socket.read_exact(&mut reply).await?;
            Ok::<_, std::io::Error>(())
        })
        .await
        .map_err(|_| VpnFailure::new("probe", "独立 TCP 回包超时"))??;
        if reply != payload {
            return Err(VpnFailure::new("probe", "独立 TCP 数据改变"));
        }
        Ok(())
    }
}

/// Keeps a protected HTTPS response and its registered upstream socket alive.
#[cfg(target_os = "linux")]
pub struct RetainedHttp {
    response: network::http::Response,
}

#[cfg(target_os = "linux")]
impl RetainedHttp {
    pub async fn open(url: String, certificate: Vec<u8>) -> Result<Self, VpnFailure> {
        let url = reqwest::Url::parse(&url)
            .map_err(|_| VpnFailure::new("probe", "独立 HTTPS 地址无效"))?;
        let lease = network::http::Lease::open(&url).await?;
        let certificate = reqwest::Certificate::from_der(&certificate)
            .map_err(|_| VpnFailure::new("probe", "独立 HTTPS 证书无效"))?;
        let client = lease
            .builder()
            .add_root_certificate(certificate)
            .build()
            .map_err(|_| VpnFailure::new("probe", "独立 HTTPS 客户端创建失败"))?;
        let inner = tokio::time::timeout(std::time::Duration::from_secs(3), client.get(url).send())
            .await
            .map_err(|_| VpnFailure::new("probe", "独立 HTTPS 响应头超时"))?
            .map_err(|_| VpnFailure::new("probe", "独立 HTTPS 握手失败"))?;
        let mut response = network::http::Response {
            inner,
            _lease: lease,
            _client: client,
        };
        let mut prefix = vec![];
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while prefix.len() < 4 {
                let chunk = response
                    .chunk()
                    .await
                    .map_err(|_| VpnFailure::new("probe", "独立 HTTPS 前缀读取失败"))?
                    .ok_or_else(|| VpnFailure::new("probe", "独立 HTTPS 提前结束"))?;
                prefix.extend_from_slice(&chunk);
            }
            Ok::<_, VpnFailure>(())
        })
        .await
        .map_err(|_| VpnFailure::new("probe", "独立 HTTPS 前缀超时"))??;
        if prefix != b"head" {
            return Err(VpnFailure::new("probe", "独立 HTTPS 前缀或延迟约束不匹配"));
        }
        Ok(Self { response })
    }

    pub async fn verify_restored_body(mut self) -> Result<(), VpnFailure> {
        let mut tail = vec![];
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while let Some(chunk) = self
                .response
                .chunk()
                .await
                .map_err(|_| VpnFailure::new("probe", "恢复后的 HTTPS 读取失败"))?
            {
                tail.extend_from_slice(&chunk);
            }
            Ok::<_, VpnFailure>(())
        })
        .await
        .map_err(|_| VpnFailure::new("probe", "恢复后的 HTTPS 响应体超时"))??;
        if tail != b"tail" {
            return Err(VpnFailure::new("probe", "恢复后的 HTTPS 响应体改变"));
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub struct WindowsProbe {
    pub interface: String,
    pub tcp: std::net::SocketAddr,
    pub udp: std::net::SocketAddr,
    pub dns: std::net::SocketAddr,
    pub url: String,
    pub certificate: Vec<u8>,
    pub directory: String,
}

#[cfg(target_os = "windows")]
pub async fn verify_windows(probe: WindowsProbe) -> anyhow::Result<String> {
    network::windows_probe::verify(probe).await
}

/// Called only by the feature-gated example, never through FRB.
pub async fn verify(
    host: String,
    url: String,
    certificate: Vec<u8>,
    directory: String,
) -> Result<(), VpnFailure> {
    let addresses = network::dns::resolve(&host).await?;
    if !addresses
        .iter()
        .any(|address| address.to_string() == "10.22.0.5")
        || !addresses
            .iter()
            .any(|address| address.to_string() == "2001:db8:22::5")
    {
        return Err(VpnFailure::new("probe", "物理 DNS 双栈回答不匹配"));
    }
    let truncated = network::dns::resolve("truncated.fixture.test").await?;
    if truncated != addresses {
        return Err(VpnFailure::new("probe", "DNS TCP 回落回答不匹配"));
    }
    if network::dns::resolve("badid.fixture.test").await.is_ok() {
        return Err(VpnFailure::new("probe", "错误 DNS 标识被接受"));
    }
    let provider = Provider {
        kind: "http".to_owned(),
        behavior: "domain".to_owned(),
        format: "yaml".to_owned(),
        payload: vec![],
        path: None,
        url: Some(url),
        interval: None,
        data: None,
    };
    let ordinary = ProviderLoader::new(std::path::Path::new(&directory))
        .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
    if ordinary.load("probe", &provider).await.is_ok() {
        return Err(VpnFailure::new("probe", "未信任的 HTTPS 探针证书被接受"));
    }
    let trusted =
        ProviderLoader::with_probe_certificate(std::path::Path::new(&directory), &certificate)
            .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
    let payload = trusted
        .load("probe", &provider)
        .await
        .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
    if payload != [".physical.fixture.test"] {
        return Err(VpnFailure::new("probe", "绑定出口的 HTTPS 下载载荷不匹配"));
    }
    Ok(())
}
