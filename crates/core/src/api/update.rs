pub use crate::update::{release::UpdateInfo, UpdateFailure};

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
    pub fn close(&self) {
        self.inner.close();
    }
}
