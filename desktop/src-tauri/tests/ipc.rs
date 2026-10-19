use serde_json::{json, Value};
use shuvpn_core::api::account::AccountSession;
use shuvpn_desktop::{command_handler as handler, host::DesktopHost};
use std::sync::Arc;
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY},
    webview::InvokeRequest,
    WebviewWindow,
};

fn call(window: &WebviewWindow<MockRuntime>, command: &str, body: Value) -> Result<Value, Value> {
    let origin = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    let url = match origin.parse() {
        Ok(url) => url,
        Err(error) => return Err(json!({"message": format!("{error}")})),
    };
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url,
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    )
    .map_err(|error| json!({"ipc":error}))?
    .deserialize()
    .map_err(|error| json!({"message":format!("{error}")}))
}

#[test]
fn offline_ipc_preserves_settings_preview_and_shutdown_contracts(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(child_directory) = std::env::var_os("SHUVPN_IPC_CHILD_DIRECTORY") else {
        let directory = tempfile::tempdir()?;
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "offline_ipc_preserves_settings_preview_and_shutdown_contracts",
                "--nocapture",
            ])
            .env("SHUVPN_IPC_CHILD_DIRECTORY", directory.path())
            .status()?;
        assert!(status.success(), "the first IPC process failed");
        // A different process must restore the committed state after all the
        // first runtime's references and native file leases have gone away.
        let reopened = tauri::async_runtime::block_on(DesktopHost::open(directory.path()))?;
        assert_eq!(reopened.settings.snapshot()?.http_port, 18443);
        assert_eq!(reopened.settings.snapshot()?.theme_mode, "dark");
        let restored: Value = serde_json::from_str(&reopened.routing.current()?)?;
        let expected: Value = serde_json::from_slice(&std::fs::read(
            directory.path().join("expected-routing.json"),
        )?)?;
        assert_eq!(restored, expected);
        tauri::async_runtime::block_on(reopened.shutdown())?;
        return Ok(());
    };
    let child_directory = std::path::PathBuf::from(child_directory);
    let root = child_directory.as_path();
    // Fail after settings opens, then prove startup did not leak its file lease.
    let blocked_account = AccountSession::open(
        root.join("account").to_string_lossy().into(),
        "[]".into(),
        None,
        "{}".into(),
    )?;
    assert!(tauri::async_runtime::block_on(DesktopHost::open(root)).is_err());
    blocked_account.close()?;
    let host = Arc::new(tauri::async_runtime::block_on(DesktopHost::open(root))?);
    let app = mock_builder()
        .manage(host.clone())
        .invoke_handler(handler())
        .build(mock_context(noop_assets()))?;
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build()?;
    let invoke = |command, body| {
        call(&window, command, body).map_err(|error| std::io::Error::other(error.to_string()))
    };
    let initial = invoke("desktop_snapshot", json!({}))?;
    assert_eq!(initial["account"]["signedIn"], false);
    assert_eq!(initial["connection"]["phase"], "disconnected");
    assert_eq!(initial["settings"]["systemProxyEnabled"], false);
    assert_eq!(initial["settings"]["vpnEnabled"], false);
    let invalid = call(
        &window,
        "save_settings",
        json!({"edits":[{"key":"httpPort","value":70000}]}),
    );
    assert!(invalid.is_err());
    assert_eq!(
        invoke("desktop_snapshot", json!({}))?["settings"],
        initial["settings"]
    );
    let gate = tauri::async_runtime::block_on(host.configuration.lock());
    let queued_window = window.clone();
    let (finished, observed) = std::sync::mpsc::channel();
    let saving = std::thread::spawn(move || {
        let response = call(
            &queued_window,
            "save_settings",
            json!({"edits":[
                {"key":"themeMode","value":"dark"},
                {"key":"httpPort","value":18443}
            ]}),
        );
        let _ = finished.send(response);
    });
    assert!(
        matches!(
            observed.recv_timeout(std::time::Duration::from_millis(100)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ),
        "settings must wait for the same gate that freezes connection startup settings"
    );
    drop(gate);
    let saved = observed
        .recv_timeout(std::time::Duration::from_secs(3))?
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    assert!(saving.join().is_ok());
    let saved_again = invoke(
        "save_settings",
        json!({"edits":[
            {"key":"themeMode","value":"dark"},
            {"key":"httpPort","value":18443}
        ]}),
    )?;
    assert_eq!(saved["httpPort"], 18443);
    assert_eq!(saved["themeMode"], "dark");
    assert_eq!(saved_again["httpPort"], 18443);
    assert!(call(
        &window,
        "save_settings",
        json!({"edits":[{"key":"arbitraryFile","value":"ignored"}]})
    )
    .is_err());

    let initial_routing = invoke("routing_current", json!({}))?;
    let version = initial_routing["version"]
        .as_u64()
        .ok_or("missing routing version")?;
    let profile = invoke("routing_template", json!({"name":"campus-direct"}))?;
    let discarded = invoke(
        "routing_prepare",
        json!({
            "profile":profile,
            "overrides":initial_routing["overrides_document"],
            "expectedVersion":version,
        }),
    )?;
    assert_eq!(invoke("routing_current", json!({}))?, initial_routing);
    invoke(
        "routing_discard",
        json!({"previewId":discarded["previewId"]}),
    )?;
    assert!(call(
        &window,
        "routing_commit",
        json!({"previewId":discarded["previewId"]})
    )
    .is_err());
    let first_preview = invoke(
        "routing_prepare",
        json!({
            "profile":profile,
            "overrides":initial_routing["overrides_document"],
            "expectedVersion":version,
        }),
    )?;
    let replacement = invoke(
        "routing_prepare",
        json!({
            "profile":"{\"rules\":[\"MATCH,REJECT\"]}",
            "overrides":"{}",
            "expectedVersion":version,
        }),
    )?;
    assert!(
        call(
            &window,
            "routing_commit",
            json!({"previewId":"unconfirmed-preview"})
        )
        .is_err(),
        "an unmatched confirmation must not apply a different preview"
    );
    assert_eq!(invoke("routing_current", json!({}))?, initial_routing);
    invoke(
        "routing_discard",
        json!({"previewId":first_preview["previewId"]}),
    )?;
    assert!(
        call(
            &window,
            "routing_commit",
            json!({"previewId":replacement["previewId"]})
        )
        .is_ok(),
        "a late discard for an old preview must not erase its replacement"
    );
    let version = version + 1;
    let prepared = invoke(
        "routing_prepare",
        json!({
            "profile":profile,
            "overrides":initial_routing["overrides_document"],
            "expectedVersion":version,
        }),
    )?;
    let committed = invoke("routing_commit", json!({"previewId":prepared["previewId"]}))?;
    assert_eq!(committed["version"], version + 1);
    assert!(call(
        &window,
        "routing_prepare",
        json!({
            "profile":"invalid: [",
            "overrides":"{}",
            "expectedVersion":version + 1
        })
    )
    .is_err());
    assert!(call(&window, "routing_commit", json!({})).is_err());
    assert_eq!(invoke("routing_current", json!({}))?, committed);
    assert!(call(
        &window,
        "routing_prepare",
        json!({
            "profile":profile,
            "overrides":"{}",
            "expectedVersion":version
        })
    )
    .is_err());
    assert_eq!(invoke("connection_records", json!({}))?, json!([]));
    invoke("disconnect", json!({}))?;

    tauri::async_runtime::block_on(async {
        let permit = host.enter().await?;
        let closing = host.clone();
        let task = tokio::spawn(async move { closing.shutdown().await });
        // The operation permit keeps shutdown pending until the caller drains.
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        assert!(!task.is_finished());
        drop(permit);
        task.await??;
        assert!(host.enter().await.is_err());
        assert!(host.owner.snapshot()?.closed);
        assert!(host.shutdown().await?.is_none());
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    std::fs::write(
        root.join("expected-routing.json"),
        serde_json::to_vec(&committed)?,
    )?;
    window.destroy()?;
    drop(window);
    drop(app);
    drop(host);
    Ok(())
}
