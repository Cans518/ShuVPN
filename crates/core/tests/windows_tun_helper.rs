#![cfg(target_os = "windows")]

use anyhow::{ensure, Context};
use shuvpn_core::tun_helper::Endpoint;
use std::{
    os::windows::process::CommandExt,
    process::{Child, Command},
    time::Duration,
};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn spawn(endpoint: &Endpoint) -> anyhow::Result<ChildGuard> {
    spawn_arguments(endpoint.arguments())
}

fn spawn_arguments(arguments: [String; 3]) -> anyhow::Result<ChildGuard> {
    let path = std::env::var_os("SHUVPN_TEST_TUN_HELPER")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_shuvpn_tun_helper").into());
    Ok(ChildGuard(
        Command::new(path)
            .args(arguments)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?,
    ))
}

#[tokio::test]
async fn real_helper_process_handshake_validation_permission_and_repeated_close(
) -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let mut child = spawn(&endpoint)?;
    let client =
        tokio::time::timeout(Duration::from_secs(5), endpoint.accept(child.0.id())).await??;
    let invalid = client
        .create("{}".to_owned())
        .await
        .err()
        .context("invalid plan accepted")?;
    ensure!(invalid.code == "plan", "wrong invalid-plan failure");
    if !client.elevated() {
        let plan = serde_json::json!({
            "name":"shuvpnipc", "mtu":1400, "addresses":["192.0.2.2/32"],
            "routes":["192.0.2.3/32"], "protected_ips":["127.0.0.1"],
        })
        .to_string();
        for _ in 0..2 {
            let denied = client
                .create(plan.clone())
                .await
                .err()
                .context("unprivileged create accepted")?;
            ensure!(
                denied.code == "permission",
                "wrong create failure: {denied}"
            );
        }
    }
    ensure!(
        client
            .info()
            .await
            .err()
            .is_some_and(|failure| failure.code == "state"),
        "helper unexpectedly owns TUN"
    );
    ensure!(
        client
            .write(vec![0; 9001])
            .await
            .err()
            .is_some_and(|failure| failure.code == "packet"),
        "oversized packet accepted"
    );
    ensure!(
        client
            .sync_routes(vec!["192.0.2.3".parse()?; 1025])
            .await
            .err()
            .is_some_and(|failure| failure.code == "size"),
        "oversized routes accepted"
    );
    client.close().await?;
    client.close().await?;
    ensure!(child.0.wait()?.success(), "helper exited with error");
    Ok(())
}

#[tokio::test]
async fn stale_parent_generation_rejects_real_helper_before_handshake() -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let mut arguments = endpoint.arguments();
    arguments[2] = (arguments[2].parse::<u64>()? + 1).to_string();
    let mut child = spawn_arguments(arguments)?;
    let result =
        tokio::time::timeout(Duration::from_secs(5), endpoint.accept(child.0.id())).await?;
    ensure!(result.is_err(), "stale parent generation accepted");
    ensure!(
        !child.0.wait()?.success(),
        "invalid helper startup exited successfully"
    );
    Ok(())
}

#[tokio::test]
async fn disconnecting_real_client_finishes_helper_without_an_explicit_close() -> anyhow::Result<()>
{
    let endpoint = Endpoint::new()?;
    let mut child = spawn(&endpoint)?;
    let client =
        tokio::time::timeout(Duration::from_secs(5), endpoint.accept(child.0.id())).await??;
    drop(client);
    let status = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = child.0.try_wait()? {
                return Ok::<_, anyhow::Error>(status);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    ensure!(
        status.success(),
        "helper did not finish cleanly on disconnect"
    );
    Ok(())
}

#[tokio::test]
async fn invalidated_pipe_can_confirm_the_real_helpers_successful_cleanup_exit(
) -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let mut child = spawn(&endpoint)?;
    let client =
        tokio::time::timeout(Duration::from_secs(5), endpoint.accept(child.0.id())).await??;
    // The wire rejects this frame locally, invalidating the pipe before a
    // request is sent. The real helper observes EOF and performs final cleanup.
    ensure!(
        client.write(vec![0; 256 * 1024]).await.is_err(),
        "oversized wire frame was accepted"
    );
    tokio::time::timeout(Duration::from_secs(6), client.close()).await??;
    ensure!(
        child.0.wait()?.success(),
        "helper cleanup exit was not successful"
    );
    client.close().await?;
    Ok(())
}
