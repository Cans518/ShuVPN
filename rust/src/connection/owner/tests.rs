use super::*;
use anyhow::{ensure, Context};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};
use tokio::{net::TcpStream, time::timeout};

#[allow(dead_code)]
mod gateway {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/gateway.rs"
    ));
}

fn proxies() -> ProxyStartup {
    ProxyStartup {
        http_enabled: true,
        http_address: "127.0.0.1".into(),
        http_port: 0,
        socks_enabled: true,
        socks_address: "127.0.0.1".into(),
        socks_port: 0,
        system_proxy_enabled: false,
    }
}
fn dial(plan: String, id: u64) -> Connect {
    Box::new(move |_| {
        Box::pin(async move {
            Ok(Session {
                engine: Arc::new(Engine::connect(plan, 3000).await?),
                run_id: id,
                warnings: vec![],
                startup: None,
            })
        })
    })
}
async fn connected(owner: &Owner, plan: String, id: u64) -> anyhow::Result<Session> {
    Ok(owner
        .connect(None, dial(plan, id), proxies(), None, None)
        .await?)
}

#[tokio::test]
async fn native_owner_keeps_the_session_after_its_view_drops_and_replaces_only_after_close(
) -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let owner = Owner::spawn();
    let first = connected(&owner, gateway.plan(), 1).await?;
    let port = first
        .startup
        .as_ref()
        .context("startup missing")?
        .proxies
        .http
        .as_ref()
        .context("HTTP missing")?
        .port;
    drop(first);
    ensure!(owner.snapshot()?.run_id == Some(1));
    let client = TcpStream::connect(("127.0.0.1", port)).await?;
    drop(client);
    let second = connected(&owner, gateway.plan(), 2).await?;
    ensure!(second.run_id == 2);
    ensure!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
    owner.shutdown(None).await?;
    ensure!(owner.snapshot()?.closed && owner.snapshot()?.run_id.is_none());
    Ok(())
}

#[tokio::test]
async fn failed_cleanup_retains_the_owner_and_prevents_polling_replacement_until_retry(
) -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let owner = Owner::spawn();
    let session = connected(&owner, gateway.plan(), 1).await?;
    let directory = tempfile::tempdir()?;
    let original = BTreeMap::from([
        ("mode".into(), Some("auto".into())),
        ("server".into(), None),
    ]);
    let settings = Arc::new(Mutex::new(original.clone()));
    let failures = Arc::new(AtomicUsize::new(1));
    // Enable first; inject the restore failure only after the fixture is armed.
    failures.store(0, Ordering::SeqCst);
    session
        .engine
        .fixture_proxy_cleanup(
            &directory.path().to_string_lossy(),
            settings.clone(),
            failures.clone(),
        )
        .await?;
    failures.store(1, Ordering::SeqCst);
    let weak = Arc::downgrade(&session.engine);
    drop(session);
    let polled = Arc::new(AtomicBool::new(false));
    let seen = polled.clone();
    let replacement: Connect = Box::new(move |_| {
        Box::pin(async move {
            seen.store(true, Ordering::SeqCst);
            Err(VpnFailure::new("fixture", "replacement must not run"))
        })
    });
    ensure!(owner
        .connect(None, replacement, proxies(), None, None)
        .await
        .is_err());
    ensure!(!polled.load(Ordering::SeqCst));
    ensure!(owner.snapshot()?.cleanup_pending && owner.snapshot()?.run_id == Some(1));
    ensure!(weak.upgrade().is_some());
    let authentication: Connect = Box::new(|_| {
        Box::pin(async {
            Err(VpnFailure::new(
                "authentication",
                "new authentication failure",
            ))
        })
    });
    ensure!(owner
        .connect(None, authentication, proxies(), None, None)
        .await
        .is_err_and(|error| error.code == "authentication"));
    ensure!(!owner.snapshot()?.cleanup_pending && owner.snapshot()?.run_id.is_none());
    ensure!(owner.snapshot()?.error_message.as_deref() == Some("new authentication failure"));
    ensure!(
        *settings
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture lock"))?
            == original
    );
    connected(&owner, gateway.plan(), 2).await?;
    owner.shutdown(None).await?;
    Ok(())
}

#[tokio::test]
async fn worker_failure_wakes_close_waiters_and_never_claims_cleanup_or_allows_replacement(
) -> anyhow::Result<()> {
    let owner = Owner::spawn();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let operation: Connect = {
        let entered = entered.clone();
        let release = release.clone();
        Box::new(move |_| {
            Box::pin(async move {
                entered.notify_one();
                release.notified().await;
                panic!("simulated owner worker failure");
            })
        })
    };
    let connecting = tokio::spawn({
        let owner = owner.clone();
        async move { owner.connect(None, operation, proxies(), None, None).await }
    });
    timeout(Duration::from_secs(2), entered.notified()).await?;
    let closing = tokio::spawn({
        let owner = owner.clone();
        async move { owner.shutdown(None).await }
    });
    tokio::task::yield_now().await;
    release.notify_one();
    ensure!(timeout(Duration::from_secs(2), closing).await??.is_err());
    ensure!(timeout(Duration::from_secs(2), connecting).await??.is_err());
    ensure!(owner.snapshot()?.cleanup_pending && !owner.snapshot()?.closed);
    let operation: Connect = Box::new(|_| {
        Box::pin(async {
            panic!("failed owner polled a replacement");
        })
    });
    ensure!(owner
        .connect(None, operation, proxies(), None, None)
        .await
        .is_err_and(|error| error.code == "worker"));
    Ok(())
}

#[tokio::test]
async fn shutdown_drains_an_uncooperative_late_connect_and_closes_its_real_session(
) -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let owner = Owner::spawn();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let operation: Connect = {
        let entered = entered.clone();
        let release = release.clone();
        let plan = gateway.plan();
        Box::new(move |token| {
            Box::pin(async move {
                entered.notify_one();
                release.notified().await;
                // Deliberately return a real late session despite cancellation.
                dial(plan, 1)(token).await
            })
        })
    };
    let connecting = tokio::spawn({
        let owner = owner.clone();
        async move { owner.connect(None, operation, proxies(), None, None).await }
    });
    timeout(Duration::from_secs(3), entered.notified()).await?;
    let ending = tokio::spawn({
        let owner = owner.clone();
        async move { owner.shutdown(None).await }
    });
    tokio::task::yield_now().await;
    ensure!(!ending.is_finished());
    release.notify_one();
    timeout(Duration::from_secs(3), ending).await???;
    ensure!(connecting.await?.is_err());
    ensure!(owner.snapshot()?.closed && owner.snapshot()?.run_id.is_none());
    Ok(())
}

#[tokio::test]
async fn dropping_connect_or_shutdown_waiters_does_not_drop_registered_cleanup(
) -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let owner = Owner::spawn();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let operation: Connect = {
        let entered = entered.clone();
        let release = release.clone();
        let plan = gateway.plan();
        Box::new(move |token| {
            Box::pin(async move {
                entered.notify_one();
                release.notified().await;
                dial(plan, 1)(token).await
            })
        })
    };
    let connecting = tokio::spawn({
        let owner = owner.clone();
        async move { owner.connect(None, operation, proxies(), None, None).await }
    });
    timeout(Duration::from_secs(3), entered.notified()).await?;
    connecting.abort();
    let _ = connecting.await;
    let ending = tokio::spawn({
        let owner = owner.clone();
        async move { owner.shutdown(None).await }
    });
    tokio::task::yield_now().await;
    ending.abort();
    let _ = ending.await;
    release.notify_one();
    timeout(Duration::from_secs(3), owner.shutdown(None)).await??;
    ensure!(owner.snapshot()?.closed && !owner.snapshot()?.cleanup_pending);
    Ok(())
}

#[tokio::test]
async fn an_old_frontend_or_waiter_cannot_cancel_the_new_frontend() -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let owner = Owner::spawn();
    let first = owner.reserve_control()?;
    owner.await_control(first).await?;
    connected(&owner, gateway.plan(), 1).await?;
    let old_generation = owner.snapshot()?.generation;
    let stale = Waiting {
        shared: owner.shared.clone(),
        generation: old_generation,
        token: CancellationToken::new(),
        armed: true,
    };
    let second = owner.reserve_control()?;
    owner.await_control(second).await?;
    let session = owner
        .connect(Some(second), dial(gateway.plan(), 2), proxies(), None, None)
        .await?;
    drop(stale);
    ensure!(owner
        .cancel(Some(first))
        .is_err_and(|error| error.code == "stale"));
    ensure!(owner
        .disconnect(Some(first))
        .await
        .is_err_and(|error| error.code == "stale"));
    ensure!(owner.snapshot()?.run_id == Some(2));
    ensure!(
        session
            .engine
            .shared
            .snapshot(super::super::model::VpnEventKind::State, 0.0, 0.0)
            .state
            == VpnState::Connected
    );
    owner.shutdown(Some(second)).await?;
    Ok(())
}

#[tokio::test]
async fn a_failed_engine_during_startup_keeps_its_original_diagnostic() -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let owner = Owner::spawn();
    let plan = gateway.plan();
    let operation: Connect = Box::new(move |token| {
        Box::pin(async move {
            let session = dial(plan, 1)(token).await?;
            session
                .engine
                .shared
                .fail(VpnFailure::new("fixture", "original connection failure"));
            Ok(session)
        })
    });
    let error = owner
        .connect(None, operation, proxies(), None, None)
        .await
        .err()
        .context("failure lost")?;
    ensure!(error.message.contains("original connection failure"));
    ensure!(owner.snapshot()?.error_message.as_deref() == Some("original connection failure"));
    owner.shutdown(None).await?;
    Ok(())
}

#[tokio::test]
async fn exit_during_replacement_cleanup_never_polls_the_replacement() -> anyhow::Result<()> {
    let gateway = gateway::Gateway::start(gateway::Mode::Echo).await?;
    let owner = Owner::spawn();
    let first = connected(&owner, gateway.plan(), 1).await?;
    let release = Arc::new(Notify::new());
    first
        .engine
        .fixture_hold_driver_completion(release.clone())
        .await?;
    let polled = Arc::new(AtomicBool::new(false));
    let seen = polled.clone();
    let connecting = tokio::spawn({
        let owner = owner.clone();
        async move {
            owner
                .connect(
                    None,
                    Box::new(move |_| {
                        Box::pin(async move {
                            seen.store(true, Ordering::SeqCst);
                            Err(VpnFailure::new("fixture", "replacement was polled"))
                        })
                    }),
                    proxies(),
                    None,
                    None,
                )
                .await
        }
    });
    timeout(Duration::from_secs(2), async {
        while !first.engine.fixture_driver_is_joining() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    let ending = tokio::spawn({
        let owner = owner.clone();
        async move { owner.shutdown(None).await }
    });
    timeout(Duration::from_secs(2), async {
        while owner
            .snapshot()
            .is_ok_and(|snapshot| snapshot.phase != OwnerPhase::Disconnecting)
        {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    ensure!(!ending.is_finished());
    release.notify_one();
    timeout(Duration::from_secs(2), ending).await???;
    ensure!(connecting.await?.is_err());
    ensure!(!polled.load(Ordering::SeqCst));
    Ok(())
}
