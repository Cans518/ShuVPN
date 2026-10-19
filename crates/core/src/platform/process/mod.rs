//! Best-effort local socket ownership snapshots; never guess an absent owner.
use std::{
    fmt,
    net::SocketAddr,
    time::{Duration, Instant},
};

use crate::routing::model::{Network, ProcessConfidence, ProcessIdentity};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

const MAX_ROWS: usize = 100_000;
const BUDGET: Duration = Duration::from_millis(500);
static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Clone, Copy)]
pub(crate) struct Query {
    pub network: Network,
    pub source: SocketAddr,
    pub destination: SocketAddr,
}

impl Query {
    fn normalized(self) -> Self {
        Self {
            source: normalize_address(self.source),
            destination: normalize_address(self.destination),
            ..self
        }
    }
}

pub(super) fn normalize_address(value: SocketAddr) -> SocketAddr {
    if let SocketAddr::V6(value6) = value {
        if value6.scope_id() == 0 {
            if let Some(ip) = value6.ip().to_ipv4_mapped() {
                return SocketAddr::from((ip, value.port()));
            }
        }
    }
    value
}

#[derive(Debug)]
pub(crate) struct Failure(pub String);
impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for Failure {}
impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Self(format!("无法读取本机进程归属：{error}"))
    }
}

pub(crate) fn deadline(deadline: Instant) -> Result<(), Failure> {
    if Instant::now() >= deadline {
        return Err(Failure("进程归属查询超过时间预算".into()));
    }
    Ok(())
}

pub(crate) fn identity(pid: u32, path: String, source: &str) -> Result<ProcessIdentity, Failure> {
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Failure("可执行文件名称不可表示".into()))?
        .to_owned();
    if pid == 0
        || name.is_empty()
        || path.len() > 32768
        || !std::path::Path::new(&path).is_absolute()
        || path.chars().any(char::is_control)
    {
        return Err(Failure("可执行文件身份无效".into()));
    }
    Ok(ProcessIdentity {
        pid,
        name,
        path,
        source: source.to_owned(),
        confidence: ProcessConfidence::SocketSnapshot,
    })
}

pub(crate) async fn lookup(query: Query) -> Result<ProcessIdentity, Failure> {
    let query = query.normalized();
    if query.source.port() == 0
        || query.destination.port() == 0
        || query.source.is_ipv4() != query.destination.is_ipv4()
    {
        return Err(Failure("进程查询套接字地址无效".into()));
    }
    let end = Instant::now() + BUDGET;
    let slot = tokio::time::timeout(BUDGET, SLOTS.acquire())
        .await
        .map_err(|_| Failure("进程归属查询排队超过预算".into()))?
        .map_err(|_| Failure("进程归属查询队列已关闭".into()))?;
    let task = tokio::task::spawn_blocking(move || {
        let _slot = slot;
        deadline(end)?;
        #[cfg(target_os = "linux")]
        {
            linux::lookup(query, end)
        }
        #[cfg(target_os = "windows")]
        {
            windows::lookup(query, end)
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = (query, end);
            Err(Failure("当前平台无法读取进程身份".into()))
        }
    });
    tokio::time::timeout(
        end.saturating_duration_since(Instant::now()) + Duration::from_millis(250),
        task,
    )
    .await
    .map_err(|_| Failure("进程归属查询未及时完成".into()))?
    .map_err(|_| Failure("进程归属查询线程退出失败".into()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{TcpListener, TcpStream, UdpSocket},
        process::{Child, ChildStdout, Command, Stdio},
    };
    static TEST_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    struct Fixture {
        child: Child,
        source: SocketAddr,
        _output: BufReader<ChildStdout>,
    }
    impl Fixture {
        fn start(
            network: &str,
            destination: SocketAddr,
            bind: Option<SocketAddr>,
        ) -> anyhow::Result<Self> {
            let mut child = Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "platform::process::tests::socket_fixture",
                    "--nocapture",
                ])
                .env(
                    "SHUVPN_PROCESS_SOCKET_FIXTURE",
                    format!(
                        "{network}|{destination}|{}",
                        bind.map(|address| address.to_string()).unwrap_or_default()
                    ),
                )
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()?;
            let output = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("fixture stdout"))?;
            let mut fixture = Self {
                child,
                source: "127.0.0.1:0".parse()?,
                _output: BufReader::new(output),
            };
            for line in fixture._output.by_ref().lines() {
                let line = line?;
                if let Some((_, value)) = line.split_once("PROCESS_SOCKET_READY ") {
                    fixture.source = value.parse()?;
                    return Ok(fixture);
                }
            }
            anyhow::bail!("fixture not ready")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            drop(self.child.stdin.take());
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    fn socket_fixture() -> anyhow::Result<()> {
        let Ok(value) = std::env::var("SHUVPN_PROCESS_SOCKET_FIXTURE") else {
            return Ok(());
        };
        let mut fields = value.split('|');
        let network = fields
            .next()
            .ok_or_else(|| anyhow::anyhow!("fixture network"))?;
        let destination = fields
            .next()
            .ok_or_else(|| anyhow::anyhow!("fixture destination"))?;
        let bind = fields
            .next()
            .filter(|value| !value.is_empty())
            .map(str::parse::<SocketAddr>)
            .transpose()?;
        let destination: SocketAddr = destination.parse()?;
        let mapped = network.starts_with("mapped-");
        let peer = if mapped {
            let SocketAddr::V4(address) = destination else {
                anyhow::bail!("mapped fixture needs IPv4");
            };
            SocketAddr::from((address.ip().to_ipv6_mapped(), address.port()))
        } else {
            destination
        };
        let (_tcp, _udp, source) = if network.ends_with("tcp") {
            let socket = if mapped {
                let socket = socket2::Socket::new(
                    socket2::Domain::IPV6,
                    socket2::Type::STREAM,
                    Some(socket2::Protocol::TCP),
                )?;
                socket.set_only_v6(false)?;
                socket.connect(&peer.into())?;
                TcpStream::from(socket)
            } else {
                TcpStream::connect(destination)?
            };
            let source = socket.local_addr()?;
            (Some(socket), None, source)
        } else {
            let socket = if network == "udp-v6-only" {
                let socket = socket2::Socket::new(
                    socket2::Domain::IPV6,
                    socket2::Type::DGRAM,
                    Some(socket2::Protocol::UDP),
                )?;
                socket.set_only_v6(true)?;
                socket.bind(&"[::]:0".parse::<SocketAddr>()?.into())?;
                UdpSocket::from(socket)
            } else if mapped {
                let socket = socket2::Socket::new(
                    socket2::Domain::IPV6,
                    socket2::Type::DGRAM,
                    Some(socket2::Protocol::UDP),
                )?;
                socket.set_only_v6(false)?;
                socket.bind(&"[::]:0".parse::<SocketAddr>()?.into())?;
                UdpSocket::from(socket)
            } else if network == "udp-reuse" {
                let socket = socket2::Socket::new(
                    if destination.is_ipv4() {
                        socket2::Domain::IPV4
                    } else {
                        socket2::Domain::IPV6
                    },
                    socket2::Type::DGRAM,
                    Some(socket2::Protocol::UDP),
                )?;
                socket.set_reuse_address(true)?;
                socket.bind(&bind.unwrap_or(SocketAddr::new(destination.ip(), 0)).into())?;
                UdpSocket::from(socket)
            } else {
                UdpSocket::bind(SocketAddr::new(destination.ip(), 0))?
            };
            if network != "udp-v6-only" {
                socket.connect(peer)?;
            }
            let source = socket.local_addr()?;
            (None, Some(socket), source)
        };
        println!("PROCESS_SOCKET_READY {source}");
        std::io::stdout().flush()?;
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input)?;
        Ok(())
    }

    #[tokio::test]
    async fn mapped_dual_stack_sockets_are_found_even_for_an_ipv4_query() -> anyhow::Result<()> {
        let _gate = TEST_GATE.lock().await;
        for network in [Network::Tcp, Network::Udp] {
            let tcp = if network == Network::Tcp {
                Some(TcpListener::bind("127.0.0.1:0")?)
            } else {
                None
            };
            let udp = if network == Network::Udp {
                Some(UdpSocket::bind("127.0.0.1:0")?)
            } else {
                None
            };
            let destination = match (&tcp, &udp) {
                (Some(socket), _) => socket.local_addr()?,
                (_, Some(socket)) => socket.local_addr()?,
                _ => anyhow::bail!("fixture socket missing"),
            };
            let child = Fixture::start(
                if network == Network::Tcp {
                    "mapped-tcp"
                } else {
                    "mapped-udp"
                },
                destination,
                None,
            )?;
            let _accepted = match tcp.as_ref() {
                Some(socket) => Some(socket.accept()?.0),
                None => None,
            };
            let observed = lookup(Query {
                network,
                source: child.source,
                destination,
            })
            .await?;
            assert_eq!(observed.pid, child.child.id());
        }
        Ok(())
    }

    #[tokio::test]
    async fn real_dual_stack_tcp_udp_socket_owners_are_the_client_child_not_the_server(
    ) -> anyhow::Result<()> {
        let _gate = TEST_GATE.lock().await;
        for ip in ["127.0.0.1", "::1"] {
            for network in [Network::Tcp, Network::Udp] {
                let ip = ip.parse::<std::net::IpAddr>()?;
                let tcp = if network == Network::Tcp {
                    Some(TcpListener::bind((ip, 0))?)
                } else {
                    None
                };
                let udp = if network == Network::Udp {
                    Some(UdpSocket::bind((ip, 0))?)
                } else {
                    None
                };
                let destination = match (&tcp, &udp) {
                    (Some(socket), _) => socket.local_addr()?,
                    (_, Some(socket)) => socket.local_addr()?,
                    _ => anyhow::bail!("fixture socket missing"),
                };
                let child = Fixture::start(
                    if network == Network::Tcp {
                        "tcp"
                    } else {
                        "udp"
                    },
                    destination,
                    None,
                )?;
                let pid = child.child.id();
                let source = child.source;
                let _accepted = match tcp.as_ref() {
                    Some(listener) => Some(listener.accept()?.0),
                    None => None,
                };
                let observed = lookup(Query {
                    network,
                    source,
                    destination,
                })
                .await?;
                assert_eq!(observed.pid, pid, "{network:?} {ip}");
                assert_ne!(observed.pid, std::process::id());
                assert!(!observed.name.is_empty());
                assert!(std::path::Path::new(&observed.path).is_absolute());
                assert!(observed.source.ends_with("socket-snapshot"));
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn shared_udp_endpoints_are_ambiguous_instead_of_selecting_a_process(
    ) -> anyhow::Result<()> {
        let _gate = TEST_GATE.lock().await;
        let destination = UdpSocket::bind("127.0.0.1:0")?;
        let first = Fixture::start("udp-reuse", destination.local_addr()?, None)?;
        let _second = Fixture::start("udp-reuse", destination.local_addr()?, Some(first.source))?;
        let error = lookup(Query {
            network: Network::Udp,
            source: first.source,
            destination: destination.local_addr()?,
        })
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("shared owner was guessed"))?;
        assert!(
            error.0.contains("歧义") || error.0.contains("唯一"),
            "{error}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn v6_only_wildcard_cannot_prove_an_ipv4_udp_owner() -> anyhow::Result<()> {
        let _gate = TEST_GATE.lock().await;
        let target = UdpSocket::bind("127.0.0.1:0")?;
        let child = Fixture::start("udp-v6-only", target.local_addr()?, None)?;
        assert!(
            lookup(Query {
                network: Network::Udp,
                source: SocketAddr::from(([127, 0, 0, 1], child.source.port())),
                destination: target.local_addr()?
            })
            .await
            .is_err(),
            "IPv6-only socket was falsely attributed to an IPv4 flow"
        );
        Ok(())
    }

    #[tokio::test]
    async fn short_lookup_bursts_wait_for_a_worker_instead_of_failing_immediately(
    ) -> anyhow::Result<()> {
        let _gate = TEST_GATE.lock().await;
        let server = TcpListener::bind("127.0.0.1:0")?;
        let child = Fixture::start("tcp", server.local_addr()?, None)?;
        let _accepted = server.accept()?;
        let first = SLOTS.acquire().await?;
        let second = SLOTS.acquire().await?;
        let lookup = lookup(Query {
            network: Network::Tcp,
            source: child.source,
            destination: server.local_addr()?,
        });
        tokio::pin!(lookup);
        tokio::select! {
            biased;
            result = &mut lookup => anyhow::bail!("lookup did not wait for capacity: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
        drop(first);
        drop(second);
        assert_eq!(lookup.await?.pid, child.child.id());
        Ok(())
    }

    #[tokio::test]
    async fn absent_socket_owners_and_invalid_queries_are_not_attributed_to_the_core(
    ) -> anyhow::Result<()> {
        let _gate = TEST_GATE.lock().await;
        for port in [0, 1] {
            assert!(lookup(Query {
                network: Network::Tcp,
                source: format!("192.0.2.111:{port}").parse()?,
                destination: "192.0.2.112:518".parse()?
            })
            .await
            .is_err());
        }
        Ok(())
    }
}
