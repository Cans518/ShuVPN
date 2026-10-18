use crate::connection::model::VpnFailure;
pub use crate::network::interface::NetworkInterface;

/// Read-only interface enumeration. Recommendation uses current system defaults.
pub async fn desktop_network_interfaces() -> Result<Vec<NetworkInterface>, VpnFailure> {
    crate::network::interface::available().await
}

#[flutter_rust_bridge::frb(sync)]
pub fn validate_desktop_dns(value: String) -> Result<String, VpnFailure> {
    Ok(crate::network::dns::explicit_upstream(Some(&value))?
        .and_then(|servers| servers.first().map(ToString::to_string))
        .unwrap_or_default())
}
