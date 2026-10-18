use serde_json::{json, Value};
use shuvpn_core::routing::{
    compiler, editor,
    engine::{self, Evaluation},
    model::{Action, Flow, Inbound, Network, Overrides, Profile},
};
use std::collections::BTreeMap;

#[test]
fn mixed_rule_entries_keep_disabled_rules_out_of_the_effective_order() -> anyhow::Result<()> {
    let profile: Profile = compiler::parse_document(
        r#"{
          "automatic_campus":false,
          "rules":[
            {"rule":"DOMAIN,target.example,REJECT","enabled":false},
            "DOMAIN,target.example,DIRECT",
            "MATCH,REJECT"
          ]
        }"#,
    )?;
    let compiled = compiler::compile(&profile, &Overrides::default(), &BTreeMap::new())?;
    assert_eq!(compiled.rules.len(), 2);
    let flow = Flow {
        host: "target.example".to_owned(),
        ips: vec![],
        port: 443,
        network: Network::Tcp,
        inbound: Inbound::Http,
        process: None,
        domain_candidates: Vec::new(),
    };
    let Evaluation::Decided(decision) = engine::evaluate(Some(&compiled), &[], 9, &flow)? else {
        anyhow::bail!("Unexpected address lookup");
    };
    assert_eq!(decision.action, Action::Direct);
    assert_eq!(decision.rule_id, "profile:1");
    Ok(())
}

fn edit(profile: &str, overrides: &str, operation: Value) -> anyhow::Result<Value> {
    Ok(serde_json::from_str(&editor::edit(
        profile,
        overrides,
        &operation.to_string(),
    )?)?)
}

#[test]
fn disabled_rules_are_validated_and_cannot_remove_the_final_fallback() -> anyhow::Result<()> {
    for rules in [
        json!([{"rule":"MATCH,DIRECT","enabled":false}]),
        json!(["MATCH,DIRECT", {"rule":"MATCH,REJECT","enabled":false}]),
        json!([{"rule":"IP-CIDR,10.0.0.0/99,DIRECT","enabled":false}, "MATCH,DIRECT"]),
        json!([{"rule":"RULE-SET,missing,REJECT","enabled":false}, "MATCH,DIRECT"]),
        json!([{"rule":"DOMAIN,example,unknown","enabled":false}, "MATCH,DIRECT"]),
    ] {
        let profile: Profile = compiler::parse_document(&json!({"rules":rules}).to_string())?;
        assert!(compiler::compile(&profile, &Overrides::default(), &BTreeMap::new()).is_err());
    }
    for entry in [
        json!({"rule":"MATCH,DIRECT","enabled":true,"ignored":true}),
        json!({"rule":"MATCH,DIRECT"}),
        json!({"rule":"MATCH,DIRECT","enabled":"false"}),
    ] {
        assert!(
            compiler::parse_document::<Profile>(&json!({"rules":[entry]}).to_string()).is_err()
        );
    }
    Ok(())
}

#[test]
fn editor_reports_orphaned_selections_and_invalid_groups_before_display() {
    for (profile, overrides) in [
        (
            r#"{"rules":["MATCH,DIRECT"]}"#,
            r#"{"selections":{"deleted-group":"DIRECT"}}"#,
        ),
        (
            r#"{"rules":["MATCH,DIRECT"],"proxy-groups":[{"name":"cycle","type":"select","proxies":["cycle"]}]}"#,
            "{}",
        ),
        (
            r#"{"rules":["MATCH,DIRECT"],"proxy-groups":[{"name":"unsupported","type":"url-test","proxies":["DIRECT"]}]}"#,
            "{}",
        ),
    ] {
        let failure = editor::inspect(profile, overrides).err();
        assert!(failure.is_some_and(|failure| failure.code == "group"));
    }
}

#[test]
fn shadow_diagnostics_follow_enabled_source_order_and_do_not_change_decisions() -> anyhow::Result<()>
{
    let profile = r#"{"rules":[
      {"rule":"DOMAIN,target.example,REJECT","enabled":false},
      "DOMAIN,target.example,DIRECT",
      "DOMAIN,unique.example,DIRECT",
      "MATCH,DIRECT"
    ]}"#;
    let overrides = r#"{"prepend":["DOMAIN,target.example,SHU-VPN"],"append":["DOMAIN,unique.example,REJECT"]}"#;
    let document: Value = serde_json::from_str(&editor::inspect(profile, overrides)?)?;
    assert_eq!(document["rules"][2]["shadowed_by"], "prepend:0");
    assert_eq!(document["rules"][4]["shadowed_by"], "profile:2");
    assert!(document["rules"][1]["shadowed_by"].is_null());
    assert!(document["rules"][5]["shadowed_by"].is_null());
    let parsed: Profile = compiler::parse_document(profile)?;
    let edits: Overrides = compiler::parse_document(overrides)?;
    let compiled = compiler::compile(&parsed, &edits, &BTreeMap::new())?;
    assert!(compiled
        .notices
        .iter()
        .any(|notice| notice.contains("append:0") && notice.contains("profile:2")));
    assert_eq!(compiled.rules[0].id, "prepend:0");
    assert_eq!(compiled.rules[0].action, Action::ShuVpn);
    Ok(())
}

#[test]
fn shadow_diagnostics_ignore_disabled_predecessors_and_different_resolution_flags(
) -> anyhow::Result<()> {
    let profile = r#"{"rules":[
      {"rule":"DOMAIN,unique.example,REJECT","enabled":false},
      "DOMAIN,unique.example,DIRECT",
      "IP-CIDR,10.0.0.0/8,DIRECT",
      "IP-CIDR,10.0.0.0/8,REJECT,no-resolve",
      "DOMAIN-SUFFIX,example,DIRECT",
      "DOMAIN,sub.example,REJECT",
      "MATCH,DIRECT"
    ]}"#;
    let document: Value = serde_json::from_str(&editor::inspect(profile, "{}")?)?;
    assert!(document["rules"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Missing rules"))?
        .iter()
        .all(|rule| rule["shadowed_by"].is_null()));
    Ok(())
}

#[test]
fn compiled_shadow_notice_count_is_bounded_without_dropping_effective_rules() -> anyhow::Result<()>
{
    let mut rules = vec!["DOMAIN,duplicate.example,DIRECT"; 200];
    rules.push("MATCH,DIRECT");
    let profile: Profile = compiler::parse_document(&json!({"rules":rules}).to_string())?;
    let compiled = compiler::compile(&profile, &Overrides::default(), &BTreeMap::new())?;
    assert_eq!(compiled.rules.len(), 201);
    assert_eq!(compiled.notices.len(), 33);
    assert!(compiled
        .notices
        .last()
        .is_some_and(|notice| notice.contains("167")));
    Ok(())
}

#[test]
fn editor_parses_logic_fields_and_keeps_provider_descriptors_while_editing() -> anyhow::Result<()> {
    let expression =
        r"AND,((DOMAIN-REGEX,^api[0-9]{1,3}[.]example$),(DST-PORT,443)),gateway,no-resolve";
    let profile = json!({
        "name":"original",
        "rules":[expression,"MATCH,DIRECT"],
        "proxy-groups":[{"name":"gateway","type":"select","proxies":["DIRECT","SHU-VPN"]}],
        "rule-providers":{"domain-list":{"type":"file","behavior":"domain","path":"domains.yaml","interval":60}}
    }).to_string();
    let document: Value = serde_json::from_str(&editor::inspect(&profile, "")?)?;
    assert_eq!(document["rules"][0]["kind"], "AND");
    assert_eq!(document["rules"][0]["policy"], "gateway");
    assert_eq!(document["rules"][0]["no_resolve"], true);
    assert_eq!(document["groups"][0]["selected"], "DIRECT");
    let modified = edit(
        &profile,
        "",
        json!({"type":"update","source":"profile","index":0,"expression":expression,"enabled":false}),
    )?;
    let serialized: Value = serde_json::from_str(modified["profile"].as_str().unwrap_or(""))?;
    assert_eq!(serialized["rule-providers"]["domain-list"]["interval"], 60);
    assert_eq!(
        serialized["rule-providers"]["domain-list"]["path"],
        "domains.yaml"
    );
    assert_eq!(modified["rules"][0]["enabled"], false);
    let selected = edit(
        modified["profile"].as_str().unwrap_or(""),
        modified["overrides"].as_str().unwrap_or(""),
        json!({"type":"select","group":"gateway","policy":"SHU-VPN"}),
    )?;
    assert_eq!(selected["groups"][0]["selected"], "SHU-VPN");
    assert!(editor::edit(
        &profile,
        "{}",
        r#"{"type":"select","group":"gateway","policy":"unknown"}"#
    )
    .is_err());
    Ok(())
}

#[test]
fn editor_changes_stay_within_their_source_and_keep_fallback_fixed() -> anyhow::Result<()> {
    let profile =
        r#"{"rules":["MATCH,DIRECT","DOMAIN,a.example,REJECT","DOMAIN,b.example,DIRECT"]}"#;
    let overrides =
        r#"{"prepend":["DOMAIN,first.example,DIRECT"],"append":["DOMAIN,last.example,REJECT"]}"#;
    let normalized = edit(profile, overrides, json!({"type":"rename","name":"edited"}))?;
    let changed = edit(
        normalized["profile"].as_str().unwrap_or(""),
        normalized["overrides"].as_str().unwrap_or(""),
        json!({"type":"move","source":"profile","index":0,"to":1}),
    )?;
    assert_eq!(changed["rules"][0]["source"], "prepend");
    assert_eq!(changed["rules"][1]["expression"], "DOMAIN,b.example,DIRECT");
    assert_eq!(changed["rules"][3]["source"], "append");
    assert_eq!(changed["rules"][4]["final_rule"], true);
    for operation in [
        json!({"type":"remove","source":"profile","index":0}),
        json!({"type":"move","source":"profile","index":0,"to":1}),
        json!({"type":"update","source":"profile","index":0,"expression":"MATCH,DIRECT","enabled":false}),
        json!({"type":"add","source":"prepend","expression":"MATCH,REJECT"}),
        json!({"type":"move","source":"profile","index":99,"to":1}),
        json!({"type":"add","source":"unknown","expression":"DOMAIN,a.example,DIRECT"}),
        json!({"type":"automatic_campus","enabled":false,"ignored":true}),
    ] {
        assert!(
            editor::edit(profile, overrides, &operation.to_string()).is_err(),
            "{operation}"
        );
    }
    let added = edit(
        profile,
        overrides,
        json!({"type":"add","source":"append","expression":"DOMAIN,new.example,DIRECT"}),
    )?;
    assert_eq!(added["rules"][4]["expression"], "DOMAIN,new.example,DIRECT");
    Ok(())
}

#[tokio::test]
async fn edited_disabled_rules_survive_commit_restart_and_versioned_rollback() -> anyhow::Result<()>
{
    use shuvpn_core::api::routing::{edit_routing, routing_editor, RoutingManager};
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let original =
        r#"{"automatic_campus":false,"rules":["DOMAIN,target.example,REJECT","MATCH,DIRECT"]}"#;
    let draft: Value = serde_json::from_str(&edit_routing(
        original.to_owned(),
        "{}".to_owned(),
        json!({"type":"update","source":"profile","index":0,"expression":"DOMAIN,target.example,REJECT","enabled":false}).to_string(),
    )?)?;
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        0
    );
    let profile = draft["profile"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No profile"))?
        .to_owned();
    let overrides = draft["overrides"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No overrides"))?
        .to_owned();
    manager.apply(profile.clone(), overrides.clone(), 0).await?;
    manager
        .apply(original.to_owned(), "{}".to_owned(), 1)
        .await?;
    drop(manager);
    let restored = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    restored.rollback(2).await?;
    let current: Value = serde_json::from_str(&restored.current()?)?;
    assert_eq!(current["version"], 3);
    let view: Value = serde_json::from_str(&routing_editor(
        current["profile_document"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        current["overrides_document"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
    )?)?;
    assert_eq!(view["rules"][0]["enabled"], false);
    let result: Value = serde_json::from_str(&restored.test(
        r#"{"host":"target.example","port":443,"network":"tcp","inbound":"http"}"#.to_owned(),
        "[]".to_owned(),
    )?)?;
    assert_eq!(result["decision"]["action"], "DIRECT");
    assert!(restored.apply(profile, overrides, 2).await.is_err());
    Ok(())
}

#[tokio::test]
async fn an_uncompiled_draft_is_not_a_validation_result_or_a_published_snapshot(
) -> anyhow::Result<()> {
    use shuvpn_core::api::routing::RoutingManager;
    let directory = tempfile::tempdir()?;
    let manager = RoutingManager::open(directory.path().to_string_lossy().into_owned()).await?;
    let original = r#"{"rules":["MATCH,DIRECT"]}"#;
    manager
        .apply(original.to_owned(), "{}".to_owned(), 0)
        .await?;
    let draft = edit(
        original,
        "{}",
        json!({"type":"add","source":"profile","expression":"IP-CIDR,10.0.0.0/99,DIRECT"}),
    )?;
    assert!(manager
        .prepare(
            draft["profile"].as_str().unwrap_or("").to_owned(),
            draft["overrides"].as_str().unwrap_or("").to_owned(),
            1,
        )
        .await
        .is_err());
    assert_eq!(
        serde_json::from_str::<Value>(&manager.current()?)?["version"],
        1
    );
    Ok(())
}
