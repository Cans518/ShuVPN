use super::wire::{Command, Reply};
use crate::platform::{
    linux_tun::{ConnectionTask, Device},
    model::{validate, PlatformFailure, TunInfo},
};
use futures_util::TryStreamExt;
use socket2::{Domain, SockRef, Type};
use std::{collections::BTreeSet, os::fd::OwnedFd, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(super) struct Session {
    device: Option<Arc<Device>>,
    consumed: bool,
}

impl Session {
    pub(super) async fn execute(
        &mut self,
        command: Command,
        descriptor: Option<OwnedFd>,
    ) -> (Reply, Option<OwnedFd>) {
        match self.apply(command, descriptor).await {
            Ok(reply) => reply,
            Err(error) => (
                Reply::Failure {
                    code: error.code,
                    message: error.message,
                },
                None,
            ),
        }
    }

    async fn apply(
        &mut self,
        command: Command,
        descriptor: Option<OwnedFd>,
    ) -> Result<(Reply, Option<OwnedFd>), PlatformFailure> {
        if matches!(command, Command::BindSocket { .. }) != descriptor.is_some() {
            return Err(PlatformFailure::new(
                "descriptor",
                "helper 操作与描述符不匹配",
            ));
        }
        match command {
            Command::Create { plan } => self.create(plan, Device::duplicate_fd).await,
            #[cfg(feature = "tun-probe")]
            Command::CreateTransferFailure { plan } => {
                self.create(plan, |_| {
                    Err(PlatformFailure::new(
                        "fixture",
                        "injected descriptor transfer failure",
                    ))
                })
                .await
            }
            Command::Routes { hosts } => {
                if hosts.len() > 1024 {
                    return Err(PlatformFailure::new("size", "helper 动态路由超过预算"));
                }
                self.device
                    .as_ref()
                    .ok_or_else(|| PlatformFailure::new("state", "helper 尚未创建 TUN"))?
                    .sync_dynamic(BTreeSet::from_iter(hosts), &CancellationToken::new())
                    .await?;
                Ok((Reply::Done {}, None))
            }
            Command::BindSocket { interface } => {
                bind_socket(
                    &descriptor.ok_or_else(|| {
                        PlatformFailure::new("descriptor", "物理出口需要 socket 描述符")
                    })?,
                    interface.as_deref(),
                )?;
                Ok((Reply::Done {}, None))
            }
            Command::Close {} | Command::Shutdown {} => {
                self.close().await?;
                // Keep the broker alive for the following physical-socket
                // restoration. Closing TUN is not permission to abandon egress.
                Ok((Reply::Done {}, None))
            }
            Command::Hello { .. } => Err(PlatformFailure::new("protocol", "helper 不接受重复握手")),
        }
    }

    async fn create(
        &mut self,
        plan: String,
        transfer: impl FnOnce(&Device) -> Result<OwnedFd, PlatformFailure>,
    ) -> Result<(Reply, Option<OwnedFd>), PlatformFailure> {
        if self.consumed {
            return Err(PlatformFailure::new("busy", "helper 已持有 TUN"));
        }
        self.consumed = true;
        let created = async {
            let device = Device::create(validate(&plan)?).await?;
            self.device = Some(Arc::clone(&device));
            let descriptor = transfer(&device)?;
            let document = serde_json::to_string(&device.info())
                .map_err(|_| PlatformFailure::new("encoding", "TUN 信息编码失败"))?;
            Ok((Reply::Created { document }, Some(descriptor)))
        }
        .await;
        if created.is_err() {
            // The owner is registered before fd duplication. Failure after
            // creation must revoke that owner as part of the same transaction.
            self.close().await?;
        }
        created
    }

    pub(super) async fn close(&mut self) -> Result<(), PlatformFailure> {
        if let Some(device) = &self.device {
            device.close().await?;
            verify_removed(&device.info()).await?;
            self.device = None;
        }
        Ok(())
    }
}

pub(super) async fn verify_removed(info: &TunInfo) -> Result<(), PlatformFailure> {
    let (connection, handle, _) = rtnetlink::new_connection()?;
    let _connection = ConnectionTask(tokio::spawn(connection));
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let mut links = handle.link().get().execute();
            let mut present = false;
            while let Some(link) = links
                .try_next()
                .await
                .map_err(super::super::netlink_error)?
            {
                present |= link.header.index == info.index;
            }
            if !present {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| PlatformFailure::new("cleanup", "TUN 接口仍在内核中，清理尚未确认"))?
}

fn bind_socket(descriptor: &OwnedFd, interface: Option<&str>) -> Result<(), PlatformFailure> {
    crate::network::interface::validate(interface)
        .map_err(|error| PlatformFailure::new(&error.code, &error.message))?;
    let socket = SockRef::from(descriptor);
    if !matches!(socket.domain()?, Domain::IPV4 | Domain::IPV6)
        || !matches!(socket.r#type()?, Type::STREAM | Type::DGRAM)
    {
        return Err(PlatformFailure::new(
            "descriptor",
            "helper 只保护 IPv4/IPv6 TCP/UDP socket",
        ));
    }
    socket.bind_device(interface.map(str::as_bytes))?;
    Ok(())
}
