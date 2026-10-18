use crate::network::http::Lease;
use reqwest::{header, Url};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
pub(crate) mod release;
#[cfg(test)]
mod tests;
mod version;
use release::UpdateInfo;

#[derive(Debug)]
pub struct UpdateFailure {
    pub code: String,
    pub message: String,
}
impl std::fmt::Display for UpdateFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for UpdateFailure {}

pub(crate) struct Client {
    endpoint: Url,
    cancelled: CancellationToken,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.cancelled.cancel();
    }
}
impl Client {
    pub(crate) fn new() -> Result<Self, UpdateFailure> {
        Ok(Self {
            endpoint: Url::parse("https://api.github.com/repos/shuosc/ShuVPN/releases/latest")
                .map_err(|_| failure("config", "更新地址无效"))?,
            cancelled: CancellationToken::new(),
        })
    }
    pub(crate) fn close(&self) {
        self.cancelled.cancel();
    }
    pub(crate) async fn check(&self, current: String) -> Result<Option<UpdateInfo>, UpdateFailure> {
        if current.len() > 256 {
            return Err(failure("version", "当前版本名超过长度限制"));
        }
        tokio::select! {
            biased;
            _ = self.cancelled.cancelled() => Err(failure("closed", "更新通道已经关闭")),
            result = tokio::time::timeout(Duration::from_secs(8), self.fetch(&current)) =>
                result.map_err(|_| failure("timeout", "检查更新超时"))?,
        }
    }
    async fn fetch(&self, current: &str) -> Result<Option<UpdateInfo>, UpdateFailure> {
        let lease = Lease::open(&self.endpoint)
            .await
            .map_err(|_| failure("network", "无法连接更新服务"))?;
        let client = lease
            .builder()
            .use_rustls_tls()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(8))
            .user_agent("ShuVPN")
            .build()
            .map_err(|_| failure("network", "无法初始化更新请求"))?;
        let mut response = client
            .get(self.endpoint.clone())
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|_| failure("network", "无法连接更新服务或证书校验失败"))?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        if response.status().as_u16() != 200 {
            return Err(failure(
                "http",
                &format!(
                    "GitHub 返回 HTTP {}，可能是访问限额用尽或服务不可用",
                    response.status().as_u16()
                ),
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failure("network", "更新响应读取失败"))?
        {
            if chunk.len() > (1024 * 1024usize).saturating_sub(bytes.len()) {
                return Err(failure("response", "更新响应超过大小限制"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let release: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| failure("response", "GitHub 返回的发布对象无法解析"))?;
        if !release.is_object() {
            return Err(failure("response", "GitHub 返回的不是一个发布对象"));
        }
        Ok(release::from_release(&release, current))
    }
}
fn failure(code: &str, message: &str) -> UpdateFailure {
    UpdateFailure {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}
