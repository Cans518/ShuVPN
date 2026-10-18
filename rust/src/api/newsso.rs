pub use crate::auth::model::AuthFailure;
pub use crate::auth::newsso::model::{
    ATrustVerification, AuthorizationReceipt, JwxtAccount, LoginResult, OtpCode, QrSession,
    QrStatus, SsoSnapshot, VerificationTarget,
};
use crate::auth::newsso::Client;

/// Owns SSO cookies and challenge secrets; only presentation state crosses FFI.
#[flutter_rust_bridge::frb(opaque)]
pub struct NewssoSession {
    inner: Client,
}

#[flutter_rust_bridge::frb(opaque)]
pub struct PreparedATrustTunnel {
    pub(crate) inner: crate::auth::newsso::connection::Prepared,
}

impl PreparedATrustTunnel {
    #[flutter_rust_bridge::frb(sync)]
    pub fn warnings(&self) -> Vec<String> {
        self.inner.warnings()
    }
}

impl NewssoSession {
    pub fn new() -> Result<Self, AuthFailure> {
        Ok(Self {
            inner: Client::new()?,
        })
    }
    pub fn open(directory: String) -> Result<Self, AuthFailure> {
        Ok(Self {
            inner: Client::open(directory)?,
        })
    }
    pub async fn login(
        &self,
        username: String,
        password: String,
    ) -> Result<LoginResult, AuthFailure> {
        self.inner.login(username, password).await
    }
    pub async fn send_code(&self, method: String) -> Result<(), AuthFailure> {
        self.inner.send_code(method).await
    }
    pub async fn verify_code(
        &self,
        method: String,
        code: String,
    ) -> Result<LoginResult, AuthFailure> {
        self.inner.verify_code(method, code).await
    }
    pub async fn start_qr(&self) -> Result<QrSession, AuthFailure> {
        self.inner.start_qr().await
    }
    pub async fn poll_qr(&self) -> Result<QrStatus, AuthFailure> {
        self.inner.poll_qr().await
    }
    pub async fn wait_qr(&self, timeout_seconds: u32) -> Result<QrStatus, AuthFailure> {
        self.inner.wait_qr(timeout_seconds).await
    }
    pub async fn redeem_qr(&self) -> Result<LoginResult, AuthFailure> {
        self.inner.redeem_qr().await
    }
    pub async fn authorize_system(
        &self,
        system_id: String,
    ) -> Result<AuthorizationReceipt, AuthFailure> {
        self.inner.authorize_system(system_id).await
    }
    pub async fn sign_in_atrust(
        &self,
        device_id: String,
    ) -> Result<AuthorizationReceipt, AuthFailure> {
        self.inner.sign_in_atrust(device_id).await
    }
    pub async fn fetch_otp(&self) -> Result<OtpCode, AuthFailure> {
        self.inner.fetch_otp().await
    }
    pub async fn fetch_jwxt_account(&self) -> Result<JwxtAccount, AuthFailure> {
        self.inner.fetch_jwxt_account().await
    }
    pub async fn verify_atrust(
        &self,
        device_id: String,
        strict_anti_mitm: bool,
    ) -> Result<ATrustVerification, AuthFailure> {
        self.inner.verify_atrust(device_id, strict_anti_mitm).await
    }
    pub async fn prepare_tunnel(
        &self,
        device_id: String,
        strict_anti_mitm: bool,
    ) -> Result<PreparedATrustTunnel, AuthFailure> {
        Ok(PreparedATrustTunnel {
            inner: self
                .inner
                .prepare_tunnel(device_id, strict_anti_mitm)
                .await?,
        })
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn cancel_qr(&self) -> Result<(), AuthFailure> {
        self.inner.clear()
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn clear(&self) -> Result<(), AuthFailure> {
        self.inner.clear()
    }
    /// Releases the directory lease while keeping the committed session.
    #[flutter_rust_bridge::frb(sync)]
    pub fn close(&self) -> Result<(), AuthFailure> {
        self.inner.close()
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn snapshot(&self) -> Result<SsoSnapshot, AuthFailure> {
        self.inner.snapshot()
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn import_legacy(&self, document: String) -> Result<usize, AuthFailure> {
        self.inner.import_legacy(&document)
    }
}
