use anyhow::{ensure, Context};
use std::{net::IpAddr, time::Duration};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY},
    Storage::FileSystem::{SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT},
};

mod launch;
mod process;
mod session;
#[cfg(test)]
mod tests;
mod wire;
pub(super) use launch::installed_path;
use process::{Process, Side};
use wire::{Command, Reply, Request, Response, VERSION};

const PREFIX: &str = r"\\.\pipe\shuvpn-tun-";

/// A single-use local endpoint. Creating it does not elevate or mutate networking.
pub struct Endpoint {
    pipe: NamedPipeServer,
    nonce: String,
    created: u64,
}

impl Endpoint {
    pub fn new() -> anyhow::Result<Self> {
        let mut random = [0; 32];
        getrandom::getrandom(&mut random).context("generate helper endpoint")?;
        let nonce: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let pipe = ServerOptions::new()
            .first_pipe_instance(true)
            .max_instances(1)
            .reject_remote_clients(true)
            .create(format!("{PREFIX}{nonce}"))?;
        let created = Process::open(std::process::id())?.created()?;
        Ok(Self {
            pipe,
            nonce,
            created,
        })
    }

    /// Generated pipe name, PID and process creation time identify this endpoint.
    pub fn arguments(&self) -> [String; 3] {
        [
            format!("{PREFIX}{}", self.nonce),
            std::process::id().to_string(),
            self.created.to_string(),
        ]
    }

    /// Launches the packaged helper via Windows' runas consent flow.
    pub async fn launch(self) -> anyhow::Result<Client> {
        let arguments = self.arguments();
        let process = tokio::task::spawn_blocking(move || launch::elevated(arguments)).await??;
        self.accept_process(process).await
    }

    /// Accepts an already launched helper, retaining its process identity.
    pub async fn accept(self, helper_pid: u32) -> anyhow::Result<Client> {
        self.accept_process(Process::open(helper_pid)?).await
    }

    async fn accept_process(self, process: Process) -> anyhow::Result<Client> {
        let expected = process.id()?;
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                ensure!(process.alive()?, "helper exited before connecting");
                tokio::select! {
                    connected = self.pipe.connect() => connected?,
                    exited = parent_exit(&process) => { exited?; anyhow::bail!("helper exited before connecting"); }
                }
                if process::verify_peer(&self.pipe, expected, Side::Client).is_ok() {
                    break;
                }
                self.pipe.disconnect()?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("helper connection timed out")??;
        let client = Client {
            channel: tokio::sync::Mutex::new(Channel {
                pipe: Some(self.pipe),
                next: 0,
                close_confirmed: false,
            }),
            process,
            elevated: false,
        };
        let reply = client
            .exchange(Command::Hello {
                version: VERSION,
                nonce: self.nonce,
            })
            .await?;
        if let Reply::Ready {
            version: VERSION,
            elevated,
        } = reply
        {
            Ok(Client { elevated, ..client })
        } else {
            anyhow::bail!("invalid helper handshake response")
        }
    }
}

struct Channel {
    pipe: Option<NamedPipeServer>,
    next: u64,
    close_confirmed: bool,
}

/// Calls are serialized. Cancellation invalidates the entire pipe connection.
pub struct Client {
    channel: tokio::sync::Mutex<Channel>,
    process: Process,
    elevated: bool,
}

impl Client {
    #[cfg(feature = "tun-probe")]
    pub(crate) fn probe_identity(&self) -> anyhow::Result<(u32, u64)> {
        ensure!(
            self.process.alive()?,
            "helper exited before probe identity capture"
        );
        Ok((self.process.id()?, self.process.created()?))
    }

    pub fn elevated(&self) -> bool {
        self.elevated
    }

    pub async fn create(
        &self,
        plan: String,
    ) -> Result<String, crate::platform::model::PlatformFailure> {
        match self.call(Command::Create { plan }).await? {
            Reply::Info { document } => Ok(document),
            _ => Err(failure("protocol", "unexpected helper create response")),
        }
    }

    pub async fn info(&self) -> Result<String, crate::platform::model::PlatformFailure> {
        match self.call(Command::Info {}).await? {
            Reply::Info { document } => Ok(document),
            _ => Err(failure("protocol", "unexpected helper info response")),
        }
    }

    pub async fn read(
        &self,
        wait_ms: u32,
    ) -> Result<Option<Vec<u8>>, crate::platform::model::PlatformFailure> {
        match self.call(Command::Read { wait_ms }).await? {
            Reply::Packet { packet } => Ok(packet),
            _ => Err(failure("protocol", "unexpected helper read response")),
        }
    }

    pub async fn write(
        &self,
        packet: Vec<u8>,
    ) -> Result<(), crate::platform::model::PlatformFailure> {
        self.call(Command::Write { packet }).await.map(|_| ())
    }

    pub async fn sync_routes(
        &self,
        hosts: Vec<IpAddr>,
    ) -> Result<(), crate::platform::model::PlatformFailure> {
        self.call(Command::Routes { hosts }).await.map(|_| ())
    }

    pub async fn close(&self) -> Result<(), crate::platform::model::PlatformFailure> {
        if let Err(error) = self.call(Command::Close {}).await {
            // EOF/timeout invalidates the pipe. A clean process exit still
            // proves the helper's final Session::close completed successfully;
            // a failed or live helper must keep cleanup in the error state.
            if self.channel.lock().await.pipe.is_some() {
                return Err(error);
            }
            self.wait_exit().await?;
            self.channel.lock().await.close_confirmed = true;
            return Ok(());
        }
        self.wait_exit().await
    }

    #[cfg(feature = "tun-probe")]
    pub(crate) async fn disconnect_and_wait(
        &self,
    ) -> Result<(), crate::platform::model::PlatformFailure> {
        self.channel.lock().await.pipe.take();
        self.wait_exit().await
    }

    async fn wait_exit(&self) -> Result<(), crate::platform::model::PlatformFailure> {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.process.alive()? {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            self.process.successful_exit()?;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .map_err(|_| failure("cleanup", "helper did not exit after close"))?
        .map_err(|error| failure("cleanup", &error.to_string()))
    }

    async fn call(
        &self,
        command: Command,
    ) -> Result<Reply, crate::platform::model::PlatformFailure> {
        match self
            .exchange(command)
            .await
            .map_err(|error| failure("helper", &error.to_string()))?
        {
            Reply::Failure { code, message } => Err(failure(&code, &message)),
            reply => Ok(reply),
        }
    }

    async fn exchange(&self, command: Command) -> anyhow::Result<Reply> {
        let mut channel = self.channel.lock().await;
        let close = matches!(command, Command::Close {});
        if close && channel.close_confirmed {
            return Ok(Reply::Done {});
        }
        // While an exchange awaits IO, Channel owns no pipe. Dropping this
        // future drops the local pipe; no later call can consume a stale reply.
        let mut pipe = channel.pipe.take().context("helper channel is closed")?;
        ensure!(self.process.alive()?, "helper process exited");
        let id = channel.next;
        channel.next = id.checked_add(1).context("helper sequence exhausted")?;
        let request = Request { id, command };
        let reply = tokio::time::timeout(Duration::from_secs(12), async {
            wire::send(&mut pipe, &request).await?;
            let response: Response = wire::receive(&mut pipe)
                .await?
                .context("helper disconnected")?;
            ensure!(response.id == id, "helper response sequence mismatch");
            ensure!(
                reply_matches(&request.command, &response.result),
                "helper response type mismatch"
            );
            Ok::<_, anyhow::Error>(response.result)
        })
        .await
        .context("helper request timed out")??;
        if close {
            channel.close_confirmed = matches!(reply, Reply::Done {});
        }
        if !close || !channel.close_confirmed {
            channel.pipe = Some(pipe);
        }
        Ok(reply)
    }
}

fn reply_matches(command: &Command, reply: &Reply) -> bool {
    matches!(
        (command, reply),
        (_, Reply::Failure { .. })
            | (Command::Hello { .. }, Reply::Ready { .. })
            | (
                Command::Create { .. } | Command::Info {},
                Reply::Info { .. }
            )
            | (Command::Read { .. }, Reply::Packet { .. })
            | (
                Command::Write { .. } | Command::Routes { .. } | Command::Close {},
                Reply::Done {}
            )
    )
}

fn failure(code: &str, message: &str) -> crate::platform::model::PlatformFailure {
    crate::platform::model::PlatformFailure::new(code, message)
}

/// Binary entry point. Unprivileged mode supports IPC diagnostics but cannot create TUN.
pub async fn run(arguments: Vec<String>) -> anyhow::Result<()> {
    ensure!(
        arguments.len() == 3,
        "helper requires pipe name, parent PID and creation time"
    );
    let nonce = arguments[0]
        .strip_prefix(PREFIX)
        .context("invalid helper pipe prefix")?;
    ensure!(
        nonce.len() == 64
            && nonce
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid helper endpoint"
    );
    let parent = Process::open(arguments[1].parse()?)?;
    ensure!(
        parent.created()? == arguments[2].parse::<u64>()?,
        "helper parent generation mismatch"
    );
    let parent_pid = parent.id()?;
    let mut pipe = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            ensure!(parent.alive()?, "helper parent exited");
            match ClientOptions::new()
                .security_qos_flags(SECURITY_IDENTIFICATION | SECURITY_SQOS_PRESENT)
                .open(&arguments[0]) {
                Ok(pipe) => return Ok::<_, anyhow::Error>(pipe),
                Err(error) if matches!(error.raw_os_error(), Some(code) if code == ERROR_FILE_NOT_FOUND as i32 || code == ERROR_PIPE_BUSY as i32) => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }).await.context("helper parent connection timed out")??;
    process::verify_peer(&pipe, parent_pid, Side::Server)?;
    let hello: Request = tokio::time::timeout(Duration::from_secs(5), wire::receive(&mut pipe))
        .await??
        .context("missing helper handshake")?;
    ensure!(
        hello.id == 0
            && matches!(&hello.command, Command::Hello { version: VERSION, nonce: received } if received == nonce),
        "helper handshake rejected"
    );
    tokio::time::timeout(
        Duration::from_secs(5),
        wire::send(
            &mut pipe,
            &Response {
                id: 0,
                result: Reply::Ready {
                    version: VERSION,
                    elevated: super::administrator()?,
                },
            },
        ),
    )
    .await??;
    let (mut input, mut output) = tokio::io::split(pipe);
    let (sender, mut requests) = tokio::sync::mpsc::channel(1);
    let disconnected = tokio_util::sync::CancellationToken::new();
    let ended = disconnected.clone();
    let mut readers = tokio::task::JoinSet::new();
    readers.spawn(async move {
        let result = async {
            loop {
                let request = tokio::time::timeout(
                    Duration::from_secs(60),
                    wire::receive::<Request>(&mut input),
                )
                .await??;
                let Some(request) = request else {
                    break;
                };
                sender
                    .send(request)
                    .await
                    .context("helper request consumer stopped")?;
            }
            Ok::<_, anyhow::Error>(())
        }
        .await;
        ended.cancel();
        result
    });
    let mut session = session::Session::default();
    let work = async {
        let mut next = 1u64;
        loop {
            let request: Option<Request> = tokio::select! {
                request = requests.recv() => request,
                exited = parent_exit(&parent) => { exited?; break; },
                _ = disconnected.cancelled() => break,
            };
            let Some(request) = request else { break; };
            ensure!(request.id == next, "helper request sequence mismatch");
            ensure!(!matches!(request.command, Command::Hello { .. }), "duplicate helper handshake");
            next = next.checked_add(1).context("helper sequence exhausted")?;
            let close = matches!(request.command, Command::Close {});
            let result = tokio::select! {
                result = tokio::time::timeout(Duration::from_secs(12), session.execute(request.command)) => result?,
                exited = parent_exit(&parent) => { exited?; break; },
                _ = disconnected.cancelled() => break,
            };
            let fatal = matches!(&result, Reply::Failure { code, .. } if code == "route_rollback");
            let closed = close && matches!(&result, Reply::Done {});
            tokio::time::timeout(
                Duration::from_secs(5),
                wire::send(&mut output, &Response { id: request.id, result }),
            ).await??;
            if closed || fatal { break; }
        }
        Ok::<_, anyhow::Error>(())
    }.await;
    let cleanup = session.close().await;
    readers.abort_all();
    let reader = readers.join_next().await;
    cleanup.context("helper cleanup failed")?;
    if let Some(reader) = reader {
        match reader {
            Ok(result) => result?,
            Err(error) if error.is_cancelled() => {}
            Err(error) => return Err(error.into()),
        }
    }
    work
}

async fn parent_exit(parent: &Process) -> anyhow::Result<()> {
    while parent.alive()? {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}
