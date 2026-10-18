use crate::connection::model::VpnFailure;
use std::net::{IpAddr, SocketAddr};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
static QUERY_GATE: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)));

#[derive(Clone, Debug)]
pub struct NetworkInterface {
    pub name: String,
    pub description: String,
    pub index: u32,
    pub recommended: bool,
}

pub(super) struct DefaultRoute {
    pub ipv4: bool,
    pub index: u32,
    pub metric: u64,
}

pub(crate) struct CaptureSnapshot {
    pub routes: Vec<ipnet::IpNet>,
    pub local_addresses: Vec<IpAddr>,
    pub source_addresses: Vec<IpAddr>,
    pub route_interfaces: Vec<(ipnet::IpNet, u32)>,
}

pub(crate) async fn capture_snapshot() -> Result<CaptureSnapshot, VpnFailure> {
    #[cfg(target_os = "linux")]
    let operation = linux::capture_snapshot();
    #[cfg(target_os = "windows")]
    let operation = async {
        let permit = QUERY_GATE
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| VpnFailure::new("worker", "网络读取队列已关闭"))?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            super::windows::capture_snapshot()
        })
        .await
        .map_err(|_| VpnFailure::new("worker", "读取捕获路由的线程退出失败"))?
    };
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let operation =
        async { Err(VpnFailure::new("platform", "当前平台没有完整捕获入口")) };
    tokio::time::timeout(std::time::Duration::from_secs(3), operation)
        .await
        .map_err(|_| VpnFailure::new("timeout", "读取完整捕获的系统路由超时"))?
}

pub(crate) async fn available() -> Result<Vec<NetworkInterface>, VpnFailure> {
    #[cfg(target_os = "windows")]
    {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let permit = QUERY_GATE
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| VpnFailure::new("worker", "网络接口读取队列已关闭"))?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                super::windows::available()
            })
            .await
            .map_err(|_| VpnFailure::new("worker", "读取网络接口的工作线程退出失败"))?
        })
        .await
        .map_err(|_| VpnFailure::new("timeout", "读取网络接口超时"))?
    }
    #[cfg(target_os = "linux")]
    {
        tokio::time::timeout(std::time::Duration::from_secs(3), linux::available())
            .await
            .map_err(|_| VpnFailure::new("timeout", "读取网络接口超时"))?
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err(VpnFailure::new("platform", "当前平台没有桌面网络接口入口"))
    }
}

/// A display hint only: this never changes the listener or the routing policy.
pub(crate) async fn proxy_endpoint(bound: SocketAddr) -> (String, Option<String>) {
    if !bound.ip().is_unspecified() {
        return (bound.to_string(), None);
    }
    let result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let choices = available().await?;
        #[cfg(target_os = "windows")]
        let addresses = {
            let permit = QUERY_GATE
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| VpnFailure::new("worker", "网络接口读取队列已关闭"))?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                super::windows::lan_addresses()
            })
            .await
            .map_err(|_| VpnFailure::new("worker", "读取局域网地址的工作线程退出失败"))??
        };
        #[cfg(target_os = "linux")]
        let addresses = linux::lan_addresses().await?;
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        let addresses = Vec::new();
        Ok::<_, VpnFailure>(select_lan_address(&choices, addresses, bound.is_ipv6()))
    })
    .await;
    match result {
        Ok(Ok(Some(address))) => (SocketAddr::new(address, bound.port()).to_string(), None),
        Ok(Ok(None)) => (
            bound.to_string(),
            Some("未找到与监听地址同族的局域网地址".into()),
        ),
        Ok(Err(error)) => (bound.to_string(), Some(error.message)),
        Err(_) => (bound.to_string(), Some("读取局域网地址超时".into())),
    }
}

fn select_lan_address(
    choices: &[NetworkInterface],
    addresses: Vec<(u32, IpAddr)>,
    ipv6: bool,
) -> Option<IpAddr> {
    addresses
        .into_iter()
        .filter(|(_, address)| {
            address.is_ipv6() == ipv6
                && !address.is_loopback()
                && !address.is_unspecified()
                && !address.is_multicast()
                && match address {
                    IpAddr::V4(ip) => !ip.is_link_local() && !ip.is_broadcast(),
                    IpAddr::V6(ip) => !ip.is_unicast_link_local() && ip.to_ipv4_mapped().is_none(),
                }
        })
        .filter_map(|(index, address)| {
            choices
                .iter()
                .find(|choice| choice.index == index)
                .map(|choice| (!choice.recommended, choice.name.as_str(), address))
        })
        .min()
        .map(|(_, _, address)| address)
}

pub(super) fn recommend(
    choices: &mut [NetworkInterface],
    mut defaults: Vec<DefaultRoute>,
) -> Result<(), VpnFailure> {
    if choices.len() > 256 {
        return Err(VpnFailure::new("interface", "可用网络接口超过数量限制"));
    }
    choices.sort_by(|left, right| left.name.cmp(&right.name));
    for choice in choices.iter_mut() {
        choice.recommended = false;
    }
    defaults.sort_by_key(|route| (!route.ipv4, route.metric, route.index));
    let selected = defaults
        .iter()
        .find(|route| choices.iter().any(|choice| choice.index == route.index))
        .map(|route| route.index);
    if let Some(choice) = choices
        .iter_mut()
        .find(|choice| Some(choice.index) == selected)
    {
        choice.recommended = true;
    }
    Ok(())
}

pub(crate) fn select(choices: &[NetworkInterface], requested: &str) -> Result<String, VpnFailure> {
    let chosen = if requested.is_empty() {
        choices
            .iter()
            .find(|choice| choice.recommended)
            .or_else(|| {
                if choices.len() == 1 {
                    choices.first()
                } else {
                    None
                }
            })
    } else {
        choices.iter().find(|choice| choice.name == requested)
    };
    chosen.map(|choice| choice.name.clone()).ok_or_else(|| {
        VpnFailure::new(
            "interface",
            "没有匹配的底层网络，请在设置中重新选择网络接口",
        )
    })
}

pub(crate) fn physical_dns(interface: &str) -> Result<Vec<std::net::IpAddr>, VpnFailure> {
    #[cfg(target_os = "windows")]
    let servers = super::windows::Interface::resolve(interface)?.dns_servers()?;
    #[cfg(target_os = "linux")]
    let servers = super::dns::servers(None)?
        .into_iter()
        .map(|server| server.ip())
        .collect::<Vec<_>>();
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = interface;
        return Err(VpnFailure::new("platform", "当前平台没有物理 DNS 入口"));
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let _ = interface;
        if servers.is_empty() {
            return Err(VpnFailure::new(
                "dns",
                "底层网络只有本地 DNS 代理，请填写明确的本次 DNS 上游",
            ));
        }
        Ok(servers)
    }
}

pub(crate) fn validate(interface: Option<&str>) -> Result<(), VpnFailure> {
    let Some(interface) = interface else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    {
        if interface.is_empty()
            || interface.len() > 15
            || !interface
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            return Err(VpnFailure::new("interface", "物理接口名称无效"));
        }
        let name = std::ffi::CString::new(interface)
            .map_err(|_| VpnFailure::new("interface", "物理接口名称无效"))?;
        // SAFETY: name is a live NUL-terminated string.
        if unsafe { libc::if_nametoindex(name.as_ptr()) } == 0 {
            return Err(VpnFailure::new("interface", "物理接口不存在"));
        }
        use std::os::fd::AsRawFd;
        let socket = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, None)?;
        // SAFETY: zero is valid for the native ifreq union.
        let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
        for (slot, byte) in request.ifr_name.iter_mut().zip(interface.bytes()) {
            *slot = byte as libc::c_char;
        }
        // SAFETY: the owned socket and native writable ifreq remain live.
        if unsafe { libc::ioctl(socket.as_raw_fd(), libc::SIOCGIFHWADDR, &mut request) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: successful SIOCGIFHWADDR initialized this sockaddr member.
        let family = unsafe { request.ifr_ifru.ifru_hwaddr.sa_family };
        if ![libc::ARPHRD_ETHER, libc::ARPHRD_LOOPBACK].contains(&family) {
            return Err(VpnFailure::new(
                "interface",
                "当前包出口仅支持以太网或回环接口，不能绑定 TUN",
            ));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        super::windows::Interface::resolve(interface).map(|_| ())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = interface;
        Err(VpnFailure::new("platform", "此平台尚未接入物理接口绑定"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn concrete_proxy_endpoints_keep_their_family_and_ipv6_brackets() -> anyhow::Result<()> {
        for address in ["127.0.0.1:1234", "[::1]:1234", "[2001:db8::1]:1234"] {
            assert_eq!(
                proxy_endpoint(address.parse()?).await,
                (address.into(), None)
            );
        }
        Ok(())
    }

    #[test]
    fn lan_hint_uses_only_eligible_same_family_addresses_and_prefers_the_default(
    ) -> anyhow::Result<()> {
        let choices = vec![
            NetworkInterface {
                name: "wifi".into(),
                description: "".into(),
                index: 2,
                recommended: true,
            },
            NetworkInterface {
                name: "ethernet".into(),
                description: "".into(),
                index: 1,
                recommended: false,
            },
        ];
        let addresses = [
            (99, "10.0.0.1"),
            (2, "127.0.0.1"),
            (2, "169.254.1.1"),
            (2, "255.255.255.255"),
            (2, "224.0.0.1"),
            (2, "0.0.0.0"),
            (2, "::"),
            (2, "::1"),
            (2, "fe80::1"),
            (2, "ff02::1"),
            (2, "::ffff:192.168.1.1"),
            (1, "192.168.2.1"),
            (2, "192.168.1.2"),
            (2, "192.168.1.1"),
            (1, "2001:db8:2::1"),
            (2, "2001:db8:1::1"),
        ]
        .into_iter()
        .map(|(index, address)| Ok((index, address.parse()?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
        assert_eq!(
            select_lan_address(&choices, addresses.clone(), false),
            Some("192.168.1.1".parse()?)
        );
        assert_eq!(
            select_lan_address(&choices, addresses, true),
            Some("2001:db8:1::1".parse()?)
        );
        assert_eq!(
            select_lan_address(&choices, vec![(99, "192.168.1.1".parse()?)], false),
            None
        );
        Ok(())
    }

    #[test]
    fn recommendation_uses_eligible_ipv4_defaults_before_ipv6_and_stable_metrics(
    ) -> anyhow::Result<()> {
        let mut choices = vec![
            NetworkInterface {
                name: "wifi".into(),
                description: "".into(),
                index: 2,
                recommended: false,
            },
            NetworkInterface {
                name: "ethernet".into(),
                description: "".into(),
                index: 1,
                recommended: false,
            },
        ];
        recommend(
            &mut choices,
            vec![
                DefaultRoute {
                    ipv4: true,
                    index: 99,
                    metric: 0,
                },
                DefaultRoute {
                    ipv4: false,
                    index: 2,
                    metric: 1,
                },
                DefaultRoute {
                    ipv4: true,
                    index: 2,
                    metric: 50,
                },
                DefaultRoute {
                    ipv4: true,
                    index: 1,
                    metric: 50,
                },
            ],
        )?;
        assert_eq!(select(&choices, "")?, "ethernet");
        assert_eq!(select(&choices, "wifi")?, "wifi");
        assert!(select(&choices, "removed").is_err());
        assert_eq!(
            choices.iter().filter(|choice| choice.recommended).count(),
            1
        );
        for choice in &mut choices {
            choice.recommended = false;
        }
        assert!(select(&choices, "").is_err());
        Ok(())
    }
}
