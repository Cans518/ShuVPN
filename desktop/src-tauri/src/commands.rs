use crate::{
    error::{CommandError, CommandResult},
    host::{DesktopHost, PreparedRouting},
    settings::{self, SettingEdit},
};
use serde_json::{json, Value};
use shuvpn_core::api::{
    account::{QrSignInPhase, SignInState},
    desktop, routing,
};
use std::sync::Arc;
use tauri::State;

type Host<'a> = State<'a, Arc<DesktopHost>>;

pub fn handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
{
    tauri::generate_handler![
        desktop_snapshot,
        save_settings,
        sign_in,
        sign_in_qr,
        send_code,
        verify_code,
        cancel_account_work,
        sign_out,
        connect,
        disconnect,
        network_interfaces,
        routing_current,
        routing_template,
        routing_prepare,
        routing_commit,
        routing_discard,
        routing_rollback,
        connection_records,
        appearance_current,
        appearance_save,
        exit_application,
        hide_window,
        startup_enabled,
        startup_set,
    ]
}

#[tauri::command]
pub fn startup_enabled() -> CommandResult<bool> {
    crate::startup::enabled()
}

#[tauri::command]
pub fn startup_set(enabled: bool) -> CommandResult<()> {
    crate::startup::set(enabled)
}

#[tauri::command]
pub async fn appearance_current(host: Host<'_>) -> CommandResult<crate::appearance::Appearance> {
    let _permit = host.enter().await?;
    Ok(host.appearance.snapshot().await)
}

#[tauri::command]
pub async fn appearance_save(
    host: Host<'_>,
    appearance: crate::appearance::Appearance,
) -> CommandResult<crate::appearance::Appearance> {
    let _permit = host.enter().await?;
    host.appearance.save(appearance).await
}

#[tauri::command]
pub fn exit_application<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    app.exit(0);
}

#[tauri::command]
pub fn hide_window<R: tauri::Runtime>(window: tauri::Window<R>) -> CommandResult<()> {
    crate::hide_window(&window).map_err(|error| CommandError::new("window", &error.to_string()))
}

#[tauri::command]
pub async fn desktop_snapshot(host: Host<'_>) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    host.snapshot().await
}

#[tauri::command]
pub async fn save_settings(host: Host<'_>, edits: Vec<SettingEdit>) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    let _configuration = host.configuration.lock().await;
    host.require_disconnected().await?;
    settings::validate_batch(&edits)?;
    let saved = host
        .settings
        .apply(edits.into_iter().map(SettingEdit::into_core).collect())
        .await?;
    Ok(settings::snapshot(&saved))
}

#[tauri::command]
pub async fn sign_in(host: Host<'_>, username: String, password: String) -> CommandResult<String> {
    let _permit = host.enter().await?;
    match host.account.sign_in_password(username, password).await? {
        SignInState::Completed => Ok("completed".into()),
        SignInState::VerificationRequired => Ok("verificationRequired".into()),
    }
}

#[tauri::command]
pub async fn sign_in_qr(host: Host<'_>, events: tauri::ipc::Channel<Value>) -> CommandResult<bool> {
    let _permit = host.enter().await?;
    Ok(host
        .account
        .sign_in_qr(move |event| {
            let phase = match event.phase {
                QrSignInPhase::Waiting => "waiting",
                QrSignInPhase::Redeeming => "redeeming",
                QrSignInPhase::Credentials => "credentials",
                QrSignInPhase::Completed => "completed",
                QrSignInPhase::Unavailable => "unavailable",
                QrSignInPhase::Failed => "failed",
            };
            events
                .send(json!({
                    "phase": phase,
                    "image": event.image.map(|image| image.image_bytes),
                }))
                .is_ok()
        })
        .await?)
}

#[tauri::command]
pub async fn send_code(host: Host<'_>, method: String) -> CommandResult<()> {
    let _permit = host.enter().await?;
    host.account.send_code(method).await?;
    Ok(())
}

#[tauri::command]
pub async fn verify_code(host: Host<'_>, method: String, code: String) -> CommandResult<()> {
    let _permit = host.enter().await?;
    host.account.sign_in_code(method, code).await?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_account_work(host: Host<'_>) -> CommandResult<()> {
    let _permit = host.enter().await?;
    host.account.cancel_work()?;
    Ok(())
}

#[tauri::command]
pub async fn sign_out(host: Host<'_>) -> CommandResult<Option<String>> {
    let _permit = host.enter().await?;
    host.account.cancel_work()?;
    host.owner.cancel()?;
    let _configuration = host.configuration.lock().await;
    let diagnostic = host.disconnect().await?;
    host.account.sign_out()?;
    Ok(diagnostic)
}

#[tauri::command]
pub async fn connect(
    host: Host<'_>,
    physical_interface: String,
    physical_dns: Option<String>,
) -> CommandResult<()> {
    let _permit = host.enter().await?;
    host.connect(physical_interface, physical_dns).await
}

#[tauri::command]
pub async fn disconnect(host: Host<'_>) -> CommandResult<Option<String>> {
    let _permit = host.enter().await?;
    host.disconnect().await
}

#[tauri::command]
pub async fn network_interfaces(host: Host<'_>) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    let interfaces = desktop::desktop_network_interfaces().await?;
    Ok(json!(interfaces
        .into_iter()
        .map(|interface| json!({
            "name": interface.name,
            "label": interface.description,
            "recommended": interface.recommended,
        }))
        .collect::<Vec<_>>()))
}

#[tauri::command]
pub async fn routing_current(host: Host<'_>) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    decode(&host.routing.current()?)
}

#[tauri::command]
pub async fn routing_template(host: Host<'_>, name: String) -> CommandResult<String> {
    let _permit = host.enter().await?;
    Ok(routing::profile_template(name)?)
}

#[tauri::command]
pub async fn routing_prepare(
    host: Host<'_>,
    profile: String,
    overrides: String,
    expected_version: u64,
) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    let mut preview = host.preview.lock().await;
    preview.take();
    let prepared = host
        .routing
        .prepare(profile, overrides, expected_version)
        .await?;
    let info = decode(&prepared.info()?)?;
    let sequence = host
        .preview_sequence
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |value| value.checked_add(1),
        )
        .map_err(|_| CommandError::new("preview", "预览标识已耗尽，请重新启动应用"))?;
    let id = sequence.to_string();
    *preview = Some(PreparedRouting {
        id: id.clone(),
        preview: prepared,
    });
    Ok(json!({"previewId": id, "configuration": info}))
}

#[tauri::command]
pub async fn routing_commit(host: Host<'_>, preview_id: String) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    let mut current = host.preview.lock().await;
    if current
        .as_ref()
        .is_none_or(|preview| preview.id != preview_id)
    {
        return Err(CommandError::new(
            "preview",
            "预览已失效，请重新校验当前草稿",
        ));
    }
    let confirmed = current
        .take()
        .ok_or_else(|| CommandError::new("preview", "请先校验当前草稿并生成预览"))?;
    decode(&confirmed.preview.commit().await?)
}

#[tauri::command]
pub async fn routing_discard(host: Host<'_>, preview_id: String) -> CommandResult<()> {
    let _permit = host.enter().await?;
    let mut current = host.preview.lock().await;
    if current
        .as_ref()
        .is_some_and(|preview| preview.id == preview_id)
    {
        current.take();
    }
    Ok(())
}

#[tauri::command]
pub async fn routing_rollback(host: Host<'_>, expected_version: u64) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    host.preview.lock().await.take();
    decode(&host.routing.rollback(expected_version).await?)
}

#[tauri::command]
pub async fn connection_records(host: Host<'_>) -> CommandResult<Value> {
    let _permit = host.enter().await?;
    let current = host.session.read().await;
    match current.as_ref() {
        Some(session) if host.owner.snapshot()?.run_id == Some(session.run_id()) => {
            decode(&session.decisions()?)
        }
        _ => Ok(json!([])),
    }
}

fn decode(encoded: &str) -> CommandResult<Value> {
    serde_json::from_str(encoded).map_err(|_| CommandError::new("encoding", "原生响应格式无效"))
}
