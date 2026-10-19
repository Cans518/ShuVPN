use anyhow::{ensure, Context};
use shuvpn_core::api::{
    system_proxy,
    vpn::{ProxyKind, VpnSession},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[allow(dead_code)]
#[path = "../tests/support/gateway.rs"]
mod gateway;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().context("mode required")?;
    let directory = args
        .next()
        .context("absolute recovery directory required")?;
    if mode == "recover" {
        system_proxy::recover_system_proxy(directory).await?;
        println!("SYSTEM_PROXY_RECOVERED");
        return Ok(());
    }
    let gateway = gateway::Gateway::start(if mode == "consumer" {
        gateway::Mode::HttpResponse
    } else {
        gateway::Mode::Echo
    })
    .await?;
    let session = VpnSession::connect(gateway.plan(), 3000).await?;
    let binding = session
        .start_proxy(ProxyKind::Http, "127.0.0.1".to_owned(), 0)
        .await?;
    session.enable_system_proxy(directory).await?;
    ensure!(
        system_proxy::system_proxy_uses_port(binding.port)?,
        "OS proxy does not match actual listening port"
    );
    if mode == "consumer" {
        let executable = args.next().context("KIO consumer executable required")?;
        let status = std::process::Command::new(executable)
            .arg("http://consumer.internal.invalid:443/kde-proof")
            .status()?;
        ensure!(status.success(), "KIO consumer failed");
        ensure!(
            !gateway.destinations.lock().await.is_empty(),
            "consumer did not reach campus TLS"
        );
        session.stop().await?;
        ensure!(
            !system_proxy::system_proxy_uses_port(binding.port)?,
            "consumer proxy was not restored"
        );
        println!("KIO_CONFIGURED_PROXY_CAMPUS_HTTP_RESPONSE_AND_RESTORE_PASSED");
        return Ok(());
    }
    let mut client = tokio::net::TcpStream::connect(("127.0.0.1", binding.port)).await?;
    client
        .write_all(
            b"CONNECT fixture.internal.invalid:443 HTTP/1.1\r\nHost: fixture.internal.invalid:443\r\n\r\n",
        )
        .await?;
    let mut headers = vec![];
    while !headers.ends_with(b"\r\n\r\n") {
        headers.push(client.read_u8().await?);
        ensure!(headers.len() < 4096, "HTTP header exceeded budget");
    }
    ensure!(
        headers.starts_with(b"HTTP/1.1 200"),
        "proxy CONNECT rejected"
    );
    let mut prelude = [0; 7];
    client.read_exact(&mut prelude).await?;
    ensure!(&prelude == b"prelude", "TLS fixture prelude missing");
    client.write_all(b"system-proxy-byte-proof").await?;
    let mut reply = [0; 23];
    client.read_exact(&mut reply).await?;
    ensure!(&reply == b"system-proxy-byte-proof", "proxy bytes differ");
    drop(client);
    if mode == "crash" {
        println!("SYSTEM_PROXY_CRASH_READY");
        std::future::pending::<()>().await;
    }
    match mode.as_str() {
        "stop" => session.stop().await?,
        "http-stop" => {
            session.stop_proxy(ProxyKind::Http).await?;
            session.stop().await?;
        }
        "cancel" => {
            session.cancel();
            session.stop().await?;
        }
        "external-bypass" => {
            let backend = system_proxy::system_proxy_backend()?;
            let status = match backend.as_str() {
                "gnome" => std::process::Command::new("gsettings")
                    .args([
                        "set",
                        "org.gnome.system.proxy",
                        "ignore-hosts",
                        "['external.local']",
                    ])
                    .status()?,
                "kde" => {
                    let writer = if std::path::Path::new("/usr/bin/kwriteconfig6").is_file() {
                        "/usr/bin/kwriteconfig6"
                    } else {
                        "/usr/bin/kwriteconfig5"
                    };
                    std::process::Command::new(writer)
                        .args([
                            "--file",
                            "kioslaverc",
                            "--group",
                            "Proxy Settings",
                            "--key",
                            "NoProxyFor",
                            "external.local",
                        ])
                        .status()?
                }
                _ => anyhow::bail!("external bypass probe requires GNOME/KDE"),
            };
            ensure!(status.success(), "external bypass change failed");
            ensure!(
                system_proxy::system_proxy_uses_port(binding.port)?,
                "test lost the active local endpoint"
            );
            ensure!(
                session.restore_system_proxy().await?,
                "external change was not reported"
            );
            session.stop().await?;
        }
        "external-original" | "external-original-crash" => {
            ensure!(
                system_proxy::system_proxy_backend()? == "kde",
                "requires KDE"
            );
            let original = args.next().context("original proxy endpoint required")?;
            let writer = if std::path::Path::new("/usr/bin/kwriteconfig6").is_file() {
                "/usr/bin/kwriteconfig6"
            } else {
                "/usr/bin/kwriteconfig5"
            };
            ensure!(
                std::process::Command::new(writer)
                    .args([
                        "--file",
                        "kioslaverc",
                        "--group",
                        "Proxy Settings",
                        "--key",
                        "httpProxy",
                        "--",
                        &original,
                    ])
                    .status()?
                    .success(),
                "external original endpoint update failed"
            );
            if mode == "external-original-crash" {
                println!("SYSTEM_PROXY_CRASH_READY");
                std::future::pending::<()>().await;
            }
            ensure!(
                session.restore_system_proxy().await?,
                "external original endpoint was not reported"
            );
            session.stop().await?;
        }
        _ => anyhow::bail!("unknown mode"),
    }
    ensure!(
        !system_proxy::system_proxy_uses_port(binding.port)?,
        "OS proxy was not restored"
    );
    println!("SYSTEM_PROXY_{mode}_RESTORED");
    Ok(())
}
