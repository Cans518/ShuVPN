use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::network::http::{Lease as HttpLease, Response};
use base64::{engine::general_purpose::STANDARD, Engine};
use cookie::Cookie;
use reqwest::{header, Client, Method, Url};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::model::{
    AuthConfig, AuthCookie, AuthFailure, AuthSeed, AuthSession, TunnelOptions, TunnelPreparation,
};

const RESPONSE_LIMIT: usize = 1024 * 1024;
const SHARED_QUERY: [(&str, &str); 3] = [
    ("clientType", "SDPClient"),
    ("platform", "Linux"),
    ("lang", "en-US"),
];
type AuthResult<T> = Result<T, AuthFailure>;
type CookieJar = BTreeMap<(String, String, String), AuthCookie>;

pub(crate) struct ATrustClient {
    config: AuthConfig,
    gateway: Url,
    sso: Url,
    state: Mutex<AuthState>,
    cancellation: CancellationToken,
    operation: Mutex<()>,
    #[cfg(test)]
    fixture: Option<(Vec<u8>, std::net::SocketAddr)>,
}

#[derive(Default)]
struct AuthState {
    gateway_cookies: CookieJar,
    sso_cookies: CookieJar,
    csrf: String,
    owned: Option<super::cookies::Jar>,
}

struct AuthMethod {
    login_domain: String,
    login_url: String,
}

enum ConfigPhase {
    BeforeTicket,
    AfterTicket,
}

impl ATrustClient {
    pub(crate) fn new(config: AuthConfig) -> AuthResult<Self> {
        let gateway = base_url(&config.gateway_base)?;
        let sso = base_url(&config.sso_base)?;
        let redirect = Url::parse(&config.redirect_uri)
            .map_err(|_| AuthFailure::new("atrustConfig", "认证回调地址无效"))?;
        if redirect.origin() != gateway.origin()
            || redirect.path() != "/passport/v1/auth/httpsOauth2"
            || gateway.origin() == sso.origin()
            || config.client_id.is_empty()
        {
            return Err(AuthFailure::new("atrustConfig", "认证端点配置不匹配"));
        }
        Client::builder()
            .use_rustls_tls()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .user_agent(&config.user_agent)
            .build()
            .map_err(network_failure)?;
        Ok(Self {
            config,
            gateway,
            sso,
            state: Mutex::new(AuthState::default()),
            cancellation: CancellationToken::new(),
            operation: Mutex::new(()),
            #[cfg(test)]
            fixture: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_fixture(
        mut self,
        certificate: Vec<u8>,
        address: std::net::SocketAddr,
    ) -> Self {
        self.fixture = Some((certificate, address));
        self
    }

    pub(crate) fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub(crate) async fn sign_in(&self, seed: AuthSeed) -> AuthResult<AuthSession> {
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(AuthFailure::new("cancelled", "认证已取消")),
            result = async { let _operation = self.operation.lock().await; self.sign_in_inner(seed).await } => result,
        }
    }

    pub(super) async fn sign_in_owned(
        &self,
        jar: super::cookies::Jar,
        device_id: String,
    ) -> AuthResult<(AuthSession, super::cookies::Jar)> {
        let _operation = self.operation.lock().await;
        let seed = AuthSeed {
            gateway_cookies: jar.auth_seed(&self.gateway),
            sso_cookies: jar.auth_seed(&self.sso),
            device_id,
        };
        let session = self.sign_in_using(seed, Some(jar)).await?;
        let jar = self
            .state
            .lock()
            .await
            .owned
            .take()
            .ok_or_else(|| AuthFailure::new("atrustCookie", "账号 Cookie 所有权丢失"))?;
        Ok((session, jar))
    }

    pub(crate) async fn prepare_tunnel(
        &self,
        seed: AuthSeed,
        options: TunnelOptions,
    ) -> AuthResult<TunnelPreparation> {
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(AuthFailure::new("cancelled", "认证已取消")),
            result = async {
                let _operation = self.operation.lock().await;
                self.prepare_inner(seed, options, None).await
            } => result,
        }
    }

    pub(super) async fn prepare_tunnel_owned(
        &self,
        jar: super::cookies::Jar,
        device_id: String,
        options: TunnelOptions,
    ) -> AuthResult<(TunnelPreparation, super::cookies::Jar)> {
        let _operation = self.operation.lock().await;
        let seed = AuthSeed {
            gateway_cookies: jar.auth_seed(&self.gateway),
            sso_cookies: jar.auth_seed(&self.sso),
            device_id,
        };
        let prepared = self.prepare_inner(seed, options, Some(jar)).await?;
        let jar = self
            .state
            .lock()
            .await
            .owned
            .take()
            .ok_or_else(|| AuthFailure::new("atrustCookie", "账号 Cookie 所有权丢失"))?;
        Ok((prepared, jar))
    }

    async fn prepare_inner(
        &self,
        seed: AuthSeed,
        options: TunnelOptions,
        owned: Option<super::cookies::Jar>,
    ) -> AuthResult<TunnelPreparation> {
        if options.connection_id.is_empty()
            || options.connection_id.len() > 256
            || options.sign_key.len() != 32
            || !matches!(options.process_platform.as_str(), "Windows" | "Linux")
        {
            return Err(AuthFailure::new(
                "atrustConfig",
                "隧道标识、签名密钥或平台无效",
            ));
        }
        let device_id = seed.device_id.clone();
        let mut session = self.sign_in_using(seed, owned).await?;
        let mut state = self.state.lock().await;
        let url = self.gateway_url("/passport/v1/public/authConfig", &[("needTicket", "1")])?;
        let envelope = self
            .request_json(&mut state, Method::GET, url, None)
            .await?;
        if integer(&envelope["data"]["isLogin"]) != Some(1) {
            return Err(AuthFailure::new(
                "atrustNotLoggedIn",
                "准备资源时网关会话已失效",
            ));
        }
        let csrf = envelope["data"]["csrfToken"]
            .as_str()
            .or_else(|| envelope["data"]["security"]["csrfToken"].as_str())
            .filter(|csrf| !csrf.is_empty())
            .ok_or_else(|| AuthFailure::new("atrustNoCsrf", "资源请求缺少 CSRF token"))?;
        state.csrf = csrf.to_owned();
        let mut warnings = vec![];
        let pins = match super::anti_mitm::verify(&envelope) {
            Ok(pins) => pins,
            Err(error) if options.strict_anti_mitm => return Err(error),
            Err(error) => {
                warnings.push(format!(
                    "{}；沿用兼容上报策略，保留网关公布的节点证书固定",
                    error.message
                ));
                super::anti_mitm::advertised_pins(&envelope)?
            }
        };
        let url = self.gateway_url("/controller/v1/user/clientResource", &[])?;
        let resource = self.request_json(&mut state, Method::POST, url, Some(json!({
            "resourceType":{"sdpPolicy":{},"appList":{},"favoriteAppList":{},"featureCenter":{},
                "uemSpace":{"params":{"action":"login"}}}
        }))).await?;
        let resource = super::resource::parse(&resource, self.gateway.host_str().unwrap_or(""))?;
        warnings.extend(resource.warnings);
        warnings.truncate(64);
        session.gateway_cookies = state.gateway_cookies.values().cloned().collect();
        session.sso_cookies = state.sso_cookies.values().cloned().collect();
        session.sid = self.sid(&state)?;
        let process_path = std::env::current_exe()
            .map_err(|error| {
                AuthFailure::new(
                    "clientIdentity",
                    &format!("无法读取客户端可执行路径：{error}"),
                )
            })?
            .into_os_string()
            .into_string()
            .map_err(|_| AuthFailure::new("clientIdentity", "客户端可执行路径无法编码"))?;
        let plan = sangfor_core::plan::SessionPlan {
            schema_version: 1,
            sid: session.sid.clone(),
            device_id,
            connection_id: options.connection_id,
            username: session.username.clone(),
            sign_key_base64: STANDARD.encode(options.sign_key),
            lang: "en-US".to_owned(),
            process_name: "shuvpn".to_owned(),
            process_path,
            process_platform: options.process_platform,
            nodes: resource.nodes,
            major_node_group: resource.major,
            routes: resource.routes,
            dns_servers: resource.dns,
            virtual_address: None,
            accept_any_certificate: pins.is_empty(),
            certificate_digests: pins,
            dial_hosts: BTreeMap::new(),
            heartbeat_seconds: 5.0,
            mtu: 1400,
        };
        let plan_json = serde_json::to_string(&plan)
            .map_err(|_| AuthFailure::new("atrustResource", "隧道配置编码失败"))?;
        Ok(TunnelPreparation {
            session,
            plan_json,
            warnings,
        })
    }

    async fn sign_in_inner(&self, seed: AuthSeed) -> AuthResult<AuthSession> {
        self.sign_in_using(seed, None).await
    }

    async fn sign_in_using(
        &self,
        seed: AuthSeed,
        owned: Option<super::cookies::Jar>,
    ) -> AuthResult<AuthSession> {
        if seed.device_id.is_empty() {
            return Err(AuthFailure::new("atrustDeviceId", "设备标识为空"));
        }
        let mut state = self.state.lock().await;
        // Legacy callers seed snapshots; account callers retain the full jar.
        *state = AuthState {
            gateway_cookies: seed_jar(seed.gateway_cookies, self.gateway.host_str().unwrap_or(""))?,
            sso_cookies: seed_jar(seed.sso_cookies, self.sso.host_str().unwrap_or(""))?,
            csrf: String::new(),
            owned,
        };
        let config = self
            .auth_config(&mut state, ConfigPhase::BeforeTicket)
            .await?;
        let method = auth_method(&config, &self.config.fallback_login_domain);
        let mut ticket = String::new();
        if integer(&config["isLogin"]) != Some(1) {
            let code = self
                .authorization_code(&mut state, &method.login_url)
                .await?;
            ticket = self
                .redeem_ticket(&mut state, &method.login_domain, &code)
                .await?;
            self.auth_config(&mut state, ConfigPhase::AfterTicket)
                .await?;
            let url = self.gateway_url("/controller/v1/public/reportEnv", &[])?;
            self.request_json(&mut state, Method::POST, url, Some(json!({
                "ticket": ticket,
                "deviceId": seed.device_id,
                "env": {"endpoint": {"device_id": seed.device_id, "device": {"type": "browser"}}}
            }))).await?;
            self.run_auth_steps(&mut state, &seed.device_id).await?;
        }
        let url = self.gateway_url("/passport/v1/user/onlineInfo", &[])?;
        let online = self
            .request_json(&mut state, Method::GET, url, None)
            .await?;
        purge_expired(&mut state.gateway_cookies);
        purge_expired(&mut state.sso_cookies);
        let sid = self.sid(&state)?;
        Ok(AuthSession {
            username: online["data"]["username"].as_str().unwrap_or("").to_owned(),
            sid,
            ticket,
            login_domain: method.login_domain,
            gateway_cookies: state.gateway_cookies.values().cloned().collect(),
            sso_cookies: state.sso_cookies.values().cloned().collect(),
        })
    }

    fn sid(&self, state: &AuthState) -> AuthResult<String> {
        let value = if let Some(jar) = &state.owned {
            let source = self.gateway_url("/passport/v1/user/onlineInfo", &[])?;
            jar.value(&source, "sid")?
                .or(jar.value(&source, "sid-legacy")?)
        } else {
            session_cookie(&state.gateway_cookies, "sid")
                .or_else(|| session_cookie(&state.gateway_cookies, "sid-legacy"))
                .map(|cookie| cookie.value.clone())
        };
        value
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AuthFailure::new("atrustNoSid", "aTrust 未下发有效会话凭证（sid）"))
    }

    async fn auth_config(&self, state: &mut AuthState, phase: ConfigPhase) -> AuthResult<Value> {
        let extras = match phase {
            ConfigPhase::AfterTicket => vec![("mod", "1"), ("needTicket", "0")],
            ConfigPhase::BeforeTicket => vec![("needTicket", "1")],
        };
        let url = self.gateway_url("/passport/v1/public/authConfig", &extras)?;
        let envelope = self.request_json(state, Method::GET, url, None).await?;
        let data = &envelope["data"];
        let csrf = data["csrfToken"]
            .as_str()
            .or_else(|| data["security"]["csrfToken"].as_str())
            .filter(|value| !value.is_empty());
        if let Some(csrf) = csrf {
            state.csrf = csrf.to_owned();
        }
        if state.csrf.is_empty() {
            return Err(AuthFailure::new(
                "atrustNoCsrf",
                "aTrust 未下发会话令牌（csrfToken）",
            ));
        }
        Ok(data.clone())
    }

    async fn authorization_code(
        &self,
        state: &mut AuthState,
        login_url: &str,
    ) -> AuthResult<String> {
        if let Ok(url) = Url::parse(login_url) {
            // A gateway-provided URL must not receive cookies for another origin.
            if url.origin() == self.sso.origin() || url.origin() == self.gateway.origin() {
                if let Ok(response) = self.send(state, Method::GET, url, None).await {
                    if response.status().is_redirection() {
                        if let Ok(raw) = location(&response) {
                            if let Ok(destination) = response.url().join(&raw) {
                                if let Some(code) = query_value(destination.as_str(), "code")
                                    .filter(|code| !code.is_empty())
                                {
                                    let _ = code;
                                    return self.callback_code(&destination);
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut url = self
            .sso
            .join("/oauth/authorize")
            .map_err(|_| AuthFailure::new("atrustConfig", "统一认证地址无效"))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri);
        if !self.config.scope.is_empty() {
            url.query_pairs_mut()
                .append_pair("scope", &self.config.scope);
        }
        let response = self.send(state, Method::GET, url, None).await?;
        let location = location(&response)?;
        if !response.status().is_redirection() {
            return Err(AuthFailure::new(
                "authorizeFailed",
                "统一认证未返回 aTrust 的授权地址",
            ));
        }
        let destination = self
            .sso
            .join(&location)
            .map_err(|_| AuthFailure::new("authorizeFailed", "统一认证返回了无效跳转"))?;
        if destination.path().contains("/oauth2/login/") {
            return Err(AuthFailure::new(
                "sessionNotReused",
                "统一身份认证会话未能复用，请重新登录校园账户",
            ));
        }
        self.callback_code(&destination)
    }

    fn callback_code(&self, destination: &Url) -> AuthResult<String> {
        let codes: Vec<_> = destination
            .query_pairs()
            .filter(|(name, _)| name == "code")
            .collect();
        let states: Vec<_> = destination
            .query_pairs()
            .filter(|(name, _)| name == "state")
            .collect();
        if destination.origin() != self.gateway.origin()
            || destination.path() != "/passport/v1/auth/httpsOauth2"
            || !destination.username().is_empty()
            || destination.password().is_some()
            || destination.fragment().is_some()
            || codes.len() != 1
            || codes[0].1.is_empty()
            || codes[0].1.len() > 4096
            || states.len() > 1
        {
            return Err(AuthFailure::new(
                "authorizeFailed",
                "统一认证返回了未预期的回调地址",
            ));
        }
        Ok(codes[0].1.to_string())
    }

    async fn redeem_ticket(
        &self,
        state: &mut AuthState,
        domain: &str,
        code: &str,
    ) -> AuthResult<String> {
        let mut url = self
            .gateway
            .join("/passport/v1/auth/httpsOauth2")
            .map_err(|_| AuthFailure::new("atrustConfig", "认证回调地址无效"))?;
        url.query_pairs_mut().extend_pairs([
            ("sfDomain", domain),
            ("code", code),
            ("state", "null"),
        ]);
        let response = self.send(state, Method::GET, url, None).await?;
        let destination = self
            .gateway
            .join(&location(&response)?)
            .map_err(|_| AuthFailure::new("atrustTicketFailed", "aTrust 返回了无效跳转"))?;
        if !response.status().is_redirection()
            || destination.origin() != self.gateway.origin()
            || destination.path() != "/portal/shortcut.html"
        {
            return Err(AuthFailure::new(
                "atrustTicketFailed",
                "aTrust 返回了未预期的跳转地址",
            ));
        }
        let data = query_value(destination.as_str(), "data")
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
        data.and_then(|data| data["ticket"].as_str().map(str::to_owned))
            .filter(|ticket| !ticket.is_empty())
            .ok_or_else(|| {
                AuthFailure::new("atrustTicketFailed", "aTrust 未下发登录票据（ticket）")
            })
    }

    async fn run_auth_steps(&self, state: &mut AuthState, device: &str) -> AuthResult<()> {
        let mut service = "auth/authCheck".to_owned();
        let mut auth_id: Option<String> = None;
        for _ in 0..8 {
            match service.as_str() {
                "auth/authCheck"
                | "auth/accessCheck"
                | "auth/preEnhancedAuth"
                | "auth/enhancedConfirm"
                | "auth/enhancedDone"
                | "auth/bindAuthDevice" => {}
                _ => {
                    return Err(AuthFailure::new(
                        "atrustChallenge",
                        "aTrust 要求额外验证，请稍后重试",
                    ))
                }
            }
            let mut extras = vec![];
            if !matches!(service.as_str(), "auth/authCheck" | "auth/accessCheck") {
                if let Some(id) = auth_id.as_deref() {
                    extras.push(("authId", id));
                }
            }
            if service == "auth/bindAuthDevice" {
                extras.push(("deviceId", device));
            }
            let url = self.gateway_url(&format!("/passport/v1/{service}"), &extras)?;
            let envelope = self.request_json(state, Method::GET, url, None).await?;
            let data = &envelope["data"];
            let services = data["nextServiceList"].as_array();
            let raw = data["nextService"].as_str();
            let selected = services.and_then(|items| {
                items
                    .iter()
                    .find(|item| item["authType"].as_str() == raw)
                    .or_else(|| items.first())
            });
            auth_id = selected
                .and_then(|item| item["authId"].as_str())
                .map(str::to_owned);
            service = raw
                .or_else(|| selected.and_then(|item| item["authType"].as_str()))
                .unwrap_or("")
                .to_owned();
            if service == "auth/sendSms" || (service.is_empty() && auth_id.is_some()) {
                service = "auth/sms".to_owned();
            }
            if service.is_empty() {
                return Ok(());
            }
        }
        Err(AuthFailure::new(
            "atrustAuthChain",
            "aTrust 认证步骤过多，已中止",
        ))
    }

    fn gateway_url(&self, path: &str, extras: &[(&str, &str)]) -> AuthResult<Url> {
        let mut url = self
            .gateway
            .join(path)
            .map_err(|_| AuthFailure::new("atrustConfig", "aTrust 端点地址无效"))?;
        url.query_pairs_mut()
            .extend_pairs(SHARED_QUERY)
            .extend_pairs(extras.iter().copied());
        Ok(url)
    }

    async fn send(
        &self,
        state: &mut AuthState,
        method: Method,
        url: Url,
        body: Option<Value>,
    ) -> AuthResult<Response> {
        let gateway_request = url.origin() == self.gateway.origin();
        if !gateway_request && url.origin() != self.sso.origin() {
            return Err(AuthFailure::new("atrustOrigin", "认证请求来源不匹配"));
        }
        let cookies = {
            let jar = if gateway_request {
                &mut state.gateway_cookies
            } else {
                &mut state.sso_cookies
            };
            purge_expired(jar);
            let mut matching: Vec<_> = jar
                .values()
                .filter(|cookie| {
                    path_matches(url.path(), &cookie.path)
                        && domain_matches(url.host_str().unwrap_or(""), &cookie.domain)
                })
                .collect();
            matching.sort_by_key(|cookie| std::cmp::Reverse(cookie.path.len()));
            matching
                .into_iter()
                .map(|cookie| format!("{}={}", cookie.name, cookie.value))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let cookies = if let Some(owned) = &state.owned {
            owned.header(&url)?
        } else if cookies.is_empty() {
            None
        } else {
            let mut value = header::HeaderValue::from_str(&cookies)
                .map_err(|_| AuthFailure::new("atrustCookie", "认证 Cookie 格式无效"))?;
            value.set_sensitive(true);
            Some(value)
        };
        let lease = HttpLease::open(&url)
            .await
            .map_err(|error| AuthFailure::new(&error.code, &error.message))?;
        let builder = lease
            .builder()
            .use_rustls_tls()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .user_agent(&self.config.user_agent);
        #[cfg(test)]
        let builder = if let Some((certificate, address)) = &self.fixture {
            builder
                .add_root_certificate(
                    reqwest::Certificate::from_der(certificate).map_err(network_failure)?,
                )
                .resolve(url.host_str().unwrap_or(""), *address)
        } else {
            builder
        };
        let client = builder.build().map_err(network_failure)?;
        let mut request = client
            .request(method, url)
            .header("x-sdp-rid", STANDARD.encode(self.gateway.authority()))
            .header("x-sdp-traceid", random_hex(4)?);
        if let Some(value) = cookies {
            request = request.header(header::COOKIE, value);
        }
        if gateway_request && !state.csrf.is_empty() {
            let mut value = header::HeaderValue::from_str(&state.csrf)
                .map_err(|_| AuthFailure::new("atrustNoCsrf", "会话令牌格式无效"))?;
            value.set_sensitive(true);
            request = request.header("x-csrf-token", value);
        }
        request = if let Some(body) = body {
            request.json(&body)
        } else {
            request.header(header::ACCEPT, "application/json")
        };
        let response = request.send().await.map_err(network_failure)?;
        if let Some(owned) = state.owned.as_mut() {
            for value in response.headers().get_all(header::SET_COOKIE) {
                if let Ok(raw) = value.to_str() {
                    owned.save(response.url(), raw)?;
                }
            }
            state.gateway_cookies = seed_jar(
                owned.auth_seed(&self.gateway),
                self.gateway.host_str().unwrap_or(""),
            )?;
            state.sso_cookies = seed_jar(
                owned.auth_seed(&self.sso),
                self.sso.host_str().unwrap_or(""),
            )?;
            return Ok(Response {
                inner: response,
                _lease: lease,
                _client: client,
            });
        }
        let jar = if gateway_request {
            &mut state.gateway_cookies
        } else {
            &mut state.sso_cookies
        };
        for header in response.headers().get_all(header::SET_COOKIE) {
            if let Ok(raw) = header.to_str() {
                if let Ok(cookie) = Cookie::parse(raw.to_owned()) {
                    let name = cookie.name().to_owned();
                    let domain = cookie
                        .domain()
                        .unwrap_or(response.url().host_str().unwrap_or(""))
                        .trim_start_matches('.')
                        .to_ascii_lowercase();
                    if !domain_matches(response.url().host_str().unwrap_or(""), &domain) {
                        continue;
                    }
                    let path = cookie
                        .path()
                        .filter(|path| path.starts_with('/'))
                        .unwrap_or_else(|| default_cookie_path(response.url().path()))
                        .to_owned();
                    let key = (name.clone(), domain.clone(), path.clone());
                    let expired = cookie.max_age().is_some_and(|age| age.whole_seconds() <= 0);
                    let expires = cookie
                        .max_age()
                        .map(|age| now_seconds().saturating_add(age.whole_seconds()))
                        .or_else(|| cookie.expires_datetime().map(|time| time.unix_timestamp()));
                    if cookie.value().is_empty()
                        || expired
                        || expires.is_some_and(|time| time <= now_seconds())
                    {
                        jar.remove(&key);
                    } else {
                        jar.insert(
                            key,
                            AuthCookie {
                                name,
                                domain,
                                value: cookie.value().to_owned(),
                                path,
                                expires_unix_seconds: expires,
                            },
                        );
                    }
                }
            }
        }
        Ok(Response {
            inner: response,
            _lease: lease,
            _client: client,
        })
    }

    async fn request_json(
        &self,
        state: &mut AuthState,
        method: Method,
        url: Url,
        body: Option<Value>,
    ) -> AuthResult<Value> {
        let mut response = self.send(state, method, url, body).await?;
        if !response.status().is_success() {
            return Err(AuthFailure::new(
                &format!("atrustHttp{}", response.status().as_u16()),
                "aTrust 认证服务请求失败",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_failure)? {
            if chunk.len() > RESPONSE_LIMIT.saturating_sub(bytes.len()) {
                return Err(AuthFailure::new(
                    "atrustBadResponse",
                    "认证响应超过大小限制",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let envelope: Value = serde_json::from_slice(&bytes).map_err(|_| {
            AuthFailure::new("atrustBadResponse", "aTrust 认证服务返回了无法识别的内容")
        })?;
        if !envelope.is_object() || integer(&envelope["code"]).is_none() {
            return Err(AuthFailure::new(
                "atrustBadResponse",
                "aTrust 认证响应格式无效",
            ));
        }
        if integer(&envelope["code"]) != Some(0) {
            return Err(AuthFailure::new(
                "atrustApiFailed",
                "aTrust 网关拒绝了认证请求",
            ));
        }
        Ok(envelope)
    }
}

pub(crate) fn random_hex(size: usize) -> AuthResult<String> {
    let mut bytes = vec![0; size];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| AuthFailure::new("atrustRandom", "无法生成安全随机标识"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn network_failure(error: reqwest::Error) -> AuthFailure {
    // OAuth codes and tickets must not appear in a transport error URL.
    AuthFailure::new(
        "atrustNetwork",
        &format!("aTrust 认证请求失败：{}", error.without_url()),
    )
}

fn base_url(raw: &str) -> AuthResult<Url> {
    let url = Url::parse(raw).map_err(|_| AuthFailure::new("atrustConfig", "认证端点地址无效"))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(AuthFailure::new(
            "atrustConfig",
            "认证端点必须是 HTTPS 来源地址",
        ));
    }
    Ok(url)
}

fn seed_jar(cookies: Vec<AuthCookie>, host: &str) -> AuthResult<CookieJar> {
    let mut jar = CookieJar::new();
    for mut cookie in cookies {
        cookie.domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
        if !domain_matches(host, &cookie.domain)
            || !cookie.path.starts_with('/')
            || cookie.name.is_empty()
            || cookie
                .name
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && !b"!#$%&'*+-.^_`|~".contains(&byte))
            || cookie.value.contains(['\r', '\n', ';'])
        {
            return Err(AuthFailure::new("atrustCookie", "认证 Cookie 格式无效"));
        }
        if !cookie.value.is_empty() {
            jar.insert(
                (
                    cookie.name.clone(),
                    cookie.domain.clone(),
                    cookie.path.clone(),
                ),
                cookie,
            );
        }
    }
    purge_expired(&mut jar);
    Ok(jar)
}

fn purge_expired(jar: &mut CookieJar) {
    jar.retain(|_, cookie| {
        cookie
            .expires_unix_seconds
            .is_none_or(|time| time > now_seconds())
    });
}

fn session_cookie<'a>(jar: &'a CookieJar, name: &str) -> Option<&'a AuthCookie> {
    jar.values()
        .filter(|cookie| {
            cookie.name == name && path_matches("/passport/v1/user/onlineInfo", &cookie.path)
        })
        .max_by_key(|cookie| cookie.path.len())
}

fn path_matches(request: &str, cookie: &str) -> bool {
    request == cookie
        || (request.starts_with(cookie)
            && (cookie.ends_with('/') || request.as_bytes().get(cookie.len()) == Some(&b'/')))
}

fn domain_matches(host: &str, domain: &str) -> bool {
    host == domain
        || (!domain.is_empty()
            && host.parse::<std::net::IpAddr>().is_err()
            && host.ends_with(&format!(".{domain}")))
}

fn default_cookie_path(request: &str) -> &str {
    match request.rfind('/') {
        Some(index) if index > 0 => &request[..index],
        _ => "/",
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn auth_method(data: &Value, fallback: &str) -> AuthMethod {
    let method = data["authServerInfoList"].as_array().and_then(|methods| {
        methods
            .iter()
            .find(|method| method["authType"] == "auth/httpsOauth2")
    });
    AuthMethod {
        login_domain: method
            .and_then(|method| method["loginDomain"].as_str())
            .filter(|domain| !domain.is_empty())
            .unwrap_or(fallback)
            .to_owned(),
        login_url: method
            .and_then(|method| method["loginUrl"].as_str())
            .unwrap_or("")
            .to_owned(),
    }
}

fn integer(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn location(response: &Response) -> AuthResult<String> {
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .ok_or_else(|| AuthFailure::new("authorizeFailed", "认证服务未返回跳转地址"))
}

fn query_value(raw: &str, name: &str) -> Option<String> {
    Url::parse(raw)
        .or_else(|_| Url::parse("https://relative.invalid/").and_then(|base| base.join(raw)))
        .ok()?
        .query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}
