use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use rustls::pki_types::PrivatePkcs8KeyDer;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::{JoinHandle, JoinSet};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

#[path = "gateway_trace.rs"]
mod trace;

pub struct Gateway {
    pub address: String,
    pub digest: String,
    pub destinations: Arc<Mutex<Vec<String>>>,
    pub connections: Arc<AtomicUsize>,
    pub uploaded: Arc<AtomicUsize>,
    pub dns_queries: Arc<Mutex<Vec<String>>>,
    pub errors: Arc<Mutex<Vec<String>>>,
    mode: Mode,
    task: JoinHandle<()>,
    shutdown: tokio_util::sync::CancellationToken,
}

#[derive(Clone, Copy)]
pub enum Mode {
    Echo,
    HttpResponse,
    NoHeartbeat,
    RefuseTcp,
    SecondVip,
    HalfClose,
    HoldHandshake,
    L3Echo,
    L3Ipv4Echo,
    L3Refuse,
    L3BadHash,
    L3WrongPacket,
    Dns(DnsMode),
    DomainDns(DnsMode),
    MixedDomainDns(DnsMode),
}

#[derive(Clone, Copy)]
pub enum DnsMode {
    Valid,
    Mapping,
    FailA,
    FailAaaa,
    HangAaaa,
    Unrelated,
    BadId,
    ZeroTtl,
}
impl Drop for Gateway {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Gateway {
    pub async fn start(mode: Mode) -> anyhow::Result<Self> {
        Self::start_on(mode, "127.0.0.1".parse()?).await
    }

    pub async fn start_on(mode: Mode, host: std::net::IpAddr) -> anyhow::Result<Self> {
        let certificate = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])?;
        let digest = sangfor_core::crypto::certificate_digest(certificate.cert.der().as_ref());
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der()).into(),
            )?;
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind((host, 0)).await?;
        let address = listener.local_addr()?.to_string();
        let destinations = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let uploaded = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&destinations);
        let count = Arc::clone(&connections);
        let uploads = Arc::clone(&uploaded);
        let dns_queries = Arc::new(Mutex::new(Vec::new()));
        let queries = Arc::clone(&dns_queries);
        let errors = Arc::new(Mutex::new(Vec::new()));
        let failures = Arc::clone(&errors);
        let shutdown = tokio_util::sync::CancellationToken::new();
        let stop = shutdown.clone();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break };
                        let acceptor = acceptor.clone();
                        let recorded = Arc::clone(&recorded);
                        let count = Arc::clone(&count);
                        let uploads = Arc::clone(&uploads);
                        let queries = Arc::clone(&queries);
                        let failures = Arc::clone(&failures);
                        clients.spawn(async move {
                            count.fetch_add(1, Ordering::Relaxed);
                            // A rejected pin closes TLS before the app protocol.
                            if let Ok(stream) = acceptor.accept(socket).await {
                                if let Err(error) = gateway_connection(stream, mode, recorded, uploads, queries).await {
                                    failures.lock().await.push(error.to_string());
                                }
                            }
                            count.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Some(_) = clients.join_next(), if !clients.is_empty() => {}
                }
            }
            clients.shutdown().await;
        });
        Ok(Self {
            address,
            digest,
            destinations,
            connections,
            uploaded,
            dns_queries,
            errors,
            mode,
            task,
            shutdown,
        })
    }

    pub async fn finish(&mut self) -> anyhow::Result<()> {
        self.shutdown.cancel();
        (&mut self.task).await?;
        Ok(())
    }

    pub fn plan(&self) -> String {
        let mut plan = json!({
            "schemaVersion":1, "sid":"test-sid", "deviceId":"test-device",
            "connectionId":"test-connection", "username":"test-account",
            "signKeyBase64":STANDARD.encode((1..=32).collect::<Vec<u8>>()),
            "lang":"en-US","processName":"ShuVPN","processPath":"/test/shuvpn","processPlatform":"Linux",
            "nodes":{"group":[self.address]}, "majorNodeGroup":"group",
            "routes":[{"host":"*.internal.invalid","protocol":"tcp","portMin":1,"portMax":65535,
                "appId":"test-app","nodeGroupId":"group","addrPretend":true,"enableTcpPrefL3":true}],
            "dnsServers":["10.99.0.53"],"virtualAddress":"10.1.1.1",
            "certificateDigests":[self.digest],"acceptAnyCertificate":false,
            "heartbeatSeconds":0.05,"mtu":1400
        });
        if matches!(
            self.mode,
            Mode::Dns(_) | Mode::DomainDns(_) | Mode::MixedDomainDns(_)
        ) {
            if let Some(routes) = plan["routes"].as_array_mut() {
                routes.push(json!({"host":"10.99.0.53","protocol":"tcp","portMin":53,"portMax":53,
                    "appId":"test-app","nodeGroupId":"group","addrPretend":false,"enableTcpPrefL3":false}));
                if matches!(self.mode, Mode::DomainDns(_) | Mode::MixedDomainDns(_)) {
                    routes.push(json!({"host":"fixture-dns.shu.edu.cn","protocol":"tcp","portMin":443,"portMax":443,
                        "appId":"test-app","nodeGroupId":"group","addrPretend":true,"enableTcpPrefL3":false}));
                    routes.push(json!({"host":"fixture-dns.shu.edu.cn","protocol":"udp","portMin":9001,"portMax":9001,
                        "appId":"test-app","nodeGroupId":"group","addrPretend":true,"enableTcpPrefL3":false}));
                } else {
                    routes.push(json!({"host":"10.99.0.0/24","protocol":"tcp","portMin":443,"portMax":443,
                        "appId":"test-app","nodeGroupId":"group","addrPretend":false,"enableTcpPrefL3":false}));
                    routes.push(json!({"host":"2001:db8::/32","protocol":"tcp","portMin":443,"portMax":443,
                        "appId":"test-app","nodeGroupId":"group","addrPretend":false,"enableTcpPrefL3":false}));
                }
            }
        }
        if matches!(
            self.mode,
            Mode::L3Echo
                | Mode::L3Ipv4Echo
                | Mode::L3Refuse
                | Mode::L3BadHash
                | Mode::L3WrongPacket
                | Mode::MixedDomainDns(_)
        ) {
            if let Some(routes) = plan["routes"].as_array_mut() {
                for host in ["10.21.0.0/16", "2001:db8:21::/64"] {
                    routes.push(json!({"host":host,"protocol":"udp","portMin":1,"portMax":65535,
                        "appId":"test-app","nodeGroupId":"group","addrPretend":false,"enableTcpPrefL3":false}));
                    routes.push(json!({"host":host,"protocol":"tcp","portMin":1,"portMax":65535,
                        "appId":"test-app","nodeGroupId":"group","addrPretend":false,"enableTcpPrefL3":false}));
                }
            }
        }
        plan.to_string()
    }
}

async fn gateway_connection(
    mut stream: TlsStream<TcpStream>,
    mode: Mode,
    destinations: Arc<Mutex<Vec<String>>>,
    uploaded: Arc<AtomicUsize>,
    dns_queries: Arc<Mutex<Vec<String>>>,
) -> anyhow::Result<()> {
    let mut opening = [0; 7];
    stream.read_exact(&mut opening).await?;
    let length = usize::from(u16::from_be_bytes([opening[5], opening[6]]));
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await?;
    let auth: serde_json::Value = serde_json::from_slice(&body)?;
    anyhow::ensure!(auth["sid"] == "test-sid", "session ID was not passed");
    if opening[2] == 0xd0 {
        let mut trailer = [0; 10];
        stream.read_exact(&mut trailer).await?;
        anyhow::ensure!(trailer == [5, 4, 0, 1, 0, 0, 0, 0, 0, 0], "bad VIP trailer");
        let payload = br#"{"code":0,"data":{"deviceId":"test-device"}}"#;
        let mut response = [
            vec![5, 0xd0, 0x53, 0, 0, payload.len() as u8],
            payload.to_vec(),
            vec![5, 0, 0, 1, 10, 99, 0, 2, 0, 0],
        ]
        .concat();
        let is_l3 = matches!(
            mode,
            Mode::L3Echo
                | Mode::L3Ipv4Echo
                | Mode::L3Refuse
                | Mode::L3BadHash
                | Mode::L3WrongPacket
                | Mode::DomainDns(_)
                | Mode::MixedDomainDns(_)
        );
        if is_l3 || matches!(mode, Mode::DomainDns(_)) {
            response.truncate(response.len() - 10);
            response.extend_from_slice(&[
                5,
                0,
                0,
                if matches!(mode, Mode::L3Ipv4Echo) {
                    1
                } else {
                    5
                },
                10,
                99,
                0,
                2,
            ]);
            if !matches!(mode, Mode::L3Ipv4Echo) {
                response.extend_from_slice(&"2001:db8::2".parse::<std::net::Ipv6Addr>()?.octets());
            }
            response.extend_from_slice(&[0, 0]);
        }
        if matches!(mode, Mode::HoldHandshake) {
            let mut byte = [0; 1];
            let _ = stream.read(&mut byte).await;
            return Ok(());
        }
        if matches!(mode, Mode::SecondVip) {
            let vip = br#"{"vip":"10.99.0.3"}"#;
            response.extend_from_slice(&[5, 0x96, 0, 0, vip.len() as u8]);
            response.extend_from_slice(vip);
            stream.write_all(&response).await?;
            stream.flush().await?;
        } else {
            // Every split position can occur on the real transport.
            for part in response.chunks(3) {
                stream.write_all(part).await?;
                stream.flush().await?;
            }
        }
        if is_l3 {
            return l3_connection(stream, mode, destinations, uploaded).await;
        }
        loop {
            let mut request = [0; 4];
            if stream.read_exact(&mut request).await.is_err() {
                break;
            }
            anyhow::ensure!(request == [5, 0x15, 0, 0], "unexpected control frame");
            if !matches!(mode, Mode::NoHeartbeat) {
                stream.write_all(&[5, 0x95, 0, 0]).await?;
                stream.flush().await?;
            }
        }
        return Ok(());
    }
    anyhow::ensure!(opening[..5] == [5, 1, 0x81, 0x53, 3], "bad TCP header");
    let body_text = std::str::from_utf8(&body)?;
    let signature_offset = body_text
        .rfind(",\"xRequestSig\":")
        .ok_or_else(|| anyhow::anyhow!("missing signature"))?;
    let unsigned = format!("{}}}", &body_text[..signature_offset]);
    let expected = sangfor_core::crypto::request_signature(
        &(1..=32).collect::<Vec<u8>>(),
        unsigned.as_bytes(),
    );
    anyhow::ensure!(auth["xRequestSig"] == expected, "signature mismatch");
    anyhow::ensure!(auth["appId"] == "test-app", "wrong routed resource");
    let mut destination = [0; 4];
    stream.read_exact(&mut destination).await?;
    anyhow::ensure!(destination[..3] == [5, 1, 0], "unexpected zero-RTT request");
    let host = match destination[3] {
        3 => {
            let length = stream.read_u8().await?;
            let mut host = vec![0; usize::from(length)];
            stream.read_exact(&mut host).await?;
            String::from_utf8(host)?
        }
        1 => {
            let mut address = [0; 4];
            stream.read_exact(&mut address).await?;
            std::net::Ipv4Addr::from(address).to_string()
        }
        4 => {
            let mut address = [0; 16];
            stream.read_exact(&mut address).await?;
            std::net::Ipv6Addr::from(address).to_string()
        }
        _ => anyhow::bail!("unexpected destination address family"),
    };
    let port = stream.read_u16().await?;
    let signed_address = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    anyhow::ensure!(
        auth["destAddr"] == signed_address,
        "wrong signed destination"
    );
    if matches!(mode, Mode::DomainDns(_) | Mode::MixedDomainDns(_)) && port == 443 {
        anyhow::ensure!(
            destination[3] == 3 && host == "fixture-dns.shu.edu.cn",
            "domain-only packet flow lost its resource name"
        );
        anyhow::ensure!(
            matches!(auth["destIP"].as_str(), Some("10.99.0.7" | "2001:db8::7")),
            "domain-only packet flow lost its original IP"
        );
    }
    destinations.lock().await.push(host);
    let reject_tcp = matches!(mode, Mode::RefuseTcp);
    let payload = if reject_tcp {
        br#"{"code":9}"#.as_slice()
    } else {
        br#"{"code":0}"#.as_slice()
    };
    let mut response = [
        vec![5, 0x81, 0x53, 0, 0, payload.len() as u8],
        payload.to_vec(),
        vec![5, 0, 1, 1, 127, 0, 0, 1, 0, 0],
    ]
    .concat();
    if !(reject_tcp
        || matches!(mode, Mode::HttpResponse)
        || port == 53
            && matches!(
                mode,
                Mode::Dns(_) | Mode::DomainDns(_) | Mode::MixedDomainDns(_)
            ))
    {
        response.extend_from_slice(b"prelude");
    }
    stream.write_all(&response).await?;
    stream.flush().await?;
    if reject_tcp {
        return Ok(());
    }
    if let Mode::Dns(dns_mode) | Mode::DomainDns(dns_mode) | Mode::MixedDomainDns(dns_mode) = mode {
        if port == 53 {
            return serve_dns(stream, dns_mode, dns_queries).await;
        }
    }
    if matches!(mode, Mode::HttpResponse) {
        let mut request = vec![];
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await?);
            anyhow::ensure!(
                request.len() < 16 * 1024,
                "fixture HTTP headers exceeded budget"
            );
        }
        anyhow::ensure!(
            request.starts_with(b"GET /"),
            "fixture expected origin-form GET"
        );
        let body = b"kde-proxy-response";
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len(),
                )
                .as_bytes(),
            )
            .await?;
        stream.write_all(body).await?;
        stream.shutdown().await?;
        return Ok(());
    }
    if matches!(mode, Mode::HalfClose) {
        stream.shutdown().await?;
    }
    let mut chunk = [0; 16 * 1024];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        uploaded.fetch_add(read, Ordering::Relaxed);
        if !matches!(mode, Mode::HalfClose) {
            stream.write_all(&chunk[..read]).await?;
            stream.flush().await?;
        }
    }
    stream.shutdown().await?;
    // Drain the peer's TCP FIN after its TLS close_notify before dropping the
    // transport; otherwise a host TCP reset can discard the final TLS alert.
    let mut trailing = [0; 1];
    let eof = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        stream.get_mut().0.read(&mut trailing),
    )
    .await??;
    anyhow::ensure!(eof == 0, "unexpected TCP bytes after TLS close_notify");
    Ok(())
}

async fn l3_connection(
    mut stream: TlsStream<TcpStream>,
    mode: Mode,
    destinations: Arc<Mutex<Vec<String>>>,
    uploaded: Arc<AtomicUsize>,
) -> anyhow::Result<()> {
    let mut heartbeat_trace =
        if std::env::var("SHUVPN_FIXTURE_HEARTBEAT_TRACE").as_deref() == Ok("1") {
            Some(trace::HeartbeatTrace::new(stream.get_ref().0.peer_addr()?))
        } else {
            None
        };
    let mut authorized = None;
    loop {
        let mut header = [0; 2];
        if let Err(error) = stream.read_exact(&mut header).await {
            if let Some(trace) = &mut heartbeat_trace {
                trace.read_end(&error);
            }
            return Ok(());
        }
        anyhow::ensure!(header[0] == 5, "bad L3 version");
        match header[1] {
            0x15 => {
                anyhow::ensure!(stream.read_u16().await? == 0, "heartbeat has payload");
                if let Some(trace) = &mut heartbeat_trace {
                    trace.request();
                }
                stream.write_all(&[5, 0x95, 0, 0]).await?;
            }
            0x13 => {
                let length = usize::from(stream.read_u16().await?);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).await?;
                let auth: serde_json::Value = serde_json::from_slice(&body)?;
                let text = std::str::from_utf8(&body)?;
                let offset = text
                    .rfind(",\"xRequestSig\":")
                    .ok_or_else(|| anyhow::anyhow!("missing L3 signature"))?;
                let unsigned = format!("{}}}", &text[..offset]);
                let expected = sangfor_core::crypto::request_signature(
                    &(1..=32).collect::<Vec<u8>>(),
                    unsigned.as_bytes(),
                );
                anyhow::ensure!(auth["xRequestSig"] == expected, "bad L3 signature");
                anyhow::ensure!(
                    auth["appId"] == "test-app" && auth["sid"] == "test-sid",
                    "wrong L3 identity"
                );
                anyhow::ensure!(
                    auth["ip"]["protocol"] == 17 && auth["ip"]["atype"] == 0x0800,
                    "wrong UDP tuple"
                );
                anyhow::ensure!(auth["ip"]["srcAddr"] == "10.99.0.2", "wrong virtual source");
                if matches!(mode, Mode::DomainDns(_))
                    || matches!(mode, Mode::MixedDomainDns(_))
                        && auth["ip"]["destAddr"] == "10.99.0.7"
                {
                    anyhow::ensure!(
                        auth["domain"] == "fixture-dns.shu.edu.cn"
                            && auth["ip"]["destAddr"] == "10.99.0.7",
                        "UDP domain identity missing"
                    );
                } else if matches!(mode, Mode::MixedDomainDns(_)) {
                    anyhow::ensure!(
                        auth["ip"]["destAddr"] == "10.21.0.5",
                        "unexpected mixed-fixture campus UDP target"
                    );
                }
                let destination = auth["ip"]["destAddr"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing target"))?;
                let port = auth["ip"]["destPort"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("missing port"))?;
                anyhow::ensure!(
                    auth["url"] == format!("udp:{destination}:{port}"),
                    "wrong UDP URL"
                );
                destinations.lock().await.push(destination.to_owned());
                let id = auth["conntrackHash"]
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("missing hash"))?;
                let reply = json!({"code":0,"data":{
                    "code":if matches!(mode, Mode::L3Refuse) { 42 } else { 0 },
                    "conntrackHash":if matches!(mode, Mode::L3BadHash) { id + 1 } else { id },
                    "connectToken":"udp-fixture-token"
                }})
                .to_string()
                .into_bytes();
                stream.write_all(&[5, 0x93, 0]).await?;
                stream
                    .write_all(&(reply.len() as u16).to_be_bytes())
                    .await?;
                stream.write_all(&reply).await?;
                if !matches!(mode, Mode::L3Refuse | Mode::L3BadHash) {
                    authorized = Some(auth);
                }
            }
            0x14 => {
                let token_length = usize::from(stream.read_u8().await?);
                let mut token = vec![0; token_length];
                stream.read_exact(&mut token).await?;
                anyhow::ensure!(
                    token == b"udp-fixture-token" && authorized.is_some(),
                    "packet sent without authorization"
                );
                let mut trailer = [0; 3];
                stream.read_exact(&mut trailer).await?;
                anyhow::ensure!(trailer == [0, 0, 1], "bad data trailer");
                let length = usize::from(stream.read_u16().await?);
                let mut packet = vec![0; length];
                stream.read_exact(&mut packet).await?;
                anyhow::ensure!(
                    packet.len() >= 28 && packet[0] == 0x45 && packet[9] == 17,
                    "bad UDP packet"
                );
                uploaded.fetch_add(packet.len(), Ordering::Relaxed);
                let source: [u8; 4] = packet[12..16].try_into()?;
                let destination: [u8; 4] = packet[16..20].try_into()?;
                packet[12..16].copy_from_slice(&destination);
                packet[16..20].copy_from_slice(&source);
                packet.swap(20, 22);
                packet.swap(21, 23);
                if matches!(mode, Mode::L3WrongPacket) {
                    packet[16] ^= 1;
                }
                packet[10..12].fill(0);
                let checksum = ip_checksum(&packet[..20]);
                packet[10..12].copy_from_slice(&checksum.to_be_bytes());
                // IPv4 UDP permits zero checksum; this also avoids relying on
                // the application's reply encoder in a path-verification test.
                packet[26..28].fill(0);
                let mut response = vec![5, 0x94];
                response.extend_from_slice(&(packet.len() as u16).to_be_bytes());
                response.extend_from_slice(&packet);
                for chunk in response.chunks(7) {
                    stream.write_all(chunk).await?;
                }
            }
            _ => anyhow::bail!("unexpected L3 command"),
        }
        stream.flush().await?;
        if header[1] == 0x15 {
            if let Some(trace) = &mut heartbeat_trace {
                trace.response_flushed();
            }
        }
    }
}

fn ip_checksum(bytes: &[u8]) -> u16 {
    let mut sum = bytes.chunks(2).fold(0u32, |sum, pair| {
        sum + u32::from(u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)]))
    });
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

async fn serve_dns(
    mut stream: TlsStream<TcpStream>,
    mode: DnsMode,
    queries: Arc<Mutex<Vec<String>>>,
) -> anyhow::Result<()> {
    use hickory_proto::{
        op::{Message, MessageType, ResponseCode},
        rr::{
            rdata::{A, AAAA, CNAME},
            Name, RData, Record, RecordType,
        },
    };
    let length = stream.read_u16().await?;
    let mut bytes = vec![0; usize::from(length)];
    stream.read_exact(&mut bytes).await?;
    let query = Message::from_vec(&bytes)?;
    let question = query
        .queries()
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing DNS question"))?;
    queries
        .lock()
        .await
        .push(format!("{}:{}", question.name(), question.query_type()));
    if matches!(mode, DnsMode::HangAaaa) && question.query_type() == RecordType::AAAA {
        let mut buffer = [0; 16];
        let _ = stream.read(&mut buffer).await;
        return Ok(());
    }
    let mut response = query.clone();
    response.set_message_type(MessageType::Response);
    if matches!(mode, DnsMode::BadId) {
        response.set_id(query.id().wrapping_add(1));
    }
    if (matches!(mode, DnsMode::FailAaaa) && question.query_type() == RecordType::AAAA)
        || (matches!(mode, DnsMode::FailA) && question.query_type() == RecordType::A)
    {
        response.set_response_code(ResponseCode::ServFail);
    } else {
        let alias = Name::from_ascii("resolved.shu.edu.cn.")?;
        let owner = if matches!(mode, DnsMode::Unrelated) {
            Name::from_ascii("unrelated.invalid.")?
        } else {
            alias.clone()
        };
        let ttl = if matches!(mode, DnsMode::ZeroTtl) {
            0
        } else if matches!(mode, DnsMode::Mapping) {
            3
        } else {
            1
        };
        if !matches!(mode, DnsMode::Unrelated) {
            response.add_answer(Record::from_rdata(
                question.name().clone(),
                ttl,
                RData::CNAME(CNAME(alias)),
            ));
        }
        let record = match question.query_type() {
            RecordType::A => RData::A(A("10.99.0.7".parse()?)),
            RecordType::AAAA => RData::AAAA(AAAA("2001:db8::7".parse()?)),
            _ => anyhow::bail!("unexpected DNS question"),
        };
        response.add_answer(Record::from_rdata(owner, 10, record));
    }
    let bytes = response.to_vec()?;
    stream.write_u16(bytes.len() as u16).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}
