use super::{cancelled_failure, live, Client};
use crate::auth::model::{AuthFailure, TunnelOptions};
use std::{sync::Mutex, time::Duration};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroize;

pub(crate) struct Prepared {
    plan: Mutex<Option<String>>,
    warnings: Vec<String>,
    validity: CancellationToken,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if let Ok(plan) = self.plan.get_mut() {
            plan.zeroize();
        }
    }
}

impl Prepared {
    pub(crate) fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }
    pub(crate) fn validity(&self) -> &CancellationToken {
        &self.validity
    }
    pub(crate) fn take(&self) -> Result<String, AuthFailure> {
        if self.validity.is_cancelled() {
            return Err(cancelled_failure());
        }
        self.plan
            .lock()
            .map_err(|_| AuthFailure::new("planState", "隧道计划状态不可用"))?
            .take()
            .ok_or_else(|| AuthFailure::new("planUsed", "隧道计划已经使用"))
    }
}

impl Client {
    pub(crate) async fn prepare_tunnel(
        &self,
        device_id: String,
        strict_anti_mitm: bool,
    ) -> Result<Prepared, AuthFailure> {
        if device_id.is_empty() || device_id.len() > 256 {
            return Err(AuthFailure::new("atrustDeviceId", "设备标识无效"));
        }
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
                self.require_session(&transaction)?;
                let identity = crate::connection::identity::new(&device_id)
                    .map_err(|failure| AuthFailure::new(&failure.code, &failure.message))?;
                let platform = match std::env::consts::OS {
                    "windows" => "Windows", "linux" => "Linux",
                    _ => return Err(AuthFailure::new("platform", "当前平台尚未支持账号隧道准备")),
                };
                let options = TunnelOptions {
                    connection_id: identity.connection_id, sign_key: identity.sign_key,
                    process_platform: platform.to_owned(), strict_anti_mitm,
                };
                let client = self.atrust_client()?;
                let result = tokio::time::timeout(Duration::from_secs(90),
                    client.prepare_tunnel_owned(transaction.jar.clone(), device_id, options)).await
                    .map_err(|_| AuthFailure::new("timeout", "原生连接准备超时"))?;
                let result = result.map(|(prepared, jar)| {
                    transaction.jar = jar;
                    Prepared {
                        plan: Mutex::new(Some(prepared.plan_json)),
                        warnings: prepared.warnings, validity: cancelled.clone(),
                    }
                });
                self.finish(transaction, result)
            } => result,
        }
    }
}
