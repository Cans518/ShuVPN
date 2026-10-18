use super::{
    account::AccountSession,
    routing::RoutingManager,
    vpn::{ProxyStartup, TunStartup, VpnConnectAttempt, VpnFailure, VpnSession},
};
pub use crate::connection::owner::{OwnerPhase, OwnerSnapshot};
use crate::{connection::owner::Owner, frb_generated::StreamSink};
use std::sync::{Arc, Mutex, OnceLock};

// A failed close must survive a Flutter handle or widget being disposed.
// The process has one desktop connection authority; release only after shutdown.
static DESKTOP: OnceLock<Mutex<Option<Arc<Owner>>>> = OnceLock::new();

pub struct ConnectionStartup {
    pub proxies: ProxyStartup,
    pub tun: Option<TunStartup>,
    pub recovery_directory: Option<String>,
}

#[flutter_rust_bridge::frb(opaque)]
pub struct DesktopConnectionOwner {
    inner: Arc<Owner>,
    control: Mutex<Option<u64>>,
}

impl DesktopConnectionOwner {
    #[cfg(test)]
    #[flutter_rust_bridge::frb(ignore)]
    pub(crate) fn isolated() -> Self {
        Self {
            inner: Owner::spawn(),
            control: Mutex::new(None),
        }
    }

    pub async fn open() -> Result<Self, VpnFailure> {
        loop {
            let inner = {
                let mut current = DESKTOP
                    .get_or_init(|| Mutex::new(None))
                    .lock()
                    .map_err(|_| VpnFailure::new("worker", "无法取得桌面连接所有者"))?;
                current.get_or_insert_with(Owner::spawn).clone()
            };
            if !inner.snapshot()?.closed {
                return Ok(Self {
                    inner,
                    control: Mutex::new(None),
                });
            }
            // A caller may have dropped its shutdown future after the actor
            // committed closure. Join that actor before replacing the registry.
            inner.shutdown(None).await?;
            let mut current = DESKTOP
                .get_or_init(|| Mutex::new(None))
                .lock()
                .map_err(|_| VpnFailure::new("worker", "无法释放桌面连接所有者"))?;
            if current
                .as_ref()
                .is_some_and(|owner| Arc::ptr_eq(owner, &inner))
            {
                current.take();
            }
        }
    }

    /// Reserve synchronously so an older, delayed frontend cannot take control
    /// after a newer frontend has reserved it. Then await_frontend drains jobs.
    #[flutter_rust_bridge::frb(sync)]
    pub fn claim_frontend(&self) -> Result<(), VpnFailure> {
        let mut control = self
            .control
            .lock()
            .map_err(|_| VpnFailure::new("worker", "前端控制标识不可用"))?;
        if control.is_some() {
            return Err(VpnFailure::new("state", "此句柄已经取得前端控制标识"));
        }
        *control = Some(self.inner.reserve_control()?);
        Ok(())
    }
    pub async fn await_frontend(&self) -> Result<(), VpnFailure> {
        let control = self
            .control_value()?
            .ok_or_else(|| VpnFailure::new("state", "尚未取得前端控制标识"))?;
        self.inner.await_control(control).await
    }
    #[flutter_rust_bridge::frb(ignore)]
    fn control_value(&self) -> Result<Option<u64>, VpnFailure> {
        self.control
            .lock()
            .map(|control| *control)
            .map_err(|_| VpnFailure::new("worker", "前端控制标识不可用"))
    }

    pub async fn connect_account(
        &self,
        account: &AccountSession,
        routing: &RoutingManager,
        gateway_origin: String,
        timeout_ms: u32,
        startup: ConnectionStartup,
    ) -> Result<VpnSession, VpnFailure> {
        let account = AccountSession {
            inner: Arc::clone(&account.inner),
        };
        let routing = RoutingManager {
            store: Arc::clone(&routing.store),
        };
        let recovery = startup.recovery_directory.clone();
        self.inner
            .connect(
                self.control_value()?,
                Box::new(move |token| {
                    Box::pin(async move {
                        VpnConnectAttempt::for_owner(token)
                            .connect_desktop_account(
                                &account,
                                gateway_origin,
                                false,
                                timeout_ms,
                                &routing,
                                recovery,
                            )
                            .await
                            .map(VpnSession::into_owned)
                    })
                }),
                startup.proxies,
                startup.tun,
                startup.recovery_directory,
            )
            .await
            .map(VpnSession::from_owned)
    }

    /// Native plan ingress for protocol probes; the desktop UI uses account ingress.
    pub async fn connect_plan(
        &self,
        plan_json: String,
        timeout_ms: u32,
        startup: ConnectionStartup,
    ) -> Result<VpnSession, VpnFailure> {
        self.inner
            .connect(
                self.control_value()?,
                Box::new(move |token| {
                    Box::pin(async move {
                        VpnConnectAttempt::for_owner(token)
                            .connect(plan_json, timeout_ms)
                            .await
                            .map(VpnSession::into_owned)
                    })
                }),
                startup.proxies,
                startup.tun,
                startup.recovery_directory,
            )
            .await
            .map(VpnSession::from_owned)
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn snapshot(&self) -> Result<OwnerSnapshot, VpnFailure> {
        self.inner.snapshot()
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn cancel(&self) -> Result<(), VpnFailure> {
        self.inner.cancel(self.control_value()?)
    }
    pub async fn disconnect(&self) -> Result<Option<String>, VpnFailure> {
        self.inner.disconnect(self.control_value()?).await
    }
    pub async fn shutdown(&self) -> Result<Option<String>, VpnFailure> {
        let result = self.inner.shutdown(self.control_value()?).await?;
        let mut current = DESKTOP
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|_| VpnFailure::new("worker", "无法释放桌面连接所有者"))?;
        if current
            .as_ref()
            .is_some_and(|owner| Arc::ptr_eq(owner, &self.inner))
        {
            current.take();
        }
        Ok(result)
    }
    pub async fn events(&self, sink: StreamSink<OwnerSnapshot>) {
        let mut events = self.inner.events();
        tokio::spawn(async move {
            loop {
                let snapshot = events.borrow_and_update().clone();
                let closed = snapshot.closed;
                if sink.add(snapshot).is_err() || closed {
                    break;
                }
                if events.changed().await.is_err() {
                    break;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::ensure;
    use tokio::{
        sync::Notify,
        time::{timeout, Duration},
    };

    #[tokio::test]
    async fn reopening_after_an_abandoned_shutdown_releases_only_the_closed_registry(
    ) -> anyhow::Result<()> {
        let first = Arc::new(DesktopConnectionOwner::open().await?);
        first.claim_frontend()?;
        first.await_frontend().await?;
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let control = first.control_value()?;
        let connecting = tokio::spawn({
            let inner = first.inner.clone();
            let entered = entered.clone();
            let release = release.clone();
            async move {
                inner
                    .connect(
                        control,
                        Box::new(move |_| {
                            Box::pin(async move {
                                entered.notify_one();
                                release.notified().await;
                                Err(VpnFailure::new("cancelled", "fixture cancelled"))
                            })
                        }),
                        ProxyStartup::default(),
                        None,
                        None,
                    )
                    .await
            }
        });
        timeout(Duration::from_secs(2), entered.notified()).await?;
        let ending = tokio::spawn({
            let first = first.clone();
            async move { first.shutdown().await }
        });
        timeout(Duration::from_secs(2), async {
            while first
                .snapshot()
                .is_ok_and(|snapshot| snapshot.phase != OwnerPhase::Disconnecting)
            {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        ending.abort();
        let _ = ending.await;
        release.notify_one();
        ensure!(timeout(Duration::from_secs(2), connecting).await??.is_err());
        timeout(Duration::from_secs(2), async {
            while first.snapshot().is_ok_and(|snapshot| !snapshot.closed) {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        let second = DesktopConnectionOwner::open().await?;
        ensure!(!second.snapshot()?.closed && !Arc::ptr_eq(&first.inner, &second.inner));
        first.shutdown().await?;
        let third = DesktopConnectionOwner::open().await?;
        ensure!(
            Arc::ptr_eq(&second.inner, &third.inner),
            "old shutdown removed the replacement registry"
        );
        second.shutdown().await?;
        Ok(())
    }
}
