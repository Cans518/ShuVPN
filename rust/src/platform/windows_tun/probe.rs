//! Opt-in kernel/ring verification; never exposed through FRB.
use super::{
    helper::Client,
    ip::{self, RouteIo},
    Device,
};
use anyhow::{ensure, Context};
use std::{collections::BTreeSet, net::IpAddr, sync::Arc, time::Duration};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use windows_sys::Win32::{
    NetworkManagement::IpHelper::{
        ConvertInterfaceIndexToLuid, GetIpInterfaceEntry, GetUnicastIpAddressEntry,
        InitializeIpInterfaceEntry, MIB_IPFORWARD_ROW2, MIB_IPINTERFACE_ROW,
    },
    NetworkManagement::Ndis::NET_LUID_LH,
    Networking::WinSock::{IpDadStatePreferred, AF_INET, AF_INET6},
};

mod packet;

const V4_LOCAL: &str = "192.0.2.2";
const V6_LOCAL: &str = "2001:db8:518::2";
const STATIC: [&str; 2] = ["192.0.2.3", "2001:db8:518::3"];
const DYNAMIC: [&str; 2] = ["198.51.100.3", "2001:db8:519::3"];

struct Setup {
    before: Vec<MIB_IPFORWARD_ROW2>,
    name: String,
    document: String,
}

pub async fn verify() -> anyhow::Result<String> {
    ensure!(
        super::administrator()?,
        "permission: Windows TUN probe requires an administrator process"
    );
    let Setup {
        before,
        name,
        document,
    } = prepare()?;
    let device = Device::create(crate::platform::model::validate(&document)?).await?;
    let index = device.info().index;
    // Cleanup is outside the work timeout, so timeout still awaits close and
    // verifies the kernel state. Drop remains a fallback for process failure.
    let work = tokio::time::timeout(Duration::from_secs(30), async {
        device.wait_ready().await?;
        exercise(&device).await
    })
    .await
    .context("Windows TUN probe timed out")
    .and_then(|result| result);
    let close = tokio::time::timeout(Duration::from_secs(10), device.close())
        .await
        .context("Windows TUN cleanup timed out; kernel cleanup unproven")
        .and_then(|result| result.context("close Windows TUN probe"));
    let after = ip::NativeRoutes.entries()?;
    let unchanged = same_routes(&before, &after);
    ensure!(
        close.is_ok() && unchanged,
        "probe cleanup failed: close={close:?}, route snapshot unchanged={unchanged}, work={work:?}"
    );
    work?;
    Ok(serde_json::json!({
        "result":"WINDOWS_TUN_DUAL_STACK_RING_DYNAMIC_ROUTES_AND_CLEANUP_PASSED",
        "name":name, "index":index, "routeSnapshotUnchanged":true,
        "emptyRingReadClosed":true, "closedTwice":true,
        "preCancelledUpdateRejected":true,
    })
    .to_string())
}

fn prepare() -> anyhow::Result<Setup> {
    let before = ip::NativeRoutes.entries()?;
    let hosts = [
        V4_LOCAL, V6_LOCAL, STATIC[0], STATIC[1], DYNAMIC[0], DYNAMIC[1],
    ]
    .into_iter()
    .map(str::parse::<IpAddr>)
    .collect::<Result<Vec<_>, _>>()?;
    ensure!(
        before.iter().all(|row| {
            row.DestinationPrefix.PrefixLength == 0
                || ip::address(&row.DestinationPrefix.Prefix).is_none_or(|address| {
                    ipnet::IpNet::new(address, row.DestinationPrefix.PrefixLength)
                        .is_ok_and(|network| hosts.iter().all(|host| !network.contains(host)))
                })
        }),
        "probe documentation addresses overlap an existing non-default route"
    );
    let mut random = [0; 4];
    getrandom::getrandom(&mut random).context("generate exclusive probe name")?;
    let name = format!("shuvpn{:08x}", u32::from_be_bytes(random));
    let document = serde_json::json!({
        "name":name, "mtu":1400,
        "addresses":[format!("{V4_LOCAL}/32"),format!("{V6_LOCAL}/128")],
        "routes":[format!("{}/32",STATIC[0]),format!("{}/128",STATIC[1])],
        "protected_ips":["127.0.0.1","::1"],
    })
    .to_string();
    crate::platform::model::validate(&document)?;
    Ok(Setup {
        before,
        name,
        document,
    })
}

pub async fn verify_helper(client: Client) -> anyhow::Result<String> {
    verify_bridge(client, Shutdown::Close).await
}

pub async fn verify_helper_disconnect(client: Client) -> anyhow::Result<String> {
    verify_bridge(client, Shutdown::Disconnect).await
}

enum Shutdown {
    Close,
    Disconnect,
}

async fn verify_bridge(client: Client, shutdown: Shutdown) -> anyhow::Result<String> {
    if !client.elevated() {
        client.close().await?;
        anyhow::bail!("permission: Windows helper TUN probe requires elevation");
    }
    let setup = prepare();
    let Setup {
        before,
        name,
        document,
    } = match setup {
        Ok(setup) => setup,
        Err(error) => {
            client.close().await?;
            return Err(error);
        }
    };
    let mut identity = None;
    let work = tokio::time::timeout(Duration::from_secs(30), async {
        let created = client.create(document).await?;
        let info = serde_json::from_str::<crate::platform::model::TunInfo>(&created)?;
        ensure!(
            info.name == name && info.index != 0 && info.mtu == 1400 && !info.closed,
            "helper returned an inconsistent interface identity"
        );
        let mut luid = NET_LUID_LH::default();
        // SAFETY: lookup writes a live LUID; no system mutation occurs.
        ip::check(
            unsafe { ConvertInterfaceIndexToLuid(info.index, &mut luid) },
            "读取探针接口 LUID",
        )?;
        let current = identity.insert(Identity { luid, info });
        exercise_bridge(&client, current).await
    })
    .await
    .context("helper TUN probe timed out")
    .and_then(|result| result);
    let close = match shutdown {
        Shutdown::Close => {
            let result = client.close().await;
            if result.is_ok() {
                client.close().await
            } else {
                result
            }
        }
        Shutdown::Disconnect => client.disconnect_and_wait().await,
    };
    let removed = if let Some(identity) = &identity {
        ip::removed(identity.luid)
    } else {
        Ok(())
    };
    let absent = ip::find_device(&name)?.is_none();
    let unchanged = same_routes(&before, &ip::NativeRoutes.entries()?);
    ensure!(
        close.is_ok() && removed.is_ok() && absent && unchanged,
        "helper kernel cleanup unproven: close={close:?}, removed={removed:?}, absent={absent}, unchanged={unchanged}, work={work:?}"
    );
    work?;
    let result = match shutdown {
        Shutdown::Close => "WINDOWS_TUN_HELPER_DUAL_STACK_RING_DYNAMIC_ROUTES_AND_CLEANUP_PASSED",
        Shutdown::Disconnect => "WINDOWS_TUN_HELPER_DISCONNECT_WITH_OWNED_ADAPTER_CLEANUP_PASSED",
    };
    let identity = identity.context("missing observed helper interface")?;
    Ok(serde_json::json!({
        "result":result, "name":name, "index":identity.info.index,
        "routeSnapshotUnchanged":true, "helperExited":true, "adapterRemoved":true,
    })
    .to_string())
}

async fn exercise_bridge(client: &Client, identity: &Identity) -> anyhow::Result<()> {
    verify_interface(identity)?;
    for host in STATIC {
        verify_route(identity, host.parse()?)?;
        round_trip(client, host.parse()?).await?;
    }
    let hosts = DYNAMIC
        .into_iter()
        .map(str::parse::<IpAddr>)
        .collect::<Result<Vec<_>, _>>()?;
    client.sync_routes(hosts.clone()).await?;
    client.sync_routes(hosts.clone()).await?;
    for host in &hosts {
        verify_route(identity, *host)?;
        round_trip(client, *host).await?;
    }
    client.sync_routes(vec![]).await?;
    ensure!(
        ip::NativeRoutes.entries()?.iter().all(|row| {
            (unsafe { row.InterfaceLuid.Value }) != unsafe { identity.luid.Value }
                || ip::address(&row.DestinationPrefix.Prefix).is_none_or(|ip| !hosts.contains(&ip))
        }),
        "helper left dynamic routes after removal"
    );
    // Both shutdown variants must remove an actual dynamic route, not only a
    // session with no kernel objects.
    client.sync_routes(hosts.clone()).await?;
    for host in hosts {
        verify_route(identity, host)?;
    }
    Ok(())
}

async fn exercise(device: &Arc<Device>) -> anyhow::Result<()> {
    let identity = Identity {
        luid: device.luid,
        info: device.info(),
    };
    verify_interface(&identity)?;
    for host in STATIC {
        verify_route(&identity, host.parse()?)?;
        round_trip(device, host.parse()?).await?;
    }
    let dynamic = DYNAMIC
        .into_iter()
        .map(str::parse::<IpAddr>)
        .collect::<Result<BTreeSet<_>, _>>()?;
    let cancellation = CancellationToken::new();
    device.sync_dynamic(dynamic.clone(), &cancellation).await?;
    for host in &dynamic {
        verify_route(&identity, *host)?;
        round_trip(device, *host).await?;
    }
    device.sync_dynamic(dynamic.clone(), &cancellation).await?;
    cancellation.cancel();
    let failure = device
        .sync_dynamic(BTreeSet::new(), &cancellation)
        .await
        .err()
        .context("cancelled dynamic update unexpectedly succeeded")?;
    ensure!(failure.code == "cancelled", "wrong cancellation failure");
    for host in &dynamic {
        verify_route(&identity, *host)?;
    }
    device
        .sync_dynamic(BTreeSet::new(), &CancellationToken::new())
        .await?;
    ensure!(
        ip::NativeRoutes.entries()?.iter().all(|row| {
            (unsafe { row.InterfaceLuid.Value }) != unsafe { device.luid.Value }
                || ip::address(&row.DestinationPrefix.Prefix)
                    .is_none_or(|ip| !dynamic.contains(&ip))
        }),
        "dynamic host routes remain after removal"
    );
    // Leave a dynamic route installed to test close's ownership cleanup.
    device
        .sync_dynamic(dynamic.clone(), &CancellationToken::new())
        .await?;
    for host in dynamic {
        verify_route(&identity, host)?;
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        while device.read(10).await?.is_some() {}
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    let reader = device.clone();
    let (ready, waiting) = tokio::sync::oneshot::channel();
    let mut pending = tokio::task::JoinSet::new();
    pending.spawn(async move {
        let mut ready = Some(ready);
        reader
            .read_with_wait(5000, move || {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(());
                }
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), waiting).await??;
    device.close().await?;
    let result = tokio::time::timeout(Duration::from_secs(5), pending.join_next())
        .await
        .context("pending read did not finish after close")?
        .context("missing read worker")?;
    let failure = result?
        .err()
        .context("pending read unexpectedly succeeded")?;
    ensure!(
        failure.code == "closed",
        "pending read returned wrong error"
    );
    device.close().await?;
    ensure!(device.info().closed, "device did not report closed");
    Ok(())
}

struct Identity {
    luid: NET_LUID_LH,
    info: crate::platform::model::TunInfo,
}

fn verify_interface(device: &Identity) -> anyhow::Result<()> {
    for host in [V4_LOCAL, V6_LOCAL] {
        let host: IpAddr = host.parse()?;
        let mut address = ip::address_row(
            device.luid,
            ipnet::IpNet::new(host, if host.is_ipv4() { 32 } else { 128 })?,
        );
        // SAFETY: IP Helper receives an initialized row identifying our LUID
        // and virtual address; the call fills only this live stack object.
        ip::check(
            unsafe { GetUnicastIpAddressEntry(&mut address) },
            "核验探针虚拟地址",
        )?;
        ensure!(
            address.InterfaceIndex == device.info.index
                && address.DadState == IpDadStatePreferred
                && address.OnLinkPrefixLength == if host.is_ipv4() { 32 } else { 128 },
            "kernel VIP is not preferred or has the wrong interface/prefix"
        );
        let mut interface = MIB_IPINTERFACE_ROW::default();
        // SAFETY: initialization and lookup write only the live row; LUID is
        // retained by Device until close.
        unsafe { InitializeIpInterfaceEntry(&mut interface) };
        interface.Family = if host.is_ipv4() { AF_INET } else { AF_INET6 };
        interface.InterfaceLuid = device.luid;
        ip::check(
            unsafe { GetIpInterfaceEntry(&mut interface) },
            "核验探针接口 MTU",
        )?;
        ensure!(
            interface.NlMtu == device.info.mtu && interface.InterfaceIndex == device.info.index,
            "kernel IP interface MTU or index differs from the plan"
        );
    }
    Ok(())
}

fn verify_route(device: &Identity, host: IpAddr) -> anyhow::Result<()> {
    ip::check_device(device.luid, device.info.index, &device.info.name)?;
    let expected = ip::route_row(
        device.luid,
        device.info.index,
        ipnet::IpNet::new(host, if host.is_ipv4() { 32 } else { 128 })?,
    );
    let routes = ip::NativeRoutes.entries()?;
    ensure!(
        routes
            .iter()
            .filter(|row| ip::same_prefix(row, &expected))
            .count()
            == 1
            && routes
                .iter()
                .any(|row| ip::same_owned_route(row, &expected)),
        "kernel route does not match the owned host route for {host}"
    );
    Ok(())
}

trait PacketIo {
    async fn read(
        &self,
        wait_ms: u32,
    ) -> Result<Option<Vec<u8>>, crate::platform::model::PlatformFailure>;
    async fn write(&self, bytes: &[u8]) -> Result<(), crate::platform::model::PlatformFailure>;
}

impl PacketIo for Arc<Device> {
    async fn read(
        &self,
        wait_ms: u32,
    ) -> Result<Option<Vec<u8>>, crate::platform::model::PlatformFailure> {
        Device::read(self, wait_ms).await
    }
    async fn write(&self, bytes: &[u8]) -> Result<(), crate::platform::model::PlatformFailure> {
        Device::write(self, bytes).await
    }
}

impl PacketIo for Client {
    async fn read(
        &self,
        wait_ms: u32,
    ) -> Result<Option<Vec<u8>>, crate::platform::model::PlatformFailure> {
        Client::read(self, wait_ms).await
    }
    async fn write(&self, bytes: &[u8]) -> Result<(), crate::platform::model::PlatformFailure> {
        Client::write(self, bytes.to_vec()).await
    }
}

async fn round_trip(device: &impl PacketIo, host: IpAddr) -> anyhow::Result<()> {
    let local: IpAddr = if host.is_ipv4() { V4_LOCAL } else { V6_LOCAL }.parse()?;
    let socket = UdpSocket::bind((local, 0)).await?;
    let peer = std::net::SocketAddr::new(host, 19518);
    let source = socket.local_addr()?;
    let payload = format!("shuvpn-ring-{peer}");
    socket.send_to(payload.as_bytes(), peer).await?;
    let reply = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(bytes) = device.read(500).await? {
                if let Some(reply) = packet::reply(&bytes, source, peer, payload.as_bytes())? {
                    return Ok::<_, anyhow::Error>(reply);
                }
            }
        }
    })
    .await
    .context("kernel UDP packet did not enter Wintun")??;
    device.write(&reply).await?;
    let mut received = [0; 1400];
    let (length, sender) =
        tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut received))
            .await
            .context("injected Wintun reply did not reach UDP socket")??;
    ensure!(
        sender == peer && received[..length] == *payload.as_bytes(),
        "kernel received the wrong reply tuple or payload"
    );
    Ok(())
}

fn same_routes(before: &[MIB_IPFORWARD_ROW2], after: &[MIB_IPFORWARD_ROW2]) -> bool {
    before.len() == after.len()
        && before
            .iter()
            .all(|expected| after.iter().any(|row| ip::same_owned_route(row, expected)))
}
