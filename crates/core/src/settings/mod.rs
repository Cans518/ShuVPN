pub(crate) mod model;
mod schema;
mod store;
use model::{failure, Defaults, Edit, SettingKey, SettingsFailure, SettingsSnapshot};
use std::sync::{Arc, Mutex, MutexGuard};
use store::{Record, Store};

pub(crate) fn validate_value(key: SettingKey, value_json: &str) -> Result<String, SettingsFailure> {
    if value_json.len() > 4096 {
        return Err(failure("size", "设置值过长"));
    }
    let value =
        serde_json::from_str(value_json).map_err(|_| failure("format", "设置值格式无效"))?;
    Ok(schema::normalize(key, value)?.to_string())
}

struct State {
    record: Record,
    store: Option<Store>,
    uncertain: bool,
}
pub(crate) struct Session {
    state: Mutex<State>,
    defaults: Defaults,
}
impl Session {
    pub(crate) fn open(
        directory: String,
        legacy: &str,
        defaults: Defaults,
    ) -> Result<Arc<Self>, SettingsFailure> {
        let store = Store::open(directory)?;
        let record = match store.load()? {
            Some(record) => {
                // Validate persisted known fields without silently repairing corruption.
                for key in SettingKey::ALL {
                    if let Some(value) = record.values.get(key.storage_key()) {
                        if schema::normalize(key, value.clone())? != *value {
                            return Err(failure("format", "设置文件包含未规范化的值"));
                        }
                    }
                }
                if record
                    .values
                    .get(schema::VERSION)
                    .and_then(serde_json::Value::as_u64)
                    .is_none_or(|value| value < 4 || value > u32::MAX as u64)
                {
                    return Err(failure("schema", "设置版本无效"));
                }
                if record
                    .values
                    .get("settings.systemProxyEnabled")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && record
                        .values
                        .get("settings.httpProxyEnabled")
                        .and_then(serde_json::Value::as_bool)
                        != Some(true)
                {
                    return Err(failure("format", "系统代理配置缺少 HTTP 监听"));
                }
                record
            }
            None => {
                let record = Record {
                    format_version: 1,
                    revision: 0,
                    values: schema::migrate(legacy)?,
                };
                store.save(&record)?;
                record
            }
        };
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                record,
                store: Some(store),
                uncertain: false,
            }),
            defaults,
        }))
    }
    fn lock(&self) -> Result<MutexGuard<'_, State>, SettingsFailure> {
        self.state
            .lock()
            .map_err(|_| failure("state", "设置服务不可用"))
    }
    pub(crate) fn snapshot(&self) -> Result<SettingsSnapshot, SettingsFailure> {
        let state = self.lock()?;
        if state.store.is_none() {
            return Err(failure("closed", "设置服务已经关闭"));
        }
        Ok(schema::snapshot(
            &state.record.values,
            state.record.revision,
            self.defaults,
        ))
    }
    pub(crate) fn apply(&self, edits: Vec<Edit>) -> Result<SettingsSnapshot, SettingsFailure> {
        if edits.len() > SettingKey::ALL.len() {
            return Err(failure("size", "一次修改的设置过多"));
        }
        let normalized = edits
            .into_iter()
            .map(|edit| {
                if edit.value_json.len() > 4096 {
                    return Err(failure("size", "设置值过长"));
                }
                let value = serde_json::from_str(&edit.value_json)
                    .map_err(|_| failure("format", "设置值格式无效"))?;
                Ok((edit.key, schema::normalize(edit.key, value)?))
            })
            .collect::<Result<Vec<_>, SettingsFailure>>()?;
        self.commit(move |values| {
            for (key, value) in normalized {
                if matches!(
                    key,
                    SettingKey::EasyConnectEnabled | SettingKey::OpenVpnEnabled
                ) {
                    continue;
                }
                if key == SettingKey::SystemProxyEnabled && value.as_bool() == Some(true) {
                    values.insert(
                        SettingKey::HttpProxyEnabled.storage_key().into(),
                        true.into(),
                    );
                }
                if key == SettingKey::HttpProxyEnabled && value.as_bool() == Some(false) {
                    values.insert(
                        SettingKey::SystemProxyEnabled.storage_key().into(),
                        false.into(),
                    );
                }
                values.insert(key.storage_key().into(), value);
            }
        })
    }
    pub(crate) fn restore_connection_defaults(&self) -> Result<SettingsSnapshot, SettingsFailure> {
        self.commit(|values| {
            for key in [
                SettingKey::Server,
                SettingKey::LoginDomain,
                SettingKey::TimeoutSeconds,
                SettingKey::AtrustEnabled,
                SettingKey::EasyConnectEnabled,
                SettingKey::OpenVpnEnabled,
            ] {
                values.remove(key.storage_key());
            }
        })
    }
    fn commit(
        &self,
        update: impl FnOnce(&mut schema::Values),
    ) -> Result<SettingsSnapshot, SettingsFailure> {
        let mut state = self.lock()?;
        let store = state
            .store
            .as_ref()
            .ok_or_else(|| failure("closed", "设置服务已经关闭"))?;
        if state.uncertain {
            return Err(failure("uncertain", "请重新启动应用以恢复设置保存状态"));
        }
        let mut record = state.record.clone();
        update(&mut record.values);
        if record.values == state.record.values {
            return Ok(schema::snapshot(
                &record.values,
                record.revision,
                self.defaults,
            ));
        }
        record.revision = record
            .revision
            .checked_add(1)
            .ok_or_else(|| failure("version", "设置修订号溢出"))?;
        if let Err(error) = store.save(&record) {
            if error.code == "uncertain" {
                state.uncertain = true;
            }
            return Err(error);
        }
        state.record = record;
        Ok(schema::snapshot(
            &state.record.values,
            state.record.revision,
            self.defaults,
        ))
    }
    pub(crate) fn close(&self) -> Result<(), SettingsFailure> {
        self.lock()?.store.take();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
