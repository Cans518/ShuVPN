use super::{
    model::{LoginResult, QrSession, QrStatus},
    Client,
};
use crate::auth::model::AuthFailure;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;
pub(crate) mod model;
mod persistence;
mod qr_sign_in;
mod sign_in;
#[cfg(test)]
mod tests;
pub(crate) mod verification;
use model::{Credential, CredentialState, Persisted, Profile, Snapshot, SYSTEMS};

struct View {
    epoch: u64,
    cancelled: CancellationToken,
    display: Persisted,
    credentials: Vec<Credential>,
    signed_in_at: Option<i64>,
    busy: bool,
    progress: Option<String>,
    error: Option<String>,
    session_expired: bool,
    qr_active: bool,
    qr_flow: bool,
    verification: verification::Delivery,
}

pub(crate) struct Account {
    client: Arc<Client>,
    device_id: String,
    view: Mutex<View>,
    operation: tokio::sync::Mutex<()>,
}

impl Drop for Account {
    fn drop(&mut self) {
        if let Ok(view) = self.view.lock() {
            view.cancelled.cancel();
        }
    }
}

impl Account {
    pub(crate) fn open(
        directory: String,
        cookies: &str,
        device_id: Option<String>,
        snapshot: &str,
    ) -> Result<Self, AuthFailure> {
        Self::adopt(
            Arc::new(Client::open(directory)?),
            cookies,
            device_id,
            snapshot,
        )
    }

    fn adopt(
        client: Arc<Client>,
        cookies: &str,
        device_id: Option<String>,
        snapshot: &str,
    ) -> Result<Self, AuthFailure> {
        let (device_id, display) = client.initialize_account(cookies, device_id, snapshot)?;
        let credentials = SYSTEMS
            .into_iter()
            .map(|id| Credential {
                system_id: id.to_owned(),
                state: display
                    .credentials
                    .get(id)
                    .copied()
                    .unwrap_or(CredentialState::Missing),
                error_code: None,
                error_message: None,
            })
            .collect();
        Ok(Self {
            client,
            device_id,
            view: Mutex::new(View {
                epoch: 0,
                cancelled: CancellationToken::new(),
                display,
                credentials,
                signed_in_at: None,
                busy: false,
                progress: None,
                error: None,
                session_expired: false,
                qr_active: false,
                qr_flow: false,
                verification: verification::Delivery::default(),
            }),
            operation: tokio::sync::Mutex::new(()),
        })
    }

    fn view(&self) -> Result<std::sync::MutexGuard<'_, View>, AuthFailure> {
        self.view
            .lock()
            .map_err(|_| AuthFailure::new("accountState", "账户展示状态不可用"))
    }

    pub(crate) fn snapshot(&self) -> Result<Snapshot, AuthFailure> {
        let view = self.view()?;
        let session = self.client.snapshot()?;
        let signed_in = session.has_session;
        let credentials: Vec<_> = view
            .credentials
            .iter()
            .cloned()
            .map(|mut item| {
                if !signed_in {
                    item.state = CredentialState::Missing;
                    item.error_code = None;
                    item.error_message = None;
                }
                item
            })
            .collect();
        let available_count = credentials
            .iter()
            .filter(|item| item.state == CredentialState::Available)
            .count() as u32;
        let stale = view
            .display
            .verified_at
            .is_none_or(|at| at > now() || now().saturating_sub(at) >= 300);
        let characters: Vec<_> = self.device_id.chars().collect();
        let masked_device_id = if characters.len() <= 8 {
            "••••".to_owned()
        } else {
            format!(
                "{}••••{}",
                characters[..4].iter().collect::<String>(),
                characters[characters.len() - 4..]
                    .iter()
                    .collect::<String>()
            )
        };
        Ok(Snapshot {
            signed_in,
            account_id: if signed_in {
                view.display.account_id.clone()
            } else {
                None
            },
            profile: if signed_in {
                view.display.profile.clone()
            } else {
                None
            },
            signed_in_at: if signed_in { view.signed_in_at } else { None },
            busy: view.busy,
            progress_label: view.progress.clone(),
            error_message: view.error.clone(),
            credentials,
            available_count,
            total_count: 3,
            needs_reverification: signed_in && (stale || available_count == 0),
            session_expired: view.session_expired,
            masked_device_id,
            qr_status: session.qr_status,
        })
    }

    async fn run<T, F>(&self, label: &str, work: F) -> Result<T, AuthFailure>
    where
        F: std::future::Future<Output = Result<T, AuthFailure>>,
    {
        self.run_notifying(label, work, |_| {}).await
    }

    // The notifier runs under the view lock and must not re-enter the account.
    async fn run_notifying<T, F>(
        &self,
        label: &str,
        work: F,
        notify: impl FnOnce(&Result<T, AuthFailure>),
    ) -> Result<T, AuthFailure>
    where
        F: std::future::Future<Output = Result<T, AuthFailure>>,
    {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| AuthFailure::new("busy", "账户正在处理其他请求"))?;
        let (epoch, cancelled) = {
            let mut view = self.view()?;
            view.busy = true;
            view.progress = Some(label.to_owned());
            view.error = None;
            (view.epoch, view.cancelled.clone())
        };
        let result = tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_error()),
            result = work => result,
        };
        let mut view = self.view()?;
        if view.epoch != epoch {
            return Err(cancelled_error());
        }
        if let Err(error) = &result {
            for credential in &mut view.credentials {
                if credential.state == CredentialState::Refreshing {
                    credential.state = CredentialState::Failing;
                    credential.error_code = Some(error.code.clone());
                    credential.error_message = Some(error.message.clone());
                }
            }
        }
        view.busy = false;
        view.qr_flow = false;
        view.progress = None;
        view.error = result.as_ref().err().map(|error| error.message.clone());
        notify(&result);
        result
    }

    fn active(&self) -> Result<std::sync::MutexGuard<'_, View>, AuthFailure> {
        let view = self.view()?;
        if !view.busy || view.cancelled.is_cancelled() {
            return Err(cancelled_error());
        }
        Ok(view)
    }

    pub(crate) async fn login(
        &self,
        username: String,
        password: String,
    ) -> Result<LoginResult, AuthFailure> {
        sign_in::validate_credentials(&username, &password)?;
        self.run(
            "正在登录统一身份认证",
            self.login_identity(username, password),
        )
        .await
    }
    pub(crate) async fn send_code(&self, method: String) -> Result<(), AuthFailure> {
        self.run("正在发送验证码", async {
            let methods = self.client.snapshot()?.challenge_methods;
            self.active()?.verification.begin(&methods, &method)?;
            let result = self.client.send_code(method.clone()).await;
            if let Err(error) = &result {
                self.active()?
                    .verification
                    .failed(&methods, &method, &error.code);
            }
            result
        })
        .await
    }
    pub(crate) fn verification_state(
        &self,
    ) -> Result<Option<verification::VerificationState>, AuthFailure> {
        let view = self.view()?;
        Ok(view
            .verification
            .snapshot(self.client.snapshot()?.challenge_methods))
    }
    pub(crate) fn select_verification_method(&self, method: String) -> Result<(), AuthFailure> {
        let mut view = self.view()?;
        if view.busy {
            return Err(AuthFailure::new("busy", "账户正在处理其他请求"));
        }
        view.verification
            .select(&self.client.snapshot()?.challenge_methods, method)
    }
    pub(crate) async fn verify_code(
        &self,
        method: String,
        code: String,
    ) -> Result<LoginResult, AuthFailure> {
        self.run("正在校验验证码", self.verify_identity(method, code))
            .await
    }
    pub(crate) async fn start_qr(&self) -> Result<QrSession, AuthFailure> {
        self.run("正在创建二维码", async {
            {
                let mut view = self.active()?;
                clear_display(&mut view);
                view.qr_active = true;
            }
            self.client.start_qr().await
        })
        .await
    }
    pub(crate) async fn poll_qr(&self) -> Result<QrStatus, AuthFailure> {
        self.run("等待扫码确认", self.client.poll_qr()).await
    }
    pub(crate) async fn wait_qr(&self) -> Result<QrStatus, AuthFailure> {
        self.run("等待扫码确认", self.client.wait_qr(180)).await
    }
    pub(crate) async fn redeem_qr(&self) -> Result<LoginResult, AuthFailure> {
        self.run("正在接管企业微信会话", async {
            let result = self.client.redeem_qr().await?;
            let mut view = self.active()?;
            view.signed_in_at = Some(now());
            view.qr_active = false;
            Ok(result)
        })
        .await
    }
    pub(crate) fn cancel_qr(&self) -> Result<(), AuthFailure> {
        let mut view = self.view()?;
        let session = self.client.snapshot()?;
        if view.qr_flow && session.has_session && session.qr_status.is_none() {
            view.qr_active = false;
            view.signed_in_at = Some(now());
            return invalidate(&mut view);
        }
        if view.qr_active || session.qr_status.is_some() {
            invalidate(&mut view)?;
            clear_display(&mut view);
            let result = self.client.clear();
            view.error = result.as_ref().err().map(|error| error.message.clone());
            return result;
        }
        if view.qr_flow {
            invalidate(&mut view)?;
        }
        Ok(())
    }
    pub(crate) fn sign_out(&self) -> Result<(), AuthFailure> {
        let mut view = self.view()?;
        invalidate(&mut view)?;
        clear_display(&mut view);
        let result = self.client.clear();
        view.error = result.as_ref().err().map(|error| error.message.clone());
        result
    }
    pub(crate) fn close(&self) -> Result<(), AuthFailure> {
        let mut view = self.view()?;
        invalidate(&mut view)?;
        self.client.close()
    }
    pub(crate) fn clear_session_expired(&self) -> Result<(), AuthFailure> {
        self.view()?.session_expired = false;
        Ok(())
    }
    pub(crate) fn cancel_work(&self) -> Result<(), AuthFailure> {
        invalidate(&mut *self.view()?)
    }

    pub(crate) async fn complete_sign_in(&self) -> Result<bool, AuthFailure> {
        self.run("正在获取凭据", self.exchange_all()).await
    }
    pub(crate) async fn verify_if_stale(&self) -> Result<bool, AuthFailure> {
        let snapshot = self.snapshot()?;
        if !snapshot.signed_in {
            return Ok(false);
        }
        if !snapshot.needs_reverification {
            return Ok(snapshot.available_count > 0);
        }
        if snapshot.busy {
            return Ok(snapshot.available_count > 0);
        }
        self.complete_sign_in().await
    }

    async fn exchange_all(&self) -> Result<bool, AuthFailure> {
        if !self.client.snapshot()?.has_session {
            return Ok(false);
        }
        let result = tokio::time::timeout(Duration::from_secs(90), async {
            for (index, id) in SYSTEMS.into_iter().enumerate() {
                {
                    let mut view = self.active()?;
                    view.credentials[index].state = CredentialState::Refreshing;
                    view.credentials[index].error_code = None;
                    view.credentials[index].error_message = None;
                    view.progress = Some(format!("正在获取凭据（{index}/3）"));
                }
                let result = match id {
                    "jwxt" => self.exchange_jwxt().await,
                    "atrust" => self
                        .client
                        .verify_atrust(self.device_id.clone(), false)
                        .await
                        .map(|_| ()),
                    _ => self.client.fetch_otp().await.map(|_| ()),
                };
                let mut view = self.active()?;
                match result {
                    Ok(()) => view.credentials[index].state = CredentialState::Available,
                    Err(error) => {
                        if error.code == "sessionNotReused" {
                            view.session_expired = true;
                        }
                        view.credentials[index].state = CredentialState::Failing;
                        view.credentials[index].error_code = Some(error.code);
                        view.credentials[index].error_message = Some(error.message);
                    }
                }
                view.progress = Some(format!("正在获取凭据（{}/3）", index + 1));
            }
            Ok::<_, AuthFailure>(())
        })
        .await
        .map_err(|_| AuthFailure::new("timeout", "获取凭据超时"))?;
        result?;
        let mut view = self.active()?;
        let mut display = view.display.clone();
        display.credentials = view
            .credentials
            .iter()
            .map(|item| (item.system_id.clone(), item.state))
            .collect();
        let available = view
            .credentials
            .iter()
            .any(|item| item.state == CredentialState::Available);
        if !view.session_expired && available {
            display.verified_at = Some(now());
        }
        self.client.save_account(&display)?;
        view.display = display;
        Ok(available)
    }

    async fn exchange_jwxt(&self) -> Result<(), AuthFailure> {
        self.client.authorize_system("jwxt".to_owned()).await?;
        let account = self.client.fetch_jwxt_account().await?;
        let mut view = self.active()?;
        if account.student_id.is_some() {
            view.display.account_id = account.student_id.clone();
        }
        view.display.profile = account.name.map(|name| Profile {
            name: Some(name),
            student_id: account.student_id,
            grade: account.grade,
            college: account.college,
            major: account.major,
            class_name: account.class_name,
        });
        Ok(())
    }
    pub(crate) async fn prepare_tunnel(
        &self,
        strict: bool,
    ) -> Result<super::connection::Prepared, AuthFailure> {
        self.client
            .prepare_tunnel(self.device_id.clone(), strict)
            .await
    }
    pub(crate) async fn prepare_for_origin(
        &self,
        origin: String,
        strict: bool,
    ) -> Result<super::connection::Prepared, AuthFailure> {
        let uri = reqwest::Url::parse(&origin)
            .map_err(|_| AuthFailure::new("atrustOrigin", "网关地址无效"))?;
        if uri.origin() != self.client.config.system_base("atrust")?.origin()
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.fragment().is_some()
        {
            return Err(AuthFailure::new(
                "atrustOrigin",
                "连接网关与账户认证来源不一致",
            ));
        }
        self.prepare_tunnel(strict).await
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}
fn cancelled_error() -> AuthFailure {
    AuthFailure::new("cancelled", "账户请求已取消")
}
fn invalidate(view: &mut View) -> Result<(), AuthFailure> {
    let epoch = view
        .epoch
        .checked_add(1)
        .ok_or_else(|| AuthFailure::new("accountState", "账户请求代次超过限制"))?;
    view.cancelled.cancel();
    view.cancelled = CancellationToken::new();
    view.epoch = epoch;
    view.busy = false;
    view.qr_flow = false;
    view.progress = None;
    for item in &mut view.credentials {
        if item.state == CredentialState::Refreshing {
            item.state = CredentialState::Unknown;
        }
    }
    Ok(())
}
fn clear_display(view: &mut View) {
    view.verification.reset_selection();
    view.display = Persisted::default();
    view.signed_in_at = None;
    view.session_expired = false;
    view.qr_active = false;
    view.qr_flow = false;
    view.error = None;
    for item in &mut view.credentials {
        item.state = CredentialState::Missing;
        item.error_code = None;
        item.error_message = None;
    }
}
