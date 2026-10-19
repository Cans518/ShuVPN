pub(crate) mod capture;
#[cfg(target_os = "linux")]
pub(crate) mod linux_tun;
pub(crate) mod model;
pub(crate) mod process;
pub(crate) mod system_proxy;
#[cfg(target_os = "windows")]
pub(crate) mod windows_tun;

pub(crate) async fn tun_driver_info() -> Result<String, model::PlatformFailure> {
    #[cfg(target_os = "windows")]
    {
        windows_tun::driver_info()
    }
    #[cfg(target_os = "linux")]
    {
        let elevated = linux_tun::privileges_ready()?;
        let helper = if elevated {
            Ok(())
        } else {
            linux_tun::helper::Client::installed_ready().await
        };
        let available = helper.is_ok();
        let unavailable_reason = helper.err().map(|error| format!(
            "VPN 接管权限尚未就绪，请安装桌面包并确认当前登录会话属于 shuvpn 组；当前可使用本机代理。{error}"
        ));
        Ok(serde_json::json!({
            "backend":"native-linux", "wintun":false,
            "elevated":elevated, "available":available,
            "helperReady":available && !elevated,
            "sessionDataPlaneReady":true, "permissionPrompt":false,
            "captureMode":"campus-with-domain-routes", "fullCaptureReady":false,
            "unavailableReason":unavailable_reason,
        })
        .to_string())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err(model::PlatformFailure::new(
            "platform",
            "当前平台没有 TUN 驱动",
        ))
    }
}

#[cfg(target_os = "linux")]
use linux_tun::Device as NativeTunDevice;
#[cfg(target_os = "windows")]
use windows_tun::Device as NativeTunDevice;

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl crate::connection::capture::RouteCapture for std::sync::Arc<NativeTunDevice> {
    async fn set_routes(
        &self,
        wanted: std::collections::BTreeSet<std::net::IpAddr>,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(), crate::connection::model::VpnFailure> {
        self.sync_dynamic(wanted, cancellation)
            .await
            .map_err(|error| crate::connection::model::VpnFailure::new(&error.code, &error.message))
    }
}
#[cfg(target_os = "windows")]
impl crate::connection::capture::RouteCapture for std::sync::Arc<windows_tun::managed::Device> {
    async fn set_routes(
        &self,
        wanted: std::collections::BTreeSet<std::net::IpAddr>,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(), crate::connection::model::VpnFailure> {
        self.sync_dynamic(wanted, cancellation)
            .await
            .map_err(|error| crate::connection::model::VpnFailure::new(&error.code, &error.message))
    }
}
#[cfg(target_os = "linux")]
impl crate::connection::capture::RouteCapture for std::sync::Arc<linux_tun::managed::Device> {
    async fn set_routes(
        &self,
        wanted: std::collections::BTreeSet<std::net::IpAddr>,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(), crate::connection::model::VpnFailure> {
        self.sync_dynamic(wanted, cancellation)
            .await
            .map_err(|error| crate::connection::model::VpnFailure::new(&error.code, &error.message))
    }
}

pub(crate) mod files;
#[cfg(target_os = "linux")]
pub(crate) mod packet_fd;
