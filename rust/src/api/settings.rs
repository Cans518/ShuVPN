pub use crate::settings::model::{Defaults, Edit, SettingKey, SettingsFailure, SettingsSnapshot};
use crate::settings::Session;
use std::sync::Arc;

pub fn validate_setting(key: SettingKey, value_json: String) -> Result<String, SettingsFailure> {
    crate::settings::validate_value(key, &value_json)
}

pub struct SettingsSession {
    inner: Arc<Session>,
}
impl SettingsSession {
    pub async fn open(
        directory: String,
        legacy_json: String,
        defaults: Defaults,
    ) -> Result<Self, SettingsFailure> {
        tokio::task::spawn_blocking(move || Session::open(directory, &legacy_json, defaults))
            .await
            .map_err(|_| crate::settings::model::failure("state", "设置服务启动失败"))?
            .map(|inner| Self { inner })
    }
    pub fn snapshot(&self) -> Result<SettingsSnapshot, SettingsFailure> {
        self.inner.snapshot()
    }
    pub async fn apply(&self, edits: Vec<Edit>) -> Result<SettingsSnapshot, SettingsFailure> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || inner.apply(edits))
            .await
            .map_err(|_| crate::settings::model::failure("state", "设置保存任务失败"))?
    }
    pub async fn restore_connection_defaults(&self) -> Result<SettingsSnapshot, SettingsFailure> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || inner.restore_connection_defaults())
            .await
            .map_err(|_| crate::settings::model::failure("state", "设置恢复任务失败"))?
    }
    pub fn close(&self) -> Result<(), SettingsFailure> {
        self.inner.close()
    }
}
