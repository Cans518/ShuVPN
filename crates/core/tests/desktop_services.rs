use anyhow::{ensure, Context};
use shuvpn_core::api::vpn::{ProxyStartup, TunStartup, VpnSession};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;

fn proxies() -> ProxyStartup {
    ProxyStartup {
        http_enabled: true,
        http_address: "127.0.0.1".into(),
        http_port: 0,
        socks_enabled: true,
        socks_address: "127.0.0.1".into(),
        socks_port: 0,
        system_proxy_enabled: false,
    }
}

async fn socks_reply(port: u16) -> anyhow::Result<()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
    stream.write_all(&[5, 1, 0]).await?;
    let mut reply = [0; 2];
    timeout(Duration::from_secs(2), stream.read_exact(&mut reply)).await??;
    ensure!(reply == [5, 0]);
    Ok(())
}

#[tokio::test]
async fn native_startup_preserves_independent_success_and_reads_actual_ports() -> anyhow::Result<()>
{
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let occupied = TcpListener::bind("127.0.0.1:0").await?;
    let mut request = proxies();
    request.http_port = occupied.local_addr()?.port();
    request.system_proxy_enabled = true;
    let report = session.start_desktop_services(request, None, None).await?;
    ensure!(report.failures.len() == 1 && report.failures[0].service == "systemProxy");
    let http = report.proxies.http.context("HTTP listener was lost")?;
    let socks = report.proxies.socks.context("SOCKS listener was lost")?;
    ensure!(http.port != occupied.local_addr()?.port());
    ensure!(!report.proxies.system_proxy_enabled);
    ensure!(report.tun.is_none());
    socks_reply(socks.port).await?;
    let existing = session
        .start_desktop_services(ProxyStartup::default(), None, None)
        .await?;
    ensure!(existing.proxies.http.context("existing HTTP missing")?.port == http.port);
    ensure!(
        existing
            .proxies
            .socks
            .context("existing SOCKS missing")?
            .port
            == socks.port
    );
    session.finish_stop().await?;
    ensure!(TcpStream::connect(("127.0.0.1", http.port)).await.is_err());
    ensure!(TcpStream::connect(("127.0.0.1", socks.port)).await.is_err());
    Ok(())
}

#[tokio::test]
async fn failed_http_prevents_system_mutation_but_does_not_skip_socks() -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let mut request = proxies();
    request.http_enabled = false;
    request.http_address = "not-an-ip".into();
    request.system_proxy_enabled = true;
    let report = session
        .start_desktop_services(request, None, Some("\0".into()))
        .await?;
    ensure!(report.proxies.http.is_none());
    ensure!(report.failures.len() == 1 && report.failures[0].service == "http");
    ensure!(report.failures[0].code == "listen");
    ensure!(!report.proxies.system_proxy_enabled);
    socks_reply(report.proxies.socks.context("SOCKS was skipped")?.port).await?;
    let recovered = session
        .start_desktop_services(proxies(), None, None)
        .await?;
    ensure!(recovered.failures.is_empty());
    ensure!(recovered.proxies.http.is_some() && recovered.proxies.socks.is_some());
    session.finish_stop().await?;
    Ok(())
}

#[tokio::test]
async fn disabled_or_failed_tun_keeps_working_proxy_services_and_reports_cleanup(
) -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let tun = |enabled| TunStartup {
        enabled,
        mtu: 1400,
        physical_interface: "\0".into(),
        physical_dns: None,
        capture_mode: None,
    };
    let disabled = session
        .start_desktop_services(proxies(), Some(tun(false)), None)
        .await?;
    ensure!(disabled.tun.is_none());
    let report = session
        .start_desktop_services(proxies(), Some(tun(true)), None)
        .await?;
    let outcome = report.tun.context("missing TUN outcome")?;
    ensure!(outcome.diagnostic.is_some());
    let state: serde_json::Value = serde_json::from_str(&outcome.status_json)?;
    ensure!(state["state"] == "stopped" && state["cleanup_pending"] == false);
    socks_reply(
        report
            .proxies
            .socks
            .context("SOCKS lost after TUN failure")?
            .port,
    )
    .await?;
    let stopped = session.stop_tun_service().await?;
    ensure!(stopped.diagnostic.is_none());
    session.finish_stop().await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_startup_commands_reuse_owned_listeners_and_closed_sessions_reject_startup(
) -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let (left, right) = tokio::join!(
        session.start_desktop_services(proxies(), None, None),
        session.start_desktop_services(proxies(), None, None),
    );
    let (left, right) = (left?, right?);
    ensure!(
        left.proxies.http.context("HTTP missing")?.port
            == right.proxies.http.context("HTTP missing")?.port
    );
    ensure!(
        left.proxies.socks.context("SOCKS missing")?.port
            == right.proxies.socks.context("SOCKS missing")?.port
    );
    session.finish_stop().await?;
    ensure!(session
        .start_desktop_services(proxies(), None, None)
        .await
        .is_err());
    Ok(())
}

#[tokio::test]
async fn native_rebind_replaces_only_the_requested_listener_and_never_starts_a_stopped_one(
) -> anyhow::Result<()> {
    use shuvpn_core::api::vpn::ProxyKind;
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 2000).await?;
    let initial = session
        .start_desktop_services(proxies(), None, None)
        .await?;
    let old_http = initial.proxies.http.context("HTTP missing")?;
    let socks = initial.proxies.socks.context("SOCKS missing")?;
    let mut client = TcpStream::connect(("127.0.0.1", old_http.port)).await?;
    let occupied = TcpListener::bind("[::1]:0").await?;
    let requested = occupied.local_addr()?.port();
    let changed = session
        .rebind_proxy(ProxyKind::Http, "::1".into(), requested, None, false)
        .await?;
    ensure!(changed.failures.is_empty());
    let http = changed.proxies.http.context("rebound HTTP missing")?;
    ensure!(http.address == "::1" && http.port != requested);
    ensure!(changed.proxies.socks.context("SOCKS lost")?.port == socks.port);
    let mut byte = [0];
    let ended = timeout(Duration::from_secs(2), client.read(&mut byte)).await?;
    ensure!(
        matches!(ended, Ok(0) | Err(_)),
        "old client survived rebind"
    );
    drop(TcpStream::connect(("::1", http.port)).await?);
    socks_reply(socks.port).await?;
    session.stop_proxy(ProxyKind::Http).await?;
    let stopped = session
        .rebind_proxy(ProxyKind::Http, "127.0.0.1".into(), 0, None, false)
        .await?;
    ensure!(stopped.proxies.http.is_none() && stopped.proxies.socks.is_some());
    session.finish_stop().await?;
    Ok(())
}
