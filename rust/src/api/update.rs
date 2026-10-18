pub use crate::update::{release::UpdateInfo, UpdateFailure};

#[flutter_rust_bridge::frb(opaque)]
pub struct UpdateClient {
    inner: crate::update::Client,
}

impl UpdateClient {
    pub fn new() -> Result<Self, UpdateFailure> {
        Ok(Self {
            inner: crate::update::Client::new()?,
        })
    }
    pub async fn check(
        &self,
        current_version: String,
    ) -> Result<Option<UpdateInfo>, UpdateFailure> {
        self.inner.check(current_version).await
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn close(&self) {
        self.inner.close();
    }
}
