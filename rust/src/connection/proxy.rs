use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use super::{
    model::{ProxyKind, VpnFailure},
    session::Shared,
    tunnel,
};

struct Request {
    host: String,
    port: u16,
    success: Vec<u8>,
    initial: Vec<u8>,
}

pub(crate) async fn serve(
    listener: TcpListener,
    kind: ProxyKind,
    shared: Arc<Shared>,
    token: CancellationToken,
) {
    let mut clients = JoinSet::new();
    let limit = Arc::new(Semaphore::new(256));
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => break,
            accepted = listener.accept() => {
                let (socket, _) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) if matches!(error.kind(), std::io::ErrorKind::Interrupted
                        | std::io::ErrorKind::WouldBlock | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset) => continue,
                    Err(error) => { shared.fail(error.into()); break; }
                };
                let Ok(permit) = Arc::clone(&limit).try_acquire_owned() else { drop(socket); continue };
                let shared = Arc::clone(&shared);
                let token = token.clone();
                clients.spawn(async move {
                    let _permit = permit;
                    tokio::select! {
                        _ = token.cancelled() => {}
                        _ = serve_client(socket, kind, shared) => {}
                    }
                });
            }
            Some(_) = clients.join_next(), if !clients.is_empty() => {}
        }
    }
    drop(listener);
    token.cancel();
    while clients.join_next().await.is_some() {}
}

async fn serve_client(
    mut socket: TcpStream,
    kind: ProxyKind,
    shared: Arc<Shared>,
) -> Result<(), VpnFailure> {
    let request = timeout(Duration::from_secs(30), async {
        match kind {
            ProxyKind::Http => http_request(&mut socket).await,
            ProxyKind::Socks => socks_request(&mut socket).await,
        }
    })
    .await
    .map_err(|_| VpnFailure::new("timeout", "代理握手超时"))?;
    let request = match request {
        Ok(request) => request,
        Err(error) => {
            if kind == ProxyKind::Http {
                socket.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").await?;
            }
            return Err(error);
        }
    };
    let dialed = timeout(
        shared.dial_timeout,
        tunnel::dial_proxy(
            &shared,
            &request.host,
            request.port,
            match kind {
                ProxyKind::Http => crate::routing::model::Inbound::Http,
                ProxyKind::Socks => crate::routing::model::Inbound::Socks,
            },
            socket.peer_addr()?,
            socket.local_addr()?,
        ),
    )
    .await
    .map_err(|_| VpnFailure::new("timeout", "代理拨号超时"))
    .and_then(|result| result);
    let mut dialed = match dialed {
        Ok(dialed) => dialed,
        Err(error) => {
            if !matches!(error.code.as_str(), "unrouted" | "rejected" | "protected") {
                shared.stats.failures.fetch_add(1, Ordering::Relaxed);
            }
            match kind {
                ProxyKind::Http => socket.write_all(if matches!(error.code.as_str(), "unrouted" | "rejected" | "protected") {
                    b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
                } else {
                    b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
                }).await?,
                ProxyKind::Socks => socket.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await?,
            }
            return Err(error);
        }
    };
    socket.write_all(&request.success).await?;
    if !request.initial.is_empty() {
        dialed.io.write_all(&request.initial).await?;
        dialed.io.flush().await?;
        shared
            .stats
            .upload
            .fetch_add(request.initial.len() as u64, Ordering::Relaxed);
    }
    bridge(socket, dialed, shared).await
}

async fn bridge(
    mut socket: TcpStream,
    dialed: tunnel::Dialed,
    shared: Arc<Shared>,
) -> Result<(), VpnFailure> {
    if !dialed.leftover.is_empty() {
        socket.write_all(&dialed.leftover).await?;
        shared
            .stats
            .download
            .fetch_add(dialed.leftover.len() as u64, Ordering::Relaxed);
    }
    let (mut client_read, mut client_write) = socket.into_split();
    let (mut remote_read, mut remote_write) = tokio::io::split(dialed.io);
    let up_shared = Arc::clone(&shared);
    let upload = async move {
        let mut chunk = [0; 16 * 1024];
        loop {
            let read = client_read.read(&mut chunk).await?;
            if read == 0 {
                remote_write.shutdown().await?;
                break;
            }
            remote_write.write_all(&chunk[..read]).await?;
            remote_write.flush().await?;
            up_shared
                .stats
                .upload
                .fetch_add(read as u64, Ordering::Relaxed);
        }
        Ok::<_, std::io::Error>(())
    };
    let download = async move {
        let mut chunk = [0; 16 * 1024];
        loop {
            let read = remote_read.read(&mut chunk).await?;
            if read == 0 {
                client_write.shutdown().await?;
                break;
            }
            client_write.write_all(&chunk[..read]).await?;
            shared
                .stats
                .download
                .fetch_add(read as u64, Ordering::Relaxed);
        }
        Ok::<_, std::io::Error>(())
    };
    tokio::try_join!(upload, download)?;
    Ok(())
}

async fn socks_request(socket: &mut TcpStream) -> Result<Request, VpnFailure> {
    let mut greeting = [0; 2];
    socket.read_exact(&mut greeting).await?;
    if greeting[0] != 5 || greeting[1] == 0 {
        return Err(VpnFailure::new("socks", "SOCKS5 握手格式无效"));
    }
    let mut methods = vec![0; usize::from(greeting[1])];
    socket.read_exact(&mut methods).await?;
    if !methods.contains(&0) {
        socket.write_all(&[5, 0xff]).await?;
        return Err(VpnFailure::new(
            "socks",
            "客户端未提供可用的 SOCKS5 认证方式",
        ));
    }
    socket.write_all(&[5, 0]).await?;
    let mut header = [0; 4];
    socket.read_exact(&mut header).await?;
    if header[0] != 5 || header[1] != 1 || header[2] != 0 {
        socket.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        return Err(VpnFailure::new("socks", "SOCKS5 仅支持 TCP CONNECT"));
    }
    let host = match header[3] {
        1 => {
            let mut address = [0; 4];
            socket.read_exact(&mut address).await?;
            std::net::Ipv4Addr::from(address).to_string()
        }
        4 => {
            let mut address = [0; 16];
            socket.read_exact(&mut address).await?;
            std::net::Ipv6Addr::from(address).to_string()
        }
        3 => {
            let length = socket.read_u8().await?;
            if length == 0 {
                return Err(VpnFailure::new("socks", "SOCKS5 目标为空"));
            }
            let mut host = vec![0; usize::from(length)];
            socket.read_exact(&mut host).await?;
            String::from_utf8(host).map_err(|_| VpnFailure::new("socks", "SOCKS5 域名编码无效"))?
        }
        _ => {
            socket.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(VpnFailure::new("socks", "SOCKS5 地址类型不支持"));
        }
    };
    let port = socket.read_u16().await?;
    Ok(Request {
        host,
        port,
        success: vec![5, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        initial: vec![],
    })
}

async fn http_request(socket: &mut TcpStream) -> Result<Request, VpnFailure> {
    let mut bytes = vec![];
    let header_end = loop {
        let mut chunk = [0; 4096];
        let read = socket.read(&mut chunk).await?;
        if read == 0 {
            return Err(VpnFailure::new("http", "HTTP 代理请求不完整"));
        }
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(offset) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            if offset > 32 * 1024 {
                return Err(VpnFailure::new("http", "HTTP 请求头过大"));
            }
            break offset + 4;
        }
        if bytes.len() > 32 * 1024 {
            return Err(VpnFailure::new("http", "HTTP 请求头过大"));
        }
    };
    let head = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| VpnFailure::new("http", "HTTP 请求头编码无效"))?;
    let mut lines = head.split("\r\n");
    let words: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    if words.len() != 3 || !header_token(words[0]) || !matches!(words[2], "HTTP/1.0" | "HTTP/1.1") {
        return Err(VpnFailure::new("http", "HTTP 请求行无效"));
    }
    let connect = words[0] == "CONNECT";
    let target = if connect {
        format!("http://{}/", words[1])
    } else {
        words[1].to_owned()
    };
    let url =
        reqwest::Url::parse(&target).map_err(|_| VpnFailure::new("http", "HTTP 代理目标无效"))?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(VpnFailure::new("http", "HTTP 代理目标格式不支持"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| VpnFailure::new("http", "HTTP 代理目标没有主机名"))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| VpnFailure::new("http", "HTTP 代理目标没有端口"))?;
    if connect {
        if url.path() != "/"
            || url.query().is_some()
            || url.port().is_none() && !words[1].ends_with(":80")
        {
            return Err(VpnFailure::new("http", "CONNECT 目标应为主机和端口"));
        }
        return Ok(Request {
            host,
            port,
            success: b"HTTP/1.1 200 Connection Established\r\n\r\n".to_vec(),
            initial: bytes[header_end..].to_vec(),
        });
    }
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    };
    let mut headers = vec![];
    let mut content_length = false;
    let mut transfer_encoding = false;
    let mut hop_headers = std::collections::BTreeSet::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| VpnFailure::new("http", "HTTP 请求头格式无效"))?;
        if !header_token(name)
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(VpnFailure::new("http", "HTTP 请求头格式无效"));
        }
        let name = name.to_ascii_lowercase();
        if name == "content-length" {
            let value = value.trim();
            if content_length
                || value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || value.parse::<u64>().is_err()
            {
                return Err(VpnFailure::new("http", "HTTP 请求长度无效"));
            }
            content_length = true;
        }
        if name == "transfer-encoding" {
            if transfer_encoding || !value.trim().eq_ignore_ascii_case("chunked") {
                return Err(VpnFailure::new("http", "HTTP 传输编码无效"));
            }
            transfer_encoding = true;
        }
        if name == "connection" {
            for nominated in value.split(',').map(str::trim) {
                if !header_token(nominated)
                    || matches!(
                        nominated.to_ascii_lowercase().as_str(),
                        "host" | "content-length" | "transfer-encoding"
                    )
                {
                    return Err(VpnFailure::new("http", "HTTP Connection 字段无效"));
                }
                hop_headers.insert(nominated.to_ascii_lowercase());
            }
        }
        headers.push((name, value));
    }
    if content_length && transfer_encoding {
        return Err(VpnFailure::new("http", "HTTP 请求长度冲突"));
    }
    let mut forwarded = format!(
        "{} {path} {}\r\nHost: {}\r\nConnection: close\r\n",
        words[0],
        words[2],
        url.authority()
    );
    for (name, value) in headers {
        if matches!(
            name.as_str(),
            "host" | "connection" | "proxy-connection" | "proxy-authorization"
        ) || hop_headers.contains(&name)
        {
            continue;
        }
        forwarded.push_str(&name);
        forwarded.push(':');
        forwarded.push_str(value);
        forwarded.push_str("\r\n");
    }
    forwarded.push_str("\r\n");
    let mut initial = forwarded.into_bytes();
    initial.extend_from_slice(&bytes[header_end..]);
    Ok(Request {
        host,
        port,
        success: vec![],
        initial,
    })
}

fn header_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}
