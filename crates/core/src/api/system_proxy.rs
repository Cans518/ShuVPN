use crate::connection::model::VpnFailure;

pub fn system_proxy_backend() -> Result<String, VpnFailure> {
    crate::platform::system_proxy::backend_name()
}

pub fn system_proxy_uses_port(port: u16) -> Result<bool, VpnFailure> {
    crate::platform::system_proxy::uses_port(port)
}

/// Replays a previous interrupted system-proxy lease before new connections.
pub async fn recover_system_proxy(directory: String) -> Result<(), VpnFailure> {
    tokio::task::spawn_blocking(move || crate::platform::system_proxy::recover(&directory))
        .await
        .map_err(|_| VpnFailure::new("systemProxy", "系统代理恢复线程退出失败"))?
}
