#![cfg(any(target_os = "windows", target_os = "linux"))]

use anyhow::Context;
use shuvpn_core::api::vpn::VpnSession;

#[allow(dead_code)]
#[path = "support/gateway.rs"]
mod gateway;

#[tokio::test]
async fn desktop_plan_comes_from_session_resources_and_contains_no_tunnel_credentials(
) -> anyhow::Result<()> {
    let gateway =
        gateway::Gateway::start(gateway::Mode::MixedDomainDns(gateway::DnsMode::Valid)).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let document = session
        .desktop_tun_plan(1500, Some("1.1.1.1".into()), None, None)
        .await?;
    let plan: serde_json::Value = serde_json::from_str(&document)?;
    assert_eq!(plan["mtu"], 1400);
    assert_eq!(plan["physical_dns_servers"], serde_json::json!(["1.1.1.1"]));
    assert!(plan["addresses"]
        .as_array()
        .context("addresses")?
        .iter()
        .any(|address| address == "10.99.0.2/32"));
    assert!(plan["protected_ips"]
        .as_array()
        .context("protected")?
        .iter()
        .any(|address| address == "127.0.0.1"));
    for secret in ["sid", "sign_key", "signKey", "cookie", "nodes"] {
        assert!(
            plan.get(secret).is_none(),
            "secret field entered preview: {secret}"
        );
    }
    let failure = session
        .start_desktop_tun(1400, "\0".into(), None, None)
        .await
        .err()
        .context("unknown interface accepted")?;
    assert_eq!(failure.code, "interface");
    let status: serde_json::Value = serde_json::from_str(&session.tun_status().await?)?;
    assert_eq!(status["state"], "stopped");
    assert_eq!(status["cleanup_pending"], false);
    session.stop().await?;
    assert!(session.finish_stop().await?.is_none());
    assert_eq!(
        session
            .desktop_tun_plan(1400, None, None, None)
            .await
            .err()
            .context("closed plan accepted")?
            .code,
        "closed"
    );
    assert_eq!(
        session
            .start_desktop_tun(1400, "\0".into(), None, None)
            .await
            .err()
            .context("closed session accepted desktop start")?
            .code,
        "closed"
    );
    Ok(())
}

#[tokio::test]
async fn live_desktop_interface_enumeration_is_bounded_and_never_recommends_two_adapters(
) -> anyhow::Result<()> {
    let choices = shuvpn_core::api::desktop::desktop_network_interfaces().await?;
    assert!(choices.len() <= 256);
    assert!(choices
        .iter()
        .all(|choice| choice.index != 0 && !choice.name.is_empty()));
    assert!(choices.iter().filter(|choice| choice.recommended).count() <= 1);
    assert!(choices.windows(2).all(|pair| pair[0].name <= pair[1].name));
    Ok(())
}

#[tokio::test]
async fn desktop_tun_validates_interface_without_elevation_or_network_mutation(
) -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let session = VpnSession::connect(gateway.plan(), 1000).await?;
    let failure = session
        .start_tun("{}".to_owned(), String::new())
        .await
        .err()
        .context("empty physical interface was accepted")?;
    assert_eq!(failure.code, "interface");
    session.stop_tun().await?;
    session.stop_tun().await?;
    let proxy = session
        .start_proxy(
            shuvpn_core::api::vpn::ProxyKind::Http,
            "127.0.0.1".to_owned(),
            0,
        )
        .await?;
    assert_ne!(proxy.port, 0);
    session.stop().await?;
    Ok(())
}
