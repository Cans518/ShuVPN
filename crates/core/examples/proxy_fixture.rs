#[allow(dead_code)]
#[path = "../tests/support/gateway.rs"]
mod gateway;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mode = match std::env::args().nth(1).as_deref() {
        None => gateway::Mode::Echo,
        Some("dns") => gateway::Mode::Dns(gateway::DnsMode::Valid),
        Some("domain-dns") => gateway::Mode::DomainDns(gateway::DnsMode::Mapping),
        Some("udp") => gateway::Mode::L3Echo,
        Some(argument) => anyhow::bail!("Unknown fixture mode: {argument}"),
    };
    let mut gateway = gateway::Gateway::start(mode).await?;
    println!("{}", gateway.plan());
    // EOF is the parent test's shutdown signal.
    tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    })
    .await?;
    gateway.finish().await?;
    for error in gateway.errors.lock().await.iter() {
        eprintln!("FIXTURE_CONNECTION_ERROR {error}");
    }
    Ok(())
}
