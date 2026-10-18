use super::{
    super::{live, store, Client},
    model::Persisted,
};
use crate::auth::model::AuthFailure;

impl Client {
    pub(super) fn initialize_account(
        &self,
        legacy_cookies: &str,
        legacy_device_id: Option<String>,
        legacy_snapshot: &str,
    ) -> Result<(String, Persisted), AuthFailure> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| AuthFailure::new("busy", "账户正在处理其他请求"))?;
        let mut state = self.state()?;
        live(&state)?;
        let next = state
            .generation
            .checked_add(1)
            .ok_or_else(|| AuthFailure::new("ssoState", "认证会话代次超过限制"))?;
        let importing = state.import_open;
        let jar = if importing {
            self.prepare_legacy(legacy_cookies)?
        } else {
            state.jar.clone()
        };
        let device_id = state
            .device_id
            .clone()
            .or(legacy_device_id)
            .filter(|value| {
                !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
            })
            .map(Ok)
            .unwrap_or_else(|| crate::auth::atrust::random_hex(16))?;
        let source = self
            .config
            .sso
            .join("/oauth/authorize")
            .map_err(|_| AuthFailure::new("ssoState", "统一认证地址无效"))?;
        let signed_in = jar.names(&source).iter().any(|name| name == "SHU_OAUTH2");
        let display = if !signed_in {
            Persisted::default()
        } else if let Some(value) = state.account.clone() {
            Persisted::restore(value)
        } else if importing {
            Persisted::legacy(legacy_snapshot)
        } else {
            Persisted::default()
        };
        let account = serde_json::to_value(&display).map_err(|_| storage_failure())?;
        if let Some(store) = &state.store {
            store.save(&store::Record {
                version: 2,
                origin: self.config.sso.origin().ascii_serialization(),
                legacy_imported: state.legacy_imported || importing,
                import_open: false,
                cookies: jar.session_records(&self.config.sso)?,
                device_id: Some(device_id.clone()),
                account: Some(account.clone()),
            })?;
        }
        state.cancelled.cancel();
        state.cancelled = tokio_util::sync::CancellationToken::new();
        state.generation = next;
        state.jar = jar;
        state.legacy_imported |= importing;
        state.import_open = false;
        state.device_id = Some(device_id.clone());
        state.account = Some(account);
        Ok((device_id, display))
    }

    pub(super) fn save_account(&self, display: &Persisted) -> Result<(), AuthFailure> {
        let mut state = self.state()?;
        live(&state)?;
        let account = serde_json::to_value(display).map_err(|_| storage_failure())?;
        let previous = state.account.replace(account);
        if let Err(error) = self.persist(
            &state,
            Some(&state.jar),
            state.legacy_imported,
            state.import_open,
        ) {
            state.account = previous;
            return Err(error);
        }
        Ok(())
    }
}

fn storage_failure() -> AuthFailure {
    AuthFailure::new("ssoStorage", "无法保存账户展示状态")
}
