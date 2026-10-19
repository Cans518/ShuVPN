use serde_json::Value;
use shuvpn_core::api::routing::RoutingManager;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        let mut bytes = [0; 16];
        getrandom::getrandom(&mut bytes).map_err(std::io::Error::other)?;
        let id: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = std::env::temp_dir().join(format!("shuvpn-routing-{id}"));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn text(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn validated_preview_is_committed_once_and_invalid_updates_preserve_the_snapshot(
) -> anyhow::Result<()> {
    let directory = Directory::new()?;
    let manager = RoutingManager::open(directory.text()).await?;
    let profile =
        "name: rules\nrules:\n - DOMAIN,blocked.example,REJECT\n - MATCH,DIRECT\n".to_owned();
    let preview = manager.prepare(profile.clone(), "{}".to_owned(), 0).await?;
    let info: Value = serde_json::from_str(&preview.info()?)?;
    assert_eq!(info["version"], 1);
    let current: Value = serde_json::from_str(&manager.current()?)?;
    assert_eq!(current["version"], 0);
    preview.commit().await?;
    assert!(preview.commit().await.is_err());
    assert!(manager
        .apply("rules: [UNKNOWN,foo,DIRECT]".to_owned(), "{}".to_owned(), 1)
        .await
        .is_err());
    assert!(manager
        .apply(
            r#"{"rules":["DOMAIN-REGEX,(?=unsupported),REJECT","MATCH,DIRECT"]}"#.to_owned(),
            "{}".to_owned(),
            1,
        )
        .await
        .is_err());
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        1
    );
    let selected: Value = serde_json::from_str(&manager.test(
        r#"{"host":"blocked.example","port":443,"network":"tcp","inbound":"http"}"#.to_owned(),
        "[]".to_owned(),
    )?)?;
    assert_eq!(selected["decision"]["action"], "REJECT");
    let rolled: Value = serde_json::from_str(&manager.rollback(1).await?)?;
    assert_eq!(rolled["version"], 2);
    drop(preview);
    drop(manager);
    let restored = RoutingManager::open(directory.text()).await?;
    assert_eq!(
        serde_json::from_str::<Value>(&restored.current()?)?["version"],
        2
    );
    restored.rollback(2).await?;
    assert_eq!(
        serde_json::from_str::<Value>(&restored.current()?)?["version"],
        3
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_compiled_updates_use_version_compare_and_swap() -> anyhow::Result<()> {
    let directory = Directory::new()?;
    let manager = RoutingManager::open(directory.text()).await?;
    let a = manager
        .prepare(
            "name: a\nrules: ['MATCH,DIRECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let b = manager
        .prepare(
            "name: b\nrules: ['MATCH,REJECT']".to_owned(),
            "{}".to_owned(),
            0,
        )
        .await?;
    let (a, b) = tokio::join!(a.commit(), b.commit());
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        1
    );
    Ok(())
}

#[tokio::test]
async fn file_ruleset_refresh_preserves_overrides_and_the_last_valid_payload() -> anyhow::Result<()>
{
    let directory = Directory::new()?;
    std::fs::write(
        directory.0.join("domains.yaml"),
        "payload:\n - '+.blocked.example'\n",
    )?;
    let manager = RoutingManager::open(directory.text()).await?;
    let profile = "name: subscribed\nrules:\n - RULE-SET,blocked,REJECT\n - MATCH,DIRECT\nrule-providers:\n blocked:\n  type: file\n  behavior: domain\n  format: yaml\n  path: domains.yaml\n".to_owned();
    let overrides =
        "prepend:\n - DOMAIN,override.example,DIRECT\nappend:\n - DOMAIN,after.example,REJECT\n"
            .to_owned();
    manager.apply(profile.clone(), overrides.clone(), 0).await?;
    std::fs::remove_file(directory.0.join("domains.yaml"))?;
    let info: Value =
        serde_json::from_str(&manager.apply(profile.clone(), overrides.clone(), 1).await?)?;
    assert_eq!(info["warnings"].as_array().map(Vec::len), Some(1));
    assert_eq!(info["overrides_document"], overrides);
    let selected: Value = serde_json::from_str(&manager.test(
        r#"{"host":"a.blocked.example","port":443,"network":"tcp","inbound":"socks"}"#.to_owned(),
        "[]".to_owned(),
    )?)?;
    assert_eq!(selected["decision"]["action"], "REJECT");
    drop(manager);
    let manager = RoutingManager::open(directory.text()).await?;
    let restored: Value = serde_json::from_str(&manager.current()?)?;
    assert_eq!(restored["warnings"], info["warnings"]);
    std::fs::write(
        directory.0.join("domains.yaml"),
        "payload:\n - 'new.example'\n",
    )?;
    manager.apply(profile, overrides, 2).await?;
    Ok(())
}

#[tokio::test]
async fn a_preview_pins_provider_content_instead_of_refetching_at_commit() -> anyhow::Result<()> {
    let directory = Directory::new()?;
    std::fs::write(directory.0.join("ips.txt"), "127.0.0.0/8\n")?;
    let manager = RoutingManager::open(directory.text()).await?;
    let profile = "rules: ['RULE-SET,local,REJECT,no-resolve', 'MATCH,DIRECT']\nrule-providers:\n local:\n  type: file\n  behavior: ipcidr\n  format: text\n  path: ips.txt\n".to_owned();
    let preview = manager.prepare(profile, "{}".to_owned(), 0).await?;
    std::fs::write(directory.0.join("ips.txt"), "192.0.2.0/24\n")?;
    preview.commit().await?;
    let selected: Value = serde_json::from_str(&manager.test(
        r#"{"host":"127.0.0.1","port":443,"network":"tcp","inbound":"tun"}"#.to_owned(),
        "[]".to_owned(),
    )?)?;
    assert_eq!(selected["decision"]["action"], "REJECT");
    Ok(())
}

#[tokio::test]
async fn escaping_paths_unknown_provider_types_and_unencrypted_downloads_are_rejected(
) -> anyhow::Result<()> {
    let directory = Directory::new()?;
    let manager = RoutingManager::open(directory.text()).await?;
    for descriptor in [
        "type: file\nbehavior: domain\npath: ../outside.yaml",
        "type: http\nbehavior: domain\nurl: http://example.com/rules.yaml",
        "type: inline\nbehavior: domain\nformat: mrs\npayload: ['example.com']",
        "type: inline\nbehavior: domain\ninterval: 60\npayload: ['example.com']",
        "type: file\nbehavior: domain\ninterval: 0\npath: domains.yaml",
        "type: file\nbehavior: domain\ninterval: 18446744073709551615\npath: domains.yaml",
    ] {
        let descriptor = descriptor
            .lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let profile = format!(
            "rules: ['RULE-SET,p,DIRECT', 'MATCH,REJECT']\nrule-providers:\n p:\n{descriptor}\n"
        );
        assert!(manager.apply(profile, "{}".to_owned(), 0).await.is_err());
    }
    assert!(RoutingManager::open(directory.text()).await.is_err());
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        0
    );
    Ok(())
}
