use super::Egress;
use crate::connection::model::VpnFailure;
use reqwest::{
    dns::{Addrs, Name, Resolve, Resolving},
    ClientBuilder, Url,
};
use std::sync::Arc;
use tokio::sync::OwnedRwLockReadGuard;

pub(crate) struct Lease {
    gate: Arc<OwnedRwLockReadGuard<()>>,
    egress: Option<Arc<Egress>>,
    host: String,
    addresses: Option<Vec<std::net::IpAddr>>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    tunnel: Option<super::http_tunnel::Tunnel>,
}
impl Lease {
    pub(crate) async fn open(url: &Url) -> Result<Self, VpnFailure> {
        let gate = super::read_gate().await;
        let egress = super::current()?;
        let host = url
            .host_str()
            .ok_or_else(|| VpnFailure::new("url", "HTTP 请求缺少主机"))?
            .trim_matches(['[', ']'])
            .to_owned();
        let addresses = if let Some(egress) = &egress {
            Some(super::dns::resolve_with(Some(egress), &host, None).await?)
        } else {
            None
        };
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let tunnel = if let (Some(egress), Some(addresses)) = (&egress, &addresses) {
            Some(
                super::http_tunnel::Tunnel::open(url, addresses.clone(), Arc::clone(egress))
                    .await?,
            )
        } else {
            None
        };
        Ok(Self {
            gate,
            egress,
            host,
            addresses,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            tunnel,
        })
    }
    pub(crate) fn builder(&self) -> ClientBuilder {
        let builder = reqwest::Client::builder()
            .no_proxy()
            .pool_max_idle_per_host(0)
            .connect_timeout(std::time::Duration::from_secs(5))
            .dns_resolver(Arc::new(Resolver {
                gate: Arc::clone(&self.gate),
                egress: self.egress.clone(),
                host: self.host.clone(),
                addresses: self.addresses.clone(),
            }));
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let builder = if let Some(tunnel) = &self.tunnel {
            builder.proxy(tunnel.proxy.clone())
        } else {
            builder
        };
        builder
    }
}
struct Resolver {
    gate: Arc<OwnedRwLockReadGuard<()>>,
    egress: Option<Arc<Egress>>,
    host: String,
    addresses: Option<Vec<std::net::IpAddr>>,
}
impl Resolve for Resolver {
    fn resolve(&self, name: Name) -> Resolving {
        let gate = Arc::clone(&self.gate);
        let egress = self.egress.clone();
        let name = name.as_str().to_owned();
        let pinned = if name == self.host {
            self.addresses.clone()
        } else {
            None
        };
        Box::pin(async move {
            let addresses = match pinned {
                Some(addresses) => addresses,
                None => super::dns::resolve_with(egress.as_deref(), &name, Some(gate))
                    .await
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?,
            };
            Ok(Box::new(
                addresses
                    .into_iter()
                    .map(|address| std::net::SocketAddr::new(address, 0)),
            ) as Addrs)
        })
    }
}

pub(crate) struct Response {
    pub inner: reqwest::Response,
    pub _lease: Lease,
    pub _client: reqwest::Client,
}
impl std::ops::Deref for Response {
    type Target = reqwest::Response;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Response {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        sync::{Notify, RwLock},
        time::{timeout, Duration},
    };

    #[tokio::test]
    async fn http_response_retains_the_transition_gate_until_body_and_owner_are_released(
    ) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let release = Arc::new(Notify::new());
        let release_server = Arc::clone(&release);
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            let mut headers = vec![];
            while !headers.ends_with(b"\r\n\r\n") {
                headers.push(socket.read_u8().await?);
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n")
                .await?;
            release_server.notified().await;
            socket.write_all(b"body").await?;
            Ok::<_, std::io::Error>(())
        });
        // A private gate isolates this lifecycle test from parallel global
        // authentication/download tests. It is the same owned guard type.
        let gate = Arc::new(RwLock::new(()));
        let lease = Lease {
            gate: Arc::new(gate.clone().read_owned().await),
            egress: None,
            host: "127.0.0.1".to_owned(),
            addresses: None,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            tunnel: None,
        };
        let client = lease.builder().no_proxy().build()?;
        let inner = client.get(format!("http://{address}/")).send().await?;
        let mut response = Response {
            inner,
            _lease: lease,
            _client: client,
        };
        let writing = gate.clone().write_owned();
        tokio::pin!(writing);
        assert!(timeout(Duration::from_millis(20), &mut writing)
            .await
            .is_err());
        release.notify_one();
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await? {
            bytes.extend_from_slice(&chunk);
        }
        assert_eq!(bytes, b"body");
        drop(response);
        timeout(Duration::from_secs(2), &mut writing).await?;
        server.await??;
        Ok(())
    }
}
