#[allow(dead_code)]
#[path = "../tests/support/gateway.rs"]
mod gateway;

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use shuvpn_core::api::vpn::VpnSession;
    use std::process::Command;
    anyhow::ensure!(
        std::fs::read_link("/proc/self/ns/net")? != std::fs::read_link("/proc/1/ns/net")?,
        "isolated namespace required"
    );
    let arguments: Vec<_> = std::env::args().collect();
    anyhow::ensure!(
        arguments.len() == 3,
        "Expected HTTPS port and public DER certificate path"
    );
    let https_port: u16 = arguments[1].parse()?;
    let certificate = std::fs::read(&arguments[2])?;
    let gateway = gateway::Gateway::start(gateway::Mode::L3Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let before: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
    anyhow::ensure!(before["active"] == false, "unexpected active egress");
    let plan = serde_json::json!({
        "name":"shuvpnegress","mtu":1400,
        "addresses":["10.99.0.2/32","2001:db8::2/128"],
        "routes":["10.22.0.0/16","2001:db8:22::/64","10.99.0.53/32"],
        "protected_ips":["127.0.0.1","192.0.2.2"],
        "physical_dns_servers":["10.99.0.53"],
    })
    .to_string();
    anyhow::ensure!(
        Command::new("sysctl")
            .args(["-q", "-w", "net.ipv6.conf.default.disable_ipv6=1"])
            .status()?
            .success(),
        "test setup failed"
    );
    let failed = session.start_tun(plan.clone(), "shuphys".to_owned()).await;
    anyhow::ensure!(failed.is_err(), "partial setup unexpectedly succeeded");
    let restored: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
    anyhow::ensure!(
        restored["active"] == false && restored["pending_cleanup"] == false,
        "failed TUN retained egress"
    );
    anyhow::ensure!(
        restored["sockets"] == before["sockets"],
        "preexisting socket binding was not restored"
    );
    anyhow::ensure!(
        Command::new("sysctl")
            .args(["-q", "-w", "net.ipv6.conf.default.disable_ipv6=0"])
            .status()?
            .success(),
        "test restore failed"
    );
    session.start_tun(plan, "shuphys".to_owned()).await?;
    let active: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
    anyhow::ensure!(
        active["active"] == true && active["interface"] == "shuphys",
        "physical policy missing"
    );
    anyhow::ensure!(
        active["sockets"].as_array().is_some_and(|sockets| sockets
            .iter()
            .any(|socket| socket["peer"] == "127.0.0.1" && socket["binding"] == "lo")),
        "existing control socket was not protected"
    );
    let directory = tempfile::tempdir()?;
    shuvpn_core::egress_probe::verify(
        "direct.fixture.test".to_owned(),
        format!("https://provider.fixture.test:{https_port}/rules"),
        certificate,
        directory.path().to_string_lossy().into_owned(),
    )
    .await?;
    session.stop_tun().await?;
    let restored: serde_json::Value = serde_json::from_str(&session.egress_status()?)?;
    anyhow::ensure!(
        restored["active"] == false && restored["pending_cleanup"] == false,
        "egress lease remained active"
    );
    anyhow::ensure!(
        restored["sockets"] == before["sockets"],
        "control binding changed after stop_tun"
    );
    session.stop().await?;
    println!("TUN_PHYSICAL_DNS_TCP_FALLBACK_HTTPS_AND_BINDING_ROLLBACK_PASSED");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Linux namespace required");
    std::process::exit(1);
}
