use super::{
    compiler::{MAX_DOCUMENT, MAX_RULES},
    model::*,
};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CachedProvider {
    pub descriptor: Provider,
    pub payload: Vec<String>,
}

pub(crate) struct ProviderLoader {
    root: PathBuf,
    roots: Vec<reqwest::Certificate>,
}

impl ProviderLoader {
    #[cfg(feature = "egress-probe")]
    pub(crate) fn with_probe_certificate(
        root: &Path,
        certificate: &[u8],
    ) -> Result<Self, RoutingFailure> {
        let mut loader = Self::new(root)?;
        loader.roots.push(
            reqwest::Certificate::from_der(certificate)
                .map_err(|_| RoutingFailure::new("certificate", "探针测试证书无效"))?,
        );
        Ok(loader)
    }

    pub(crate) fn new(root: &Path) -> Result<Self, RoutingFailure> {
        let root = root
            .canonicalize()
            .map_err(|_| RoutingFailure::new("directory", "规则目录不存在"))?;
        download_client(reqwest::Client::builder().no_proxy())
            .build()
            .map_err(|_| RoutingFailure::new("download", "无法创建规则集下载器"))?;
        Ok(Self {
            root,
            roots: vec![],
        })
    }

    pub(crate) async fn load(
        &self,
        name: &str,
        provider: &Provider,
    ) -> Result<Vec<String>, RoutingFailure> {
        validate_descriptor(provider)?;
        let limit = if super::geodata::is_geo(provider) {
            super::geodata::MAX_BYTES
        } else {
            MAX_DOCUMENT
        };
        if provider.kind == "inline" {
            if provider.path.is_some() || provider.url.is_some() || provider.interval.is_some() {
                return Err(RoutingFailure::new(
                    "provider",
                    "inline 规则集不能包含路径、URL 或刷新间隔",
                ));
            }
            return validate_payload(provider.payload.clone());
        }
        if !provider.payload.is_empty() {
            return Err(RoutingFailure::new(
                "provider",
                "非 inline 规则集不能包含 payload",
            ));
        }
        let bytes = match provider.kind.as_str() {
            "file" => {
                if provider.url.is_some() {
                    return Err(RoutingFailure::new("provider", "file 规则集不能包含 URL"));
                }
                let path = provider
                    .path
                    .as_deref()
                    .ok_or_else(|| RoutingFailure::new("provider", "file 规则集缺少路径"))?;
                let path = self.relative_file(path)?;
                let file = tokio::fs::File::open(path)
                    .await
                    .map_err(|_| RoutingFailure::new("file", "读取规则集失败"))?;
                use tokio::io::AsyncReadExt;
                let mut bytes = Vec::new();
                file.take((limit + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|_| RoutingFailure::new("file", "读取规则集失败"))?;
                bytes
            }
            "http" => {
                let url = provider
                    .url
                    .as_deref()
                    .ok_or_else(|| RoutingFailure::new("provider", "远端规则集缺少 URL"))?;
                let parsed = reqwest::Url::parse(url)
                    .map_err(|_| RoutingFailure::new("provider", "规则集 URL 无效"))?;
                if parsed.scheme() != "https"
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                {
                    return Err(RoutingFailure::new("provider", "远端规则集必须使用 HTTPS"));
                }
                if let Some(path) = &provider.path {
                    if Path::new(path).is_absolute()
                        || Path::new(path)
                            .components()
                            .any(|part| !matches!(part, Component::Normal(_)))
                    {
                        return Err(RoutingFailure::new(
                            "provider",
                            "规则集缓存路径必须位于规则目录",
                        ));
                    }
                }
                let lease = crate::network::http::Lease::open(&parsed)
                    .await
                    .map_err(|_| RoutingFailure::new("download", "无法取得规则集物理出口"))?;
                let mut builder = download_client(lease.builder());
                for certificate in &self.roots {
                    builder = builder.add_root_certificate(certificate.clone());
                }
                let client = builder
                    .build()
                    .map_err(|_| RoutingFailure::new("download", "无法创建规则集下载器"))?;
                let mut response = client.get(parsed).send().await.map_err(|_error| {
                    #[cfg(feature = "egress-probe")]
                    eprintln!("HTTPS probe transport: {}", _error.without_url());
                    RoutingFailure::new("download", "下载规则集失败")
                })?;
                if !response.status().is_success() {
                    return Err(RoutingFailure::new(
                        "download",
                        "规则集服务器返回了错误状态",
                    ));
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| RoutingFailure::new("download", "读取远端规则集失败"))?
                {
                    if chunk.len() > limit.saturating_sub(bytes.len()) {
                        return Err(RoutingFailure::new("size", "规则集超过大小限制"));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                bytes
            }
            _ => return Err(RoutingFailure::new("provider", "规则集来源类型不支持")),
        };
        if bytes.len() > limit {
            return Err(RoutingFailure::new("size", "规则集超过大小限制"));
        }
        if super::geodata::is_geo(provider) {
            return super::geodata::decode(&bytes, provider);
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| RoutingFailure::new("provider", "规则集不是 UTF-8"))?
            .trim_start_matches('\u{feff}');
        let payload = if provider.format == "text" {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
                .collect()
        } else {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Payload {
                payload: Vec<String>,
            }
            serde_yaml_ng::from_str::<Payload>(text)
                .map_err(|_| RoutingFailure::new("provider", "YAML 规则集载荷无效"))?
                .payload
        };
        let _ = name;
        validate_payload(payload)
    }

    fn relative_file(&self, path: &str) -> Result<PathBuf, RoutingFailure> {
        let path = Path::new(path);
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(RoutingFailure::new(
                "directory",
                "规则集文件必须位于规则目录",
            ));
        }
        let resolved = self
            .root
            .join(path)
            .canonicalize()
            .map_err(|_| RoutingFailure::new("file", "规则集文件不存在"))?;
        if !resolved.starts_with(&self.root) {
            return Err(RoutingFailure::new("directory", "规则集路径越出规则目录"));
        }
        Ok(resolved)
    }
}

pub(crate) fn validate_descriptor(provider: &Provider) -> Result<(), RoutingFailure> {
    if super::geodata::is_geo(provider) {
        super::geodata::validate(provider)?;
        if !matches!(provider.kind.as_str(), "file" | "http")
            || provider
                .interval
                .is_some_and(|seconds| !(1..=31_536_000).contains(&seconds))
        {
            return Err(RoutingFailure::new(
                "provider",
                "GeoData 来源或更新间隔无效",
            ));
        }
        return Ok(());
    }
    if !matches!(provider.kind.as_str(), "inline" | "file" | "http")
        || provider.data.is_some()
        || !matches!(provider.format.as_str(), "yaml" | "text")
        || !matches!(
            provider.behavior.as_str(),
            "domain" | "ipcidr" | "classical"
        )
        || provider
            .interval
            .is_some_and(|seconds| !(1..=31_536_000).contains(&seconds))
        || (provider.kind == "inline" && provider.interval.is_some())
    {
        return Err(RoutingFailure::new(
            "provider",
            "规则集来源、格式、行为或更新间隔不支持",
        ));
    }
    Ok(())
}

fn download_client(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
}

fn validate_payload(payload: Vec<String>) -> Result<Vec<String>, RoutingFailure> {
    if payload.len() > MAX_RULES
        || payload.iter().map(String::len).sum::<usize>() > MAX_DOCUMENT
        || payload
            .iter()
            .any(|line| line.trim().is_empty() || line.len() > 4096)
    {
        return Err(RoutingFailure::new("size", "规则集条数或内容大小无效"));
    }
    Ok(payload)
}

pub(crate) async fn fetch_all(
    loader: &ProviderLoader,
    profile: &Profile,
    previous: &BTreeMap<String, CachedProvider>,
) -> Result<(BTreeMap<String, CachedProvider>, Vec<String>), RoutingFailure> {
    let mut caches = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut paths: BTreeMap<&str, &Provider> = BTreeMap::new();
    for (name, descriptor) in &profile.providers {
        if let Some(path) = &descriptor.path {
            if let Some(previous) = paths.insert(path, descriptor) {
                let shared_geo_file = previous.kind == "file"
                    && descriptor.kind == "file"
                    && super::geodata::is_geo(previous)
                    && super::geodata::is_geo(descriptor)
                    && previous.behavior == descriptor.behavior
                    && previous
                        .data
                        .as_ref()
                        .zip(descriptor.data.as_ref())
                        .is_some_and(|(left, right)| {
                            left.version == right.version
                                && left.sha256.eq_ignore_ascii_case(&right.sha256)
                        });
                if !shared_geo_file {
                    return Err(RoutingFailure::new(
                        "provider",
                        "规则集文件路径重复或 GeoData 来源不一致",
                    ));
                }
            }
        }
        let payload = match loader.load(name, descriptor).await {
            Ok(payload) => payload,
            Err(error) => match previous
                .get(name)
                .filter(|cached| cached.descriptor == *descriptor)
            {
                Some(cached) if matches!(error.code.as_str(), "download" | "file") => {
                    warnings.push(format!("规则集 {name} 更新失败，保留有效版本"));
                    cached.payload.clone()
                }
                _ => return Err(error),
            },
        };
        caches.insert(
            name.clone(),
            CachedProvider {
                descriptor: descriptor.clone(),
                payload,
            },
        );
    }
    Ok((caches, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct HttpsFixture {
        url: String,
        certificate: Vec<u8>,
        body: Arc<RwLock<Vec<u8>>>,
        task: tokio::task::JoinHandle<()>,
    }

    impl HttpsFixture {
        async fn start() -> anyhow::Result<Self> {
            let rcgen::CertifiedKey { cert, key_pair } =
                rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_owned()])?;
            let certificate = cert.der().to_vec();
            let key = rustls::pki_types::PrivatePkcs8KeyDer::from(key_pair.serialize_der());
            let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], key.into())?;
            let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
            let url = format!("https://127.0.0.1:{}/rules", listener.local_addr()?.port());
            let body = Arc::new(RwLock::new(b"payload: ['+.example.com']".to_vec()));
            let response_body = Arc::clone(&body);
            let task = tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    let Ok(mut stream) = acceptor.accept(socket).await else {
                        continue;
                    };
                    let mut header = vec![];
                    while !header.ends_with(b"\r\n\r\n") && header.len() < 8192 {
                        let Ok(byte) = stream.read_u8().await else {
                            return;
                        };
                        header.push(byte);
                    }
                    let bytes = match response_body.read() {
                        Ok(body) => body.clone(),
                        Err(_) => return,
                    };
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    );
                    if stream.write_all(header.as_bytes()).await.is_err() {
                        continue;
                    }
                    let _ = stream.write_all(&bytes).await;
                    let _ = stream.shutdown().await;
                }
            });
            Ok(Self {
                url,
                certificate,
                body,
                task,
            })
        }

        fn loader(&self, root: &Path) -> anyhow::Result<ProviderLoader> {
            let certificate = reqwest::Certificate::from_der(&self.certificate)?;
            Ok(ProviderLoader {
                root: root.canonicalize()?,
                roots: vec![certificate],
            })
        }

        fn body(&self, bytes: Vec<u8>) -> anyhow::Result<()> {
            *self
                .body
                .write()
                .map_err(|_| anyhow::anyhow!("fixture body lock poisoned"))? = bytes;
            Ok(())
        }

        fn provider(&self, format: &str) -> Provider {
            Provider {
                kind: "http".to_owned(),
                behavior: "domain".to_owned(),
                format: format.to_owned(),
                payload: vec![],
                path: None,
                url: Some(self.url.clone()),
                interval: Some(3600),
                data: None,
            }
        }
    }

    impl Drop for HttpsFixture {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    #[tokio::test]
    async fn pinned_https_geodata_validates_binary_and_preserves_selected_manifest(
    ) -> anyhow::Result<()> {
        #[allow(dead_code)]
        mod geo_fixture {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/support/geodata.rs"
            ));
        }
        let directory = tempfile::tempdir()?;
        let fixture = HttpsFixture::start().await?;
        let bytes = geo_fixture::site();
        fixture.body(bytes.clone())?;
        let mut provider = fixture.provider("dat");
        provider.behavior = "geosite".to_owned();
        provider.data = Some(GeoDataSpec {
            code: "TEST@ads".to_owned(),
            version: "https-fixture-v1".to_owned(),
            sha256: sangfor_core::crypto::hex_upper(&sangfor_core::crypto::sha256(&bytes)),
        });
        let loader = fixture.loader(directory.path())?;
        let payload = loader.load("geo", &provider).await?;
        let selected = super::super::geodata::cached(&payload, &provider)?;
        assert_eq!(selected.lines.len(), 3);
        fixture.body(b"not dat".to_vec())?;
        assert_eq!(
            loader
                .load("geo", &provider)
                .await
                .err()
                .map(|error| error.code),
            Some("checksum".to_owned())
        );
        Ok(())
    }

    #[tokio::test]
    async fn https_downloads_validate_tls_and_parse_actual_yaml_and_text() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let fixture = HttpsFixture::start().await?;
        let untrusted = ProviderLoader::new(directory.path())?;
        let error = untrusted
            .load("p", &fixture.provider("yaml"))
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("untrusted TLS unexpectedly succeeded"))?;
        assert_eq!(error.code, "download");
        let loader = fixture.loader(directory.path())?;
        assert_eq!(
            loader.load("p", &fixture.provider("yaml")).await?,
            vec!["+.example.com"]
        );
        fixture.body(b"\xef\xbb\xbf# comment\n.example.com\n\n".to_vec())?;
        assert_eq!(
            loader.load("p", &fixture.provider("text")).await?,
            vec![".example.com"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn https_malformed_and_oversized_payloads_are_rejected() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let fixture = HttpsFixture::start().await?;
        let loader = fixture.loader(directory.path())?;
        fixture.body(b"payload: [".to_vec())?;
        assert!(loader.load("p", &fixture.provider("yaml")).await.is_err());
        fixture.body(vec![b'x'; MAX_DOCUMENT + 1])?;
        let error = loader
            .load("p", &fixture.provider("text"))
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("oversized body unexpectedly succeeded"))?;
        assert_eq!(error.code, "size");
        Ok(())
    }
}
