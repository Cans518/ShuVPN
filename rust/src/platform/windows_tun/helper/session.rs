use super::wire::{Command, Reply};
use crate::platform::{
    model::{validate, PlatformFailure},
    windows_tun::Device,
};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(super) struct Session {
    device: Option<Arc<Device>>,
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(device) = &self.device {
            let _ = device.request_close();
        }
    }
}

impl Session {
    fn device(&self) -> Result<&Arc<Device>, PlatformFailure> {
        self.device
            .as_ref()
            .ok_or_else(|| PlatformFailure::new("state", "helper 尚未创建 TUN"))
    }

    pub(super) async fn execute(&mut self, command: Command) -> Reply {
        match self.apply(command).await {
            Ok(reply) => reply,
            Err(failure) => Reply::Failure {
                code: failure.code,
                message: failure.message,
            },
        }
    }

    async fn apply(&mut self, command: Command) -> Result<Reply, PlatformFailure> {
        match command {
            Command::Create { plan } => {
                if self.device.is_some() {
                    return Err(PlatformFailure::new("state", "helper 已持有 TUN"));
                }
                let device = Device::create(validate(&plan)?).await?;
                self.device = Some(Arc::clone(&device));
                device.wait_ready().await?;
                let document = serde_json::to_string(&device.info())
                    .map_err(|_| PlatformFailure::new("encoding", "helper TUN 信息编码失败"))?;
                Ok(Reply::Info { document })
            }
            Command::Info {} => Ok(Reply::Info {
                document: serde_json::to_string(&self.device()?.info())
                    .map_err(|_| PlatformFailure::new("encoding", "helper TUN 信息编码失败"))?,
            }),
            Command::Read { wait_ms } => Ok(Reply::Packet {
                packet: self.device()?.read(wait_ms).await?,
            }),
            Command::Write { packet } => {
                if packet.len() > 9000 {
                    return Err(PlatformFailure::new("packet", "helper IP 包超过预算"));
                }
                self.device()?.write(&packet).await?;
                Ok(Reply::Done {})
            }
            Command::Routes { hosts } => {
                if hosts.len() > 1024 {
                    return Err(PlatformFailure::new("size", "helper 动态路由超过预算"));
                }
                self.device()?
                    .sync_dynamic(BTreeSet::from_iter(hosts), &CancellationToken::new())
                    .await?;
                Ok(Reply::Done {})
            }
            Command::Close {} => {
                self.close().await?;
                Ok(Reply::Done {})
            }
            Command::Hello { .. } => Err(PlatformFailure::new("protocol", "helper 不接受重复握手")),
        }
    }

    pub(super) async fn close(&mut self) -> Result<(), PlatformFailure> {
        if let Some(device) = &self.device {
            tokio::time::timeout(Duration::from_secs(10), device.close())
                .await
                .map_err(|_| {
                    PlatformFailure::new("cleanup", "helper TUN 清理超时，状态未经证明")
                })??;
            self.device = None;
        }
        Ok(())
    }
}
