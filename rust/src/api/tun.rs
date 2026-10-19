#[cfg(target_os = "linux")]
use crate::platform::linux_tun::Device;
pub use crate::platform::model::PlatformFailure;
#[cfg(target_os = "windows")]
use crate::platform::windows_tun::Device;

pub struct TunInterface {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    device: std::sync::Arc<Device>,
}

impl Drop for TunInterface {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            let _ = self.device.request_close();
        }
    }
}

impl TunInterface {
    pub async fn create(plan_json: String) -> Result<Self, PlatformFailure> {
        let plan = crate::platform::model::validate(&plan_json)?;
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let device = Device::create(plan).await?;
            #[cfg(target_os = "windows")]
            if let Err(error) = device.wait_ready().await {
                device.close().await?;
                return Err(error);
            }
            Ok(Self { device })
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = plan;
            Err(PlatformFailure::new(
                "platform",
                "当前平台尚未接入此 TUN 适配器",
            ))
        }
    }

    pub fn info(&self) -> Result<String, PlatformFailure> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            serde_json::to_string(&self.device.info())
                .map_err(|_| PlatformFailure::new("encoding", "TUN 信息编码失败"))
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err(PlatformFailure::new(
                "platform",
                "当前平台尚未接入此 TUN 适配器",
            ))
        }
    }

    pub async fn read_packet(&self, wait_ms: u32) -> Result<Option<Vec<u8>>, PlatformFailure> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            self.device.read(wait_ms).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = wait_ms;
            Err(PlatformFailure::new(
                "platform",
                "当前平台尚未接入此 TUN 适配器",
            ))
        }
    }

    pub async fn write_packet(&self, packet: Vec<u8>) -> Result<(), PlatformFailure> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            self.device.write(&packet).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = packet;
            Err(PlatformFailure::new(
                "platform",
                "当前平台尚未接入此 TUN 适配器",
            ))
        }
    }

    pub async fn close(&self) -> Result<(), PlatformFailure> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            self.device.close().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Ok(())
        }
    }
}

pub async fn tun_driver_info() -> Result<String, PlatformFailure> {
    crate::platform::tun_driver_info().await
}
