use super::full_process::{Identity, Process};
use anyhow::{ensure, Context};
use serde_json::{json, Value};
use std::{os::windows::io::AsRawHandle, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId};

pub const ENV_TOKEN: &str = "SHUVPN_WINDOWS_FULL_OBSERVATION_TOKEN";
pub const RPC_BUDGET: Duration = Duration::from_secs(45);
const FRAME_LIMIT: u32 = 4 * 1024 * 1024;

pub fn pipe_name(token: &str) -> anyhow::Result<String> {
    ensure!(
        token.len() == 32
            && token
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "observation token must be 32 lower-case hexadecimal characters"
    );
    Ok(format!(r"\\.\pipe\shuvpn-full-observer-{token}"))
}

pub fn pipe_process(pipe: &impl AsRawHandle, server: bool) -> anyhow::Result<Process> {
    let mut pid = 0;
    // SAFETY: borrowed local pipe and writable u32 output remain live.
    let code = unsafe {
        if server {
            GetNamedPipeServerProcessId(pipe.as_raw_handle().cast(), &mut pid)
        } else {
            GetNamedPipeClientProcessId(pipe.as_raw_handle().cast(), &mut pid)
        }
    };
    ensure!(
        code != 0 && pid != 0,
        "named-pipe process identity query failed"
    );
    Process::open(pid)
}

pub async fn read_frame(pipe: &mut (impl AsyncRead + Unpin)) -> anyhow::Result<Value> {
    let size = pipe.read_u32().await?;
    ensure!(
        (2..=FRAME_LIMIT).contains(&size),
        "invalid observation frame length"
    );
    let mut bytes = vec![0; size as usize];
    pipe.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub async fn write_frame(
    pipe: &mut (impl AsyncWrite + Unpin),
    value: &Value,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(
        bytes.len() <= FRAME_LIMIT as usize,
        "observation response exceeds budget"
    );
    pipe.write_u32(bytes.len() as u32).await?;
    pipe.write_all(&bytes).await?;
    pipe.flush().await?;
    Ok(())
}

struct Channel {
    pipe: NamedPipeServer,
    token: String,
    next: u64,
    server: Process,
    identity: Identity,
}

pub struct Observation(Option<Channel>);

impl Observation {
    pub async fn connect(mode: &str) -> anyhow::Result<Self> {
        let Some(token) = std::env::var_os(ENV_TOKEN) else {
            return Ok(Self(None));
        };
        let token = token
            .into_string()
            .map_err(|_| anyhow::anyhow!("observation token is not text"))?;
        let name = pipe_name(&token)?;
        // The ordinary probe owns the endpoint, as in the production helper path.
        let pipe = ServerOptions::new()
            .first_pipe_instance(true)
            .max_instances(1)
            .reject_remote_clients(true)
            .create(name)?;
        tokio::time::timeout(Duration::from_secs(30), pipe.connect())
            .await
            .context("independent observer did not connect")??;
        let server = pipe_process(&pipe, false)?;
        let identity = server.identity()?;
        let current = Process::open(std::process::id())?.identity()?;
        ensure!(
            identity.pid != current.pid
                && identity.image == current.image
                && identity.sha256 == current.sha256,
            "observer is not a separate matching probe executable"
        );
        ensure!(
            mode == "--plan" || identity.elevated,
            "runtime observer is not elevated"
        );
        let mut observed = Self(Some(Channel {
            pipe,
            token,
            next: 0,
            server,
            identity,
        }));
        observed.step("begin", 0, json!({"mode":mode})).await?;
        Ok(observed)
    }

    pub async fn step(&mut self, stage: &str, round: usize, data: Value) -> anyhow::Result<()> {
        let Some(channel) = self.0.as_mut() else {
            return Ok(());
        };
        ensure!(channel.server.alive()?, "observation server exited");
        let request = json!({"token":channel.token,"sequence":channel.next,
            "stage":stage,"round":round,"pid":std::process::id(),"data":data,
            // SAFETY: read-only current-thread compartment query.
            "compartment_id":unsafe { windows_sys::Win32::NetworkManagement::IpHelper::GetCurrentThreadCompartmentId() }});
        let reply = tokio::time::timeout(RPC_BUDGET, async {
            write_frame(&mut channel.pipe, &request).await?;
            read_frame(&mut channel.pipe).await
        })
        .await
        .context("observation exchange timed out")??;
        ensure!(
            reply["token"] == channel.token
                && reply["sequence"] == channel.next
                && reply["stage"] == stage,
            "observation response identity/order mismatch"
        );
        if stage == "begin" {
            let identity: Identity = serde_json::from_value(reply["observer_identity"].clone())?;
            ensure!(
                identity == channel.identity,
                "observer identity differs from its retained pipe process"
            );
        }
        println!(
            "WINDOWS_FULL_EXTERNAL_EVENT {}",
            json!({"request":request,"reply":reply})
        );
        ensure!(
            reply["ok"] == true,
            "independent observation failed: {}",
            reply["error"]
        );
        channel.next += 1;
        Ok(())
    }

    pub async fn abort(&mut self, error: &anyhow::Error) {
        if let Err(report) = self
            .step("abort", 0, json!({"error":format!("{error:#}")}))
            .await
        {
            eprintln!("WINDOWS_FULL_OBSERVER_ABORT_REPORT_FAILED {report:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_observation_endpoint_tokens_are_rejected() {
        for token in ["", "../pipe", "0123456789abcdef0123456789abcdeF", "0"] {
            assert!(pipe_name(token).is_err());
        }
        assert!(pipe_name("0123456789abcdef0123456789abcdef").is_ok());
    }

    #[tokio::test]
    async fn oversized_frames_are_rejected_before_allocating_a_body() -> anyhow::Result<()> {
        let (mut writer, mut reader) = tokio::io::duplex(16);
        writer.write_u32(FRAME_LIMIT + 1).await?;
        assert!(read_frame(&mut reader).await.is_err());
        Ok(())
    }
}
