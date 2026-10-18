use crate::{
    auth::{cookies::Jar, model::AuthFailure},
    network::http::Lease,
};
use reqwest::{header, Method, StatusCode, Url};
use rsa::RsaPublicKey;
use serde_json::Value;
use std::time::Duration;

pub(super) struct Config {
    pub sso: Url,
    pub entry: Url,
    pub key: RsaPublicKey,
    #[cfg(test)]
    pub fixture: Option<(Option<Vec<u8>>, std::net::SocketAddr)>,
}

pub(super) struct Response {
    pub status: StatusCode,
    pub location: Option<String>,
    pub refresh: Option<String>,
    pub body: Vec<u8>,
}

pub(super) enum QrRequest {
    Page,
    Poll,
    Image,
}

struct RequestSpec<'a> {
    method: Method,
    uri: Url,
    body: Option<Body>,
    referer: Option<&'a Url>,
    accept: &'static str,
    timeout: Duration,
    ajax: bool,
    cache_control: Option<&'static str>,
}

enum Body {
    Json(Value),
    Form(Vec<(String, String)>),
}

impl Config {
    pub(super) fn system_base(&self, id: &str) -> Result<Url, AuthFailure> {
        let host = match id {
            "jwxt" => "jwxt.shu.edu.cn",
            "otp" => "otp.shu.edu.cn",
            "atrust" => "atrust.shu.edu.cn",
            _ => return Err(AuthFailure::new("unknownSystem", "未接入该系统")),
        };
        #[cfg(test)]
        if self.fixture.is_some() {
            let mut uri = self.entry.clone();
            uri.set_host(Some(match id {
                "jwxt" => "jwxt.fixture.test",
                "otp" => "otp.fixture.test",
                _ => "atrust.fixture.test",
            }))
            .map_err(|_| unsafe_redirect())?;
            return Ok(uri);
        }
        Url::parse(&format!("https://{host}/")).map_err(|_| unsafe_redirect())
    }

    pub(super) fn production(key: RsaPublicKey) -> Result<Self, AuthFailure> {
        Ok(Self {
            sso: Url::parse("https://newsso.shu.edu.cn/").map_err(|_| unsafe_redirect())?,
            entry: Url::parse("https://jwxt.shu.edu.cn/").map_err(|_| unsafe_redirect())?,
            key,
            #[cfg(test)]
            fixture: None,
        })
    }

    pub(super) fn validate(&self, uri: &Url) -> Result<(), AuthFailure> {
        if uri.scheme() != "https"
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.fragment().is_some()
        {
            return Err(unsafe_redirect());
        }
        #[cfg(test)]
        if let Some((_, address)) = &self.fixture {
            if [
                self.sso.host_str(),
                self.entry.host_str(),
                Some("jwxt.fixture.test"),
                Some("otp.fixture.test"),
                Some("atrust.fixture.test"),
            ]
            .contains(&uri.host_str())
                && uri.port_or_known_default() == Some(address.port())
            {
                return Ok(());
            }
            return Err(unsafe_redirect());
        }
        let host = uri.host_str().unwrap_or("");
        if (host == "shu.edu.cn" || host.ends_with(".shu.edu.cn"))
            && uri.port_or_known_default() == Some(443)
        {
            Ok(())
        } else {
            Err(unsafe_redirect())
        }
    }

    pub(super) fn redirect(&self, current: &Url, location: &str) -> Result<Url, AuthFailure> {
        let mut next = current.join(location).map_err(|_| unsafe_redirect())?;
        if next.scheme() == "http" {
            next.set_scheme("https").map_err(|_| unsafe_redirect())?;
        }
        self.validate(&next)?;
        Ok(next)
    }

    pub(super) fn wecom(&self) -> Result<Url, AuthFailure> {
        #[cfg(test)]
        if self.fixture.is_some() {
            let mut uri = self.entry.clone();
            uri.set_host(Some("wecom.fixture.test"))
                .map_err(|_| unsafe_redirect())?;
            return Ok(uri);
        }
        Url::parse("https://open.work.weixin.qq.com/").map_err(|_| unsafe_redirect())
    }

    pub(super) async fn qr_request(
        &self,
        jar: &mut Jar,
        uri: Url,
        kind: QrRequest,
    ) -> Result<Response, AuthFailure> {
        let origin = self.wecom()?;
        let (path, accept, timeout, ajax) = match kind {
            QrRequest::Page => (
                "/wwopen/sso/qrConnect",
                "text/html,application/xhtml+xml,*/*;q=0.8",
                Duration::from_secs(15),
                false,
            ),
            QrRequest::Poll => (
                "/wwopen/sso/l/qrConnect",
                "text/javascript, application/javascript, application/ecmascript, */*; q=0.01",
                Duration::from_secs(45),
                true,
            ),
            QrRequest::Image => (
                "/wwopen/sso/qrImg",
                "image/png,image/jpeg,image/gif,image/webp",
                Duration::from_secs(15),
                false,
            ),
        };
        if uri.origin() != origin.origin()
            || uri.path() != path
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.fragment().is_some()
        {
            return Err(unsafe_redirect());
        }
        let referer = origin
            .join("/wwopen/sso/qrConnect")
            .map_err(|_| unsafe_redirect())?;
        self.send(
            jar,
            RequestSpec {
                method: Method::GET,
                uri,
                body: None,
                referer: Some(&referer),
                accept,
                timeout,
                ajax,
                cache_control: None,
            },
        )
        .await
    }

    pub(super) async fn request(
        &self,
        jar: &mut Jar,
        method: Method,
        uri: Url,
        body: Option<Value>,
        referer: Option<&Url>,
    ) -> Result<Response, AuthFailure> {
        self.validate(&uri)?;
        if let Some(referer) = referer {
            self.validate(referer)?;
        }
        self.send(
            jar,
            RequestSpec {
                method,
                uri,
                body: body.map(Body::Json),
                referer,
                accept: "application/json, text/plain, */*",
                timeout: Duration::from_secs(15),
                ajax: false,
                cache_control: None,
            },
        )
        .await
    }

    pub(super) async fn page(
        &self,
        jar: &mut Jar,
        uri: Url,
        referer: &Url,
    ) -> Result<Response, AuthFailure> {
        self.validate(&uri)?;
        self.validate(referer)?;
        self.send(
            jar,
            RequestSpec {
                method: Method::GET,
                uri,
                body: None,
                referer: Some(referer),
                accept: "text/html,application/xhtml+xml,*/*;q=0.8",
                timeout: Duration::from_secs(15),
                ajax: false,
                cache_control: None,
            },
        )
        .await
    }

    pub(super) async fn otp_page(
        &self,
        jar: &mut Jar,
        mut uri: Url,
    ) -> Result<Response, AuthFailure> {
        self.validate(&uri)?;
        uri.query_pairs_mut()
            .append_pair("_", &super::super::atrust::random_hex(8)?);
        self.send(
            jar,
            RequestSpec {
                method: Method::GET,
                uri,
                body: None,
                referer: None,
                accept: "text/html,application/xhtml+xml,*/*;q=0.8",
                timeout: Duration::from_secs(15),
                ajax: false,
                cache_control: Some("no-cache"),
            },
        )
        .await
    }

    pub(super) async fn form(
        &self,
        jar: &mut Jar,
        uri: Url,
        referer: &Url,
        fields: Vec<(String, String)>,
    ) -> Result<Response, AuthFailure> {
        self.validate(&uri)?;
        self.validate(referer)?;
        self.send(
            jar,
            RequestSpec {
                method: Method::POST,
                uri,
                body: Some(Body::Form(fields)),
                referer: Some(referer),
                accept: "*/*",
                timeout: Duration::from_secs(15),
                ajax: true,
                cache_control: None,
            },
        )
        .await
    }

    async fn send(&self, jar: &mut Jar, spec: RequestSpec<'_>) -> Result<Response, AuthFailure> {
        let RequestSpec {
            method,
            uri,
            body,
            referer,
            accept,
            timeout,
            ajax,
            cache_control,
        } = spec;
        let lease = Lease::open(&uri)
            .await
            .map_err(|_| AuthFailure::new("network", "无法连接学校认证服务"))?;
        let builder = lease.builder()
            .use_rustls_tls().redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .user_agent("Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36");
        #[cfg(test)]
        let builder = if let Some((certificate, address)) = &self.fixture {
            let builder = builder.resolve(uri.host_str().ok_or_else(unsafe_redirect)?, *address);
            if let Some(certificate) = certificate {
                builder.add_root_certificate(
                    reqwest::Certificate::from_der(certificate).map_err(|_| unsafe_redirect())?,
                )
            } else {
                builder
            }
        } else {
            builder
        };
        let client = builder.build().map_err(network_failure)?;
        let mut request = client
            .request(method, uri.clone())
            .header(header::ACCEPT, accept);
        if ajax {
            request = request.header("x-requested-with", "XMLHttpRequest");
        }
        if let Some(value) = cache_control {
            request = request
                .header(header::CACHE_CONTROL, value)
                .header("pragma", value);
        }
        if let Some(header) = jar.header(&uri)? {
            request = request.header(header::COOKIE, header);
        }
        if let Some(referer) = referer {
            let mut value =
                header::HeaderValue::from_str(referer.as_str()).map_err(|_| unsafe_redirect())?;
            value.set_sensitive(true);
            request = request
                .header(header::REFERER, value)
                .header(header::ORIGIN, referer.origin().ascii_serialization());
        }
        if let Some(body) = body {
            request = match body {
                Body::Json(value) => request.json(&value),
                Body::Form(fields) => request.form(&fields),
            };
        }
        let mut response = request.send().await.map_err(network_failure)?;
        let status = response.status();
        let refresh = response
            .headers()
            .get("refresh")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let location = response
            .headers()
            .get(header::LOCATION)
            .map(|value| {
                value
                    .to_str()
                    .map(str::to_owned)
                    .map_err(|_| unsafe_redirect())
            })
            .transpose()?;
        let cookies: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok().map(str::to_owned))
            .collect();
        if cookies.len() > 256 {
            return Err(AuthFailure::new(
                "invalidResponse",
                "认证 Cookie 数量超过限制",
            ));
        }
        let mut body = vec![];
        while let Some(chunk) = response.chunk().await.map_err(network_failure)? {
            if chunk.len() > (1024 * 1024usize).saturating_sub(body.len()) {
                return Err(AuthFailure::new("invalidResponse", "认证响应超过大小限制"));
            }
            body.extend_from_slice(&chunk);
        }
        for cookie in cookies {
            jar.save(&uri, &cookie)?;
        }
        Ok(Response {
            status,
            location,
            refresh,
            body,
        })
    }
}

fn unsafe_redirect() -> AuthFailure {
    AuthFailure::new("unsafeRedirect", "认证服务返回了不受信任的跳转地址")
}
fn network_failure(error: reqwest::Error) -> AuthFailure {
    if error.is_timeout() {
        AuthFailure::new("timeout", "连接学校认证服务超时")
    } else {
        // reqwest errors can contain a URL with one-time OAuth parameters.
        AuthFailure::new("network", "无法连接学校认证服务或证书校验失败")
    }
}
