use super::model::Block;
use super::*;
use anyhow::{ensure, Context};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};

struct Fixture {
    client: Arc<Client>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
    hits: Arc<AtomicUsize>,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    job: tokio::task::JoinHandle<()>,
    origin: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.job.abort();
    }
}
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let certificate = rcgen::generate_simple_self_signed(vec!["newits.fixture.test".into()])?;
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der())
                    .into(),
            )?;
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let mut client = Client::new();
        client.fixture = Some((certificate.cert.der().to_vec(), address));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let hits = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let observed = entered.clone();
        let resumed = release.clone();
        let counter = hits.clone();
        let active_count = active.clone();
        let maximum = peak.clone();
        let job = tokio::spawn(async move {
            let mut peers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = peers.join_next(), if !peers.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((stream,_)) = accepted else { break; };
                        let acceptor = acceptor.clone();
                        let observed = observed.clone();
                        let resumed = resumed.clone();
                        let counter = counter.clone();
                        let active_count = active_count.clone();
                        let maximum = maximum.clone();
                        peers.spawn(async move {
                            let mut stream = acceptor.accept(stream).await?;
                            let mut header = Vec::new();
                            while !header.ends_with(b"\r\n\r\n") { header.push(stream.read_u8().await?); }
                            let request = String::from_utf8(header)?;
                            let path = request.split_whitespace().nth(1).context("no target")?;
                            counter.fetch_add(1,Ordering::Relaxed);
                            let active = active_count.fetch_add(1, Ordering::SeqCst) + 1;
                            maximum.fetch_max(active, Ordering::SeqCst);
                            let _active = Active(active_count);
                            let mut status = "200 OK";
                            let mut extra = "";
                            let body: Vec<u8> = match path {
                                "/list" => include_bytes!("../../tests/fixtures/announcements/list.html").to_vec(),
                                "/old/list" => { status="302 Found"; extra="Location: /new/list\r\n"; vec![] },
                                "/new/list" => b"<li id='line_u4_0'><a href='detail'>item</a></li><span class='p_next'><a href='2.htm'>next</a></span>".to_vec(),
                                "/empty-next" => b"<span class='p_next'><a href='2.htm'>next</a></span>".to_vec(),
                                "/self-next" => b"<li id='line_u4_0'><a href='detail'>item</a></li><span class='p_next'><a href='/self-next'>next</a></span>".to_vec(),
                                "/external-page" => b"<span class='p_next'><a href='https://foreign.example/next'>next</a></span>".to_vec(),
                                "/old/detail" => { status="302 Found"; extra="Location: /new/detail\r\n"; vec![] },
                                "/new/detail" => b"<div class='v_news_content'><p><a href='next.htm'>next</a></p><img src='image.png'></div>".to_vec(),
                                path if path.starts_with("/slow") => {
                                    observed.notify_one();
                                    tokio::select! {
                                        _ = resumed.notified() => {},
                                        _ = stream.read_u8() => return Ok::<_, anyhow::Error>(()),
                                    }
                                    include_bytes!("../../tests/fixtures/announcements/detail.html").to_vec()
                                },
                                "/foreign" => { status="302 Found"; extra="Location: https://external.example/steal\r\n"; vec![] },
                                "/missing" => { status="404 Missing"; vec![] },
                                _ => include_bytes!("../../tests/fixtures/announcements/detail.html").to_vec(),
                            };
                            stream.write_all(format!("HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await?;
                            let _ = stream.write_all(&body).await;
                            let _ = stream.shutdown().await;
                            Ok::<_,anyhow::Error>(())
                        });
                    }
                }
            }
        });
        Ok(Self {
            client: Arc::new(client),
            entered,
            release,
            hits,
            active,
            peak,
            job,
            origin: format!("https://newits.fixture.test:{}", address.port()),
        })
    }
    fn item(&self, path: &str) -> Item {
        Item {
            title: "fallback".into(),
            url: format!("{}{path}", self.origin),
            date_text: String::new(),
            published_at: None,
        }
    }
}

struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn prefetch_filters_deduplicates_reuses_cache_and_reports_individual_failure(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let cached = fixture.item("/cached");
    fixture.client.detail(cached.clone()).await?;
    let before = fixture.hits.load(Ordering::SeqCst);
    let alias = Item {
        url: cached
            .url
            .replace("newits.fixture.test", "NEWITS.fixture.test"),
        ..cached.clone()
    };
    let foreign = Item {
        url: "https://foreign.invalid/detail".into(),
        ..cached.clone()
    };
    let batch = prefetch::Prefetch::new(
        fixture.client.clone(),
        vec![
            cached.clone(),
            cached,
            alias,
            foreign,
            fixture.item("/missing"),
        ],
    )?;
    let mut previews = vec![];
    batch
        .run(|preview| {
            previews.push(preview);
            true
        })
        .await?;
    ensure!(previews.len() == 3);
    ensure!(
        previews
            .iter()
            .filter(|preview| preview.text.is_some())
            .count()
            == 2
    );
    ensure!(
        previews
            .iter()
            .filter(|preview| preview.error.is_some())
            .count()
            == 1
    );
    ensure!(fixture.hits.load(Ordering::SeqCst) == before + 1);
    ensure!(batch
        .run(|_| true)
        .await
        .is_err_and(|error| error.code == "state"));
    Ok(())
}

#[tokio::test]
async fn concurrent_batches_share_three_slots_and_cancel_active_and_queued_requests(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let first = prefetch::Prefetch::new(
        fixture.client.clone(),
        (0..5)
            .map(|i| fixture.item(&format!("/slow/{i}")))
            .collect(),
    )?;
    let second = prefetch::Prefetch::new(
        fixture.client.clone(),
        (5..10)
            .map(|i| fixture.item(&format!("/slow/{i}")))
            .collect(),
    )?;
    let deliveries = Arc::new(AtomicUsize::new(0));
    let start = |batch: Arc<prefetch::Prefetch>| {
        let deliveries = deliveries.clone();
        tokio::spawn(async move {
            batch
                .run(|_| {
                    deliveries.fetch_add(1, Ordering::SeqCst);
                    true
                })
                .await
        })
    };
    let one = start(first.clone());
    let two = start(second.clone());
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.hits.load(Ordering::SeqCst) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    ensure!(fixture.peak.load(Ordering::SeqCst) == 3);
    first.cancel();
    second.cancel();
    tokio::time::timeout(Duration::from_secs(3), async {
        one.await??;
        two.await??;
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.active.load(Ordering::SeqCst) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    ensure!(fixture.hits.load(Ordering::SeqCst) == 3);
    ensure!(deliveries.load(Ordering::SeqCst) == 0);
    fixture.client.detail(fixture.item("/after-cancel")).await?;
    Ok(())
}

#[tokio::test]
async fn preview_cancellation_does_not_cancel_a_foreground_detail_and_shared_url_uses_one_request(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let foreground = tokio::spawn({
        let client = fixture.client.clone();
        let item = fixture.item("/slow/shared");
        async move { client.detail(item).await }
    });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    let batch =
        prefetch::Prefetch::new(fixture.client.clone(), vec![fixture.item("/slow/shared")])?;
    let running = tokio::spawn({
        let batch = batch.clone();
        async move { batch.run(|_| true).await }
    });
    tokio::task::yield_now().await;
    batch.cancel();
    tokio::time::timeout(Duration::from_secs(2), running).await???;
    fixture.release.notify_one();
    foreground.await??;
    ensure!(fixture.hits.load(Ordering::SeqCst) == 1);
    let cached =
        prefetch::Prefetch::new(fixture.client.clone(), vec![fixture.item("/slow/shared")])?;
    let mut count = 0;
    cached
        .run(|_| {
            count += 1;
            true
        })
        .await?;
    ensure!(count == 1 && fixture.hits.load(Ordering::SeqCst) == 1);
    Ok(())
}

#[tokio::test]
async fn cancelling_one_batch_releases_slots_for_the_surviving_batch() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let first = prefetch::Prefetch::new(
        fixture.client.clone(),
        (0..5)
            .map(|i| fixture.item(&format!("/slow/{i}")))
            .collect(),
    )?;
    let first_job = tokio::spawn({
        let first = first.clone();
        async move { first.run(|_| true).await }
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.hits.load(Ordering::SeqCst) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    let second = prefetch::Prefetch::new(
        fixture.client.clone(),
        (0..4)
            .map(|i| fixture.item(&format!("/live/{i}")))
            .collect(),
    )?;
    let second_job = tokio::spawn(async move {
        let mut delivered = 0;
        second
            .run(|preview| {
                if preview.text.is_some() {
                    delivered += 1;
                }
                true
            })
            .await?;
        Ok::<_, AnnouncementFailure>(delivered)
    });
    first.cancel();
    first_job.await??;
    ensure!(tokio::time::timeout(Duration::from_secs(3), second_job).await??? == 4);
    ensure!(fixture.hits.load(Ordering::SeqCst) == 7);
    ensure!(fixture.client.preview_slots.available_permits() == 3);
    Ok(())
}

#[tokio::test]
async fn foreground_takes_over_when_the_prefetch_holding_its_url_gate_is_cancelled(
) -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let item = fixture.item("/slow/handoff");
    let batch = prefetch::Prefetch::new(fixture.client.clone(), vec![item.clone()])?;
    let job = tokio::spawn({
        let batch = batch.clone();
        async move { batch.run(|_| true).await }
    });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    let foreground = tokio::spawn({
        let client = fixture.client.clone();
        let item = item.clone();
        async move { client.detail(item).await }
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting = fixture
                .client
                .details
                .lock()
                .ok()
                .and_then(|pending| pending.get(&item.url).map(Weak::strong_count))
                .unwrap_or(0);
            if waiting >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    batch.cancel();
    job.await??;
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.active.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    fixture.release.notify_one();
    foreground.await??;
    ensure!(fixture.hits.load(Ordering::SeqCst) == 2);
    ensure!(fixture.client.cached(item.url)?.is_some());
    Ok(())
}

#[tokio::test]
async fn detail_deadline_includes_waiting_for_an_existing_url_request() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let item = fixture.item("/waiting");
    let gate = Arc::new(tokio::sync::Mutex::new(()));
    let held = gate.lock().await;
    fixture
        .client
        .details
        .lock()
        .map_err(|_| anyhow::anyhow!("request map lock"))?
        .insert(item.url.clone(), Arc::downgrade(&gate));
    tokio::time::pause();
    let mut job = tokio::spawn({
        let client = fixture.client.clone();
        let item = item.clone();
        async move { client.detail(item).await }
    });
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(10)).await;
    let result = match tokio::time::timeout(Duration::from_secs(1), &mut job).await {
        Ok(result) => result?,
        Err(_) => {
            job.abort();
            let _ = job.await;
            anyhow::bail!("detail exceeded its deadline while waiting for the URL gate");
        }
    };
    ensure!(result.is_err_and(|error| error.code == "timeout"));
    ensure!(fixture.hits.load(Ordering::SeqCst) == 0);
    tokio::time::resume();
    drop(held);
    fixture.client.detail(item).await?;
    Ok(())
}

#[tokio::test]
async fn prefetch_budget_pre_cancel_and_parent_close_prevent_dispatch() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    ensure!(
        prefetch::Prefetch::new(fixture.client.clone(), vec![fixture.item("/x"); 2049])
            .is_err_and(|error| error.code == "size")
    );
    let mut large = fixture.item("/x");
    large.title = "x".repeat(4 * 1024 * 1024 + 1);
    ensure!(prefetch::Prefetch::new(fixture.client.clone(), vec![large]).is_err());
    let batch = prefetch::Prefetch::new(fixture.client.clone(), vec![fixture.item("/x")])?;
    batch.cancel();
    batch
        .run(|_| panic!("pre-cancelled batch delivered a preview"))
        .await?;
    let parent = prefetch::Prefetch::new(fixture.client.clone(), vec![fixture.item("/x")])?;
    fixture.client.close();
    parent
        .run(|_| panic!("closed parent delivered a preview"))
        .await?;
    ensure!(fixture.hits.load(Ordering::SeqCst) == 0);
    Ok(())
}

#[tokio::test]
async fn empty_and_self_referencing_pages_end_pagination_in_native() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    for path in ["/empty-next", "/self-next"] {
        ensure!(fixture
            .client
            .list(Some(fixture.item(path).url))
            .await?
            .next_page
            .is_none());
    }
    Ok(())
}

#[tokio::test]
async fn same_origin_redirect_uses_final_base_and_original_cache_key() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let page = fixture
        .client
        .list(Some(format!("{}/old/list", fixture.origin)))
        .await?;
    ensure!(
        page.next_page == Some(format!("{}/new/2.htm", fixture.origin)),
        "pagination used pre-redirect base"
    );
    let item = fixture.item("/old/detail");
    let detail = fixture.client.detail(item.clone()).await?;
    ensure!(detail.url == item.url, "redirect lost stable item identity");
    ensure!(
        matches!(&detail.blocks[1], Block::Image { url, .. } if url == &format!("{}/new/image.png", fixture.origin)),
        "image used pre-redirect base"
    );
    let hits = fixture.hits.load(Ordering::Relaxed);
    fixture.client.detail(item).await?;
    ensure!(
        fixture.hits.load(Ordering::Relaxed) == hits,
        "redirect result not cached"
    );
    let page = fixture
        .client
        .list(Some(format!("{}/external-page", fixture.origin)))
        .await?;
    ensure!(page.next_page.is_none(), "unfetchable pagination exposed");
    Ok(())
}

#[test]
fn upgraded_urls_share_a_single_cache_identity() -> anyhow::Result<()> {
    let client = Client::new();
    let raw = "http://newits.shu.edu.cn/info/1.htm";
    let canonical = client.target(raw)?.to_string();
    let detail = parser::detail(
        b"",
        &Item {
            title: "cached".into(),
            url: canonical.clone(),
            date_text: String::new(),
            published_at: None,
        },
    )?;
    client
        .cache
        .lock()
        .map_err(|_| anyhow::anyhow!("cache poisoned"))?
        .push_back(detail);
    ensure!(
        client.cached(raw.into())?.is_some(),
        "HTTP alias missed HTTPS cache"
    );
    ensure!(
        client.cached(canonical)?.is_some(),
        "canonical cache missing"
    );
    Ok(())
}

#[tokio::test]
async fn actual_https_cache_is_bounded_and_reuses_detail() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let list = fixture
        .client
        .list(Some(format!("{}/list", fixture.origin)))
        .await?;
    ensure!(list.items.len() == 2, "HTTPS list not parsed");
    let first = fixture.client.detail(fixture.item("/first")).await?;
    let hit = fixture.hits.load(Ordering::Relaxed);
    let cached = fixture.client.detail(fixture.item("/first")).await?;
    ensure!(
        first.title == cached.title && fixture.hits.load(Ordering::Relaxed) == hit,
        "cache repeated network"
    );
    for index in 0..65 {
        fixture
            .client
            .detail(fixture.item(&format!("/{index}")))
            .await?;
    }
    ensure!(
        fixture
            .client
            .cache
            .lock()
            .map_err(|_| anyhow::anyhow!("cache poisoned"))?
            .len()
            == 64
            && fixture.client.cached(fixture.item("/first").url)?.is_none(),
        "cache eviction failed"
    );
    Ok(())
}

#[tokio::test]
async fn origin_and_status_failures_do_not_enter_cache() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    ensure!(
        fixture
            .client
            .detail(Item {
                url: "https://foreign.example/".into(),
                ..fixture.item("/x")
            })
            .await
            .err()
            .is_some_and(|error| error.code == "origin"),
        "offsite request accepted"
    );
    ensure!(
        fixture.hits.load(Ordering::Relaxed) == 0,
        "offsite touched HTTP"
    );
    ensure!(
        fixture
            .client
            .detail(fixture.item("/foreign"))
            .await
            .err()
            .is_some_and(|error| error.code == "origin"),
        "foreign redirect accepted"
    );
    ensure!(
        fixture
            .client
            .detail(fixture.item("/missing"))
            .await
            .err()
            .is_some_and(|error| error.status_code == Some(404)),
        "HTTP status lost"
    );
    ensure!(
        fixture
            .client
            .cache
            .lock()
            .map_err(|_| anyhow::anyhow!("cache poisoned"))?
            .is_empty(),
        "failed detail cached"
    );
    Ok(())
}

#[tokio::test]
async fn close_discards_inflight_detail_and_clears_cache() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    fixture.client.detail(fixture.item("/first")).await?;
    let caller = fixture.client.clone();
    let item = fixture.item("/slow");
    let job = tokio::spawn(async move { caller.detail(item).await });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    fixture.client.close();
    fixture.release.notify_one();
    ensure!(
        job.await?.err().is_some_and(|error| error.code == "closed"),
        "late detail survived close"
    );
    ensure!(
        fixture
            .client
            .cache
            .lock()
            .map_err(|_| anyhow::anyhow!("cache poisoned"))?
            .is_empty(),
        "closed cache retained"
    );
    ensure!(
        fixture
            .client
            .list(None)
            .await
            .err()
            .is_some_and(|error| error.code == "closed"),
        "closed client reused"
    );
    Ok(())
}
