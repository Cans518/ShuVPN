use super::{
    cancelled_failure, live,
    model::{LoginResult, QrSession, QrStatus},
    transport::QrRequest,
    Client, Transaction,
};
use crate::auth::model::AuthFailure;
use reqwest::{Method, Url};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

mod protocol;
#[cfg(test)]
pub(in crate::auth::newsso) mod tests;
use protocol::failure;

#[derive(Clone)]
pub(super) struct Pending {
    pub status: QrStatus,
    key: String,
    code: Option<zeroize::Zeroizing<String>>,
    deadline: Instant,
}

impl Client {
    pub(crate) async fn start_qr(&self) -> Result<QrSession, AuthFailure> {
        let (generation, cancelled) = self.reset()?;
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                let result = self.create_qr(&mut transaction).await;
                self.finish(transaction, result)
            } => result,
        }
    }

    async fn create_qr(&self, transaction: &mut Transaction) -> Result<QrSession, AuthFailure> {
        let deadline = Instant::now() + Duration::from_secs(180);
        let provider = self.config.wecom()?;
        let redirect = self
            .config
            .sso
            .join("/oauth/wecom/qrcode")
            .map_err(|_| failure("qrcodeConfig", "扫码回调配置无效"))?;
        let mut uri = provider
            .join("/wwopen/sso/qrConnect")
            .map_err(|_| failure("qrcodeConfig", "扫码配置无效"))?;
        uri.query_pairs_mut().extend_pairs([
            ("appid", protocol::APP_ID),
            ("agentid", protocol::AGENT_ID),
            ("redirect_uri", redirect.as_str()),
            ("state", protocol::state().as_str()),
            ("lang", "zh"),
            ("version", "1.2.7"),
            ("login_type", "jssdk"),
        ]);
        let response = self
            .config
            .qr_request(&mut transaction.jar, uri, QrRequest::Page)
            .await?;
        if !response.status.is_success() {
            return Err(failure("qrcodeHttp", "无法获取企业微信二维码"));
        }
        let key = protocol::key(&response.body)?;
        let mut image = provider
            .join("/wwopen/sso/qrImg")
            .map_err(|_| failure("qrcodeConfig", "二维码配置无效"))?;
        image.query_pairs_mut().append_pair("key", &key);
        let image = self
            .config
            .qr_request(&mut transaction.jar, image, QrRequest::Image)
            .await?;
        if !image.status.is_success() {
            return Err(failure("qrcodeImage", "无法获取二维码图片"));
        }
        protocol::image(&image.body)?;
        let mut confirm = provider
            .join("/wwopen/sso/confirm2")
            .map_err(|_| failure("qrcodeConfig", "扫码配置无效"))?;
        confirm
            .query_pairs_mut()
            .append_pair("k", &key)
            .append_pair("notretry", "yes");
        let mut scheme =
            Url::parse("wxwork://sso/jump").map_err(|_| failure("qrcodeConfig", "扫码配置无效"))?;
        scheme
            .query_pairs_mut()
            .append_pair("url", confirm.as_str());
        transaction.qr = Some(Pending {
            key,
            status: QrStatus::Waiting,
            code: None,
            deadline,
        });
        Ok(QrSession {
            image_bytes: image.body,
            confirm_url: confirm.into(),
            wx_work_scheme_url: scheme.into(),
        })
    }

    pub(crate) async fn poll_qr(&self) -> Result<QrStatus, AuthFailure> {
        let (generation, cancelled) = {
            let state = self.state()?;
            live(&state)?;
            (state.generation, state.cancelled.clone())
        };
        self.poll_generation(generation, cancelled).await
    }

    async fn poll_generation(
        &self,
        generation: u64,
        cancelled: CancellationToken,
    ) -> Result<QrStatus, AuthFailure> {
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                let result = self.poll_inner(&mut transaction).await;
                self.finish(transaction, result)
            } => result,
        }
    }

    async fn poll_inner(&self, transaction: &mut Transaction) -> Result<QrStatus, AuthFailure> {
        let pending = transaction
            .qr
            .as_mut()
            .ok_or_else(|| failure("qrMissing", "没有活动的扫码会话"))?;
        if Instant::now() >= pending.deadline {
            pending.status = QrStatus::Expired;
            pending.code = None;
        }
        if matches!(pending.status, QrStatus::Succeeded | QrStatus::Expired) {
            return Ok(pending.status);
        }
        let key = pending.key.clone();
        let deadline = pending.deadline;
        let provider = self.config.wecom()?;
        let redirect = self
            .config
            .sso
            .join("/oauth/wecom/qrcode")
            .map_err(|_| failure("qrcodeConfig", "扫码配置无效"))?;
        let mut uri = provider
            .join("/wwopen/sso/l/qrConnect")
            .map_err(|_| failure("qrcodeConfig", "扫码配置无效"))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
            .to_string();
        uri.query_pairs_mut().extend_pairs([
            ("callback", "jsonpCallback"),
            ("key", key.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("appid", protocol::APP_ID),
            ("_", stamp.as_str()),
        ]);
        let result = tokio::time::timeout_at(
            deadline,
            self.config
                .qr_request(&mut transaction.jar, uri, QrRequest::Poll),
        )
        .await;
        let (status, code) = match result {
            Ok(Ok(response)) if response.status.is_success() => protocol::scan(&response.body),
            Err(_) => (QrStatus::Expired, None),
            Ok(Err(error)) if matches!(error.code.as_str(), "network" | "timeout") => {
                (QrStatus::Waiting, None)
            }
            Ok(Err(error)) => return Err(error),
            _ => (QrStatus::Waiting, None),
        };
        let (status, code) = if Instant::now() >= deadline {
            (QrStatus::Expired, None)
        } else {
            (status, code)
        };
        let pending = transaction
            .qr
            .as_mut()
            .ok_or_else(|| failure("qrMissing", "扫码会话已丢失"))?;
        pending.status = status;
        pending.code = code;
        Ok(status)
    }

    pub(crate) async fn wait_qr(&self, timeout_seconds: u32) -> Result<QrStatus, AuthFailure> {
        if !(1..=180).contains(&timeout_seconds) {
            return Err(failure("qrTimeout", "扫码等待时间必须为 1 至 180 秒"));
        }
        let (generation, cancelled) = {
            let state = self.state()?;
            live(&state)?;
            if state.qr.is_none() {
                return Err(failure("qrMissing", "没有活动的扫码会话"));
            }
            (state.generation, state.cancelled.clone())
        };
        let deadline = Instant::now() + Duration::from_secs(u64::from(timeout_seconds));
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = tokio::time::timeout_at(deadline, async {
                loop {
                    let status = self.poll_generation(generation, cancelled.clone()).await?;
                    if matches!(status, QrStatus::Succeeded | QrStatus::Expired) { return Ok(status); }
                    tokio::time::sleep(if status == QrStatus::ConfirmedPending { Duration::from_millis(500) } else { Duration::from_secs(1) }).await;
                }
            }) => match result {
                Ok(result) => result,
                Err(_) => {
                    let mut state = self.state()?;
                    if state.generation != generation || state.cancelled.is_cancelled() { return Err(cancelled_failure()); }
                    let next = state.generation.checked_add(1).ok_or_else(|| failure("ssoState", "认证会话代次超过限制"))?;
                    state.cancelled.cancel();
                    state.generation = next;
                    state.cancelled = CancellationToken::new();
                    if let Some(qr) = state.qr.as_mut() { qr.status = QrStatus::Expired; qr.code = None; }
                    Ok(QrStatus::Expired)
                },
            },
        }
    }

    pub(crate) async fn redeem_qr(&self) -> Result<LoginResult, AuthFailure> {
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
                let code = {
                    let mut state = self.state()?;
                    if state.generation != generation { return Err(cancelled_failure()); }
                    let pending = state.qr.as_mut().ok_or_else(|| failure("qrMissing", "没有活动的扫码会话"))?;
                    if pending.status != QrStatus::Succeeded || Instant::now() >= pending.deadline {
                        return Err(failure("qrNotConfirmed", "扫码尚未确认或已经过期"));
                    }
                    let code = pending.code.take().ok_or_else(|| failure("qrUsed", "扫码授权码已使用"))?;
                    // Do not replay a single-use code after a failed network or disk commit.
                    pending.status = QrStatus::Expired;
                    transaction.qr = Some(pending.clone());
                    code
                };
                let result = self.redeem_inner(&mut transaction, &code).await;
                self.finish(transaction, result)
            } => result,
        }
    }

    async fn redeem_inner(
        &self,
        transaction: &mut Transaction,
        code: &str,
    ) -> Result<LoginResult, AuthFailure> {
        let mut uri = self
            .config
            .sso
            .join("/oauth/wecom/qrcode")
            .map_err(|_| failure("qrcodeConfig", "扫码配置无效"))?;
        uri.query_pairs_mut().extend_pairs([
            ("code", code),
            ("state", protocol::state().as_str()),
            ("appid", protocol::APP_ID),
        ]);
        let response = self
            .config
            .request(
                &mut transaction.jar,
                Method::GET,
                uri,
                None,
                Some(&self.config.sso),
            )
            .await?;
        let location = response
            .location
            .as_deref()
            .filter(|location| !location.contains("wecomAuthFailed"))
            .ok_or_else(|| failure("redeemFailed", "企业微信授权失败，请重新尝试"))?;
        if !response.status.is_redirection() {
            return Err(failure("redeemFailed", "企业微信授权失败，请重新尝试"));
        }
        let callback = self.config.redirect(&self.config.sso, location)?;
        let session_uri = self
            .config
            .sso
            .join("/oauth/authorize")
            .map_err(|_| failure("qrcodeConfig", "扫码配置无效"))?;
        if !transaction
            .jar
            .names(&session_uri)
            .iter()
            .any(|name| name == "SHU_OAUTH2")
        {
            return Err(failure("redeemFailed", "企业微信未建立统一认证会话"));
        }
        transaction.qr = None;
        transaction.challenge = None;
        Ok(LoginResult {
            methods: vec![],
            callback_uri: Some(callback.into()),
        })
    }
}
