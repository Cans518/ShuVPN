use super::*;
use anyhow::{ensure, Context};
use serde_json::{json, Value};

fn defaults() -> Defaults {
    Defaults {
        debug_logging: false,
    }
}
fn edit(key: SettingKey, value: Value) -> Edit {
    Edit {
        key,
        value_json: value.to_string(),
    }
}
fn saved(root: &std::path::Path) -> anyhow::Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(
        root.join("settings-state.json"),
    )?)?)
}

#[test]
fn frozen_legacy_vectors_preserve_all_schema_migrations() -> anyhow::Result<()> {
    let vectors: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/settings/legacy.json"))?;
    for vector in vectors.as_array().context("vectors not array")? {
        let directory = tempfile::tempdir()?;
        let session = Session::open(
            directory.path().to_string_lossy().into(),
            &vector["input"].to_string(),
            defaults(),
        )?;
        let state = saved(directory.path())?;
        for (key, expected) in vector["expected"]
            .as_object()
            .context("expected not object")?
        {
            ensure!(
                &state["values"][key] == expected,
                "{}: {key} changed",
                vector["name"]
            );
        }
        for key in vector["absent"].as_array().context("absent not array")? {
            ensure!(
                state["values"]
                    .get(key.as_str().context("key not string")?)
                    .is_none(),
                "{}: unexpected {key}",
                vector["name"]
            );
        }
        session.close()?;
    }
    Ok(())
}

#[test]
fn defaults_and_legacy_listen_decoding_are_native() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let session = Session::open(
        root.path().to_string_lossy().into(),
        r#"{"settings.socksListen":"any","settings.httpListen":"bad","settings.logLevel":"debug","settings.protocol.easyconnect":true}"#,
        Defaults {
            debug_logging: false,
        },
    )?;
    let snapshot = session.snapshot()?;
    ensure!(
        !snapshot.vpn_enabled
            && snapshot.socks_port == 2233
            && snapshot.http_port == 3322
            && snapshot.vpn_mtu == 1400
            && snapshot.timeout_seconds == 60,
        "defaults changed"
    );
    ensure!(
        snapshot.socks_listen == "0.0.0.0"
            && snapshot.http_listen == "127.0.0.1"
            && snapshot.log_level == "DEBUG"
            && snapshot.enabled_protocols == ["atrust"],
        "legacy decoding changed"
    );
    Ok(())
}

#[test]
fn capture_mode_defaults_to_campus_and_commits_only_supported_modes() -> anyhow::Result<()> {
    use crate::platform::model::CaptureMode;
    let root = tempfile::tempdir()?;
    let path = root.path().to_string_lossy().to_string();
    let session = Session::open(path.clone(), "{}", defaults())?;
    assert_eq!(session.snapshot()?.vpn_capture_mode, CaptureMode::Campus);
    let full = session.apply(vec![edit(SettingKey::VpnCaptureMode, json!("full"))])?;
    assert_eq!(full.vpn_capture_mode, CaptureMode::Full);
    assert!(session
        .apply(vec![edit(SettingKey::VpnCaptureMode, json!("unknown"))])
        .is_err());
    assert_eq!(session.snapshot()?.revision, full.revision);
    session.close()?;
    let restored = Session::open(path, "ignored after adoption", defaults())?;
    assert_eq!(restored.snapshot()?.vpn_capture_mode, CaptureMode::Full);
    assert_eq!(
        restored
            .apply(vec![edit(SettingKey::VpnCaptureMode, json!("campus"))])?
            .vpn_capture_mode,
        CaptureMode::Campus
    );
    Ok(())
}

#[test]
fn onboarding_snapshot_distinguishes_fresh_and_existing_installations() -> anyhow::Result<()> {
    for (input, expected) in [
        (json!({}), false),
        (json!({"settings.schema.version":0}), false),
        (
            json!({"settings.schema.version":0,"settings.welcomeCompleted":true}),
            true,
        ),
        (
            json!({"settings.schema.version":1,"settings.welcomeCompleted":false}),
            true,
        ),
        (
            json!({"settings.schema.version":2,"settings.welcomeCompleted":false}),
            true,
        ),
    ] {
        let root = tempfile::tempdir()?;
        let session = Session::open(
            root.path().to_string_lossy().into(),
            &input.to_string(),
            defaults(),
        )?;
        ensure!(
            session.snapshot()?.welcome_completed == expected,
            "onboarding snapshot changed"
        );
        session.close()?;
    }
    Ok(())
}

#[test]
fn future_schema_unknown_values_survive_edit_and_reopen() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().to_string_lossy().to_string();
    let session = Session::open(
        path.clone(),
        r#"{"settings.schema.version":7,"settings.future":{"keep":[1,2]},"settings.welcomeCompleted":false}"#,
        defaults(),
    )?;
    session.apply(vec![edit(SettingKey::Server, json!("updated.example"))])?;
    session.close()?;
    let restored = Session::open(path, "not parsed after adoption", defaults())?;
    let snapshot = restored.snapshot()?;
    ensure!(
        snapshot.schema_version == 7 && snapshot.server == "updated.example",
        "future schema or edit lost"
    );
    ensure!(
        !snapshot.welcome_completed,
        "future onboarding state changed"
    );
    ensure!(
        saved(root.path())?["values"]["settings.future"] == json!({"keep":[1,2]}),
        "unknown future data lost"
    );
    restored.close()?;
    Ok(())
}

#[test]
fn conflicting_proxy_edits_follow_their_native_transaction_order() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let session = Session::open(root.path().to_string_lossy().into(), "{}", defaults())?;
    let first = session.apply(vec![
        edit(SettingKey::SystemProxyEnabled, json!(true)),
        edit(SettingKey::HttpProxyEnabled, json!(false)),
    ])?;
    ensure!(
        !first.system_proxy_enabled && !first.http_proxy_enabled,
        "later HTTP disable lost"
    );
    let second = session.apply(vec![
        edit(SettingKey::HttpProxyEnabled, json!(false)),
        edit(SettingKey::SystemProxyEnabled, json!(true)),
    ])?;
    ensure!(
        second.system_proxy_enabled && second.http_proxy_enabled,
        "later system enable lost"
    );
    session.close()?;
    Ok(())
}

#[test]
fn committed_updates_restore_defaults_and_never_reimport() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().to_string_lossy().to_string();
    let session = Session::open(
        path.clone(),
        r#"{"settings.schema.version":2,"settings.server":"old.example","settings.socksPort":1080}"#,
        defaults(),
    )?;
    session.apply(vec![
        edit(SettingKey::Server, json!("new.example")),
        edit(SettingKey::SocksPort, json!(0)),
    ])?;
    session.close()?;
    let session = Session::open(path.clone(), "not JSON", defaults())?;
    ensure!(
        session.snapshot()?.server == "new.example" && session.snapshot()?.socks_port == 0,
        "old settings reimported"
    );
    session.restore_connection_defaults()?;
    let snapshot = session.snapshot()?;
    ensure!(
        snapshot.server == "atrust.shu.edu.cn"
            && snapshot.socks_port == 0
            && snapshot.welcome_completed,
        "restore cleared unrelated values"
    );
    let persisted = saved(root.path())?;
    ensure!(
        persisted["values"].get("settings.server").is_none(),
        "restore wrote default instead of removing key"
    );
    session.close()?;
    let restored = Session::open(path, "{}", defaults())?;
    ensure!(
        restored.snapshot()?.server == "atrust.shu.edu.cn",
        "restore not durable"
    );
    Ok(())
}

#[test]
fn invalid_batch_preserves_disk_and_memory() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let session = Session::open(root.path().to_string_lossy().into(), "{}", defaults())?;
    let before = std::fs::read(root.path().join("settings-state.json"))?;
    for (key, value) in [
        (SettingKey::SocksPort, json!(-1)),
        (SettingKey::HttpPort, json!(65536)),
        (SettingKey::VpnMtu, json!(0)),
        (SettingKey::TimeoutSeconds, json!(0)),
        (SettingKey::VpnDns, json!("999.1.2.3")),
        (SettingKey::HttpListen, json!("hostname.example")),
        (SettingKey::Server, json!("bad host")),
        (SettingKey::LoginDomain, json!("")),
        (SettingKey::LogEnabled, json!("true")),
        (SettingKey::ThemeMode, json!("unknown")),
        (SettingKey::LogLevel, json!("verbose")),
    ] {
        ensure!(
            session
                .apply(vec![
                    edit(SettingKey::Server, json!("valid.example")),
                    edit(key, value)
                ])
                .is_err(),
            "invalid update accepted"
        );
        ensure!(
            session.snapshot()?.server == "atrust.shu.edu.cn"
                && std::fs::read(root.path().join("settings-state.json"))? == before,
            "invalid batch partially published"
        );
    }
    Ok(())
}

#[test]
fn dependent_proxy_flags_commit_as_one_transaction() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let session = Session::open(root.path().to_string_lossy().into(), "{}", defaults())?;
    let enabled = session.apply(vec![edit(SettingKey::SystemProxyEnabled, json!(true))])?;
    ensure!(
        enabled.system_proxy_enabled && enabled.http_proxy_enabled,
        "system proxy has no HTTP listener"
    );
    let disabled = session.apply(vec![edit(SettingKey::HttpProxyEnabled, json!(false))])?;
    ensure!(
        !disabled.system_proxy_enabled && !disabled.http_proxy_enabled,
        "HTTP disabled without disabling system proxy"
    );
    ensure!(
        saved(root.path())?["revision"] == json!(2),
        "dependent flags used multiple commits"
    );
    Ok(())
}

#[test]
fn imported_system_proxy_enables_its_required_http_listener() -> anyhow::Result<()> {
    for legacy in [
        r#"{"settings.systemProxyEnabled":true}"#,
        r#"{"settings.systemProxyEnabled":true,"settings.httpProxyEnabled":false}"#,
    ] {
        let root = tempfile::tempdir()?;
        let session = Session::open(root.path().to_string_lossy().into(), legacy, defaults())?;
        let snapshot = session.snapshot()?;
        ensure!(
            snapshot.system_proxy_enabled && snapshot.http_proxy_enabled,
            "import kept an impossible proxy combination"
        );
        ensure!(
            saved(root.path())?["values"]["settings.httpProxyEnabled"] == true,
            "required listener not durable"
        );
    }
    Ok(())
}

#[test]
fn persisted_noncanonical_or_inconsistent_settings_fail_closed() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().to_string_lossy().to_string();
    let session = Session::open(path.clone(), "{}", defaults())?;
    session.close()?;
    for values in [
        json!({"settings.schema.version":4,"settings.themeMode":" dark "}),
        json!({"settings.schema.version":4,"settings.httpListen":" 127.0.0.1 "}),
        json!({"settings.schema.version":4,"settings.systemProxyEnabled":true}),
        json!({"settings.schema.version":0}),
    ] {
        std::fs::write(
            root.path().join("settings-state.json"),
            json!({"format_version":1,"revision":0,"values":values}).to_string(),
        )?;
        ensure!(
            Session::open(path.clone(), "{}", defaults()).is_err(),
            "invalid saved state was accepted"
        );
    }
    Ok(())
}

#[test]
fn adding_to_full_settings_never_writes_an_unreadable_record() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let mut values = serde_json::Map::new();
    values.insert("settings.schema.version".into(), json!(4));
    for index in 0..255 {
        values.insert(format!("settings.future{index}"), json!(index));
    }
    let path = root.path().to_string_lossy().to_string();
    let session = Session::open(path.clone(), &Value::Object(values).to_string(), defaults())?;
    ensure!(
        session
            .apply(vec![edit(SettingKey::Server, json!("overflow.example"))])
            .is_err(),
        "record exceeded reader entry budget"
    );
    session.close()?;
    ensure!(
        Session::open(path, "{}", defaults())?.snapshot()?.server == "atrust.shu.edu.cn",
        "saved record could not reopen"
    );
    Ok(())
}

#[test]
fn lock_close_corruption_and_format_versions_fail_closed() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().to_string_lossy().to_string();
    let session = Session::open(path.clone(), "{}", defaults())?;
    ensure!(
        Session::open(path.clone(), "{}", defaults()).is_err(),
        "second owner accepted"
    );
    session.close()?;
    session.close()?;
    ensure!(
        session
            .apply(vec![edit(SettingKey::WelcomeCompleted, json!(true))])
            .is_err(),
        "closed owner wrote"
    );
    for body in [
        "corrupt",
        r#"{"format_version":99,"revision":0,"values":{}}"#,
    ] {
        std::fs::write(root.path().join("settings-state.json"), body)?;
        ensure!(
            Session::open(path.clone(), "{}", defaults()).is_err(),
            "invalid disk record reimported"
        );
    }
    Ok(())
}

#[test]
fn write_failures_keep_snapshot_and_uncertain_commit_requires_reopen() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().to_string_lossy().to_string();
    let session = Session::open(path.clone(), "{}", defaults())?;
    session
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("poisoned"))?
        .store
        .as_ref()
        .context("closed")?
        .fail_writes
        .store(true, std::sync::atomic::Ordering::SeqCst);
    ensure!(
        session
            .apply(vec![edit(SettingKey::Server, json!("before.example"))])
            .is_err(),
        "injected write succeeded"
    );
    ensure!(
        session.snapshot()?.server == "atrust.shu.edu.cn",
        "failed write published"
    );
    {
        let state = session
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("poisoned"))?;
        let store = state.store.as_ref().context("closed")?;
        store
            .fail_writes
            .store(false, std::sync::atomic::Ordering::SeqCst);
        store
            .fail_after_replace
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    ensure!(
        session
            .apply(vec![edit(SettingKey::Server, json!("committed.example"))])
            .err()
            .is_some_and(|error| error.code == "uncertain"),
        "uncertain write not reported"
    );
    ensure!(
        session.snapshot()?.server == "atrust.shu.edu.cn",
        "uncertain result published"
    );
    ensure!(
        session
            .apply(vec![edit(SettingKey::SocksPort, json!(9999))])
            .is_err(),
        "uncertain owner overwrote disk"
    );
    session.close()?;
    let reopened = Session::open(path, "{}", defaults())?;
    ensure!(
        reopened.snapshot()?.server == "committed.example",
        "whole new record not restored"
    );
    Ok(())
}
