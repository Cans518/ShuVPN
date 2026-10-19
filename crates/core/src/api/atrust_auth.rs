use crate::auth::atrust::ATrustClient;
pub use crate::auth::model::{AuthConfig, AuthCookie, AuthFailure, AuthSeed, AuthSession};
pub use crate::auth::model::{TunnelOptions, TunnelPreparation};

pub struct ATrustAuthClient {
    inner: ATrustClient,
}

impl ATrustAuthClient {
    pub fn new(config: AuthConfig) -> Result<Self, AuthFailure> {
        Ok(Self {
            inner: ATrustClient::new(config)?,
        })
    }

    pub async fn sign_in(&self, seed: AuthSeed) -> Result<AuthSession, AuthFailure> {
        self.inner.sign_in(seed).await
    }

    pub async fn prepare_tunnel(
        &self,
        seed: AuthSeed,
        options: TunnelOptions,
    ) -> Result<TunnelPreparation, AuthFailure> {
        self.inner.prepare_tunnel(seed, options).await
    }

    /// Cancellation does not wait for the in-flight HTTP request or state lock.
    pub fn cancel(&self) {
        self.inner.cancel();
    }
}

pub fn generate_device_id() -> Result<String, AuthFailure> {
    crate::auth::atrust::random_hex(16)
}
