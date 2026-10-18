#[cfg(target_os = "windows")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use std::{net::IpAddr, sync::Arc};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, UdpSocket},
        task::JoinSet,
    };
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(
        args.len() == 3,
        "expected physical interface alias and its IPv4 address"
    );
    let peer: IpAddr = args[2].parse()?;
    anyhow::ensure!(
        peer.is_ipv4() && !peer.is_loopback(),
        "physical IPv4 address required"
    );
    let mut fixtures = JoinSet::new();
    let tcp = TcpListener::bind((peer, 0)).await?;
    let tcp_address = tcp.local_addr()?;
    fixtures.spawn(async move {
        let mut clients = JoinSet::new();
        loop {
            tokio::select! {
                _ = clients.join_next(), if !clients.is_empty() => {}
                accepted = tcp.accept() => {
                    let (mut socket, _) = accepted?;
                    clients.spawn(async move {
                        let (mut input, mut output) = socket.split();
                        tokio::io::copy(&mut input, &mut output).await
                    });
                }
            }
        }
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });
    let udp = UdpSocket::bind((peer, 0)).await?;
    let udp_address = udp.local_addr()?;
    fixtures.spawn(async move {
        let mut bytes = [0; 65536];
        loop {
            let (length, client) = udp.recv_from(&mut bytes).await?;
            udp.send_to(&bytes[..length], client).await?;
        }
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });
    let dns_tcp = TcpListener::bind((peer, 0)).await?;
    let dns_address = dns_tcp.local_addr()?;
    let dns_udp = UdpSocket::bind(dns_address).await?;
    fixtures.spawn(async move {
        let mut bytes = [0; 65536];
        loop {
            let (length, client) = dns_udp.recv_from(&mut bytes).await?;
            let answer = dns_answer(&bytes[..length], peer, false)?;
            dns_udp.send_to(&answer, client).await?;
        }
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });
    fixtures.spawn(async move {
        loop {
            let (mut socket, _) = dns_tcp.accept().await?;
            let length = socket.read_u16().await? as usize;
            let mut bytes = vec![0; length];
            socket.read_exact(&mut bytes).await?;
            let answer = dns_answer(&bytes, peer, true)?;
            socket.write_u16(answer.len() as u16).await?;
            socket.write_all(&answer).await?;
        }
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });
    let certificate = rcgen::generate_simple_self_signed(vec!["provider.fixture.test".to_owned()])?;
    let public = certificate.cert.der().to_vec();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der())
                .into(),
        )?;
    let tls = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let https = TcpListener::bind((peer, 0)).await?;
    let https_port = https.local_addr()?.port();
    fixtures.spawn(async move {
        loop {
            let (socket, _) = https.accept().await?;
            if let Ok(mut socket) = tls.accept(socket).await {
                let mut header = vec![];
                while !header.ends_with(b"\r\n\r\n") {
                    anyhow::ensure!(header.len() < 16 * 1024, "header exceeded budget");
                    header.push(socket.read_u8().await?);
                }
                let body = b"payload: ['.physical.fixture.test']\n";
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await?;
                socket.write_all(body).await?;
            }
        }
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });
    let directory = tempfile::tempdir()?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        shuvpn_core::egress_probe::verify_windows(shuvpn_core::egress_probe::WindowsProbe {
            interface: args[1].clone(),
            tcp: tcp_address,
            udp: udp_address,
            dns: dns_address,
            url: format!("https://provider.fixture.test:{https_port}/rules"),
            certificate: public,
            directory: directory.path().to_string_lossy().into_owned(),
        }),
    )
    .await??;
    println!("{result}");
    fixtures.abort_all();
    while fixtures.join_next().await.is_some() {}
    Ok(())
}

#[cfg(target_os = "windows")]
fn dns_answer(bytes: &[u8], peer: std::net::IpAddr, tcp: bool) -> anyhow::Result<Vec<u8>> {
    use hickory_proto::{
        op::{Message, MessageType},
        rr::{
            rdata::{A, AAAA},
            RData, Record, RecordType,
        },
    };
    let question = Message::from_vec(bytes)?;
    let query = question
        .queries()
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing DNS question"))?;
    let mut answer = Message::new();
    answer
        .set_id(question.id())
        .set_message_type(MessageType::Response)
        .set_recursion_desired(true)
        .set_recursion_available(true)
        .add_query(query.clone());
    let name = query.name().to_ascii();
    if name.starts_with("badid.") {
        answer.set_id(question.id().wrapping_add(1));
    }
    if name.starts_with("truncated.") && !tcp {
        answer.set_truncated(true);
    } else {
        let data = match (query.query_type(), peer) {
            (RecordType::A, std::net::IpAddr::V4(peer)) => RData::A(A(peer)),
            (RecordType::AAAA, _) => RData::AAAA(AAAA(std::net::Ipv6Addr::LOCALHOST)),
            _ => anyhow::bail!("unexpected DNS query"),
        };
        answer.add_answer(Record::from_rdata(query.name().clone(), 10, data));
    }
    Ok(answer.to_vec()?)
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("Windows physical interface required");
}
