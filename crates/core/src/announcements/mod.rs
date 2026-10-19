use crate::network::http::Lease;
use reqwest::{header, Url};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio_util::sync::CancellationToken;
pub(crate) mod model;
mod parser;
pub(crate) mod prefetch;
#[cfg(test)]
mod tests;
use model::{failure, AnnouncementFailure, Detail, Item, Page};

pub(crate) struct Client {
    cancelled: CancellationToken,
    cache: Mutex<VecDeque<Detail>>,
    preview_slots: Arc<tokio::sync::Semaphore>,
    details: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
    #[cfg(test)]
    fixture: Option<(Vec<u8>, std::net::SocketAddr)>,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.cancelled.cancel();
    }
}
impl Client {
    pub(crate) fn new() -> Self {
        Self {
            cancelled: CancellationToken::new(),
            cache: Mutex::new(VecDeque::new()),
            preview_slots: Arc::new(tokio::sync::Semaphore::new(3)),
            details: Mutex::new(HashMap::new()),
            #[cfg(test)]
            fixture: None,
        }
    }
    pub(crate) fn close(&self) {
        self.cancelled.cancel();
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }
    fn live(&self) -> Result<(), AnnouncementFailure> {
        if self.cancelled.is_cancelled() {
            Err(failure("closed", "公告通道已经关闭"))
        } else {
            Ok(())
        }
    }
    fn target(&self, raw: &str) -> Result<Url, AnnouncementFailure> {
        let mut uri = Url::parse(raw).map_err(|_| failure("url", "公告地址无效"))?;
        if !uri.username().is_empty() || uri.password().is_some() || uri.fragment().is_some() {
            return Err(failure("url", "公告地址无效"));
        }
        #[cfg(test)]
        if let Some((_, address)) = &self.fixture {
            if uri.host_str() == Some("newits.fixture.test")
                && uri.scheme() == "https"
                && uri.port() == Some(address.port())
            {
                return Ok(uri);
            }
            return Err(failure("origin", "只获取公告站点的页面"));
        }
        if uri.host_str() != Some("newits.shu.edu.cn")
            || !matches!(uri.scheme(), "http" | "https")
            || (uri.scheme() == "http" && uri.port_or_known_default() != Some(80))
            || (uri.scheme() == "https" && uri.port_or_known_default() != Some(443))
        {
            return Err(failure("origin", "只获取公告站点的页面"));
        }
        if uri.scheme() == "http" {
            uri.set_scheme("https")
                .map_err(|_| failure("url", "公告地址无效"))?;
            uri.set_port(None)
                .map_err(|_| failure("url", "公告地址无效"))?;
        }
        Ok(uri)
    }
    pub(crate) fn cached(&self, raw: String) -> Result<Option<Detail>, AnnouncementFailure> {
        self.live()?;
        let canonical = self.target(&raw)?.to_string();
        let cache = self
            .cache
            .lock()
            .map_err(|_| failure("cache", "公告缓存不可用"))?;
        self.live()?;
        Ok(cache.iter().find(|detail| detail.url == canonical).cloned())
    }
    pub(crate) async fn list(&self, page: Option<String>) -> Result<Page, AnnouncementFailure> {
        self.live()?;
        let target = self.target(
            page.as_deref()
                .unwrap_or("https://newits.shu.edu.cn/index/tzgg.htm"),
        )?;
        tokio::select! {
            biased;
            _ = self.cancelled.cancelled() => Err(failure("closed", "公告通道已经关闭")),
            result = async {
                let (bytes, final_url) = self.fetch(target).await?;
                let page_url = final_url.clone();
                let mut parsed = tokio::task::spawn_blocking(move || parser::list(&bytes, &final_url)).await
                    .map_err(|_| failure("parser", "公告解析服务失败"))??;
                parsed.next_page = if parsed.items.is_empty() { None } else {
                    parsed.next_page.and_then(|raw| self.target(&raw).ok())
                        .filter(|next| next != &page_url).map(|url| url.to_string())
                };
                self.live()?;
                Ok(parsed)
            } => result,
        }
    }
    pub(crate) async fn detail(&self, mut item: Item) -> Result<Detail, AnnouncementFailure> {
        self.live()?;
        let target = self.target(&item.url)?;
        let cache_key = target.to_string();
        if let Some(cached) = self.cached(item.url.clone())? {
            return Ok(cached);
        }
        tokio::select! {
            biased;
            _ = self.cancelled.cancelled() => Err(failure("closed", "公告通道已经关闭")),
            result = tokio::time::timeout(Duration::from_secs(10), async {
                let gate = {
                    let mut pending = self.details.lock().map_err(|_| failure("cache", "公告请求状态不可用"))?;
                    pending.retain(|_, value| value.strong_count() > 0);
                    if let Some(gate) = pending.get(&cache_key).and_then(Weak::upgrade) { gate }
                    else {
                        let gate = Arc::new(tokio::sync::Mutex::new(()));
                        pending.insert(cache_key.clone(), Arc::downgrade(&gate));
                        gate
                    }
                };
                let _request = gate.lock().await;
                self.live()?;
                if let Some(cached) = self.cached(item.url.clone())? { return Ok(cached); }
                let (bytes, final_url) = self.fetch(target).await?;
                item.url = final_url.to_string();
                let mut parsed = tokio::task::spawn_blocking(move || parser::detail(&bytes, &item)).await
                    .map_err(|_| failure("parser", "公告解析服务失败"))??;
                parsed.url = cache_key;
                let mut cache = self.cache.lock().map_err(|_| failure("cache", "公告缓存不可用"))?;
                self.live()?;
                if let Some(index) = cache.iter().position(|detail| detail.url == parsed.url) {
                    cache.remove(index);
                }
                cache.push_back(parsed.clone());
                while cache.len() > 64 { cache.pop_front(); }
                Ok(parsed)
            }) => result.map_err(|_| failure("timeout", "公告请求超时"))?,
        }
    }
    async fn fetch(&self, mut uri: Url) -> Result<(Vec<u8>, Url), AnnouncementFailure> {
        tokio::time::timeout(Duration::from_secs(10), async {
            for hop in 0..=5 {
                uri = self.target(uri.as_str())?;
                let lease = Lease::open(&uri).await.map_err(|_| failure("network", "无法连接公告服务"))?;
                let builder = lease.builder().use_rustls_tls()
                    .redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(10))
                    .user_agent("Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36");
                #[cfg(test)]
                let builder = if let Some((certificate, address)) = &self.fixture {
                    builder.resolve(uri.host_str().unwrap_or(""), *address)
                        .add_root_certificate(reqwest::Certificate::from_der(certificate).map_err(|_| failure("config", "公告测试证书无效"))?)
                } else { builder };
                let client = builder.build().map_err(|_| failure("network", "无法初始化公告请求"))?;
                let mut response = client.get(uri.clone())
                    .header(header::ACCEPT, "text/html,application/xhtml+xml").send().await
                    .map_err(|_| failure("network", "无法连接公告服务或证书校验失败"))?;
                if response.status().is_redirection() {
                    if hop == 5 { return Err(failure("redirect", "公告跳转次数过多")); }
                    let location = response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok())
                        .ok_or_else(|| failure("redirect", "公告跳转地址无效"))?;
                    uri = uri.join(location).map_err(|_| failure("redirect", "公告跳转地址无效"))?;
                    self.target(uri.as_str())?;
                    continue;
                }
                if !response.status().is_success() {
                    return Err(AnnouncementFailure { code:"http".into(), message:format!("信息办公告返回 HTTP {}", response.status().as_u16()), status_code:Some(response.status().as_u16()) });
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|_| failure("network", "公告响应读取失败"))? {
                    if chunk.len() > (1024*1024usize).saturating_sub(bytes.len()) { return Err(failure("size", "公告页面超过大小限制")); }
                    bytes.extend_from_slice(&chunk);
                }
                return Ok((bytes, uri));
            }
            Err(failure("redirect", "公告跳转次数过多"))
        }).await.map_err(|_| failure("timeout", "公告请求超时"))?
    }
}
