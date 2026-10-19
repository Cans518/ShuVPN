//! Session-owned local device or capability-helper fd and configuration lease.
use super::{
    helper::{Broker, Client, Lease},
    Device as LocalDevice,
};
use crate::platform::model::{PlatformFailure, TunInfo, ValidatedTunPlan};
use std::{
    collections::BTreeSet,
    net::IpAddr,
    sync::{Arc, Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;

pub(crate) struct Prepared {
    broker: Option<Arc<Broker>>,
}
impl Prepared {
    pub(crate) async fn prepare() -> Result<Self, PlatformFailure> {
        Ok(Self {
            broker: Broker::prepare().await?,
        })
    }
    pub(crate) fn broker(&self) -> Option<Arc<Broker>> {
        self.broker.clone()
    }
    pub(crate) async fn close(&self) -> Result<(), PlatformFailure> {
        if let Some(broker) = &self.broker {
            broker.close().await?;
        }
        Ok(())
    }
    pub(crate) async fn create(
        self,
        plan: ValidatedTunPlan,
        owner: &Mutex<Option<Arc<Device>>>,
    ) -> Result<Arc<Device>, PlatformFailure> {
        let Some(broker) = self.broker else {
            let local = LocalDevice::create(plan).await?;
            let device = Arc::new(Device {
                backend: Backend::Local(local),
            });
            *owner.lock().map_err(|_| lock_error())? = Some(Arc::clone(&device));
            return Ok(device);
        };
        let client = broker.client().await?;
        let mut document = serde_json::json!({
            "name":plan.name, "mtu":plan.mtu,
            "addresses":plan.addresses.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "routes":plan.routes.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "protected_ips":plan.protected_ips, "physical_dns_servers":plan.physical_dns_servers,
            "dns_capture":plan.dns_capture,
        });
        if plan.capture_mode == crate::platform::model::CaptureMode::Full {
            document["capture_mode"] = serde_json::json!(plan.capture_mode);
        }
        let document = document.to_string();
        let device = Arc::new(Device {
            backend: Backend::Remote(Remote {
                client: client.clone(),
                lease: RwLock::new(None),
                initial: TunInfo {
                    name: plan.name,
                    index: 0,
                    mtu: plan.mtu,
                    addresses: plan.addresses.iter().map(ToString::to_string).collect(),
                    routes: plan.routes.iter().map(ToString::to_string).collect(),
                    closed: false,
                },
            }),
        });
        *owner.lock().map_err(|_| lock_error())? = Some(Arc::clone(&device));
        let lease = Arc::new(client.create(document).await?);
        let Backend::Remote(remote) = &device.backend else {
            return Err(lock_error());
        };
        *remote.lease.write().map_err(|_| lock_error())? = Some(lease);
        Ok(device)
    }
}
enum Backend {
    Local(Arc<LocalDevice>),
    Remote(Remote),
}
struct Remote {
    client: Client,
    lease: RwLock<Option<Arc<Lease>>>,
    initial: TunInfo,
}
impl Remote {
    fn lease(&self) -> Result<Arc<Lease>, PlatformFailure> {
        self.lease
            .read()
            .map_err(|_| lock_error())?
            .clone()
            .ok_or_else(|| PlatformFailure::new("closed", "TUN 尚未接管描述符"))
    }
}
pub(crate) struct Device {
    backend: Backend,
}
impl Device {
    pub(crate) fn info(&self) -> Result<TunInfo, PlatformFailure> {
        match &self.backend {
            Backend::Local(device) => Ok(device.info()),
            Backend::Remote(remote) => {
                match remote.lease.read().map_err(|_| lock_error())?.as_ref() {
                    Some(lease) => serde_json::from_str(&lease.info()?).map_err(|_| lock_error()),
                    None => Ok(remote.initial.clone()),
                }
            }
        }
    }
    pub(crate) async fn read(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        match &self.backend {
            Backend::Local(device) => device.read(wait_ms).await,
            Backend::Remote(remote) => remote.lease()?.read_packet(wait_ms).await,
        }
    }
    pub(crate) async fn write(&self, packet: &[u8]) -> Result<(), PlatformFailure> {
        match &self.backend {
            Backend::Local(device) => device.write(packet).await,
            Backend::Remote(remote) => remote.lease()?.write_packet(packet.to_vec()).await,
        }
    }
    pub(crate) async fn sync_dynamic(
        &self,
        hosts: BTreeSet<IpAddr>,
        cancellation: &CancellationToken,
    ) -> Result<(), PlatformFailure> {
        match &self.backend {
            Backend::Local(device) => device.sync_dynamic(hosts, cancellation).await,
            Backend::Remote(remote) => {
                let lease = remote.lease()?;
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => Err(PlatformFailure::new("cancelled", "动态路由已取消")),
                    result = lease.sync_routes(hosts.into_iter().collect()) => result,
                }
            }
        }
    }
    pub(crate) async fn close(&self) -> Result<(), PlatformFailure> {
        match &self.backend {
            Backend::Local(device) => device.close().await,
            Backend::Remote(remote) => {
                let lease = remote.lease.read().map_err(|_| lock_error())?.clone();
                match lease {
                    Some(lease) => lease.close().await,
                    None => match remote.client.close_tun().await {
                        Ok(()) => Ok(()),
                        Err(_) if remote.client.is_closed() => remote.client.shutdown().await,
                        Err(error) => Err(error),
                    },
                }
            }
        }
    }
}
fn lock_error() -> PlatformFailure {
    PlatformFailure::new("state", "TUN 描述符状态不可用")
}
