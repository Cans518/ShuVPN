use serde_json::Value;
use shuvpn_core::api::routing::RoutingManager;
use tokio::time::{timeout, Duration};

fn profile(interval: Option<u64>) -> String {
    let interval = interval
        .map(|seconds| format!("  interval: {seconds}\n"))
        .unwrap_or_default();
    format!("name: subscribed\nautomatic_campus: false\nrules: ['RULE-SET,p,REJECT', 'MATCH,DIRECT']\nrule-providers:\n p:\n  type: file\n  behavior: domain\n  path: domains.yaml\n{interval}")
}

fn version(manager: &RoutingManager) -> anyhow::Result<u64> {
    serde_json::from_str::<Value>(&manager.current()?)?["version"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("missing configuration version"))
}

fn action(manager: &RoutingManager, host: &str) -> anyhow::Result<Value> {
    let flow = serde_json::json!({"host":host,"port":443,"network":"tcp","inbound":"http"});
    let decision: Value = serde_json::from_str(&manager.test(flow.to_string(), "[]".to_owned())?)?;
    Ok(decision["decision"]["action"].clone())
}

async fn wait_for(mut ready: impl FnMut() -> anyhow::Result<bool>) -> anyhow::Result<()> {
    timeout(Duration::from_secs(6), async {
        loop {
            if ready()? {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    Ok(())
}

#[tokio::test]
async fn manual_refresh_changes_only_valid_payloads_and_preserves_overrides() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("domains.yaml");
    std::fs::write(&path, "payload: ['old.example']")?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let overrides = "prepend: ['DOMAIN,override.example,DIRECT']";
    manager
        .apply(profile(None), overrides.to_owned(), 0)
        .await?;
    manager.refresh(1).await?;
    assert_eq!(version(&manager)?, 1);
    std::fs::write(&path, "payload: ['new.example']")?;
    let refreshed: Value = serde_json::from_str(&manager.refresh(1).await?)?;
    assert_eq!(refreshed["version"], 2);
    assert_eq!(refreshed["overrides_document"], overrides);
    assert_eq!(action(&manager, "new.example")?, "REJECT");
    assert_eq!(action(&manager, "old.example")?, "DIRECT");
    std::fs::write(&path, "payload: ['bad/domain']")?;
    assert!(manager.refresh(2).await.is_err());
    assert_eq!(version(&manager)?, 2);
    assert_eq!(action(&manager, "new.example")?, "REJECT");
    let status: Value = serde_json::from_str(&manager.status()?)?;
    assert_eq!(status["refreshing"], false);
    assert!(status["refresh_error"].is_string());
    assert!(status["last_checked_unix_ms"].is_u64());
    drop(manager);
    let restored = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    assert_eq!(version(&restored)?, 2);
    assert_eq!(action(&restored, "new.example")?, "REJECT");
    Ok(())
}

#[tokio::test]
async fn timer_refreshes_each_provider_at_its_own_interval_and_retries_failed_content(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    std::fs::write(
        directory.path().join("domains.yaml"),
        "payload: ['old.example']",
    )?;
    std::fs::write(
        directory.path().join("slow.yaml"),
        "payload: ['slow.example']",
    )?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let configuration = format!(
        "{} slow:\n  type: file\n  behavior: domain\n  path: slow.yaml\n  interval: 3\n",
        profile(Some(1))
    )
    .replace("'MATCH,DIRECT'", "'RULE-SET,slow,REJECT', 'MATCH,DIRECT'");
    manager.apply(configuration, "{}".to_owned(), 0).await?;
    std::fs::write(
        directory.path().join("domains.yaml"),
        "payload: ['new.example']",
    )?;
    std::fs::write(directory.path().join("slow.yaml"), "payload: [")?;
    wait_for(|| Ok(version(&manager)? == 2)).await?;
    assert_eq!(action(&manager, "new.example")?, "REJECT");
    wait_for(|| {
        Ok(serde_json::from_str::<Value>(&manager.status()?)?["refresh_error"].is_string())
    })
    .await?;
    assert_eq!(version(&manager)?, 2);
    std::fs::write(
        directory.path().join("domains.yaml"),
        "payload: ['newest.example']",
    )?;
    wait_for(|| Ok(version(&manager)? == 3)).await?;
    assert_eq!(action(&manager, "newest.example")?, "REJECT");
    let status: Value = serde_json::from_str(&manager.status()?)?;
    assert!(status["provider_errors"]["slow"].is_string());
    std::fs::write(
        directory.path().join("slow.yaml"),
        "payload: ['fixed.example']",
    )?;
    wait_for(|| Ok(version(&manager)? == 4)).await?;
    assert_eq!(version(&manager)?, 4);
    assert_eq!(action(&manager, "fixed.example")?, "REJECT");
    wait_for(|| Ok(serde_json::from_str::<Value>(&manager.status()?)?["refresh_error"].is_null()))
        .await?;
    Ok(())
}

#[tokio::test]
async fn changed_intervals_take_effect_and_dropping_the_owner_releases_the_directory(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    std::fs::write(
        directory.path().join("domains.yaml"),
        "payload: ['old.example']",
    )?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    manager
        .apply(profile(Some(3600)), "{}".to_owned(), 0)
        .await?;
    tokio::task::yield_now().await;
    manager.apply(profile(Some(1)), "{}".to_owned(), 1).await?;
    std::fs::write(
        directory.path().join("domains.yaml"),
        "payload: ['new.example']",
    )?;
    wait_for(|| Ok(version(&manager)? == 3)).await?;
    assert_eq!(action(&manager, "new.example")?, "REJECT");
    manager
        .apply(profile(Some(3600)), "{}".to_owned(), 3)
        .await?;
    tokio::task::yield_now().await;
    drop(manager);
    let restored = timeout(Duration::from_secs(2), async {
        loop {
            match RoutingManager::open(directory.path().to_string_lossy().into_owned()).await {
                Ok(manager) => return manager,
                Err(_) => tokio::task::yield_now().await,
            }
        }
    })
    .await?;
    assert_eq!(version(&restored)?, 4);
    Ok(())
}
