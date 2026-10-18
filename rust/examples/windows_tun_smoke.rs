#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    anyhow::ensure!(
        std::env::args().len() <= 2
            && ["", "--helper", "--helper-disconnect", "--helper-inherit"].contains(&mode.as_str()),
        "expected no arguments, --helper, --helper-disconnect or --helper-inherit"
    );
    let budget = if mode.starts_with("--helper") {
        180
    } else {
        60
    };
    let (done, watch) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if matches!(
            watch.recv_timeout(std::time::Duration::from_secs(budget)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Windows TUN probe watchdog expired; kernel cleanup unproven");
            std::process::exit(124);
        }
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        if mode.is_empty() {
            return shuvpn_core::verify_windows_tun().await;
        }
        let endpoint = shuvpn_core::tun_helper::Endpoint::new()?;
        let client = if mode == "--helper-inherit" {
            use std::os::windows::process::CommandExt;
            let helper = std::env::current_exe()?
                .parent()
                .ok_or_else(|| anyhow::anyhow!("missing probe directory"))?
                .join("shuvpn_tun_helper.exe");
            let child = std::process::Command::new(helper)
                .args(endpoint.arguments())
                .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
                .spawn()?;
            endpoint.accept(child.id()).await?
        } else {
            let driver: serde_json::Value =
                serde_json::from_str(&shuvpn_core::api::tun::tun_driver_info().await?)?;
            anyhow::ensure!(
                driver["elevated"] == false,
                "run UAC helper modes from a non-administrator terminal"
            );
            eprintln!("Requesting Windows UAC consent for the packaged TUN helper");
            endpoint.launch().await?
        };
        if mode == "--helper-disconnect" {
            shuvpn_core::verify_windows_tun_helper_disconnect(client).await
        } else {
            shuvpn_core::verify_windows_tun_helper(client).await
        }
    });
    // A stuck blocking reader must not stall runtime destruction after the
    // cleanup timeout. The watchdog remains armed through this shutdown.
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
    done.send(())?;
    watchdog
        .join()
        .map_err(|_| anyhow::anyhow!("probe watchdog panicked"))?;
    println!("{}", result?);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Windows administrator process required")
}
