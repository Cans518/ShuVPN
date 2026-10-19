use super::process::Process;
use super::wire::{Channel, Command, Reply, Request, Response, VERSION};
use crate::platform::{
    model::{validate, validate_dynamic, PlatformFailure, TunInfo},
    packet_fd::PacketFd,
};
use anyhow::{ensure, Context};
use std::{
    collections::BTreeSet,
    net::IpAddr,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as SyncMutex, Weak,
    },
    time::Duration,
};
use tokio::{
    io::unix::AsyncFd,
    sync::{mpsc, oneshot, Mutex, Notify},
};

const INSTALLED_HELPER: &str = "/usr/lib/shuvpn/shuvpn_tun_helper";
type Exchange = Result<(Reply, Option<OwnedFd>), PlatformFailure>;
struct Job {
    command: Command,
    descriptor: Option<OwnedFd>,
    reply: oneshot::Sender<Exchange>,
    accepted: Option<oneshot::Receiver<()>>,
}

/// A single owned helper process; callers cannot cancel a partially sent request.
#[derive(Clone)]
pub struct Client {
    jobs: mpsc::Sender<Job>,
    descriptors: Arc<SyncMutex<Vec<Weak<PacketFd>>>>,
    exit: Arc<Exit>,
    #[cfg(feature = "tun-probe")]
    process_id: u32,
}

impl Client {
    /// Check the production path and handshake without creating an interface.
    /// Diagnostic path overrides must not make the normal UI report readiness.
    pub(crate) async fn installed_ready() -> Result<(), PlatformFailure> {
        let client = Self::launch_at(trusted_path()?)
            .await
            .map_err(helper_error)?;
        client.shutdown().await
    }

    pub async fn launch() -> Result<Self, PlatformFailure> {
        #[cfg(feature = "tun-probe")]
        if let Some(path) = std::env::var_os("SHUVPN_LINUX_TUN_HELPER") {
            return Self::launch_at(path.into()).await.map_err(helper_error);
        }
        Self::launch_at(trusted_path()?).await.map_err(helper_error)
    }

    /// Only diagnostic builds can bypass the installed, root-owned helper path.
    #[cfg(feature = "tun-probe")]
    pub async fn launch_probe(path: PathBuf) -> anyhow::Result<Self> {
        Self::launch_at(path).await
    }

    async fn launch_at(path: PathBuf) -> anyhow::Result<Self> {
        let (channel, inherited) = Channel::pair()?;
        let raw = inherited.as_raw_fd();
        let mut command = tokio::process::Command::new(path);
        command
            .args(["--serve-fd", "3"])
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(false);
        // SAFETY: pre_exec performs only async-signal-safe fd syscalls. The
        // captured fd remains owned by this launcher until spawn completes.
        unsafe {
            command.pre_exec(move || {
                if raw != 3 && libc::dup2(raw, 3) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let descriptor = super::process::current_exit_descriptor()?;
                super::wire::send_process_descriptor(3, &descriptor)
            });
        }
        let child = command.spawn().context("start installed network helper")?;
        drop(inherited);
        // pre_exec opened and transferred its own pidfd before exec/spawn
        // returned. No async cancellation or numeric PID lookup occurs here.
        let descriptor = channel.process_descriptor()?;
        let child = Process::open(child, descriptor).context("watch installed network helper")?;
        #[cfg(feature = "tun-probe")]
        let process_id = child.id().context("network helper process missing")?;
        let (jobs, receiver) = mpsc::channel(32);
        let descriptors = Arc::new(SyncMutex::new(Vec::new()));
        let exit = Arc::new(Exit::default());
        tokio::spawn(serve(
            channel,
            child,
            receiver,
            PacketOwners(Arc::clone(&descriptors)),
            ExitGuard(Arc::clone(&exit)),
        ));
        let client = Self {
            jobs,
            descriptors,
            exit,
            #[cfg(feature = "tun-probe")]
            process_id,
        };
        let handshake = client
            .exchange(Command::Hello { version: VERSION }, None)
            .await;
        let (reply, descriptor) = match handshake {
            Ok(reply) => reply,
            Err(error) => {
                client.shutdown().await?;
                return Err(error.into());
            }
        };
        if descriptor.is_some()
            || !matches!(
                reply,
                Reply::Ready {
                    version: VERSION,
                    privileged: true,
                }
            )
        {
            drop(descriptor);
            client.shutdown().await?;
            anyhow::bail!(
                "network helper did not acquire required capabilities or protocol version"
            );
        }
        Ok(client)
    }

    #[cfg(feature = "tun-probe")]
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    #[cfg(feature = "tun-probe")]
    pub async fn fail_transfer_probe(&self, plan: String) -> Result<(), PlatformFailure> {
        self.done(Command::CreateTransferFailure { plan }, None)
            .await
    }

    #[cfg(feature = "tun-probe")]
    pub async fn abandon_create_probe(&self, plan: String) -> Result<(), PlatformFailure> {
        let (accepted, acceptance) = oneshot::channel();
        let (reply, descriptor) = self
            .exchange_owned(Command::Create { plan }, None, Some(acceptance))
            .await?;
        if !matches!(reply, Reply::Created { .. }) || descriptor.is_none() {
            return Err(protocol());
        }
        drop(descriptor);
        drop(accepted);
        Ok(())
    }

    async fn exchange(&self, command: Command, descriptor: Option<OwnedFd>) -> Exchange {
        self.exchange_owned(command, descriptor, None).await
    }

    async fn exchange_owned(
        &self,
        command: Command,
        descriptor: Option<OwnedFd>,
        accepted: Option<oneshot::Receiver<()>>,
    ) -> Exchange {
        let (reply, response) = oneshot::channel();
        self.jobs
            .send(Job {
                command,
                descriptor,
                reply,
                accepted,
            })
            .await
            .map_err(|_| closed())?;
        response.await.map_err(|_| closed())?
    }

    pub async fn create(&self, plan: String) -> Result<Lease, PlatformFailure> {
        let expected = validate(&plan)?;
        // Pin a generated name before crossing the process boundary. Validating
        // an omitted name independently would allocate two different identities.
        let mut document: serde_json::Value =
            serde_json::from_str(&plan).map_err(|_| protocol())?;
        document["name"] = expected.name.clone().into();
        let (accepted, acceptance) = oneshot::channel();
        let (reply, descriptor) = self
            .exchange_owned(
                Command::Create {
                    plan: document.to_string(),
                },
                None,
                Some(acceptance),
            )
            .await?;
        let Reply::Created { document } = reply else {
            return Err(protocol());
        };
        let descriptor = descriptor.ok_or_else(protocol)?;
        let info: TunInfo = serde_json::from_str(&document).map_err(|_| protocol())?;
        if info.closed
            || info.index == 0
            || info.name != expected.name
            || info.mtu != expected.mtu
            || info.addresses
                != expected
                    .addresses
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            || info.routes
                != expected
                    .routes
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
        {
            return Err(PlatformFailure::new(
                "identity",
                "helper TUN 配置与请求不一致",
            ));
        }
        let descriptor = AsyncFd::new(descriptor)?;
        super::super::verify_name(&descriptor, &info.name)?;
        let io = Arc::new(PacketFd::registered(descriptor, info.mtu));
        self.descriptors
            .lock()
            .map_err(|_| protocol())?
            .push(Arc::downgrade(&io));
        let lease = Lease {
            io,
            client: self.clone(),
            protected: expected.protected_ips,
            info,
            close: Mutex::new(false),
        };
        accepted.send(()).map_err(|_| closed())?;
        Ok(lease)
    }

    /// The referenced socket is passed with SCM_RIGHTS; GUI fd numbers never
    /// cross the process boundary as identifiers.
    pub async fn bind_socket(
        &self,
        socket: &socket2::Socket,
        interface: Option<String>,
    ) -> Result<(), PlatformFailure> {
        let descriptor: OwnedFd = socket.try_clone()?.into();
        self.done(Command::BindSocket { interface }, Some(descriptor))
            .await
    }

    async fn done(
        &self,
        command: Command,
        descriptor: Option<OwnedFd>,
    ) -> Result<(), PlatformFailure> {
        let (reply, descriptor) = self.exchange(command, descriptor).await?;
        if descriptor.is_none() && matches!(reply, Reply::Done {}) {
            Ok(())
        } else {
            Err(protocol())
        }
    }

    pub async fn close_tun(&self) -> Result<(), PlatformFailure> {
        self.done(Command::Close {}, None).await
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.jobs.is_closed()
    }

    pub async fn shutdown(&self) -> Result<(), PlatformFailure> {
        if !self.is_closed() {
            if let Err(error) = self.done(Command::Shutdown {}, None).await {
                if !matches!(error.code.as_str(), "helper" | "closed" | "timeout") {
                    return Err(error);
                }
            }
        }
        tokio::time::timeout(Duration::from_secs(5), self.exit.wait())
            .await
            .map_err(|_| PlatformFailure::new("cleanup", "网络权限程序退出尚未确认"))?
    }
}

#[derive(Default)]
struct Exit {
    finished: AtomicBool,
    confirmed: AtomicBool,
    failure: SyncMutex<Option<String>>,
    notify: Notify,
}
impl Exit {
    fn failure(&self) -> PlatformFailure {
        let detail = self.failure.lock().ok().and_then(|value| value.clone());
        PlatformFailure::new(
            "worker",
            &match detail {
                Some(detail) => format!("权限程序退出尚未确认：{detail}"),
                None => "权限程序工作线程异常，退出尚未确认".to_owned(),
            },
        )
    }

    async fn wait(&self) -> Result<(), PlatformFailure> {
        loop {
            let notified = self.notify.notified();
            if self.finished.load(Ordering::Acquire) {
                return if self.confirmed.load(Ordering::Acquire) {
                    Ok(())
                } else {
                    Err(self.failure())
                };
            }
            notified.await;
        }
    }
}
struct ExitGuard(Arc<Exit>);
impl Drop for ExitGuard {
    fn drop(&mut self) {
        self.0.finished.store(true, Ordering::Release);
        self.0.notify.notify_waiters();
    }
}

/// The unprivileged process performs packet I/O on its own transferred TUN fd.
/// The helper retains configuration authority until both fd owners are closed.
pub struct Lease {
    io: Arc<PacketFd>,
    client: Client,
    info: TunInfo,
    protected: Vec<IpAddr>,
    close: Mutex<bool>,
}

impl Lease {
    pub fn info(&self) -> Result<String, PlatformFailure> {
        let mut info = self.info.clone();
        info.closed = self.io.is_closed();
        serde_json::to_string(&info).map_err(|_| protocol())
    }
    pub async fn read_packet(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        self.io.read(wait_ms).await
    }
    pub async fn write_packet(&self, packet: Vec<u8>) -> Result<(), PlatformFailure> {
        self.io.write(&packet).await
    }
    pub async fn sync_routes(&self, hosts: Vec<IpAddr>) -> Result<(), PlatformFailure> {
        let mut wanted = BTreeSet::from_iter(hosts);
        validate_dynamic(&self.info, &self.protected, &mut wanted)?;
        self.client
            .done(
                Command::Routes {
                    hosts: wanted.into_iter().collect(),
                },
                None,
            )
            .await
    }
    pub async fn close(&self) -> Result<(), PlatformFailure> {
        let mut confirmed = self.close.lock().await;
        if !*confirmed {
            self.io.close().await?;
            if let Err(error) = self.client.close_tun().await {
                if !self.client.is_closed() {
                    return Err(error);
                }
                self.client.exit.wait().await?;
                super::session::verify_removed(&self.info).await?;
            }
            *confirmed = true;
        }
        Ok(())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        // Explicit close is awaited by the session owner. This fallback also
        // covers an abandoned lease while its socket broker is retained.
        let client = self.client.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = client.done(Command::Close {}, None).await;
            });
        }
    }
}

struct PacketOwners(Arc<SyncMutex<Vec<Weak<PacketFd>>>>);
impl Drop for PacketOwners {
    fn drop(&mut self) {
        if let Ok(descriptors) = self.0.lock() {
            for descriptor in descriptors.iter().filter_map(Weak::upgrade) {
                let _ = descriptor.request_close();
            }
        }
    }
}

async fn serve(
    channel: Channel,
    mut child: Process,
    mut jobs: mpsc::Receiver<Job>,
    _owners: PacketOwners,
    exit: ExitGuard,
) {
    let mut next = 0u64;
    let mut shutdown_reply = None;
    loop {
        let job = tokio::select! {
            biased;
            _ = child.wait() => break,
            job = jobs.recv() => match job { Some(job) => job, None => break },
        };
        let creating = matches!(job.command, Command::Create { .. });
        let shutdown = matches!(job.command, Command::Shutdown {});
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            channel
                .send(
                    &Request {
                        id: next,
                        command: job.command,
                    },
                    job.descriptor.as_ref(),
                )
                .await?;
            let (response, descriptor) = channel
                .receive::<Response>()
                .await?
                .context("network helper closed before reply")?;
            ensure!(response.id == next, "network helper response order changed");
            Ok::<_, anyhow::Error>((response.reply, descriptor))
        })
        .await;
        let result = match result {
            Ok(Ok((Reply::Failure { code, message }, descriptor))) => {
                drop(descriptor);
                Err(PlatformFailure::new(&code, &message))
            }
            Ok(Ok(reply)) => Ok(reply),
            failed => {
                let error = match failed {
                    Ok(Err(error)) => helper_error(error),
                    Err(_) => PlatformFailure::new("timeout", "网络权限操作超时，清理状态尚未确认"),
                    Ok(Ok(_)) => unreachable!(),
                };
                let _ = job.reply.send(Err(error));
                break;
            }
        };
        let created = creating && result.is_ok();
        if shutdown && matches!(&result, Ok((Reply::Done {}, None))) {
            shutdown_reply = Some(job.reply);
            break;
        }
        let delivery = job.reply.send(result);
        let delivered = delivery.is_ok();
        drop(delivery);
        let abandoned_create = if created && delivered {
            match job.accepted {
                Some(accepted) => !matches!(
                    tokio::time::timeout(Duration::from_secs(5), accepted).await,
                    Ok(Ok(()))
                ),
                None => true,
            }
        } else {
            created
        };
        let Some(incremented) = next.checked_add(1) else {
            break;
        };
        next = incremented;
        if abandoned_create {
            // The oneshot receiver disappeared while Create was in flight.
            // Its transferred fd was dropped above; release the helper's owner
            // before accepting any further socket operations.
            let cleanup = tokio::time::timeout(Duration::from_secs(5), async {
                channel
                    .send(
                        &Request {
                            id: next,
                            command: Command::Close {},
                        },
                        None,
                    )
                    .await?;
                let (response, descriptor) = channel
                    .receive::<Response>()
                    .await?
                    .context("helper abandoned-create cleanup missing")?;
                ensure!(
                    response.id == next
                        && descriptor.is_none()
                        && matches!(response.reply, Reply::Done {}),
                    "helper abandoned-create cleanup failed"
                );
                Ok::<_, anyhow::Error>(())
            })
            .await;
            if !matches!(cleanup, Ok(Ok(()))) {
                break;
            }
            let Some(incremented) = next.checked_add(1) else {
                break;
            };
            next = incremented;
        }
    }
    drop(channel);
    let waited = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
    let confirmed = match waited {
        Ok(Ok(_)) => true,
        failed => match tokio::time::timeout(Duration::from_secs(3), child.kill()).await {
            Ok(Ok(())) => true,
            Err(error) => {
                if let Ok(mut detail) = exit.0.failure.lock() {
                    *detail = Some(format!("wait={failed:?}; kill={error}"));
                }
                false
            }
            Ok(Err(error)) => {
                if let Ok(mut detail) = exit.0.failure.lock() {
                    *detail = Some(format!("wait={failed:?}; kill={error}"));
                }
                false
            }
        },
    };
    drop(_owners);
    exit.0.confirmed.store(confirmed, Ordering::Release);
    let confirmation_error = (!confirmed).then(|| exit.0.failure());
    drop(exit);
    if let Some(reply) = shutdown_reply {
        let result = match confirmation_error {
            None => Ok((Reply::Done {}, None)),
            Some(error) => Err(error),
        };
        let _ = reply.send(result);
    }
}

fn trusted_path() -> Result<PathBuf, PlatformFailure> {
    let path = Path::new(INSTALLED_HELPER);
    for component in [
        Path::new("/"),
        Path::new("/usr"),
        Path::new("/usr/lib"),
        Path::new("/usr/lib/shuvpn"),
        path,
    ] {
        let metadata = std::fs::symlink_metadata(component)?;
        if metadata.file_type().is_symlink() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0
        {
            return Err(PlatformFailure::new(
                "permission",
                "网络权限程序的安装路径不可信",
            ));
        }
    }
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.mode() & 0o111 == 0 {
        return Err(PlatformFailure::new("permission", "网络权限程序未正确安装"));
    }
    Ok(path.to_owned())
}
fn helper_error(error: anyhow::Error) -> PlatformFailure {
    PlatformFailure::new("helper", &format!("网络权限程序失败：{error:#}"))
}
fn closed() -> PlatformFailure {
    PlatformFailure::new("closed", "网络权限程序已关闭")
}
fn protocol() -> PlatformFailure {
    PlatformFailure::new("protocol", "网络权限程序返回无效信息")
}
