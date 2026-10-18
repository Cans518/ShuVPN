//! Request-scoped HTTP connector for platforms requiring explicit socket binding.
use super::Egress;
use crate::connection::model::VpnFailure;
use base64::{engine::general_purpose::STANDARD, Engine};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Limited};
use hyper::{
    body::{Body, Bytes, Incoming},
    header::{CONNECTION, CONTENT_LENGTH, PROXY_AUTHORIZATION, TRANSFER_ENCODING},
    service::service_fn,
    Method, Request, Response,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use reqwest::Url;
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type ResponseBody = BoxBody<Bytes, BoxError>;

pub(super) struct Tunnel {
    pub proxy: reqwest::Proxy,
    task: JoinHandle<()>,
    #[cfg(test)]
    address: SocketAddr,
}
impl Drop for Tunnel {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Tunnel {
    pub(super) async fn open(
        url: &Url,
        addresses: Vec<IpAddr>,
        egress: Arc<Egress>,
    ) -> Result<Self, VpnFailure> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(failure("HTTP 出口协议无效"));
        }
        let host = url
            .host_str()
            .ok_or_else(|| failure("HTTP 出口缺少目标"))?
            .trim_matches(['[', ']'])
            .to_owned();
        let port = url
            .port_or_known_default()
            .ok_or_else(|| failure("HTTP 出口缺少端口"))?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let mut random = [0u8; 32];
        getrandom::getrandom(&mut random).map_err(|_| failure("HTTP 出口随机凭据不可用"))?;
        let password = STANDARD.encode(random);
        let proxy = reqwest::Proxy::all(format!("http://{address}"))
            .map_err(|_| failure("HTTP 出口连接器配置失败"))?
            .basic_auth("shuvpn", &password);
        let target = Arc::new(Target {
            host,
            port,
            https: url.scheme() == "https",
            addresses,
            egress,
            authorization: format!("Basic {}", STANDARD.encode(format!("shuvpn:{password}"))),
        });
        let task = tokio::spawn(async move {
            let mut workers = JoinSet::new();
            loop {
                tokio::select! {
                    biased;
                    _ = workers.join_next(), if !workers.is_empty() => {}
                    incoming = listener.accept() => {
                        let Ok((socket, _)) = incoming else { break };
                        if workers.len() >= 16 { drop(socket); continue; }
                        let target = Arc::clone(&target);
                        workers.spawn(async move { let _ = serve(socket, target).await; });
                    }
                }
            }
        });
        Ok(Self {
            proxy,
            task,
            #[cfg(test)]
            address,
        })
    }
}

struct Target {
    host: String,
    port: u16,
    https: bool,
    addresses: Vec<IpAddr>,
    egress: Arc<Egress>,
    authorization: String,
}

#[derive(Default)]
struct Session {
    upgraded: Mutex<Option<(hyper::upgrade::OnUpgrade, super::Tcp)>>,
    upstream: Mutex<Option<JoinHandle<()>>>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(mut task) = self.upstream.lock() {
            if let Some(task) = task.take() {
                task.abort();
            }
        }
    }
}

async fn serve(socket: TcpStream, target: Arc<Target>) -> Result<(), BoxError> {
    let session = Arc::new(Session::default());
    let requests = Arc::clone(&session);
    // One request per connection prevents subsequent proxy credentials reaching
    // an origin. Hyper owns request framing, chunked bodies and 100-continue.
    hyper::server::conn::http1::Builder::new()
        .keep_alive(false)
        .max_buf_size(16 * 1024)
        .timer(TokioTimer::new())
        .header_read_timeout(Duration::from_secs(5))
        .serve_connection(
            TokioIo::new(socket),
            service_fn(move |request| forward(request, Arc::clone(&target), Arc::clone(&requests))),
        )
        .with_upgrades()
        .await?;
    let upgraded = session
        .upgraded
        .lock()
        .map_err(|_| failure("HTTP 出口状态不可用"))?
        .take();
    if let Some((upgrade, mut remote)) = upgraded {
        let mut client = TokioIo::new(upgrade.await?);
        tokio::io::copy_bidirectional(&mut client, &mut remote).await?;
    }
    Ok(())
}

async fn forward(
    mut request: Request<Incoming>,
    target: Arc<Target>,
    session: Arc<Session>,
) -> Result<Response<ResponseBody>, BoxError> {
    validate_request(&request, &target)?;
    if request.method() == Method::CONNECT
        && (!request.body().is_end_stream()
            || request.headers().contains_key(TRANSFER_ENCODING)
            || request.headers().contains_key(CONTENT_LENGTH))
    {
        return Err(failure("CONNECT 不能携带 HTTP 请求体").into());
    }
    if request
        .body()
        .size_hint()
        .upper()
        .is_some_and(|size| size > 64 * 1024 * 1024)
    {
        return Err(failure("HTTP 出口请求体超过预算").into());
    }
    let mut remote = None;
    let mut last = None;
    for address in &target.addresses {
        match tokio::time::timeout(
            Duration::from_secs(5),
            super::socket::connect_with(
                Some(&target.egress),
                SocketAddr::new(*address, target.port),
                None,
            ),
        )
        .await
        {
            Ok(Ok(socket)) => {
                remote = Some(socket);
                break;
            }
            Ok(Err(error)) => last = Some(error),
            Err(_) => last = Some(failure("HTTP 物理出口连接超时")),
        }
    }
    let remote = remote.ok_or_else(|| last.unwrap_or_else(|| failure("HTTP 出口没有有效地址")))?;
    if request.method() == Method::CONNECT {
        *session
            .upgraded
            .lock()
            .map_err(|_| failure("HTTP 出口状态不可用"))? =
            Some((hyper::upgrade::on(&mut request), remote));
        // reqwest negotiates and validates TLS after this response; no TLS
        // termination or trust changes occur in the local connector.
        return Ok(Response::new(
            Empty::<Bytes>::new()
                .map_err(|never| match never {})
                .boxed(),
        ));
    }
    *request.uri_mut() = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .parse()?;
    request.headers_mut().remove(PROXY_AUTHORIZATION);
    request.headers_mut().remove("proxy-connection");
    request.headers_mut().remove("upgrade");
    request
        .headers_mut()
        .insert(CONNECTION, hyper::header::HeaderValue::from_static("close"));
    let request = request.map(|body| Limited::new(body, 64 * 1024 * 1024));
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(remote)).await?;
    {
        let mut upstream = session
            .upstream
            .lock()
            .map_err(|_| failure("HTTP 出口状态不可用"))?;
        *upstream = Some(tokio::spawn(async move {
            let _ = connection.await;
        }));
    }
    let response = sender.send_request(request).await?;
    Ok(response.map(|body| body.map_err(|error| Box::new(error) as BoxError).boxed()))
}

fn validate_request<B>(request: &Request<B>, target: &Target) -> Result<(), VpnFailure> {
    let tunnel = request.method() == Method::CONNECT;
    if tunnel != target.https {
        return Err(failure("HTTP 出口协议与本次请求不一致"));
    }
    let url = Url::parse(&if tunnel {
        format!("https://{}", request.uri())
    } else {
        request.uri().to_string()
    })
    .map_err(|_| failure("HTTP 出口目标无效"))?;
    if url.host_str().map(|host| host.trim_matches(['[', ']'])) != Some(target.host.as_str())
        || url.port_or_known_default() != Some(target.port)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || (!tunnel && url.scheme() != "http")
        || (tunnel && (url.path() != "/" || url.query().is_some()))
    {
        return Err(failure("HTTP 出口只能连接本次请求的目标"));
    }
    let mut credentials = request.headers().get_all(PROXY_AUTHORIZATION).iter();
    if credentials.next().map(|value| value.as_bytes()) != Some(target.authorization.as_bytes())
        || credentials.next().is_some()
    {
        return Err(failure("HTTP 出口凭据缺失或无效"));
    }
    Ok(())
}
fn failure(message: &str) -> VpnFailure {
    VpnFailure::new("egress", message)
}

#[cfg(all(test, any(target_os = "windows", target_os = "linux")))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn egress() -> Result<Arc<Egress>, VpnFailure> {
        Ok(Arc::new(Egress {
            interface: "test".to_owned(),
            resolvers: vec![],
            epoch: 0,
            retired: std::sync::atomic::AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            adapter: super::super::windows::Interface::loopback()?,
            #[cfg(target_os = "linux")]
            broker: None,
            #[cfg(target_os = "linux")]
            operations: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(target_os = "linux")]
            idle: tokio::sync::Notify::new(),
            #[cfg(target_os = "linux")]
            cancellation: tokio_util::sync::CancellationToken::new(),
            #[cfg(target_os = "linux")]
            closed: std::sync::atomic::AtomicBool::new(false),
            #[cfg(target_os = "linux")]
            binding_probe: None,
        }))
    }
    async fn read_header(socket: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
        let mut bytes = vec![];
        while !bytes.ends_with(b"\r\n\r\n") {
            anyhow::ensure!(bytes.len() < 16 * 1024, "header exceeded budget");
            bytes.push(socket.read_u8().await?);
        }
        Ok(bytes)
    }

    #[test]
    fn request_target_and_authentication_are_checked_before_any_remote_connection(
    ) -> anyhow::Result<()> {
        let target = Target {
            host: "example.test".to_owned(),
            port: 443,
            https: true,
            addresses: vec![],
            egress: egress()?,
            authorization: "Basic secret".to_owned(),
        };
        let request = |uri: &str, password: &str| -> anyhow::Result<Request<()>> {
            Ok(Request::builder()
                .method(Method::CONNECT)
                .uri(uri)
                .header(PROXY_AUTHORIZATION, password)
                .body(())?)
        };
        validate_request(&request("example.test:443", "Basic secret")?, &target)?;
        for (uri, password) in [
            ("another.test:443", "Basic secret"),
            ("example.test:444", "Basic secret"),
            ("example.test:443", "Basic wrong"),
            ("example.test:443/path", "Basic secret"),
        ] {
            let result = request(uri, password).and_then(|request| {
                validate_request(&request, &target).map_err(anyhow::Error::from)
            });
            assert!(result.is_err());
        }
        let mut duplicate = request("example.test:443", "Basic secret")?;
        duplicate.headers_mut().append(
            PROXY_AUTHORIZATION,
            hyper::header::HeaderValue::from_static("Basic secret"),
        );
        assert!(validate_request(&duplicate, &target).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn protected_reqwest_http_and_tls_use_actual_sockets_and_cancel_the_listener(
    ) -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            let header = read_header(&mut socket).await?;
            assert!(header.starts_with(b"POST /fixture?q=1 HTTP/1.1\r\n"));
            assert!(!String::from_utf8_lossy(&header)
                .to_ascii_lowercase()
                .contains("proxy-authorization"));
            let mut body = [0; 4];
            socket.read_exact(&mut body).await?;
            assert_eq!(&body, b"body");
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nreal")
                .await?;
            Ok::<_, anyhow::Error>(())
        });
        let url = Url::parse(&format!("http://{address}/fixture?q=1"))?;
        let tunnel = Tunnel::open(&url, vec![address.ip()], egress()?).await?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .proxy(tunnel.proxy.clone())
            .timeout(Duration::from_secs(3))
            .build()?;
        assert_eq!(
            client.post(url).body("body").send().await?.text().await?,
            "real"
        );
        server.await??;
        drop(tunnel);
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])?;
        let certificate = cert.cert.der().clone();
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into(),
            )?;
        let tls = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (socket, _) = listener.accept().await?;
                if let Ok(mut socket) = tls.accept(socket).await {
                    let mut bytes = vec![];
                    while !bytes.ends_with(b"\r\n\r\n") {
                        bytes.push(socket.read_u8().await?);
                    }
                    socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\ntls",
                        )
                        .await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        });
        let url = Url::parse(&format!("https://localhost:{}/", address.port()))?;
        let tunnel = Tunnel::open(&url, vec![address.ip()], egress()?).await?;
        let ordinary = reqwest::Client::builder()
            .no_proxy()
            .proxy(tunnel.proxy.clone())
            .timeout(Duration::from_secs(3))
            .build()?;
        assert!(ordinary.get(url.clone()).send().await.is_err());
        let trusted = reqwest::Client::builder()
            .no_proxy()
            .proxy(tunnel.proxy.clone())
            .add_root_certificate(reqwest::Certificate::from_der(&certificate)?)
            .timeout(Duration::from_secs(3))
            .build()?;
        assert_eq!(trusted.get(url).send().await?.text().await?, "tls");
        server.await??;
        let local_address = tunnel.address;
        let task = tunnel.task.abort_handle();
        drop(tunnel);
        tokio::time::timeout(Duration::from_secs(1), async {
            while !task.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert!(TcpStream::connect(local_address).await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn pipelined_requests_and_chunked_bodies_do_not_leak_proxy_headers() -> anyhow::Result<()>
    {
        for chunked in [false, true] {
            let origin = TcpListener::bind("127.0.0.1:0").await?;
            let address = origin.local_addr()?;
            let proxy_listener = TcpListener::bind("127.0.0.1:0").await?;
            let proxy_address = proxy_listener.local_addr()?;
            let target = Arc::new(Target {
                host: address.ip().to_string(),
                port: address.port(),
                https: false,
                addresses: vec![address.ip()],
                egress: egress()?,
                authorization: "Basic private".to_owned(),
            });
            let proxy_task = tokio::spawn(async move {
                let (socket, _) = proxy_listener.accept().await?;
                serve(socket, target)
                    .await
                    .map_err(|error| anyhow::anyhow!("{error}"))
            });
            let server = tokio::spawn(async move {
                let (mut socket, _) = origin.accept().await?;
                let header = read_header(&mut socket).await?;
                let header = String::from_utf8(header)?.to_ascii_lowercase();
                assert!(!header.contains("proxy-authorization"));
                assert!(header.contains("connection: close"));
                if header.contains("transfer-encoding: chunked") {
                    let mut chunk = [0; 14];
                    socket.read_exact(&mut chunk).await?;
                    assert_eq!(&chunk, b"4\r\nbody\r\n0\r\n\r\n");
                } else {
                    let mut body = [0; 4];
                    socket.read_exact(&mut body).await?;
                    assert_eq!(&body, b"body");
                }
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                    .await?;
                let mut leaked = [0u8; 1];
                if let Ok(result) =
                    tokio::time::timeout(Duration::from_millis(100), socket.read(&mut leaked)).await
                {
                    assert_eq!(result?, 0, "second request reached origin");
                }
                Ok::<_, anyhow::Error>(())
            });
            let mut client = TcpStream::connect(proxy_address).await?;
            let (framing, body) = if chunked {
                ("Transfer-Encoding: chunked", "4\r\nbody\r\n0\r\n\r\n")
            } else {
                ("Content-Length: 4", "body")
            };
            client.write_all(format!("POST http://{address}/first HTTP/1.1\r\nHost: {address}\r\nProxy-Authorization: Basic private\r\n{framing}\r\n\r\n{body}GET http://evil.test/second HTTP/1.1\r\nProxy-Authorization: Basic private\r\n\r\n").as_bytes()).await?;
            let mut response = vec![];
            tokio::time::timeout(Duration::from_secs(3), client.read_to_end(&mut response))
                .await??;
            assert!(response.ends_with(b"ok"));
            server.await??;
            proxy_task.await??;
        }
        Ok(())
    }

    #[tokio::test]
    async fn invalid_connect_body_or_credentials_never_open_the_origin_socket() -> anyhow::Result<()>
    {
        for valid_credentials in [false, true] {
            let origin = TcpListener::bind("127.0.0.1:0").await?;
            let address = origin.local_addr()?;
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let local = listener.local_addr()?;
            let target = Arc::new(Target {
                host: address.ip().to_string(),
                port: address.port(),
                https: true,
                addresses: vec![address.ip()],
                egress: egress()?,
                authorization: "Basic secret".to_owned(),
            });
            let worker = tokio::spawn(async move {
                let (socket, _) = listener.accept().await?;
                Ok::<_, anyhow::Error>(serve(socket, target).await.is_err())
            });
            let credentials = if valid_credentials {
                "Basic secret"
            } else {
                "Basic invalid"
            };
            let body = if valid_credentials {
                "Content-Length: 3\r\n"
            } else {
                ""
            };
            let mut client = TcpStream::connect(local).await?;
            client.write_all(format!("CONNECT {address} HTTP/1.1\r\nHost: {address}\r\nProxy-Authorization: {credentials}\r\n{body}\r\n").as_bytes()).await?;
            assert!(tokio::time::timeout(Duration::from_secs(1), worker).await???);
            assert!(
                tokio::time::timeout(Duration::from_millis(100), origin.accept())
                    .await
                    .is_err()
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn dropping_a_tunnel_closes_an_in_flight_tls_relay_and_its_listener() -> anyhow::Result<()>
    {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (ready, started) = tokio::sync::oneshot::channel();
        let origin = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            let mut bytes = [0; 4096];
            anyhow::ensure!(socket.read(&mut bytes).await? > 0, "no TLS ClientHello");
            ready
                .send(())
                .map_err(|_| anyhow::anyhow!("cancel signal dropped"))?;
            let mut total = 0;
            loop {
                let count = socket.read(&mut bytes).await?;
                if count == 0 {
                    break;
                }
                total += count;
                anyhow::ensure!(total < 64 * 1024, "unexpected TLS byte stream");
            }
            Ok::<_, anyhow::Error>(())
        });
        let url = Url::parse(&format!("https://localhost:{}/", address.port()))?;
        let tunnel = Tunnel::open(&url, vec![address.ip()], egress()?).await?;
        let local = tunnel.address;
        let client = reqwest::Client::builder()
            .no_proxy()
            .proxy(tunnel.proxy.clone())
            .timeout(Duration::from_secs(3))
            .build()?;
        let request = tokio::spawn(async move { client.get(url).send().await });
        tokio::time::timeout(Duration::from_secs(2), started).await??;
        drop(tunnel);
        tokio::time::timeout(Duration::from_secs(2), origin).await???;
        assert!(tokio::time::timeout(Duration::from_secs(2), request)
            .await??
            .is_err());
        assert!(TcpStream::connect(local).await.is_err());
        Ok(())
    }
}
