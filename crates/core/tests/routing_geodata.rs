use sangfor_core::crypto::{hex_upper, sha256};
use serde_json::{json, Value};
use shuvpn_core::api::routing::RoutingManager;

#[path = "support/geodata.rs"]
mod fixture;

fn descriptor(behavior: &str, code: &str, file: &str, bytes: &[u8], version: &str) -> Value {
    json!({"type":"file","behavior":behavior,"format":"dat","path":file,
        "data":{"code":code,"version":version,"sha256":hex_upper(&sha256(bytes))}})
}

fn flow(host: &str, ips: &[&str]) -> String {
    json!({"host":host,"ips":ips,"port":443,"network":"tcp","inbound":"http"}).to_string()
}

fn action(manager: &RoutingManager, host: &str, ips: &[&str]) -> anyhow::Result<Value> {
    Ok(serde_json::from_str(
        &manager.test(flow(host, ips), "[]".to_owned())?,
    )?)
}

#[tokio::test]
async fn actual_dat_files_match_all_domain_kinds_and_attribute_presence() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let bytes = fixture::site();
    std::fs::write(directory.path().join("site.dat"), &bytes)?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let profile = json!({"automatic_campus":false,"rules":["GEOSITE,TEST,REJECT","MATCH,DIRECT"],
        "rule-providers":{"sites":descriptor("geosite","test","site.dat",&bytes,"fixture-v1")}});
    let current: Value = serde_json::from_str(
        &manager
            .apply(profile.to_string(), "{}".to_owned(), 0)
            .await?,
    )?;
    assert!(current["notices"]
        .as_array()
        .is_some_and(|notices| notices.iter().any(|line| line
            .as_str()
            .is_some_and(|line| line.contains("fixture-v1")))));
    for host in [
        "has-track.example",
        "api123.example",
        "campus.example",
        "sub.campus.example",
        "exact.example",
    ] {
        assert_eq!(
            action(&manager, host, &[])?["decision"]["action"],
            "REJECT",
            "{host}"
        );
    }
    for host in [
        "badcampus.example",
        "sub.exact.example",
        "api1234.example",
        "127.0.0.1",
    ] {
        assert_eq!(
            action(&manager, host, &[])?["decision"]["action"],
            "DIRECT",
            "{host}"
        );
    }
    let filtered = json!({"automatic_campus":false,"rules":["GEOSITE,test@ADS@cn,REJECT","MATCH,DIRECT"],
        "rule-providers":{"sites":descriptor("geosite","TEST@cn@ads","site.dat",&bytes,"fixture-v1")}});
    manager
        .apply(filtered.to_string(), "{}".to_owned(), 1)
        .await?;
    assert_eq!(
        action(&manager, "sub.campus.example", &[])?["decision"]["action"],
        "REJECT"
    );
    assert_eq!(
        action(&manager, "has-track.example", &[])?["decision"]["action"],
        "DIRECT"
    );
    assert_eq!(
        action(&manager, "api1.example", &[])?["decision"]["action"],
        "DIRECT"
    );
    Ok(())
}

#[tokio::test]
async fn geoip_is_dual_stack_unknown_before_dns_and_inverse_does_not_match_unknown(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let bytes = fixture::ip(false);
    std::fs::write(directory.path().join("ip.dat"), &bytes)?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let profile = json!({"automatic_campus":false,
        "rules":["AND,((GEOIP,cn,no-resolve),(NETWORK,tcp)),REJECT","MATCH,DIRECT"],
        "rule-providers":{"ips":descriptor("geoip","CN","ip.dat",&bytes,"fixture-v1")}});
    manager
        .apply(profile.to_string(), "{}".to_owned(), 0)
        .await?;
    for host in ["10.1.2.3", "2001:db8::5"] {
        assert_eq!(
            action(&manager, host, &[])?["decision"]["action"],
            "REJECT",
            "{host}"
        );
    }
    assert_eq!(
        action(&manager, "unknown.example", &[])?["decision"]["action"],
        "DIRECT"
    );
    let pending = json!({"automatic_campus":false,"rules":["GEOIP,CN,REJECT","MATCH,DIRECT"],
        "rule-providers":{"ips":descriptor("geoip","CN","ip.dat",&bytes,"fixture-v1")}});
    manager
        .apply(pending.to_string(), "{}".to_owned(), 1)
        .await?;
    assert_eq!(
        action(&manager, "unknown.example", &[])?["needsResolution"],
        true
    );
    assert_eq!(
        action(&manager, "resolved.example", &["2001:db8::5"])?["decision"]["action"],
        "REJECT"
    );
    let inverse = fixture::ip(true);
    std::fs::write(directory.path().join("ip.dat"), &inverse)?;
    let profile = json!({"automatic_campus":false,"rules":["GEOIP,CN,REJECT,no-resolve","MATCH,DIRECT"],
        "rule-providers":{"ips":descriptor("geoip","CN","ip.dat",&inverse,"fixture-v2")}});
    manager
        .apply(profile.to_string(), "{}".to_owned(), 2)
        .await?;
    assert_eq!(
        action(&manager, "203.0.113.2", &[])?["decision"]["action"],
        "REJECT"
    );
    assert_eq!(
        action(&manager, "10.1.2.3", &[])?["decision"]["action"],
        "DIRECT"
    );
    assert_eq!(
        action(&manager, "unknown.example", &[])?["decision"]["action"],
        "DIRECT"
    );
    assert_eq!(
        action(&manager, "mixed.example", &["10.1.2.3", "203.0.113.2"])?["decision"]["action"],
        "DIRECT"
    );
    Ok(())
}

#[tokio::test]
async fn checksum_parse_and_missing_category_failures_keep_the_committed_snapshot_and_restart_cache(
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let bytes = fixture::site();
    let path = directory.path().to_string_lossy().into_owned();
    std::fs::write(directory.path().join("site.dat"), &bytes)?;
    let manager = RoutingManager::open(path.clone()).await?;
    let profile = json!({"automatic_campus":false,"rules":["GEOSITE,test,REJECT","MATCH,DIRECT"],
        "rule-providers":{"site":descriptor("geosite","TEST","site.dat",&bytes,"fixture-v1")}});
    manager
        .apply(profile.to_string(), "{}".to_owned(), 0)
        .await?;
    std::fs::write(directory.path().join("site.dat"), b"bad protobuf")?;
    assert!(manager.refresh(1).await.is_err());
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        1
    );
    assert_eq!(
        action(&manager, "exact.example", &[])?["decision"]["action"],
        "REJECT"
    );
    let malformed = json!({"rules":["GEOSITE,test,REJECT","MATCH,DIRECT"],
        "rule-providers":{"site":descriptor("geosite","TEST","site.dat",b"bad protobuf","fixture-v2")}});
    assert!(manager
        .apply(malformed.to_string(), "{}".to_owned(), 1)
        .await
        .is_err());
    std::fs::write(directory.path().join("site.dat"), &bytes)?;
    let missing = json!({"rules":["GEOSITE,missing,REJECT","MATCH,DIRECT"],
        "rule-providers":{"site":descriptor("geosite","missing","site.dat",&bytes,"fixture-v1")}});
    assert!(manager
        .apply(missing.to_string(), "{}".to_owned(), 1)
        .await
        .is_err());
    drop(manager);
    std::fs::remove_file(directory.path().join("site.dat"))?;
    let restored = RoutingManager::open(path).await?;
    assert_eq!(
        action(&restored, "exact.example", &[])?["decision"]["action"],
        "REJECT"
    );
    Ok(())
}

#[tokio::test]
async fn selected_geo_index_can_exceed_the_ordinary_ruleset_line_budget() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut group = fixture::bytes(1, b"large");
    for index in 0..25_000 {
        group.extend(fixture::site_domain(
            3,
            &format!("host{index}.example"),
            &[],
        ));
    }
    let bytes = fixture::bytes(1, &group);
    std::fs::write(directory.path().join("large.dat"), &bytes)?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let profile = json!({"automatic_campus":false,"rules":["GEOSITE,large,REJECT","MATCH,DIRECT"],
        "rule-providers":{"large":descriptor("geosite","large","large.dat",&bytes,"fixture-v1")}});
    manager
        .apply(profile.to_string(), "{}".to_owned(), 0)
        .await?;
    assert_eq!(
        action(&manager, "host24999.example", &[])?["decision"]["action"],
        "REJECT"
    );
    assert_eq!(
        action(&manager, "other.example", &[])?["decision"]["action"],
        "DIRECT"
    );
    Ok(())
}
