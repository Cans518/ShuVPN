use super::{cancelled_failure, live, model::OtpCode, oauth::target::Target, Client, Transaction};
use crate::auth::model::AuthFailure;
use std::time::Duration;
mod parse;

impl Client {
    pub(crate) async fn fetch_otp(&self) -> Result<OtpCode, AuthFailure> {
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
                let result = tokio::time::timeout(Duration::from_secs(90), self.otp_exchange(&mut transaction))
                    .await.map_err(|_| AuthFailure::new("otpNetwork", "动态口令请求超时"))?;
                self.finish(transaction, result)
            } => result,
        }
    }

    async fn otp_exchange(&self, transaction: &mut Transaction) -> Result<OtpCode, AuthFailure> {
        match self.otp_retry(transaction).await {
            Ok(code) => Ok(code),
            Err(error) if matches!(error.code.as_str(), "otpSessionExpired" | "otpParseFailed") => {
                self.exchange(transaction, &Target::new(&self.config, "otp")?)
                    .await?;
                self.otp_retry(transaction).await
            }
            Err(error) => Err(error),
        }
    }

    async fn otp_retry(&self, transaction: &mut Transaction) -> Result<OtpCode, AuthFailure> {
        for attempt in 0..3 {
            let attempt_jar = transaction.jar.clone();
            let result = self.otp_once(transaction).await;
            if result.is_err() {
                transaction.jar = attempt_jar;
            }
            match result {
                Err(error) if error.code == "otpNetwork" && attempt < 2 => {
                    tokio::time::sleep(Duration::from_millis(250 * (attempt + 1))).await;
                }
                result => return result,
            }
        }
        Err(AuthFailure::new("otpNetwork", "动态口令请求失败"))
    }

    async fn otp_once(&self, transaction: &mut Transaction) -> Result<OtpCode, AuthFailure> {
        let base = self.config.system_base("otp")?;
        for path in ["/Default.aspx", "/"] {
            let mut uri = base.join(path).map_err(|_| unsafe_redirect())?;
            for hop in 0..=5 {
                let response = self
                    .config
                    .otp_page(&mut transaction.jar, uri.clone())
                    .await
                    .map_err(|error| {
                        if matches!(error.code.as_str(), "network" | "timeout") {
                            AuthFailure::new("otpNetwork", "无法连接动态口令服务")
                        } else {
                            error
                        }
                    })?;
                if response.status.is_redirection() {
                    let next = uri
                        .join(response.location.as_deref().ok_or_else(unsafe_redirect)?)
                        .map_err(|_| unsafe_redirect())?;
                    self.config.validate(&next).map_err(|_| unsafe_redirect())?;
                    if next.origin() == self.config.sso.origin()
                        && next.path().starts_with("/oauth2/login/")
                    {
                        return Err(AuthFailure::new("otpSessionExpired", "动态口令会话已失效"));
                    }
                    if next.origin() != base.origin() {
                        return Err(unsafe_redirect());
                    }
                    if hop == 5 {
                        return Err(AuthFailure::new("otpParseFailed", "动态口令页跳转次数过多"));
                    }
                    uri = next;
                    continue;
                }
                if response.status.is_server_error() {
                    return Err(AuthFailure::new("otpNetwork", "动态口令服务暂时不可用"));
                }
                if response.status.as_u16() == 200 {
                    if let Some(code) = parse::page(&response.body, response.refresh.as_deref()) {
                        return Ok(code);
                    }
                }
                break;
            }
        }
        Err(AuthFailure::new("otpParseFailed", "页面里没有找到口令节点"))
    }
}

fn unsafe_redirect() -> AuthFailure {
    AuthFailure::new("otpUnsafeRedirect", "动态口令服务返回了不受信任的跳转")
}
