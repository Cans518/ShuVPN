use anyhow::{ensure, Context};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::net::IpAddr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(super) const VERSION: u32 = 1;
const MAX_FRAME: usize = 256 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub id: u64,
    pub command: Command,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub(super) enum Command {
    Hello { version: u32, nonce: String },
    Create { plan: String },
    Info {},
    Read { wait_ms: u32 },
    Write { packet: Vec<u8> },
    Routes { hosts: Vec<IpAddr> },
    Close {},
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Response {
    pub id: u64,
    pub result: Reply,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", deny_unknown_fields)]
pub(super) enum Reply {
    Ready { version: u32, elevated: bool },
    Info { document: String },
    Packet { packet: Option<Vec<u8>> },
    Done {},
    Failure { code: String, message: String },
}

pub(super) async fn receive<T: DeserializeOwned>(
    stream: &mut (impl AsyncRead + Unpin),
) -> anyhow::Result<Option<T>> {
    let mut length = [0; 4];
    if stream.read(&mut length[..1]).await? == 0 {
        return Ok(None);
    }
    stream.read_exact(&mut length[1..]).await?;
    let length = u32::from_be_bytes(length) as usize;
    ensure!(
        (1..=MAX_FRAME).contains(&length),
        "invalid helper frame size"
    );
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await?;
    Ok(Some(
        serde_json::from_slice(&bytes).context("invalid helper frame")?,
    ))
}

pub(super) async fn send<T: Serialize>(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &T,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(
        (1..=MAX_FRAME).contains(&bytes.len()),
        "helper response too large"
    );
    stream.write_u32(bytes.len() as u32).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn framing_rejects_oversize_truncation_unknown_operations_and_fields(
    ) -> anyhow::Result<()> {
        for length in [0, MAX_FRAME as u32 + 1, u32::MAX] {
            let bytes = length.to_be_bytes();
            assert!(receive::<Request>(&mut bytes.as_slice()).await.is_err());
        }
        for payload in [
            r#"{"id":0,"command":{"op":"Shell","path":"cmd.exe"}}"#,
            r#"{"id":0,"command":{"op":"Close","path":"ignored"}}"#,
            r#"{"id":0,"command":{"op":"Close"},"extra":true}"#,
        ] {
            let mut bytes = vec![];
            bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            bytes.extend_from_slice(payload.as_bytes());
            assert!(
                receive::<Request>(&mut bytes.as_slice()).await.is_err(),
                "{payload}"
            );
        }
        let mut bytes = vec![];
        send(
            &mut bytes,
            &Request {
                id: 4,
                command: Command::Read { wait_ms: 500 },
            },
        )
        .await?;
        assert!(receive::<Request>(&mut bytes[..bytes.len() - 1].as_ref())
            .await
            .is_err());
        let request = receive::<Request>(&mut bytes.as_slice())
            .await?
            .context("missing request")?;
        assert_eq!(request.id, 4);
        assert!(matches!(request.command, Command::Read { wait_ms: 500 }));
        assert!(receive::<Request>(&mut [].as_slice()).await?.is_none());
        Ok(())
    }
}
