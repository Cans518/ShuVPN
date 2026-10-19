//! Capability boundary. The desktop executable never acquires file capabilities.
mod broker;
mod client;
mod process;
mod session;
mod wire;

use anyhow::{ensure, Context};
pub(crate) use broker::Broker;
pub use client::{Client, Lease};
use std::{os::fd::FromRawFd, time::Duration};
use wire::{Channel, Command, Reply, Request, Response, VERSION};

pub async fn run(arguments: Vec<String>) -> anyhow::Result<()> {
    ensure!(
        arguments == ["--serve-fd", "3"],
        "invalid Linux helper arguments"
    );
    // SAFETY: this only configures the current process's parent-death signal.
    ensure!(
        unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) } == 0,
        "cannot watch helper parent"
    );
    // SAFETY: F_GETFD observes fd validity without assuming it is owned yet.
    ensure!(
        unsafe { libc::fcntl(3, libc::F_GETFD) } >= 0,
        "helper fd is missing"
    );
    // SAFETY: the entry point consumes the documented inherited fd exactly once.
    let descriptor = unsafe { std::os::fd::OwnedFd::from_raw_fd(3) };
    let channel = Channel::inherited(descriptor)?;
    let peer = channel.peer()?;
    // SAFETY: getppid has no preconditions. Check after arming PDEATHSIG, so an
    // already exited parent cannot leave a detached privileged process alive.
    ensure!(
        peer.pid > 1 && peer.pid == unsafe { libc::getppid() },
        "helper parent changed"
    );
    let first = tokio::time::timeout(Duration::from_secs(5), channel.receive::<Request>())
        .await
        .context("helper handshake timeout")??
        .context("helper parent closed")?;
    ensure!(first.1.is_none(), "handshake cannot carry a descriptor");
    ensure!(
        first.0.id == 0 && matches!(first.0.command, Command::Hello { version: VERSION }),
        "invalid helper handshake"
    );
    let privileged = super::privileges_ready()?;
    channel
        .send(
            &Response {
                id: 0,
                reply: Reply::Ready {
                    version: VERSION,
                    privileged,
                },
            },
            None,
        )
        .await?;
    ensure!(privileged, "helper requires CAP_NET_ADMIN and CAP_NET_RAW");
    let mut session = session::Session::default();
    let result = async {
        let mut next = 1u64;
        while let Some((request, descriptor)) = channel.receive::<Request>().await? {
            ensure!(request.id == next, "helper request order changed");
            next = next.checked_add(1).context("helper request limit")?;
            let shutdown = matches!(request.command, Command::Shutdown {});
            let (reply, descriptor) = session.execute(request.command, descriptor).await;
            let finished = shutdown && matches!(reply, Reply::Done {});
            channel
                .send(
                    &Response {
                        id: request.id,
                        reply,
                    },
                    descriptor.as_ref(),
                )
                .await?;
            if finished {
                break;
            }
        }
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let cleanup = session.close().await;
    result?;
    cleanup?;
    Ok(())
}
