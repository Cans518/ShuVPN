use super::{cancelled_failure, live, model::ATrustVerification, Client};
use crate::{auth::model::AuthFailure, connection::session::Engine};
use std::time::Duration;

impl Client {
    /// Verifies availability with a real native control connection, then stops it.
    pub(crate) async fn verify_atrust(
        &self,
        device_id: String,
        strict_anti_mitm: bool,
    ) -> Result<ATrustVerification, AuthFailure> {
        let cancelled = {
            let state = self.state()?;
            live(&state)?;
            state.cancelled.clone()
        };
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = tokio::time::timeout(Duration::from_secs(90), async {
                let prepared = self.prepare_tunnel(device_id, strict_anti_mitm).await?;
                let plan = prepared.take()?;
                let engine = Engine::connect(plan, 15_000).await.map_err(connection_failure)?;
                engine.bind_authority(cancelled.clone());
                let snapshot = engine.shared.snapshot(crate::connection::model::VpnEventKind::State, 0.0, 0.0);
                let verified = ATrustVerification {
                    virtual_address: snapshot.virtual_address,
                    dns_servers: engine.shared.plan.dns_servers.clone(),
                    warnings: prepared.warnings(),
                };
                engine.stop().await.map_err(connection_failure)?;
                if cancelled.is_cancelled() { return Err(cancelled_failure()); }
                Ok(verified)
            }) => result.map_err(|_| AuthFailure::new("timeout", "网关可用性验证超时"))?,
        }
    }
}

fn connection_failure(failure: crate::connection::model::VpnFailure) -> AuthFailure {
    AuthFailure::new(&failure.code, &failure.message)
}
