use crate::connection::model::VpnFailure;
use crate::network::{self, Tcp, Udp};
use crate::routing::model::Flow;
use std::net::SocketAddr;

pub(crate) use crate::network::interface::validate as validate_interface;

pub(crate) async fn udp_socket(
    address: SocketAddr,
    interface: Option<&str>,
) -> Result<Udp, VpnFailure> {
    network::udp_socket(address, interface).await
}
pub(crate) async fn tcp_socket(flow: &Flow, interface: Option<&str>) -> Result<Tcp, VpnFailure> {
    validate_interface(interface)?;
    if flow.ips.is_empty() {
        if interface.is_some() {
            return Err(VpnFailure::new(
                "destination",
                "指定物理出口需要已解析的目标 IP",
            ));
        }
        return network::connect_host(&flow.host, flow.port).await;
    }
    let mut last = None;
    for address in &flow.ips {
        match network::connect_address(SocketAddr::new(*address, flow.port), interface).await {
            Ok(socket) => return Ok(socket),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| VpnFailure::new("destination", "物理出口没有可用的目标 IP")))
}
