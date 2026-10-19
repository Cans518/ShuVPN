use anyhow::{ensure, Context};
use shuvpn_core::api::{
    desktop_connection::{ConnectionStartup, OwnerPhase},
    vpn::ProxyStartup,
};
use shuvpn_desktop::host::DesktopHost;
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Duration},
};

// Reuse the protocol fixture, never a school endpoint or a real account.
#[allow(dead_code)]
#[path = "../../../crates/core/tests/support/gateway.rs"]
mod gateway;

async fn echo_http(port: u16) -> anyhow::Result<()> {
    let mut client = TcpStream::connect(("127.0.0.1", port)).await?;
    client
        .write_all(
            b"CONNECT echo.internal.invalid:443 HTTP/1.1\r\nHost: echo.internal.invalid\r\n\r\n",
        )
        .await?;
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        header.push(client.read_u8().await?);
        ensure!(header.len() < 4096, "HTTP response header exceeded budget");
    }
    ensure!(header.starts_with(b"HTTP/1.1 200"), "HTTP tunnel rejected");
    let mut prelude = [0; 7];
    client.read_exact(&mut prelude).await?;
    ensure!(&prelude == b"prelude", "unexpected gateway response");
    let payload = vec![0x5a; 64 * 1024];
    client.write_all(&payload).await?;
    let mut response = vec![0; payload.len()];
    client.read_exact(&mut response).await?;
    ensure!(response == payload, "HTTP payload changed");
    client.shutdown().await?;
    let mut trailing = Vec::new();
    client.read_to_end(&mut trailing).await?;
    ensure!(trailing.is_empty(), "unexpected bytes after HTTP echo");
    Ok(())
}

async fn echo_socks(port: u16) -> anyhow::Result<()> {
    let mut client = TcpStream::connect(("127.0.0.1", port)).await?;
    client.write_all(&[5, 1, 0]).await?;
    let mut method = [0; 2];
    client.read_exact(&mut method).await?;
    ensure!(method == [5, 0], "SOCKS negotiation failed");
    let host = b"socks.internal.invalid";
    let request = [
        vec![5, 1, 0, 3, host.len() as u8],
        host.to_vec(),
        vec![1, 0xbb],
    ]
    .concat();
    client.write_all(&request).await?;
    let mut response = [0; 10];
    client.read_exact(&mut response).await?;
    ensure!(response[1] == 0, "SOCKS tunnel rejected");
    let mut prelude = [0; 7];
    client.read_exact(&mut prelude).await?;
    ensure!(&prelude == b"prelude", "unexpected SOCKS gateway response");
    client.write_all(b"tool-roundtrip").await?;
    let mut echoed = [0; 14];
    client.read_exact(&mut echoed).await?;
    ensure!(&echoed == b"tool-roundtrip", "SOCKS payload changed");
    client.shutdown().await?;
    let mut trailing = Vec::new();
    client.read_to_end(&mut trailing).await?;
    ensure!(trailing.is_empty(), "unexpected bytes after SOCKS echo");
    Ok(())
}

fn verify_closed_ports(http: u16, socks: u16) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A closed Windows loopback port can time out under local filtering.
        // Query the actual listener table instead of treating a timeout as proof.
        let script = format!(
            "$rows = @(Get-NetTCPConnection -State Listen -ErrorAction Stop); \
             @($rows | Where-Object {{ $_.OwningProcess -eq {} -and $_.LocalPort -in @({http},{socks}) }}).Count",
            std::process::id()
        );
        let powershell = std::path::PathBuf::from(
            std::env::var_os("SystemRoot").context("SystemRoot is missing")?,
        )
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let output = std::process::Command::new(powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(0x08000000)
            .output()?;
        ensure!(
            output.status.success(),
            "listener table query failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        ensure!(
            String::from_utf8_lossy(&output.stdout).trim() == "0",
            "a proxy listener survived cleanup"
        );
    }
    #[cfg(not(windows))]
    {
        for port in [http, socks] {
            let listener = std::net::TcpListener::bind(("127.0.0.1", port))
                .context("proxy port could not be rebound after cleanup")?;
            drop(listener);
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_transfers_local_protocol_data_reconnects_and_drains_on_exit() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let host = DesktopHost::open(directory.path()).await?;
    let mut gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    for round in 0..2 {
        let permit = host.enter().await?;
        let session = Arc::new(
            host.owner
                .connect_plan(
                    gateway.plan(),
                    3000,
                    ConnectionStartup {
                        proxies: ProxyStartup {
                            http_enabled: true,
                            http_address: "127.0.0.1".into(),
                            socks_enabled: true,
                            socks_address: "127.0.0.1".into(),
                            ..Default::default()
                        },
                        tun: None,
                        recovery_directory: None,
                    },
                )
                .await?,
        );
        let services = session
            .startup_services()
            .context("missing service report")?;
        ensure!(services.failures.is_empty(), "local proxy startup failed");
        let http = services.proxies.http.context("missing HTTP binding")?;
        let socks = services.proxies.socks.context("missing SOCKS binding")?;
        *host.session.write().await = Some(session.clone());
        timeout(Duration::from_secs(5), echo_http(http.port)).await??;
        timeout(Duration::from_secs(5), echo_socks(socks.port)).await??;
        let snapshot = host.snapshot().await?;
        ensure!(snapshot["connection"]["phase"] == "connected");
        ensure!(
            snapshot["account"]["signedIn"] == false,
            "fixture must not sign in"
        );
        ensure!(
            snapshot["connection"]["network"]["httpEndpoint"] == http.display_endpoint,
            "UI must receive the actual native binding"
        );
        ensure!(host.require_disconnected().await.is_err());
        let refused = host.connect(String::new(), None).await;
        ensure!(
            refused.is_err_and(|error| error.code == "connected"),
            "a duplicate connect must be rejected before account preparation"
        );
        ensure!(
            host.owner.snapshot()?.run_id == Some(session.run_id()),
            "a duplicate request replaced the running session"
        );
        let decisions: serde_json::Value = serde_json::from_str(&session.decisions()?)?;
        let decisions = decisions.as_array().context("missing decision records")?;
        ensure!(
            decisions.len() >= 2,
            "real proxy decisions were not recorded"
        );
        ensure!(decisions
            .iter()
            .all(|record| record["outbound"] == "SHU-VPN"));
        ensure!(session.snapshot().upload_bytes >= 64 * 1024 + 14);
        if round == 0 {
            host.disconnect().await?;
            ensure!(host.owner.snapshot()?.phase == OwnerPhase::Disconnected);
            ensure!(host.session.read().await.is_none());
        } else {
            drop(permit);
            host.shutdown().await?;
            ensure!(host.owner.snapshot()?.closed);
        }
        verify_closed_ports(http.port, socks.port)?;
    }
    gateway.finish().await?;
    let errors = gateway.errors.lock().await;
    ensure!(
        errors.is_empty(),
        "fixture reported transport errors: {errors:?}"
    );
    Ok(())
}
