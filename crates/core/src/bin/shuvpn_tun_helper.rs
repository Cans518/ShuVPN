#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(shuvpn_core::tun_helper::run(
        std::env::args().skip(1).collect(),
    ));
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
    result
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Desktop network helper is unavailable on this platform")
}
