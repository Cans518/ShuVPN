pub use crate::auth::model::AuthFailure;
pub use crate::auth::newsso::account::model::SignInState;
pub use crate::auth::newsso::account::model::{Credential, CredentialState, Profile, Snapshot};
pub use crate::auth::newsso::account::model::{QrSignInEvent, QrSignInPhase};
pub use crate::auth::newsso::account::verification::VerificationState;
use crate::auth::newsso::account::Account;
pub use crate::auth::newsso::model::{QrSession, QrStatus, VerificationTarget};

pub struct AccountSession {
    pub(crate) inner: std::sync::Arc<Account>,
}

pub struct SignInResult {
    pub methods: Vec<VerificationTarget>,
}

impl AccountSession {
    pub async fn sign_in_password(
        &self,
        username: String,
        password: String,
    ) -> Result<SignInState, AuthFailure> {
        self.inner.sign_in_password(username, password).await
    }
    pub async fn sign_in_code(&self, method: String, code: String) -> Result<(), AuthFailure> {
        self.inner.sign_in_code(method, code).await
    }
    pub async fn sign_in_qr(
        &self,
        emit: impl FnMut(QrSignInEvent) -> bool + Send,
    ) -> Result<bool, AuthFailure> {
        self.inner.sign_in_qr(emit).await
    }
    pub fn open(
        directory: String,
        legacy_cookies: String,
        legacy_device_id: Option<String>,
        legacy_snapshot: String,
    ) -> Result<Self, AuthFailure> {
        let cookies = zeroize::Zeroizing::new(legacy_cookies);
        Ok(Self {
            inner: std::sync::Arc::new(Account::open(
                directory,
                &cookies,
                legacy_device_id,
                &legacy_snapshot,
            )?),
        })
    }
    pub fn snapshot(&self) -> Result<Snapshot, AuthFailure> {
        self.inner.snapshot()
    }
    pub async fn login(
        &self,
        username: String,
        password: String,
    ) -> Result<SignInResult, AuthFailure> {
        Ok(SignInResult {
            methods: self.inner.login(username, password).await?.methods,
        })
    }
    pub async fn send_code(&self, method: String) -> Result<(), AuthFailure> {
        self.inner.send_code(method).await
    }
    pub fn verification_state(&self) -> Result<Option<VerificationState>, AuthFailure> {
        self.inner.verification_state()
    }
    pub fn select_verification_method(&self, method: String) -> Result<(), AuthFailure> {
        self.inner.select_verification_method(method)
    }
    pub async fn verify_code(
        &self,
        method: String,
        code: String,
    ) -> Result<SignInResult, AuthFailure> {
        Ok(SignInResult {
            methods: self.inner.verify_code(method, code).await?.methods,
        })
    }
    pub async fn start_qr(&self) -> Result<QrSession, AuthFailure> {
        self.inner.start_qr().await
    }
    pub async fn poll_qr(&self) -> Result<QrStatus, AuthFailure> {
        self.inner.poll_qr().await
    }
    pub async fn wait_qr(&self) -> Result<QrStatus, AuthFailure> {
        self.inner.wait_qr().await
    }
    pub async fn redeem_qr(&self) -> Result<SignInResult, AuthFailure> {
        Ok(SignInResult {
            methods: self.inner.redeem_qr().await?.methods,
        })
    }
    pub async fn complete_sign_in(&self) -> Result<bool, AuthFailure> {
        self.inner.complete_sign_in().await
    }
    pub async fn verify_if_stale(&self) -> Result<bool, AuthFailure> {
        self.inner.verify_if_stale().await
    }
    pub async fn prepare_tunnel(
        &self,
        strict_anti_mitm: bool,
    ) -> Result<super::newsso::PreparedATrustTunnel, AuthFailure> {
        Ok(super::newsso::PreparedATrustTunnel {
            inner: self.inner.prepare_tunnel(strict_anti_mitm).await?,
        })
    }
    pub fn cancel_qr(&self) -> Result<(), AuthFailure> {
        self.inner.cancel_qr()
    }
    pub fn cancel_work(&self) -> Result<(), AuthFailure> {
        self.inner.cancel_work()
    }
    pub fn clear_session_expired(&self) -> Result<(), AuthFailure> {
        self.inner.clear_session_expired()
    }
    pub fn sign_out(&self) -> Result<(), AuthFailure> {
        self.inner.sign_out()
    }
    pub fn close(&self) -> Result<(), AuthFailure> {
        self.inner.close()
    }
}
