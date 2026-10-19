use super::{
    model::{ProxyBinding, ProxyKind, VpnFailure},
    session::Engine,
};
pub use crate::platform::model::CaptureMode;

#[derive(Default)]
pub struct ProxyStartup {
    pub http_enabled: bool,
    pub http_address: String,
    pub http_port: u16,
    pub socks_enabled: bool,
    pub socks_address: String,
    pub socks_port: u16,
    pub system_proxy_enabled: bool,
}

pub struct TunStartup {
    pub enabled: bool,
    pub mtu: u32,
    pub physical_interface: String,
    pub physical_dns: Option<String>,
    pub capture_mode: Option<CaptureMode>,
}

#[derive(Clone)]
pub struct ProxyServicesStatus {
    pub http: Option<ProxyBinding>,
    pub socks: Option<ProxyBinding>,
    pub system_proxy_enabled: bool,
    pub system_proxy_error: Option<String>,
}

#[derive(Clone)]
pub struct ServiceFailure {
    pub service: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone)]
pub struct TunOperation {
    pub status_json: String,
    pub diagnostic: Option<String>,
}

#[derive(Clone)]
pub struct DesktopServicesReport {
    pub proxies: ProxyServicesStatus,
    pub failures: Vec<ServiceFailure>,
    pub tun: Option<TunOperation>,
}

pub(crate) async fn start(
    engine: &Engine,
    proxies: ProxyStartup,
    tun: Option<TunStartup>,
    system_proxy_directory: Option<String>,
) -> Result<DesktopServicesReport, VpnFailure> {
    let _operation = engine.desktop_operations.lock().await;
    let mut failures = Vec::new();
    let mut http_ready = true;
    if proxies.http_enabled || proxies.system_proxy_enabled {
        if let Err(error) = engine
            .start(ProxyKind::Http, proxies.http_address, proxies.http_port)
            .await
        {
            http_ready = false;
            failures.push(failure("http", error));
        }
    }
    if proxies.system_proxy_enabled && http_ready {
        let result = match system_proxy_directory {
            Some(directory) => engine.system_proxy(directory).await,
            None => Err(VpnFailure::new("systemProxy", "系统代理恢复目录未配置")),
        };
        if let Err(error) = result {
            failures.push(failure("systemProxy", error));
        }
    }
    if proxies.socks_enabled {
        if let Err(error) = engine
            .start(ProxyKind::Socks, proxies.socks_address, proxies.socks_port)
            .await
        {
            failures.push(failure("socks", error));
        }
    }
    let tun = match tun {
        Some(request) if request.enabled => {
            Some(super::desktop_tun::start_service_unlocked(engine, request).await?)
        }
        _ => None,
    };
    Ok(DesktopServicesReport {
        proxies: engine.proxy_status().await?,
        failures,
        tun,
    })
}

fn failure(service: &str, error: VpnFailure) -> ServiceFailure {
    ServiceFailure {
        service: service.into(),
        code: error.code,
        message: error.message,
    }
}

pub(crate) async fn rebind(
    engine: &Engine,
    kind: ProxyKind,
    address: String,
    port: u16,
    system_proxy_directory: Option<String>,
    system_proxy_enabled: bool,
) -> Result<DesktopServicesReport, VpnFailure> {
    let _operation = engine.desktop_operations.lock().await;
    let before = engine.proxy_status().await?;
    let running = match kind {
        ProxyKind::Http => before.http.is_some(),
        ProxyKind::Socks => before.socks.is_some(),
    };
    if !running {
        return Ok(DesktopServicesReport {
            proxies: before,
            failures: vec![],
            tun: None,
        });
    }
    let restore_system = kind == ProxyKind::Http && system_proxy_enabled;
    let mut failures = Vec::new();
    if restore_system && system_proxy_directory.is_none() {
        failures.push(failure(
            "systemProxy",
            VpnFailure::new("systemProxy", "系统代理恢复目录未配置"),
        ));
    } else {
        let result = async {
            engine.stop_proxy(kind).await?;
            engine.start(kind, address, port).await?;
            if restore_system {
                if let Some(directory) = system_proxy_directory {
                    engine.system_proxy(directory).await?;
                }
            }
            Ok::<_, VpnFailure>(())
        }
        .await;
        if let Err(error) = result {
            let service = if error.code == "systemProxy" {
                "systemProxy"
            } else if kind == ProxyKind::Http {
                "http"
            } else {
                "socks"
            };
            failures.push(failure(service, error));
        }
    }
    Ok(DesktopServicesReport {
        proxies: engine.proxy_status().await?,
        failures,
        tun: None,
    })
}
