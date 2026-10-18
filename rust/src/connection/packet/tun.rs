use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::Port;
use crate::connection::{model::VpnFailure, physical, session::Shared};

#[cfg(target_os = "linux")]
use crate::platform::linux_tun::managed::Device;
use crate::platform::model::{validate, PlatformFailure, ValidatedTunPlan};
#[cfg(target_os = "windows")]
use crate::platform::windows_tun::managed::Device;

pub(crate) struct Service {
    token: CancellationToken,
    task: Option<JoinHandle<Result<(), VpnFailure>>>,
    outcome: Option<Result<(), VpnFailure>>,
    device: Arc<std::sync::Mutex<Option<Arc<Device>>>>,
    physical_interface: Option<String>,
    plan: Option<ValidatedTunPlan>,
}

pub(crate) struct Startup {
    cancellation: CancellationToken,
    result: tokio::sync::Mutex<StartupResult>,
}

struct StartupResult {
    task: Option<JoinHandle<Result<String, VpnFailure>>>,
    outcome: Option<Result<String, VpnFailure>>,
}

struct StartupWait {
    cancellation: CancellationToken,
    accepted: bool,
}
impl Drop for StartupWait {
    fn drop(&mut self) {
        if !self.accepted {
            self.cancellation.cancel();
        }
    }
}

impl Startup {
    async fn join(&self) -> Result<String, VpnFailure> {
        let mut result = self.result.lock().await;
        if let Some(task) = result.task.as_mut() {
            let outcome = task
                .await
                .map_err(|_| VpnFailure::new("worker", "TUN 启动工作线程退出失败"))
                .and_then(|result| result);
            result.outcome = Some(outcome);
            result.task.take();
        }
        match result.outcome.as_ref() {
            Some(Ok(info)) => Ok(info.clone()),
            Some(Err(error)) => Err(VpnFailure::new(&error.code, &error.message)),
            None => Err(VpnFailure::new("worker", "TUN 启动结果缺失")),
        }
    }
}

pub(crate) async fn start(
    shared: Arc<Shared>,
    plan_json: String,
    physical_interface: String,
) -> Result<String, VpnFailure> {
    if shared.tun_service.lock().await.is_some() {
        return Err(VpnFailure::new("busy", "此会话已有 TUN 数据面"));
    }
    let cancellation = shared.cancellation.child_token();
    let startup = {
        let mut slot = shared
            .tun_startup
            .lock()
            .map_err(|_| VpnFailure::new("worker", "TUN 启动状态不可用"))?;
        if slot.is_some() {
            return Err(VpnFailure::new("busy", "TUN 正在启动或清理"));
        }
        let worker_shared = Arc::clone(&shared);
        let worker_token = cancellation.clone();
        let task = tokio::spawn(async move {
            let result = start_owned(
                Arc::clone(&worker_shared),
                plan_json,
                physical_interface,
                worker_token.clone(),
            )
            .await;
            if result.is_err() || worker_token.is_cancelled() {
                if let Err(error) = stop_active(&worker_shared).await {
                    worker_shared.remember_cleanup_failure(&error);
                    return Err(error);
                }
                return result.and_then(|_| Err(closed()));
            }
            result
        });
        let startup = Arc::new(Startup {
            cancellation: cancellation.clone(),
            result: tokio::sync::Mutex::new(StartupResult {
                task: Some(task),
                outcome: None,
            }),
        });
        *slot = Some(Arc::clone(&startup));
        startup
    };
    let mut wait = StartupWait {
        cancellation,
        accepted: false,
    };
    let result = startup.join().await;
    wait.accepted = result.is_ok();
    clear_startup(&shared, &startup)?;
    result
}

fn clear_startup(shared: &Shared, startup: &Arc<Startup>) -> Result<(), VpnFailure> {
    let mut slot = shared
        .tun_startup
        .lock()
        .map_err(|_| VpnFailure::new("worker", "TUN 启动状态不可用"))?;
    if slot
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, startup))
    {
        slot.take();
    }
    Ok(())
}

async fn start_owned(
    shared: Arc<Shared>,
    plan_json: String,
    physical_interface: String,
    cancellation: CancellationToken,
) -> Result<String, VpnFailure> {
    let service = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(closed()),
        service = shared.tun_service.lock() => service,
    };
    if service.is_some() {
        return Err(VpnFailure::new("busy", "此会话已有 TUN 数据面"));
    }
    cleanup_egress(&shared).await?;
    shared
        .tun_cleanup
        .lock()
        .map_err(|_| VpnFailure::new("egress", "清理租约不可用"))?
        .take();
    physical::validate_interface(Some(&physical_interface))?;
    let (mut plan, dns_servers) = prepare_plan(&shared, &plan_json)?;
    let mut sources = shared
        .virtual_addresses
        .read()
        .map_err(|_| VpnFailure::new("address", "无法读取会话地址"))?
        .clone();
    let physical_index = if plan.capture_mode == crate::platform::model::CaptureMode::Full {
        Some(
            crate::network::interface::available()
                .await?
                .into_iter()
                .find(|interface| interface.name == physical_interface)
                .ok_or_else(|| VpnFailure::new("interface", "完整捕获底层接口已变化"))?
                .index,
        )
    } else {
        None
    };
    if plan.capture_mode == crate::platform::model::CaptureMode::Full {
        sources.extend(
            crate::network::interface::capture_snapshot()
                .await?
                .source_addresses,
        );
        sources.extend(plan.addresses.iter().map(ipnet::IpNet::addr));
    }
    sources.retain(|ip| {
        !ip.is_unspecified()
            && !ip.is_multicast()
            && !ip.is_loopback()
            && !matches!(ip, std::net::IpAddr::V4(ip) if ip.is_broadcast())
    });
    sources.sort();
    sources.dedup();
    let ingress = Ingress {
        sources,
        mode: plan.capture_mode,
        dns_servers,
        routes: if plan.capture_mode == crate::platform::model::CaptureMode::Full {
            plan.routes.clone()
        } else {
            Vec::new()
        },
        guard: (plan.capture_mode == crate::platform::model::CaptureMode::Full)
            .then(|| plan.clone()),
        physical_index,
    };
    #[cfg(target_os = "windows")]
    let prepared_device = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(closed()),
        result = crate::platform::windows_tun::managed::Prepared::prepare() => result.map_err(platform_error)?,
    };
    #[cfg(target_os = "linux")]
    let prepared_device = crate::platform::linux_tun::managed::Prepared::prepare()
        .await
        .map_err(platform_error)?;
    #[cfg(target_os = "linux")]
    let mut activation = {
        let result = tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(closed()),
            result = crate::network::Activation::prepare_with_broker(
                physical_interface.clone(), plan.physical_dns_servers.clone(), prepared_device.broker()
            ) => result,
        };
        match result {
            Ok(activation) => activation,
            Err(error) => {
                prepared_device.close().await.map_err(platform_error)?;
                return Err(error);
            }
        }
    };
    #[cfg(not(target_os = "linux"))]
    let mut activation = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(closed()),
        result = crate::network::Activation::prepare(physical_interface.clone(), plan.physical_dns_servers.clone()) => result?,
    };
    let cleanup_owner = activation.cleanup_owner();
    *shared
        .tun_cleanup
        .lock()
        .map_err(|_| VpnFailure::new("egress", "清理租约不可用"))? =
        Some(Arc::clone(&cleanup_owner));
    // Derive node protection from the authenticated plan; callers cannot omit
    // a node from their JSON protection list and capture its TLS transport.
    let prepare = async {
        for nodes in shared.plan.nodes.values() {
            for node in nodes {
                let (host, _port) = sangfor_core::plan::parse_endpoint(node)
                    .ok_or_else(|| VpnFailure::new("nodes", "节点地址无效"))?;
                let addresses = tokio::time::timeout(
                    shared.dial_timeout,
                    crate::network::dns::resolve_with(Some(&activation.egress), &host, None),
                )
                .await
                .map_err(|_| VpnFailure::new("timeout", "节点保护地址解析超时"))??;
                for address in addresses {
                    if plan.routes.iter().any(|route| route.contains(&address))
                        || plan.addresses.iter().any(|vip| vip.addr() == address)
                    {
                        return Err(VpnFailure::new(
                            "protected",
                            "捕获范围或虚拟地址包含会话节点",
                        ));
                    }
                    if !plan.protected_ips.contains(&address) {
                        plan.protected_ips.push(address);
                    }
                }
            }
        }
        activation.publish().await?;
        Ok::<_, VpnFailure>(())
    };
    let prepared = tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(closed()),
        result = prepare => result,
    };
    match prepared {
        Ok(()) => {}
        Err(error) => {
            activation.rollback().await?;
            finish_egress(&cleanup_owner).await?;
            return Err(error);
        }
    }
    // Retain a cleanup owner before the helper can receive Create. The startup
    // task is registered in Shared, so cancellation cannot race egress rollback
    // with a helper that is still installing an interface or routes.
    let egress = activation.commit();
    let device_slot = Arc::new(std::sync::Mutex::new(None));
    let mut service = service;
    *service = Some(Service {
        token: cancellation.clone(),
        task: None,
        outcome: Some(Ok(())),
        device: Arc::clone(&device_slot),
        physical_interface: Some(physical_interface.clone()),
        plan: Some(plan.clone()),
    });
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let device = prepared_device
        .create(plan, &device_slot)
        .await
        .map_err(platform_error)?;

    *device_slot
        .lock()
        .map_err(|_| VpnFailure::new("worker", "TUN 清理状态不可用"))? = Some(Arc::clone(&device));
    if cancellation.is_cancelled() {
        return Err(closed());
    }
    install(
        Arc::clone(&shared),
        service,
        device,
        ingress,
        physical_interface,
        egress,
        device_slot,
    )
    .await
}

fn prepare_plan(
    shared: &Shared,
    plan_json: &str,
) -> Result<(ValidatedTunPlan, Vec<std::net::IpAddr>), VpnFailure> {
    let mut plan = validate(plan_json).map_err(platform_error)?;
    let dns_servers: Vec<_> = if plan.dns_capture {
        shared
            .plan
            .dns_servers
            .iter()
            .filter_map(|server| {
                let ip = server.parse::<std::net::IpAddr>().ok()?;
                crate::routing::authorization::route_for(
                    &shared.plan.routes,
                    server,
                    53,
                    crate::routing::model::Network::Tcp,
                )
                .map(|_| ip)
            })
            .take(16)
            .collect()
    } else {
        vec![]
    };
    if plan.dns_capture
        && dns_servers.is_empty()
        && plan.capture_mode == crate::platform::model::CaptureMode::Campus
    {
        return Err(VpnFailure::new("dns", "DNS 捕获需要获授权的校园 TCP DNS"));
    }
    for ip in &dns_servers {
        if ip.is_loopback()
            || ip.is_multicast()
            || ip.is_unspecified()
            || plan.protected_ips.contains(ip)
            || plan.addresses.iter().any(|vip| vip.addr() == *ip)
        {
            return Err(VpnFailure::new(
                "protected",
                "校园 DNS 捕获地址属于保护范围",
            ));
        }
        if !plan.routes.iter().any(|route| route.contains(ip)) {
            plan.routes.push(
                ipnet::IpNet::new(*ip, if ip.is_ipv4() { 32 } else { 128 })
                    .map_err(|_| VpnFailure::new("dns", "DNS 捕获网段无效"))?,
            );
        }
    }
    {
        let addresses = shared
            .virtual_addresses
            .read()
            .map_err(|_| VpnFailure::new("address", "无法读取会话虚拟地址"))?;
        if plan.capture_mode == crate::platform::model::CaptureMode::Campus
            && plan
                .addresses
                .iter()
                .any(|address| !addresses.contains(&address.addr()))
        {
            return Err(VpnFailure::new("source", "TUN 地址必须由当前网关会话下发"));
        }
    }
    Ok((plan, dns_servers))
}

struct Ingress {
    sources: Vec<std::net::IpAddr>,
    mode: crate::platform::model::CaptureMode,
    dns_servers: Vec<std::net::IpAddr>,
    routes: Vec<ipnet::IpNet>,
    guard: Option<ValidatedTunPlan>,
    physical_index: Option<u32>,
}

async fn install(
    shared: Arc<Shared>,
    mut service: tokio::sync::MutexGuard<'_, Option<Service>>,
    device: Arc<Device>,
    ingress: Ingress,
    physical_interface: String,
    egress: Arc<crate::network::Lease>,
    device_slot: Arc<std::sync::Mutex<Option<Arc<Device>>>>,
) -> Result<String, VpnFailure> {
    let device_info = device.info().map_err(platform_error)?;
    let plan = service.as_ref().and_then(|active| active.plan.clone());
    let info = serde_json::to_string(&device_info)
        .map_err(|_| VpnFailure::new("encoding", "TUN 信息编码失败"))?;
    let port = Port::open_with_sources(
        Arc::clone(&shared),
        Some(physical_interface.clone()),
        ingress.sources,
    )
    .await?;
    let token = service
        .as_ref()
        .ok_or_else(|| VpnFailure::new("worker", "TUN 启动 owner 缺失"))?
        .token
        .clone();
    let worker_token = token.clone();
    let worker_shared = Arc::clone(&shared);
    let guard_plan = ingress.guard;
    let guard_info = device_info.clone();
    let physical_index = ingress.physical_index;
    let guard_interface = physical_interface.clone();
    let mut servers_slot = shared
        .dns_proxy_servers
        .write()
        .map_err(|_| VpnFailure::new("dns", "无法启用 DNS 捕获"))?;
    let mut capture_slot = shared
        .captures
        .lock()
        .map_err(|_| VpnFailure::new("capture", "无法启用 DNS 映射"))?;
    *shared
        .full_capture_routes
        .write()
        .map_err(|_| VpnFailure::new("capture", "无法发布捕获范围"))? = ingress.routes;
    let capture_cancel = token.child_token();
    let (capture_handle, mut capture_task) = crate::connection::capture::start(
        Arc::clone(&shared),
        Arc::clone(&device),
        capture_cancel.clone(),
    );
    *servers_slot = ingress.dns_servers;
    shared.dns_capture_full.store(
        ingress.mode == crate::platform::model::CaptureMode::Full,
        std::sync::atomic::Ordering::Release,
    );
    *capture_slot = Some(capture_handle);
    drop(servers_slot);
    drop(capture_slot);
    let task = tokio::spawn(async move {
        let mut capture_done = false;
        let result = tokio::select! {
            biased;
            _ = worker_token.cancelled() => Ok(()),
            result = bridge(&device, &port) => result,
            result = &mut capture_task => {
                capture_done = true;
                result.map_err(|_| VpnFailure::new("worker", "DNS 捕获工作线程退出失败")).and_then(|result| result)
            }
            result = monitor_full_capture(guard_plan.as_ref(), guard_info.index, &guard_interface, physical_index) => result,
        };
        let result = bridge_outcome(result, &worker_token, &worker_shared);
        worker_token.cancel();

        port.close().await;
        worker_shared
            .dns_capture_full
            .store(false, std::sync::atomic::Ordering::Release);
        if let Ok(mut routes) = worker_shared.full_capture_routes.write() {
            routes.clear();
        }
        if let Ok(mut handle) = worker_shared.captures.lock() {
            handle.take();
        }
        if let Ok(mut servers) = worker_shared.dns_proxy_servers.write() {
            servers.clear();
        }
        capture_cancel.cancel();

        let capture_result = if capture_done {
            Ok(())
        } else {
            capture_task
                .await
                .map_err(|_| VpnFailure::new("worker", "DNS 捕获工作线程退出失败"))
                .and_then(|result| result)
        };

        let capture_result = bridge_outcome(capture_result, &worker_token, &worker_shared);
        let cleanup = device.close().await.map_err(platform_error);

        let restored = if cleanup.is_ok() {
            egress.close().await
        } else {
            Ok(())
        };

        let result = result.and(capture_result).and(cleanup).and(restored);
        if let Err(error) = &result {
            worker_shared.fail(VpnFailure::new(&error.code, &error.message));
        }
        result
    });
    *service = Some(Service {
        token,
        task: Some(task),
        outcome: None,
        device: device_slot,
        physical_interface: Some(physical_interface),
        plan,
    });
    Ok(info)
}

async fn monitor_full_capture(
    plan: Option<&ValidatedTunPlan>,
    index: u32,
    physical: &str,
    physical_index: Option<u32>,
) -> Result<(), VpnFailure> {
    let Some(plan) = plan else {
        return std::future::pending().await;
    };
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        if !crate::network::interface::available()
            .await?
            .iter()
            .any(|interface| interface.name == physical && Some(interface.index) == physical_index)
        {
            return Err(VpnFailure::new(
                "capture_changed",
                "完整捕获底层接口身份已变化，停止并恢复网络",
            ));
        }
        let snapshot = crate::network::interface::capture_snapshot().await?;
        crate::platform::capture::validate_live(
            plan,
            index,
            &snapshot.route_interfaces,
            &snapshot.local_addresses,
        )
        .map_err(|error| {
            VpnFailure::new(
                "capture_changed",
                &format!("完整捕获覆盖已变化，停止并恢复网络：{}", error.message),
            )
        })?;
    }
}

fn bridge_outcome(
    result: Result<(), VpnFailure>,
    cancellation: &CancellationToken,
    shared: &Shared,
) -> Result<(), VpnFailure> {
    // Parent cancellation is propagated to sibling tokens non-atomically.
    // A port may close before this bridge's token observes the same request.
    match result {
        Err(error)
            if matches!(error.code.as_str(), "closed" | "cancelled")
                && (shared.stop_requested() || cancellation.is_cancelled()) =>
        {
            Ok(())
        }
        outcome => outcome,
    }
}

pub(crate) async fn stop(shared: &Shared) -> Result<(), VpnFailure> {
    let startup = shared
        .tun_startup
        .lock()
        .map_err(|_| VpnFailure::new("worker", "TUN 启动状态不可用"))?
        .clone();
    let mut startup_failure = None;
    if let Some(startup) = startup {
        startup.cancellation.cancel();
        if let Err(error) = startup.join().await {
            if error.code == "worker" {
                startup_failure = Some(error);
            }
        }
        clear_startup(shared, &startup)?;
    }
    let result = stop_active(shared).await.and(match startup_failure {
        Some(error) => Err(error),
        None => Ok(()),
    });
    if let Err(error) = &result {
        shared.remember_cleanup_failure(error);
    }
    result
}

pub(crate) async fn status(shared: &Shared) -> Result<String, VpnFailure> {
    let service = shared.tun_service.lock().await;
    let startup = shared
        .tun_startup
        .lock()
        .map_err(|_| VpnFailure::new("worker", "TUN 启动状态不可用"))?
        .is_some();
    let egress_pending = shared
        .tun_cleanup
        .lock()
        .map_err(|_| VpnFailure::new("egress", "出口清理状态不可用"))?
        .is_some();
    let Some(active) = service.as_ref() else {
        return Ok(serde_json::json!({
            "state": if startup { "starting" } else if egress_pending { "stopping" } else { "stopped" },
            "cleanup_pending": startup || egress_pending,
        })
        .to_string());
    };
    let running = !active.token.is_cancelled()
        && active.task.as_ref().is_some_and(|task| !task.is_finished());
    let state = if running {
        "running"
    } else if active.outcome.as_ref().is_some_and(Result::is_err) {
        "failed"
    } else if startup {
        "starting"
    } else {
        "stopping"
    };
    let device = active
        .device
        .lock()
        .map_err(|_| VpnFailure::new("worker", "TUN 清理状态不可用"))?
        .clone();
    let mut snapshot = serde_json::json!({
        "state": state, "cleanup_pending": !running,
        "physical_interface": active.physical_interface,
        "capture_mode": active.plan.as_ref().map(|plan| plan.capture_mode),
    });
    if let Some(device) = device {
        #[cfg(all(target_os = "windows", feature = "tun-probe"))]
        {
            snapshot["helperIo"] = device.probe_status();
        }
        let info = device.info().map_err(platform_error)?;
        snapshot["interface"] = serde_json::to_value(info)
            .map_err(|_| VpnFailure::new("encoding", "TUN 状态编码失败"))?;
    }
    Ok(snapshot.to_string())
}

pub(crate) async fn current_plan(shared: &Shared) -> Result<Option<String>, VpnFailure> {
    let service = shared.tun_service.lock().await;
    let Some(active) = service.as_ref() else {
        return Ok(None);
    };
    if active.token.is_cancelled() || active.task.as_ref().is_none_or(|task| task.is_finished()) {
        return Err(VpnFailure::new("busy", "TUN 正在启动或清理，暂时不能预览"));
    }
    let plan = active
        .plan
        .as_ref()
        .ok_or_else(|| VpnFailure::new("state", "运行中的 TUN 缺少接管计划"))?;
    Ok(Some(
        serde_json::json!({
            "name":plan.name, "mtu":plan.mtu,
            "addresses":plan.addresses.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "routes":plan.routes.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "protected_ips":plan.protected_ips,
            "physical_dns_servers":plan.physical_dns_servers,
            "dns_capture":plan.dns_capture, "capture_mode":plan.capture_mode,
        })
        .to_string(),
    ))
}

async fn stop_active(shared: &Shared) -> Result<(), VpnFailure> {
    let mut service = shared.tun_service.lock().await;
    let mut outcome = Ok(());
    if let Some(active) = service.as_mut() {
        active.token.cancel();
        if let Some(task) = active.task.as_mut() {
            let result = task
                .await
                .map_err(|_| VpnFailure::new("worker", "TUN 工作线程退出失败"))
                .and_then(|result| result);
            active.outcome = Some(result);
            active.task.take();
        }
        let device = active
            .device
            .lock()
            .map_err(|_| VpnFailure::new("worker", "TUN 清理状态不可用"))?
            .clone();
        if let Some(device) = device {
            device.close().await.map_err(platform_error)?;
        }
        // Restore before removing the completed service, retaining both the
        // owner and cached result when restoration needs a later retry.
        cleanup_egress(shared).await?;
        outcome = active
            .outcome
            .take()
            .ok_or_else(|| VpnFailure::new("worker", "TUN 清理结果缺失"))?;
        service.take();
    }
    cleanup_egress(shared).await?;
    shared
        .tun_cleanup
        .lock()
        .map_err(|_| VpnFailure::new("egress", "清理租约不可用"))?
        .take();
    outcome
}

async fn cleanup_egress(shared: &Shared) -> Result<(), VpnFailure> {
    let cleanup = shared
        .tun_cleanup
        .lock()
        .map_err(|_| VpnFailure::new("egress", "清理租约不可用"))?
        .clone();
    if let Some(cleanup) = cleanup {
        finish_egress(&cleanup).await?;
    }
    Ok(())
}

async fn finish_egress(cleanup: &crate::network::Lease) -> Result<(), VpnFailure> {
    cleanup.close().await?;
    Ok(())
}

async fn bridge(device: &Arc<Device>, port: &Port) -> Result<(), VpnFailure> {
    // Keep both directions alive. Cancelling a Windows blocking read when an
    // outgoing packet arrives can consume and lose the next kernel packet.
    tokio::try_join!(read_device(device, port), write_device(device, port)).map(|_| ())
}

async fn read_device(device: &Arc<Device>, port: &Port) -> Result<(), VpnFailure> {
    loop {
        if let Some(packet) = device.read(500).await.map_err(platform_error)? {
            #[cfg(all(target_os = "windows", feature = "tun-probe"))]
            let header = {
                // Keep diagnostic samples to headers; never log application payloads.
                let length = match packet.first().map(|byte| byte >> 4) {
                    Some(4) => usize::from(packet[0] & 15) * 4 + 8,
                    Some(6) => 48,
                    _ => 0,
                };
                packet
                    .iter()
                    .take(length)
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            };
            // Unsupported traffic is counted and rejected per packet;
            // it must not tear down other supported UDP flows.
            if let Err(error) = port.write(packet).await {
                if error.code == "closed" {
                    return Err(error);
                }
                #[cfg(all(target_os = "windows", feature = "tun-probe"))]
                eprintln!(
                    "WINDOWS_TUN_PACKET_REJECTED code={} error={} header={header}",
                    error.code, error.message
                );
                port.shared
                    .stats
                    .failures
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

async fn write_device(device: &Arc<Device>, port: &Port) -> Result<(), VpnFailure> {
    loop {
        if let Some(packet) = port.read(500).await? {
            device.write(&packet).await.map_err(platform_error)?;
        }
    }
}
fn platform_error(error: PlatformFailure) -> VpnFailure {
    VpnFailure::new(&error.code, &error.message)
}
fn closed() -> VpnFailure {
    VpnFailure::new("closed", "会话已断开")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::{
        sync::Notify,
        time::{timeout, Duration},
    };

    #[allow(dead_code)]
    mod gateway {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/gateway.rs"
        ));
    }

    #[tokio::test]
    async fn expected_parent_close_does_not_mask_unexpected_io_or_owner_cleanup_errors(
    ) -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let token = CancellationToken::new();
        assert!(bridge_outcome(
            Err(VpnFailure::new("closed", "port closed")),
            &token,
            &engine.shared
        )
        .is_err());
        engine.stop().await?;
        // Simulate the bridge sibling token not yet observing parent cancel.
        assert!(!token.is_cancelled());
        assert!(bridge_outcome(
            Err(VpnFailure::new("closed", "port closed")),
            &token,
            &engine.shared
        )
        .is_ok());
        assert!(bridge_outcome(
            Err(VpnFailure::new("network", "real I/O failure")),
            &token,
            &engine.shared
        )
        .is_err());
        assert!(bridge_outcome(
            Err(VpnFailure::new("cleanup", "device removal failed")),
            &token,
            &engine.shared
        )
        .is_err());
        Ok(())
    }

    #[tokio::test]
    async fn running_preview_uses_retained_plan_and_status_reports_its_mode() -> anyhow::Result<()>
    {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let plan = validate(
            &serde_json::json!({
                "name":"shuvpnpreview", "mtu":1400,
                "addresses":["10.99.0.2/32"], "routes":["10.21.0.0/16"],
                "protected_ips":["192.0.2.10"], "physical_dns_servers":["192.0.2.53"],
                "dns_capture":true,
            })
            .to_string(),
        )?;
        let token = CancellationToken::new();
        let worker_token = token.clone();
        *engine.shared.tun_service.lock().await = Some(Service {
            token,
            task: Some(tokio::spawn(async move {
                worker_token.cancelled().await;
                Ok(())
            })),
            outcome: None,
            device: Arc::new(std::sync::Mutex::new(None)),
            physical_interface: Some("fixture".into()),
            plan: Some(plan),
        });
        let snapshot: serde_json::Value = serde_json::from_str(&status(&engine.shared).await?)?;
        assert_eq!(snapshot["capture_mode"], "campus");
        let preview = crate::connection::desktop_tun::plan_for_mode(
            &engine.shared,
            1,
            Some("invalid"),
            crate::platform::model::CaptureMode::Full,
            "missing-interface",
        )
        .await?;
        let preview: serde_json::Value = serde_json::from_str(&preview)?;
        assert_eq!(preview["capture_mode"], "campus");
        assert_eq!(preview["name"], "shuvpnpreview");
        assert_eq!(preview["mtu"], 1400);
        assert_eq!(preview["routes"], serde_json::json!(["10.21.0.0/16"]));
        assert_eq!(preview["protected_ips"], serde_json::json!(["192.0.2.10"]));
        engine.stop().await?;
        assert!(current_plan(&engine.shared).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn a_public_tun_stop_cannot_hide_a_panicked_worker_from_finish() -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        *engine.shared.tun_service.lock().await = Some(Service {
            token: CancellationToken::new(),
            task: Some(tokio::spawn(async {
                panic!("injected TUN worker panic");
                #[allow(unreachable_code)]
                Ok(())
            })),
            outcome: None,
            device: Arc::new(std::sync::Mutex::new(None)),
            physical_interface: None,
            plan: None,
        });
        assert_eq!(
            stop(&engine.shared).await.err().map(|error| error.code),
            Some("worker".into())
        );
        assert!(
            engine.finish_stop().await.is_err(),
            "a consumed panic was acknowledged as safe cleanup"
        );
        Ok(())
    }

    #[tokio::test]
    async fn independent_proxy_and_tun_stop_failures_preserve_both_diagnostics(
    ) -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        engine
            .shared
            .remember_cleanup_failure(&VpnFailure::new("systemProxy", "proxy diagnostic"));
        *engine.shared.tun_service.lock().await = Some(Service {
            token: CancellationToken::new(),
            task: Some(tokio::spawn(async {
                Err(VpnFailure::new("fixture", "tun diagnostic"))
            })),
            outcome: None,
            device: Arc::new(std::sync::Mutex::new(None)),
            physical_interface: None,
            plan: None,
        });
        let failure = engine
            .stop()
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("failures disappeared"))?;
        assert!(failure.message.contains("proxy diagnostic"));
        assert!(failure.message.contains("tun diagnostic"));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn a_pending_egress_owner_is_visible_without_a_device_service() -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let activation = crate::network::Activation::prepare_with_broker(
            "lo".into(),
            Some(vec!["192.0.2.53".parse()?]),
            None,
        )
        .await?;
        *engine
            .shared
            .tun_cleanup
            .lock()
            .map_err(|_| anyhow::anyhow!("cleanup lock"))? = Some(activation.cleanup_owner());
        let snapshot: serde_json::Value = serde_json::from_str(&status(&engine.shared).await?)?;
        assert_eq!(snapshot["cleanup_pending"], true);
        assert_ne!(snapshot["state"], "stopped");
        drop(activation);
        engine.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn session_stop_preserves_the_tun_worker_failure_after_driver_cleanup(
    ) -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        *engine.shared.tun_service.lock().await = Some(Service {
            token: CancellationToken::new(),
            task: Some(tokio::spawn(async {
                Err(VpnFailure::new("fixture", "injected TUN worker failure"))
            })),
            outcome: None,
            device: Arc::new(std::sync::Mutex::new(None)),
            physical_interface: None,
            plan: None,
        });
        for _ in 0..2 {
            let failure =
                engine.stop().await.err().ok_or_else(|| {
                    anyhow::anyhow!("TUN failure was hidden by driver completion")
                })?;
            assert_eq!(failure.code, "fixture");
        }
        assert!(engine.shared.tun_service.lock().await.is_none());
        assert_eq!(
            engine.finish_stop().await?,
            Some("injected TUN worker failure".to_owned())
        );
        assert_eq!(
            engine.stop().await.err().map(|error| error.code),
            Some("fixture".to_owned())
        );
        Ok(())
    }

    #[tokio::test]
    async fn stop_waits_for_abandoned_startup_and_retains_it_when_stop_is_cancelled(
    ) -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let cancellation = engine.shared.cancellation.child_token();
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let finished = Arc::new(AtomicBool::new(false));
        let worker_token = cancellation.clone();
        let worker_entered = Arc::clone(&entered);
        let worker_release = Arc::clone(&release);
        let worker_finished = Arc::clone(&finished);
        let startup = Arc::new(Startup {
            cancellation,
            result: tokio::sync::Mutex::new(StartupResult {
                task: Some(tokio::spawn(async move {
                    worker_token.cancelled().await;
                    worker_entered.notify_one();
                    worker_release.notified().await;
                    worker_finished.store(true, Ordering::Release);
                    Err(closed())
                })),
                outcome: None,
            }),
        });
        *engine
            .shared
            .tun_startup
            .lock()
            .map_err(|_| anyhow::anyhow!("startup lock"))? = Some(startup);
        tokio::select! {
            result = stop(&engine.shared) => anyhow::bail!("stop ignored pending startup: {result:?}"),
            _ = entered.notified() => {}
        }
        assert!(!finished.load(Ordering::Acquire));
        assert!(engine
            .shared
            .tun_startup
            .lock()
            .map_err(|_| anyhow::anyhow!("startup lock"))?
            .is_some());
        release.notify_one();
        timeout(Duration::from_secs(2), stop(&engine.shared)).await??;
        assert!(finished.load(Ordering::Acquire));
        assert!(engine
            .shared
            .tun_startup
            .lock()
            .map_err(|_| anyhow::anyhow!("startup lock"))?
            .is_none());
        engine.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn cancelling_stop_keeps_the_worker_registered_until_cleanup_finishes(
    ) -> anyhow::Result<()> {
        let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
        let engine = crate::connection::session::Engine::connect(gateway.plan(), 1000).await?;
        let release = Arc::new(Notify::new());
        let finished = Arc::new(AtomicBool::new(false));
        let worker_release = Arc::clone(&release);
        let worker_finished = Arc::clone(&finished);
        *engine.shared.tun_service.lock().await = Some(Service {
            token: CancellationToken::new(),
            task: Some(tokio::spawn(async move {
                worker_release.notified().await;
                worker_finished.store(true, Ordering::Release);
                Ok(())
            })),
            outcome: None,
            device: Arc::new(std::sync::Mutex::new(None)),
            physical_interface: None,
            plan: None,
        });
        {
            let stopping = stop(&engine.shared);
            tokio::pin!(stopping);
            tokio::select! {
                biased;
                _ = &mut stopping => anyhow::bail!("cleanup unexpectedly finished"),
                _ = tokio::task::yield_now() => {},
            }
            // Dropping this future must retain the join handle in shared state.
        }
        assert!(engine.shared.tun_service.lock().await.is_some());
        let release_later = async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            release.notify_one();
        };
        let (stopped, _) = timeout(Duration::from_secs(2), async {
            tokio::join!(stop(&engine.shared), release_later)
        })
        .await?;
        stopped?;
        assert!(finished.load(Ordering::Acquire));
        assert!(engine.shared.tun_service.lock().await.is_none());
        engine.stop().await?;
        Ok(())
    }
}
