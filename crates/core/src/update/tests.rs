use super::*;
use anyhow::{ensure, Context};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};

async fn serve(
    status: &str,
    body: Vec<u8>,
) -> anyhow::Result<(Client, tokio::task::JoinHandle<anyhow::Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = Url::parse(&format!("http://{}/latest", listener.local_addr()?))?;
    let status = status.to_owned();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            ensure!(request.len() < 16 * 1024, "request too large");
            request.push(stream.read_u8().await?);
        }
        let request = String::from_utf8(request)?.to_lowercase();
        ensure!(
            request.starts_with("get /latest ")
                && request.contains("user-agent: shuvpn")
                && request.contains("accept: application/vnd.github+json")
                && request.contains("x-github-api-version: 2022-11-28"),
            "GitHub headers not preserved"
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .await?;
        // Rejection paths intentionally drop a response without reading its body.
        let _ = stream.write_all(&body).await;
        Ok(())
    });
    Ok((
        Client {
            endpoint,
            cancelled: CancellationToken::new(),
        },
        task,
    ))
}

#[tokio::test]
async fn actual_http_release_and_status_response_boundaries() -> anyhow::Result<()> {
    let body = br#"{"tag_name":"v0.4.0","assets":[{"name":"app-windows-x64.zip.sha256","browser_download_url":"checksum"},{"name":"app-windows-x64.zip","browser_download_url":"https://github.com/example/app-windows-x64.zip"},{"name":"app-linux-x64.tar.gz","browser_download_url":"https://github.com/example/app-linux-x64.tar.gz"}],"html_url":"https://github.com/example/release","published_at":"2026-10-03T19:12:25Z"}"#;
    let (client, task) = serve("200 OK", body.to_vec()).await?;
    let info = client.check("0.3.0".into()).await?.context("no update")?;
    ensure!(
        info.latest_version == "0.4.0"
            && info
                .target_url
                .contains(&format!("app-{}-x64", std::env::consts::OS)),
        "update result wrong"
    );
    task.await??;
    for (status, body, expected) in [
        ("404 Missing", Vec::new(), None),
        ("403 Forbidden", Vec::new(), Some("http")),
        ("302 Found", Vec::new(), Some("http")),
        ("200 OK", b"[]".to_vec(), Some("response")),
        ("200 OK", b"broken".to_vec(), Some("response")),
        ("200 OK", vec![b'x'; 1024 * 1024 + 1], Some("response")),
    ] {
        let (client, task) = serve(status, body).await?;
        let result = client.check("0.3.0".into()).await;
        if let Some(expected) = expected {
            ensure!(
                result.err().is_some_and(|error| error.code == expected),
                "invalid response accepted"
            );
        } else {
            ensure!(result?.is_none(), "404 not treated as absent release");
        }
        task.await??;
    }
    Ok(())
}

#[tokio::test]
async fn closing_channel_cancels_inflight_and_prevents_new_requests() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let entered = Arc::new(Notify::new());
    let address = listener.local_addr()?;
    let signal = entered.clone();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await?);
        }
        signal.notify_one();
        let mut byte = [0; 1];
        ensure!(
            stream.read(&mut byte).await? == 0,
            "cancel did not close connection"
        );
        Ok::<_, anyhow::Error>(())
    });
    let client = Arc::new(Client {
        endpoint: Url::parse(&format!("http://{address}/latest"))?,
        cancelled: CancellationToken::new(),
    });
    let caller = client.clone();
    let request = tokio::spawn(async move { caller.check("0.3.0".into()).await });
    tokio::time::timeout(Duration::from_secs(3), entered.notified()).await?;
    client.close();
    ensure!(
        request
            .await?
            .err()
            .is_some_and(|error| error.code == "closed"),
        "late update survived close"
    );
    ensure!(
        client
            .check("0.3.0".into())
            .await
            .err()
            .is_some_and(|error| error.code == "closed"),
        "closed channel reopened"
    );
    task.await??;
    Ok(())
}
