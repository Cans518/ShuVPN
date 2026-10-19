use super::{cancelled_failure, live, model::AuthorizationReceipt, Client, Transaction};
use crate::auth::model::{AuthConfig, AuthFailure};
use reqwest::Url;
use std::time::Duration;
pub(super) mod target;
#[cfg(test)]
pub(in crate::auth::newsso) mod tests;
use target::Target;

impl Client {
    pub(crate) async fn authorize_system(
        &self,
        system_id: String,
    ) -> Result<AuthorizationReceipt, AuthFailure> {
        let target = Target::new(&self.config, &system_id)?;
        let (generation, cancelled) = {
            let state = self.state()?;
            live(&state)?;
            (state.generation, state.cancelled.clone())
        };
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                self.require_session(&transaction)?;
                let result = tokio::time::timeout(Duration::from_secs(90), self.exchange(&mut transaction, &target)).await
                    .map_err(|_| AuthFailure::new("timeout", "业务授权超时"))?;
                self.finish(transaction, result)
            } => result,
        }
    }

    pub(super) fn require_session(&self, transaction: &Transaction) -> Result<(), AuthFailure> {
        let source = self
            .config
            .sso
            .join("/oauth/authorize")
            .map_err(|_| bad_response())?;
        if transaction.challenge.is_some()
            || transaction.qr.is_some()
            || !transaction
                .jar
                .names(&source)
                .iter()
                .any(|name| name == "SHU_OAUTH2")
        {
            return Err(AuthFailure::new("sessionMissing", "请先完成统一身份认证"));
        }
        Ok(())
    }

    pub(super) async fn exchange(
        &self,
        transaction: &mut Transaction,
        target: &Target,
    ) -> Result<AuthorizationReceipt, AuthFailure> {
        let state = if target.id == "jwxt" {
            crate::auth::atrust::random_hex(16)?
        } else {
            let bootstrap = self
                .config
                .page(&mut transaction.jar, target.base.clone(), &self.config.sso)
                .await?;
            let discovered =
                state_from_bootstrap(&target.base, bootstrap.location.as_deref(), &bootstrap.body)?;
            if let Some(state) = discovered {
                transaction
                    .oauth_states
                    .insert(target.id.to_owned(), state.clone());
                state
            } else {
                transaction
                    .oauth_states
                    .get(target.id)
                    .cloned()
                    .unwrap_or_default()
            }
        };
        let mut uri = self
            .config
            .sso
            .join("/oauth/authorize")
            .map_err(|_| bad_response())?;
        uri.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", target.client_id)
            .append_pair("redirect_uri", target.callback.as_str());
        if !target.scope.is_empty() {
            uri.query_pairs_mut().append_pair("scope", target.scope);
        }
        if !state.is_empty() {
            uri.query_pairs_mut().append_pair("state", &state);
        }
        let response = self
            .config
            .page(&mut transaction.jar, uri.clone(), &self.config.sso)
            .await?;
        if !response.status.is_redirection() {
            return Err(AuthFailure::new(
                "authorizeFailed",
                "统一认证未返回业务授权地址",
            ));
        }
        let location = response.location.as_deref().ok_or_else(bad_response)?;
        let callback = uri.join(location).map_err(|_| bad_response())?;
        if callback.origin() == self.config.sso.origin()
            && callback.path().starts_with("/oauth2/login/")
        {
            return Err(AuthFailure::new(
                "sessionNotReused",
                "统一身份认证会话未能复用",
            ));
        }
        self.config.validate(&callback)?;
        target.validate_callback(&callback, &state)?;
        let mut landed = self.follow_system(transaction, callback, target).await?;
        if target.id == "otp" && !matches_landing(target, &landed.0, &landed.1.body) {
            landed = self
                .follow_system(
                    transaction,
                    target
                        .base
                        .join("/Default.aspx")
                        .map_err(|_| bad_response())?,
                    target,
                )
                .await?;
        }
        if !landed.1.status.is_success() || !matches_landing(target, &landed.0, &landed.1.body) {
            return Err(AuthFailure::new(
                "systemSessionFailed",
                "业务系统未建立可确认的会话",
            ));
        }
        let mut safe = landed.0;
        safe.set_query(None);
        safe.set_fragment(None);
        Ok(AuthorizationReceipt {
            system_id: target.id.to_owned(),
            landing_url: safe.into(),
            http_status: Some(landed.1.status.as_u16()),
            username: None,
        })
    }

    async fn follow_system(
        &self,
        transaction: &mut Transaction,
        start: Url,
        target: &Target,
    ) -> Result<(Url, super::transport::Response), AuthFailure> {
        let mut uri = start;
        for hop in 0..=10 {
            if uri.origin() != target.base.origin()
                || !uri.username().is_empty()
                || uri.password().is_some()
                || uri.fragment().is_some()
            {
                return Err(AuthFailure::new(
                    "unsafeRedirect",
                    "业务系统返回了不受信任的跳转",
                ));
            }
            let response = self
                .config
                .page(&mut transaction.jar, uri.clone(), &self.config.sso)
                .await?;
            if !response.status.is_redirection() {
                return Ok((uri, response));
            }
            if hop == 10 {
                return Err(AuthFailure::new("tooManyRedirects", "业务系统跳转次数过多"));
            }
            uri = uri
                .join(response.location.as_deref().ok_or_else(bad_response)?)
                .map_err(|_| bad_response())?;
        }
        Err(bad_response())
    }

    pub(crate) async fn sign_in_atrust(
        &self,
        device_id: String,
    ) -> Result<AuthorizationReceipt, AuthFailure> {
        if device_id.is_empty() || device_id.len() > 256 {
            return Err(AuthFailure::new("atrustDeviceId", "设备标识无效"));
        }
        let (generation, cancelled) = {
            let state = self.state()?;
            live(&state)?;
            (state.generation, state.cancelled.clone())
        };
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                self.require_session(&transaction)?;
                let result = tokio::time::timeout(Duration::from_secs(90), self.atrust_owned(&mut transaction, device_id)).await
                    .map_err(|_| AuthFailure::new("timeout", "原生网关认证超时"))?;
                self.finish(transaction, result)
            } => result,
        }
    }

    async fn atrust_owned(
        &self,
        transaction: &mut Transaction,
        device_id: String,
    ) -> Result<AuthorizationReceipt, AuthFailure> {
        let gateway = self.config.system_base("atrust")?;
        let client = self.atrust_client()?;
        let (session, jar) = client
            .sign_in_owned(transaction.jar.clone(), device_id)
            .await?;
        transaction.jar = jar;
        Ok(AuthorizationReceipt {
            system_id: "atrust".to_owned(),
            landing_url: gateway
                .join("/passport/v1/user/onlineInfo")
                .map_err(|_| bad_response())?
                .into(),
            http_status: None,
            username: Some(session.username),
        })
    }

    pub(super) fn atrust_client(&self) -> Result<crate::auth::atrust::ATrustClient, AuthFailure> {
        let gateway = self.config.system_base("atrust")?;
        let callback = gateway
            .join("/passport/v1/auth/httpsOauth2")
            .map_err(|_| bad_response())?;
        let config = AuthConfig {
            gateway_base: gateway.to_string(), sso_base: self.config.sso.to_string(),
            client_id: "u5kTNw6T59kw3wkrwZ76e44kBec4HTvv".to_owned(),
            redirect_uri: callback.to_string(), scope: String::new(),
            fallback_login_domain: "customOAuth76881".to_owned(),
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) aTrustTray/2.4.10.50 Chrome/83.0.4103.94 Electron/9.0.2 Safari/537.36 aTrustTray-Linux-Plat-Ubuntu-x64 SPCClientType".to_owned(),
        };
        let client = crate::auth::atrust::ATrustClient::new(config)?;
        #[cfg(test)]
        let client = if let Some((Some(certificate), address)) = &self.config.fixture {
            client.with_fixture(certificate.clone(), *address)
        } else {
            client
        };
        Ok(client)
    }
}

fn matches_landing(target: &Target, uri: &Url, body: &[u8]) -> bool {
    let body = String::from_utf8_lossy(body);
    match target.id {
        "jwxt" => uri.path().contains("jwglxt") || body.contains("教学管理"),
        "otp" => uri.path().contains("Default.aspx") || body.contains("账户名"),
        _ => false,
    }
}

fn state_from_bootstrap(
    base: &Url,
    location: Option<&str>,
    body: &[u8],
) -> Result<Option<String>, AuthFailure> {
    let from_url = |raw: &str| -> Option<String> {
        let uri = base.join(raw).ok()?;
        let states: Vec<_> = uri
            .query_pairs()
            .filter(|(name, _)| name == "state")
            .collect();
        (states.len() == 1 && !states[0].1.is_empty() && states[0].1.len() <= 8192)
            .then(|| states[0].1.to_string())
    };
    if let Some(state) = location.and_then(from_url) {
        return Ok(Some(state));
    }
    let body = std::str::from_utf8(body).map_err(|_| bad_response())?;
    let link = regex::Regex::new(r#"(/oauth2/login/[^"'\s<>]+)"#).map_err(|_| bad_response())?;
    if let Some(state) = link
        .captures(body)
        .and_then(|capture| capture.get(1))
        .and_then(|capture| from_url(&capture.as_str().replace("&amp;", "&")))
    {
        return Ok(Some(state));
    }
    let hidden = regex::Regex::new(r#"name=["']state["'][^>]*value=["']([^"']+)["']"#)
        .map_err(|_| bad_response())?;
    Ok(hidden
        .captures(body)
        .and_then(|capture| capture.get(1))
        .filter(|value| value.as_str().len() <= 8192)
        .map(|value| value.as_str().to_owned()))
}
fn bad_response() -> AuthFailure {
    AuthFailure::new("authorizeResponse", "业务授权响应格式无效")
}
