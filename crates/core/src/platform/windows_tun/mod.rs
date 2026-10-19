use std::ptr::null_mut;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::NetworkManagement::{IpHelper::*, Ndis::NET_LUID_LH};
use windows_sys::Win32::Security::*;
use windows_sys::Win32::System::Threading::*;

use super::model::{validate_packet, PlatformFailure, TunInfo, ValidatedTunPlan};
mod api;
mod dynamic;
pub(crate) mod helper;
mod ip;
pub(crate) mod managed;
#[cfg(feature = "tun-probe")]
pub(crate) mod probe;
use api::{error, system, wide, Api, Handle};

struct Resources {
    api: Arc<Api>,
    adapter: Handle,
    session: Handle,
    read_event: HANDLE,
    close_event: HANDLE,
    config: Arc<Mutex<ip::Configuration>>,
}
// Packet API calls are documented thread-safe. Start/End/Close execute only
// during construction/destruction, with Arc leases retaining every resource.
unsafe impl Send for Resources {}
unsafe impl Sync for Resources {}

impl Drop for Resources {
    fn drop(&mut self) {
        if !self.session.is_null() {
            unsafe {
                (self.api.end)(self.session);
            }
        }
        if let Ok(mut config) = self.config.lock() {
            // Failed rows stay owned by Device for the next explicit close.
            let _ = config.close();
        }
        if !self.adapter.is_null() {
            unsafe {
                (self.api.close)(self.adapter);
            }
        }
        if !self.close_event.is_null() {
            unsafe {
                CloseHandle(self.close_event);
            }
        }
    }
}

pub(crate) struct Device {
    resources: Mutex<Option<Arc<Resources>>>,
    cancellation: CancellationToken,
    active: AtomicUsize,
    idle: Notify,
    close_gate: tokio::sync::Mutex<()>,
    info: TunInfo,
    luid: NET_LUID_LH,
    config: Arc<Mutex<ip::Configuration>>,
    protected: Vec<std::net::IpAddr>,
}

struct Operation {
    device: Arc<Device>,
    resources: Option<Arc<Resources>>,
}
impl Drop for Operation {
    fn drop(&mut self) {
        self.resources.take();
        self.device.active.fetch_sub(1, Ordering::AcqRel);
        self.device.idle.notify_waiters();
    }
}

impl Device {
    pub(crate) async fn create(plan: ValidatedTunPlan) -> Result<Arc<Self>, PlatformFailure> {
        if !administrator()? {
            return Err(error("permission", "创建 Windows TUN 需要管理员权限"));
        }
        ip::check_plan(&plan)?;
        let api = Api::load(&Api::default_path()?)?;
        let config = Arc::new(Mutex::new(ip::Configuration::new()));
        // These calls finish before yielding: no detached setup worker can
        // create an adapter after the caller abandons this create future.
        let adapter = unsafe {
            (api.create)(
                wide(&plan.name).as_ptr(),
                wide("ShuVPN").as_ptr(),
                std::ptr::null(),
            )
        };
        if adapter.is_null() {
            return Err(system("创建 Wintun 适配器失败"));
        }
        let mut resources = Resources {
            api,
            adapter,
            session: null_mut(),
            read_event: null_mut(),
            close_event: null_mut(),
            config: config.clone(),
        };
        let mut luid = NET_LUID_LH::default();
        unsafe {
            (resources.api.luid)(adapter, &mut luid);
        }
        let mut index = 0;
        ip::check(
            unsafe { ConvertInterfaceLuidToIndex(&luid, &mut index) },
            "读取 Wintun 接口索引",
        )?;
        if index == 0 || unsafe { luid.Value } == 0 {
            return Err(error("identity", "Wintun 接口身份无效"));
        }
        let mut alias = vec![0u16; 257];
        ip::check(
            unsafe { ConvertInterfaceLuidToAlias(&luid, alias.as_mut_ptr(), alias.len()) },
            "核验 Wintun 接口名称",
        )?;
        let length = alias
            .iter()
            .position(|unit| *unit == 0)
            .ok_or_else(|| error("identity", "接口名称未结束"))?;
        if String::from_utf16(&alias[..length]).ok().as_deref() != Some(plan.name.as_str()) {
            return Err(error("identity", "Wintun 没有创建指定的新接口"));
        }
        config
            .lock()
            .map_err(|_| error("lock", "TUN 配置状态不可用"))?
            .install(luid, index, &plan)?;
        resources.session = unsafe { (resources.api.start)(adapter, 4 * 1024 * 1024) };
        if resources.session.is_null() {
            return Err(system("启动 Wintun 会话失败"));
        }
        resources.read_event = unsafe { (resources.api.event)(resources.session) };
        if resources.read_event.is_null() {
            return Err(system("获取 Wintun 读取事件失败"));
        }
        resources.close_event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if resources.close_event.is_null() {
            return Err(system("创建 Wintun 关闭事件失败"));
        }
        Ok(Arc::new(Self {
            resources: Mutex::new(Some(Arc::new(resources))),
            cancellation: CancellationToken::new(),
            active: AtomicUsize::new(0),
            idle: Notify::new(),
            close_gate: tokio::sync::Mutex::new(()),
            info: TunInfo {
                name: plan.name,
                index,
                mtu: plan.mtu,
                addresses: plan.addresses.iter().map(ToString::to_string).collect(),
                routes: plan.routes.iter().map(ToString::to_string).collect(),
                closed: false,
            },
            luid,
            config,
            protected: plan.protected_ips,
        }))
    }

    fn operation(self: &Arc<Self>) -> Result<Operation, PlatformFailure> {
        let resources = self
            .resources
            .lock()
            .map_err(|_| error("lock", "TUN 状态不可用"))?;
        if self.cancellation.is_cancelled() {
            return Err(closed());
        }
        let resources = resources.as_ref().ok_or_else(closed)?.clone();
        self.active.fetch_add(1, Ordering::AcqRel);
        Ok(Operation {
            device: self.clone(),
            resources: Some(resources),
        })
    }

    pub(crate) fn info(&self) -> TunInfo {
        let mut info = self.info.clone();
        info.closed = self.cancellation.is_cancelled();
        info
    }

    pub(crate) async fn wait_ready(&self) -> Result<(), PlatformFailure> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if self.cancellation.is_cancelled() {
                return Err(closed());
            }
            if self
                .config
                .lock()
                .map_err(|_| error("lock", "TUN 配置状态不可用"))?
                .ready(self.luid, self.info.index)?
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(error("startup", "TUN 链路、地址或主机选路未在预算内就绪"));
            }
            tokio::select! {
                _ = self.cancellation.cancelled() => return Err(closed()),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
    }

    pub(crate) async fn sync_dynamic(
        self: &Arc<Self>,
        mut wanted: std::collections::BTreeSet<std::net::IpAddr>,
        cancellation: &CancellationToken,
    ) -> Result<(), PlatformFailure> {
        super::model::validate_dynamic(&self.info, &self.protected, &mut wanted)?;
        let _operation = self.operation()?;
        let mut config = self
            .config
            .lock()
            .map_err(|_| error("lock", "TUN 配置状态不可用"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let check = || {
            if self.cancellation.is_cancelled() || cancellation.is_cancelled() {
                return Err(error("cancelled", "动态路由操作已取消"));
            }
            if Instant::now() >= deadline {
                return Err(error("timeout", "动态路由操作超时"));
            }
            Ok(())
        };
        check()?;
        ip::check_device(self.luid, self.info.index, &self.info.name)?;
        config.dynamic.sync(
            &mut ip::NativeRoutes,
            wanted,
            self.luid,
            self.info.index,
            check,
        )
    }

    pub(crate) async fn read(
        self: &Arc<Self>,
        wait_ms: u32,
    ) -> Result<Option<Vec<u8>>, PlatformFailure> {
        self.read_with_wait(wait_ms, || {}).await
    }

    async fn read_with_wait(
        self: &Arc<Self>,
        wait_ms: u32,
        mut waiting: impl FnMut() + Send + 'static,
    ) -> Result<Option<Vec<u8>>, PlatformFailure> {
        if !(1..=5000).contains(&wait_ms) {
            return Err(error("timeout", "TUN 读取预算无效"));
        }
        let operation = self.operation()?;
        tokio::task::spawn_blocking(move || {
            let resources = operation.resources.as_ref().ok_or_else(closed)?;
            let deadline = Instant::now() + Duration::from_millis(u64::from(wait_ms));
            loop {
                if operation.device.cancellation.is_cancelled() {
                    return Err(closed());
                }
                let mut length = 0;
                let packet = unsafe { (resources.api.receive)(resources.session, &mut length) };
                if !packet.is_null() {
                    let result = if length > 65535 {
                        Err(error("packet", "Wintun 返回了超限 IP 包"))
                    } else {
                        let packet =
                            unsafe { std::slice::from_raw_parts(packet, length as usize) }.to_vec();
                        validate_packet(&packet, operation.device.info.mtu).map(|_| Some(packet))
                    };
                    unsafe {
                        (resources.api.release)(resources.session, packet);
                    }
                    return result;
                }
                if unsafe { GetLastError() } != ERROR_NO_MORE_ITEMS {
                    return Err(system("Wintun 读取失败"));
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(None);
                }
                let handles = [resources.close_event, resources.read_event];
                waiting();
                match unsafe {
                    WaitForMultipleObjects(
                        2,
                        handles.as_ptr(),
                        0,
                        remaining.as_millis().min(100) as u32,
                    )
                } {
                    WAIT_OBJECT_0 => return Err(closed()),
                    code if code == WAIT_OBJECT_0 + 1 || code == WAIT_TIMEOUT => {}
                    _ => return Err(system("等待 Wintun 事件失败")),
                }
            }
        })
        .await
        .map_err(|_| error("worker", "Wintun 读取线程退出失败"))?
    }

    pub(crate) async fn write(self: &Arc<Self>, packet: &[u8]) -> Result<(), PlatformFailure> {
        validate_packet(packet, self.info.mtu)?;
        let operation = self.operation()?;
        let resources = operation.resources.as_ref().ok_or_else(closed)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.cancellation.is_cancelled() {
                return Err(closed());
            }
            let destination =
                unsafe { (resources.api.allocate)(resources.session, packet.len() as u32) };
            if !destination.is_null() {
                // There is no yield between allocation and send; send releases
                // the exact buffer and Wintun offers no separate cancel API.
                unsafe {
                    std::ptr::copy_nonoverlapping(packet.as_ptr(), destination, packet.len());
                    (resources.api.send)(resources.session, destination);
                }
                return Ok(());
            }
            if unsafe { GetLastError() } != ERROR_BUFFER_OVERFLOW {
                return Err(system("Wintun 写入失败"));
            }
            if Instant::now() >= deadline {
                return Err(error("timeout", "Wintun 发送 ring 持续满载"));
            }
            tokio::select! {
                _ = self.cancellation.cancelled() => return Err(closed()),
                _ = tokio::time::sleep(Duration::from_millis(1)) => {}
            }
        }
    }

    pub(crate) async fn close(&self) -> Result<(), PlatformFailure> {
        let _gate = self.close_gate.lock().await;
        self.request_close()?;
        loop {
            let notified = self.idle.notified();
            if self.active.load(Ordering::Acquire) == 0 {
                break;
            }
            notified.await;
        }
        self.config
            .lock()
            .map_err(|_| error("lock", "无法读取 TUN 清理状态"))?
            .close()?;
        #[cfg(test)]
        if unsafe { self.luid.Value } == 0 {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match ip::removed(self.luid) {
                Ok(()) => return Ok(()),
                Err(error) if error.code == "cleanup" && Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn request_close(&self) -> Result<(), PlatformFailure> {
        self.cancellation.cancel();
        if let Some(resources) = self
            .resources
            .lock()
            .map_err(|_| error("lock", "TUN 状态不可用"))?
            .take()
        {
            unsafe {
                SetEvent(resources.close_event);
            }
            drop(resources);
        }
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut resources) = self.resources.lock() {
            if let Some(resources) = resources.take() {
                unsafe {
                    SetEvent(resources.close_event);
                }
            }
        }
    }
}

pub(crate) fn driver_info() -> Result<String, PlatformFailure> {
    let path = Api::default_path()?;
    let _api = Api::load(&path)?;
    let elevated = administrator()?;
    let helper_ready = helper::installed_path().is_ok();
    let available = elevated || helper_ready;
    Ok(serde_json::json!({
        "backend": "wintun", "version":"0.14.1", "sha256":api::DLL_SHA256,
        "elevated":elevated, "capture":"split only", "sessionDataPlaneReady":true,
        "available":available, "helperReady":helper_ready, "permissionPrompt":!elevated,
        "unavailableReason": if available { None } else {
            Some("配套的网络权限程序不可用，请保留完整桌面应用目录；当前可使用系统代理")
        },
    })
    .to_string())
}

pub(crate) fn administrator() -> Result<bool, PlatformFailure> {
    let mut sid = [0u64; 16];
    let mut length = std::mem::size_of_val(&sid) as u32;
    if unsafe {
        CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            std::ptr::null_mut(),
            sid.as_mut_ptr().cast(),
            &mut length,
        )
    } == 0
    {
        return Err(system("读取管理员权限失败"));
    }
    let mut member = 0;
    if unsafe { CheckTokenMembership(std::ptr::null_mut(), sid.as_ptr() as *mut _, &mut member) }
        == 0
    {
        return Err(system("核验管理员权限失败"));
    }
    Ok(member != 0)
}
fn closed() -> PlatformFailure {
    error("closed", "TUN 接口已关闭")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn driver_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../desktop/resources/wintun/amd64/wintun.dll")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from("invalid"))
    }

    #[test]
    fn official_pinned_driver_exports_load_without_creating_an_adapter() -> anyhow::Result<()> {
        let api = Api::load(&driver_path())?;
        drop(api);
        let directory = tempfile::tempdir()?;
        let invalid = directory.path().join("wintun.dll");
        std::fs::write(&invalid, b"untrusted DLL")?;
        assert_eq!(
            Api::load(&invalid).err().map(|error| error.code),
            Some("driver".to_owned())
        );
        Ok(())
    }

    #[test]
    fn ip_helper_rows_keep_exact_luid_prefix_and_on_link_next_hop() -> anyhow::Result<()> {
        let luid = NET_LUID_LH { Value: 123456 };
        for network in ["10.253.254.0/24", "2001:db8:ffff::/64"] {
            let network: ipnet::IpNet = network.parse()?;
            let row = ip::route_row(luid, 987, network);
            assert_eq!(unsafe { row.InterfaceLuid.Value }, 123456);
            assert_eq!(row.InterfaceIndex, 987);
            assert_eq!(row.DestinationPrefix.PrefixLength, network.prefix_len());
            assert_eq!(
                ip::address(&row.DestinationPrefix.Prefix),
                Some(network.network())
            );
            assert!(ip::address(&row.NextHop).is_some_and(|ip| ip.is_unspecified()));
            assert_eq!(row.Metric, 32765);
            assert_eq!(
                row.Protocol,
                windows_sys::Win32::Networking::WinSock::MIB_IPPROTO_NETMGMT
            );
        }
        let vip: ipnet::IpNet = "10.253.254.2/32".parse()?;
        let row = ip::address_row(luid, vip);
        assert_eq!(ip::address(&row.Address), Some(vip.addr()));
        assert_eq!(row.OnLinkPrefixLength, 32);
        assert_eq!(row.ValidLifetime, u32::MAX);
        Ok(())
    }

    #[tokio::test]
    async fn operation_lease_blocks_destruction_and_a_cancelled_close_can_be_retried(
    ) -> anyhow::Result<()> {
        let api = Api::load(&driver_path())?;
        let config = Arc::new(Mutex::new(ip::Configuration::new()));
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(system("test event").into());
        }
        let device = Arc::new(Device {
            resources: Mutex::new(Some(Arc::new(Resources {
                api,
                adapter: null_mut(),
                session: null_mut(),
                read_event: null_mut(),
                close_event: event,
                config: config.clone(),
            }))),
            cancellation: CancellationToken::new(),
            active: AtomicUsize::new(0),
            idle: Notify::new(),
            close_gate: tokio::sync::Mutex::new(()),
            luid: NET_LUID_LH::default(),
            config,
            protected: vec![],
            info: TunInfo {
                name: "shuvpntest".to_owned(),
                index: 0,
                mtu: 1400,
                addresses: vec![],
                routes: vec![],
                closed: false,
            },
        });
        let operation = device.operation()?;
        {
            let closing = device.close();
            tokio::pin!(closing);
            tokio::select! {
                biased;
                _ = &mut closing => anyhow::bail!("closed while a resource lease was active"),
                _ = tokio::task::yield_now() => {}
            }
        }
        assert!(device.cancellation.is_cancelled());
        assert_eq!(device.active.load(Ordering::Acquire), 1);
        assert!(device.operation().is_err());
        assert_eq!(unsafe { WaitForSingleObject(event, 0) }, WAIT_OBJECT_0);
        drop(operation);
        device.close().await?;
        assert!(device.info().closed);
        Ok(())
    }

    #[tokio::test]
    async fn unprivileged_creation_returns_permission_before_mutating_the_system(
    ) -> anyhow::Result<()> {
        if !administrator()? {
            let plan = crate::platform::model::validate(
                r#"{"name":"shuvpntest","mtu":1400,"addresses":["10.253.254.2/32"],
                "routes":["10.253.254.0/24"],"protected_ips":["127.0.0.1"]}"#,
            )?;
            assert_eq!(
                Device::create(plan).await.err().map(|error| error.code),
                Some("permission".to_owned())
            );
        }
        Ok(())
    }
}
