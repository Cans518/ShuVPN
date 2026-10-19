//! Feature-gated probe of actual Windows sockets; never creates adapters or routes.
use super::*;
use crate::{
    egress_probe::WindowsProbe,
    routing::{model::Provider, providers::ProviderLoader},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) async fn verify(probe: WindowsProbe) -> anyhow::Result<String> {
    let adapter = windows::Interface::resolve(&probe.interface)?;
    let physical_binding = adapter.index.to_string();
    let native_dns = adapter.dns_servers()?;
    let mut tcp = connect_address(probe.tcp, None).await?;
    let udp = udp_socket(probe.udp, None).await?;
    let before: serde_json::Value = serde_json::from_str(&status()?)?;
    require(before["active"] == false, "unexpected active egress")?;
    let mut activation =
        Activation::prepare(probe.interface.clone(), Some(vec![probe.dns.ip()])).await?;
    Arc::get_mut(&mut activation.egress)
        .ok_or_else(failure)?
        .resolvers = vec![probe.dns];
    activation.publish().await?;
    let active: serde_json::Value = serde_json::from_str(&status()?)?;
    require(
        active["active"] == true
            && active["sockets"].as_array().is_some_and(|sockets| {
                sockets
                    .iter()
                    .all(|socket| socket["binding"].as_str() == Some(physical_binding.as_str()))
            }),
        "existing sockets were not bound to the actual physical interface",
    )?;
    // Uncommitted protection must restore existing sockets as well.
    activation.rollback().await?;
    drop(activation);
    let rolled_back: serde_json::Value = serde_json::from_str(&status()?)?;
    require(
        rolled_back["sockets"] == before["sockets"],
        "rollback changed original bindings",
    )?;
    let mut activation = Activation::prepare(probe.interface, Some(vec![probe.dns.ip()])).await?;
    Arc::get_mut(&mut activation.egress)
        .ok_or_else(failure)?
        .resolvers = vec![probe.dns];
    activation.publish().await?;
    let lease = activation.commit();
    tcp.write_all(b"tcp-old").await?;
    let mut bytes = [0; 7];
    tcp.read_exact(&mut bytes).await?;
    require(&bytes == b"tcp-old", "existing physical TCP bytes mismatch")?;
    udp.send(b"udp-old").await?;
    let mut bytes = [0; 32];
    let length = udp.recv(&mut bytes).await?;
    require(
        &bytes[..length] == b"udp-old",
        "existing physical UDP bytes mismatch",
    )?;
    let mut new_tcp = connect_address(probe.tcp, None).await?;
    new_tcp.write_all(b"tcp-new").await?;
    let mut bytes = [0; 7];
    new_tcp.read_exact(&mut bytes).await?;
    require(&bytes == b"tcp-new", "new physical TCP bytes mismatch")?;
    let new_udp = udp_socket(probe.udp, None).await?;
    new_udp.send(b"udp-new").await?;
    let mut bytes = [0; 32];
    let length = new_udp.recv(&mut bytes).await?;
    require(
        &bytes[..length] == b"udp-new",
        "new physical UDP bytes mismatch",
    )?;
    let answers = dns::resolve("direct.fixture.test").await?;
    let truncated = dns::resolve("truncated.fixture.test").await?;
    require(
        answers.contains(&probe.tcp.ip())
            && answers.contains(&"::1".parse::<IpAddr>()?)
            && answers == truncated,
        "physical dual-stack DNS or TCP retry mismatch",
    )?;
    require(
        dns::resolve("badid.fixture.test").await.is_err(),
        "bad DNS id accepted",
    )?;
    let provider = Provider {
        kind: "http".to_owned(),
        behavior: "domain".to_owned(),
        format: "yaml".to_owned(),
        payload: vec![],
        path: None,
        url: Some(probe.url),
        interval: None,
        data: None,
    };
    let ordinary =
        ProviderLoader::new(std::path::Path::new(&probe.directory)).map_err(routing_error)?;
    require(
        ordinary.load("probe", &provider).await.is_err(),
        "untrusted TLS certificate accepted",
    )?;
    let trusted = ProviderLoader::with_probe_certificate(
        std::path::Path::new(&probe.directory),
        &probe.certificate,
    )
    .map_err(routing_error)?;
    require(
        trusted
            .load("probe", &provider)
            .await
            .map_err(routing_error)?
            == [".physical.fixture.test"],
        "real provider path bypassed protected HTTPS connector",
    )?;
    let active: serde_json::Value = serde_json::from_str(&status()?)?;
    require(
        active["sockets"].as_array().is_some_and(|sockets| {
            sockets.iter().all(|socket| {
                socket["binding"].as_str() == Some(physical_binding.as_str())
                    || socket["peer"]
                        .as_str()
                        .is_some_and(|peer| peer.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()))
            })
        }),
        "new physical socket lost binding",
    )?;
    lease.close().await?;
    lease.close().await?;
    let restored: serde_json::Value = serde_json::from_str(&status()?)?;
    require(
        restored["active"] == false
            && restored["pending_cleanup"] == false
            && restored["sockets"]
                .as_array()
                .is_some_and(|sockets| sockets.iter().all(|socket| socket["binding"] == "0")),
        "original socket settings were not restored",
    )?;
    drop(new_tcp);
    drop(new_udp);
    Ok(serde_json::json!({"result":"WINDOWS_PHYSICAL_TCP_UDP_DNS_TLS_BINDINGS_AND_ROLLBACK_PASSED",
        "physicalIndex":adapter.index,"nativeDnsCount":native_dns.len(),"before":before,"active":active,"restored":restored}).to_string())
}

fn require(condition: bool, message: &str) -> Result<(), VpnFailure> {
    if condition {
        Ok(())
    } else {
        Err(VpnFailure::new("probe", message))
    }
}
fn routing_error(error: crate::routing::model::RoutingFailure) -> VpnFailure {
    VpnFailure::new(&error.code, &error.message)
}
