use crate::{
    error::{CommandError, CommandResult},
    settings,
};
use serde_json::{json, Value};
use shuvpn_core::api::{
    account::AccountSession,
    desktop_connection::{ConnectionStartup, DesktopConnectionOwner, OwnerPhase},
    routing::{RoutingManager, RoutingPreview},
    settings::{Defaults, SettingsSession},
    vpn::VpnSession,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard};

pub struct DesktopHost {
    pub appearance: crate::appearance::AppearanceStore,
    pub account: AccountSession,
    pub settings: SettingsSession,
    pub routing: RoutingManager,
    pub owner: DesktopConnectionOwner,
    pub preview: Mutex<Option<PreparedRouting>>,
    pub configuration: Mutex<()>,
    pub preview_sequence: std::sync::atomic::AtomicU64,
    pub session: RwLock<Option<Arc<VpnSession>>>,
    recovery_directory: String,
    closing: AtomicBool,
    closed: AtomicBool,
    shutdown_gate: Mutex<()>,
    operations: RwLock<()>,
}

pub struct PreparedRouting {
    pub id: String,
    pub preview: RoutingPreview,
}

impl DesktopHost {
    pub async fn open(directory: &Path) -> CommandResult<Self> {
        if !directory.is_absolute() {
            return Err(CommandError::new("directory", "应用数据目录必须为绝对路径"));
        }
        let appearance =
            crate::appearance::AppearanceStore::open(directory.join("appearance.json"))?;
        let path = |name: &str| -> CommandResult<String> {
            directory
                .join(name)
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| CommandError::new("directory", "应用数据路径无法编码"))
        };
        let settings = SettingsSession::open(
            path("settings")?,
            "{}".into(),
            Defaults {
                debug_logging: false,
            },
        )
        .await?;
        // Rust owners release their file leases on drop if a later step fails.
        let account = AccountSession::open(path("account")?, "[]".into(), None, "{}".into())?;
        let routing = RoutingManager::open(path("routing")?).await?;
        let owner = DesktopConnectionOwner::open().await?;
        owner.claim_frontend()?;
        if let Err(error) = owner.await_frontend().await {
            return Err(error.into());
        }
        Ok(Self {
            appearance,
            account,
            settings,
            routing,
            owner,
            preview: Mutex::new(None),
            configuration: Mutex::new(()),
            preview_sequence: std::sync::atomic::AtomicU64::new(1),
            session: RwLock::new(None),
            recovery_directory: path("system-proxy")?,
            closing: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            shutdown_gate: Mutex::new(()),
            operations: RwLock::new(()),
        })
    }

    pub async fn enter(&self) -> CommandResult<RwLockReadGuard<'_, ()>> {
        let permit = self.operations.read().await;
        if self.closing.load(Ordering::Acquire) {
            return Err(CommandError::new("closing", "正在退出，请等待网络清理完成"));
        }
        Ok(permit)
    }

    pub async fn snapshot(&self) -> CommandResult<Value> {
        let account = self.account.snapshot()?;
        let owner = self.owner.snapshot()?;
        let session = self.session.read().await;
        let active = session
            .as_ref()
            .filter(|session| owner.run_id == Some(session.run_id()));
        let network = active.map(|session| {
            let current = session.snapshot();
            let services = session.startup_services();
            let mut warnings = session.preparation_warnings();
            if let Some(services) = services.as_ref() {
                warnings.extend(services.failures.iter().map(|failure| {
                    format!("{}：{}", failure.service, failure.message)
                }));
                if let Some(diagnostic) = services.tun.as_ref().and_then(|tun| tun.diagnostic.as_ref()) {
                    warnings.push(diagnostic.clone());
                }
            }
            json!({
                "virtualAddress": current.virtual_address,
                "dnsServers": current.dns_servers,
                "uploadBytes": current.upload_bytes,
                "downloadBytes": current.download_bytes,
                "connections": current.connections,
                "message": current.message,
                "warnings": warnings,
                "httpEndpoint": services.as_ref().and_then(|report| report.proxies.http.as_ref()).map(|binding| &binding.display_endpoint),
                "socksEndpoint": services.as_ref().and_then(|report| report.proxies.socks.as_ref()).map(|binding| &binding.display_endpoint),
            })
        });
        let verification = self.account.verification_state()?.map(|state| {
            json!({
                "methods": state.methods.into_iter().map(|method| json!({
                    "method": method.method, "target": method.target
                })).collect::<Vec<_>>(),
                "selectedMethod": state.selected_method,
                "resendAfterSeconds": state.resend_after_seconds,
            })
        });
        Ok(json!({
            "account": {
                "signedIn": account.signed_in,
                "accountId": account.account_id,
                "profile": account.profile,
                "busy": account.busy,
                "progressLabel": account.progress_label,
                "errorMessage": account.error_message,
                "needsReverification": account.needs_reverification,
                "sessionExpired": account.session_expired,
                "credentials": account.credentials.into_iter().map(|credential| json!({
                    "systemId": credential.system_id,
                    "state": credential.state,
                    "errorMessage": credential.error_message,
                })).collect::<Vec<_>>(),
                "verification": verification,
            },
            "settings": settings::snapshot(&self.settings.snapshot()?),
            "connection": {
                "generation": owner.generation.to_string(),
                "phase": match owner.phase {
                    OwnerPhase::Disconnected => "disconnected",
                    OwnerPhase::Connecting => "connecting",
                    OwnerPhase::Connected => "connected",
                    OwnerPhase::Disconnecting => "disconnecting",
                    OwnerPhase::Failed => "failed",
                },
                "runId": owner.run_id.map(|id| id.to_string()),
                "cleanupPending": owner.cleanup_pending,
                "errorMessage": owner.error_message,
                "closed": owner.closed,
                "network": network,
            },
        }))
    }

    pub async fn connect(&self, interface: String, dns: Option<String>) -> CommandResult<()> {
        let _configuration = self.configuration.lock().await;
        if self.closing.load(Ordering::Acquire) {
            return Err(CommandError::new("closing", "正在退出，连接已取消"));
        }
        self.require_disconnected().await?;
        let settings = self.settings.snapshot()?;
        let session = self
            .owner
            .connect_account(
                &self.account,
                &self.routing,
                settings.server.clone(),
                settings.timeout_seconds.saturating_mul(1000),
                ConnectionStartup {
                    proxies: settings::proxies(&settings),
                    tun: Some(settings::tun(&settings, interface, dns)),
                    recovery_directory: Some(self.recovery_directory.clone()),
                },
            )
            .await?;
        let mut current = self.session.write().await;
        let owner = self.owner.snapshot()?;
        if owner.run_id == Some(session.run_id()) && !owner.cleanup_pending {
            *current = Some(Arc::new(session));
        }
        Ok(())
    }

    pub async fn disconnect(&self) -> CommandResult<Option<String>> {
        let diagnostic = self.owner.disconnect().await?;
        let mut current = self.session.write().await;
        // A disconnect result must not discard a newer connection's view.
        if self.owner.snapshot()?.run_id.is_none() {
            current.take();
        }
        Ok(diagnostic)
    }

    pub async fn require_disconnected(&self) -> CommandResult<()> {
        let state = self.owner.snapshot()?;
        if !matches!(state.phase, OwnerPhase::Disconnected | OwnerPhase::Failed)
            || state.run_id.is_some()
            || state.cleanup_pending
        {
            return Err(CommandError::new("connected", "请先断开连接并完成清理"));
        }
        Ok(())
    }

    pub async fn shutdown(&self) -> CommandResult<Option<String>> {
        let _shutdown = self.shutdown_gate.lock().await;
        if self.closed.load(Ordering::Acquire) {
            return Ok(None);
        }
        self.closing.store(true, Ordering::Release);
        let result = self.finish_shutdown().await;
        if result.is_ok() {
            self.closed.store(true, Ordering::Release);
        } else {
            self.closing.store(false, Ordering::Release);
        }
        result
    }

    async fn finish_shutdown(&self) -> CommandResult<Option<String>> {
        // Cancel before draining permits: login/connection tasks may be awaiting I/O.
        self.account.cancel_work()?;
        self.owner.cancel()?;
        let _drained = self.operations.write().await;
        let diagnostic = self.owner.shutdown().await?;
        self.session.write().await.take();
        self.account.close()?;
        self.settings.close()?;
        Ok(diagnostic)
    }
}
