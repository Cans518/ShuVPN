use crate::{error::CommandResult, host::DesktopHost};
use shuvpn_core::api::{
    desktop_connection::{OwnerPhase, OwnerSnapshot},
    settings::{Edit, SettingKey},
};
use std::sync::Arc;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window
            .show()
            .and_then(|_| window.unminimize())
            .and_then(|_| window.set_focus())
        {
            eprintln!("打开窗口失败：{error}");
        }
    }
}

fn report(app: &tauri::AppHandle, error: impl std::fmt::Display) {
    show(app);
    if let Err(report_error) = app.emit("shutdown-error", error.to_string()) {
        eprintln!("{error}；通知失败：{report_error}");
    }
}

fn phase_label(phase: OwnerPhase) -> &'static str {
    match phase {
        OwnerPhase::Disconnected => "尚未连接",
        OwnerPhase::Connecting => "正在连接",
        OwnerPhase::Connected => "已连接校园网络",
        OwnerPhase::Disconnecting => "正在断开并清理",
        OwnerPhase::Failed => "连接需要处理",
    }
}

// A filled 16-pixel shield carries a white glyph so state does not rely on color.
fn icon(phase: OwnerPhase) -> tauri::image::Image<'static> {
    let color = match phase {
        OwnerPhase::Connected => [21, 153, 102],
        OwnerPhase::Connecting | OwnerPhase::Disconnecting => [204, 139, 23],
        OwnerPhase::Failed => [210, 61, 75],
        OwnerPhase::Disconnected => [91, 114, 144],
    };
    let mut pixels = vec![0; 32 * 32 * 4];
    for y in 0usize..16 {
        for x in 0usize..16 {
            let shield = (2..=10).contains(&y) && (2..=13).contains(&x)
                || (11..=14).contains(&y) && x >= y - 8 && x <= 23 - y;
            if !shield {
                continue;
            }
            let glyph = match phase {
                OwnerPhase::Connected => {
                    (4..=7).contains(&x) && y.abs_diff(x + 3) <= 1
                        || (7..=12).contains(&x) && (x + y).abs_diff(17) <= 1
                }
                OwnerPhase::Disconnected => (4..=11).contains(&x) && (x + y).abs_diff(15) <= 1,
                OwnerPhase::Failed => {
                    (5..=10).contains(&x) && (x.abs_diff(y) <= 1 || (x + y).abs_diff(15) <= 1)
                }
                OwnerPhase::Connecting | OwnerPhase::Disconnecting => {
                    (7..=8).contains(&y) && [4, 5, 7, 8, 10, 11].contains(&x)
                }
            };
            for dy in 0..2 {
                for dx in 0..2 {
                    let offset = ((y * 2 + dy) * 32 + x * 2 + dx) * 4;
                    pixels[offset..offset + 3].copy_from_slice(if glyph {
                        &[255, 255, 255]
                    } else {
                        &color
                    });
                    pixels[offset + 3] = 255;
                }
            }
        }
    }
    tauri::image::Image::new_owned(pixels, 32, 32)
}

async fn toggle_setting(host: &DesktopHost, id: &str) -> CommandResult<()> {
    let _permit = host.enter().await?;
    let _configuration = host.configuration.lock().await;
    host.require_disconnected().await?;
    let settings = host.settings.snapshot()?;
    let (key, enabled) = match id {
        "http" => (SettingKey::HttpProxyEnabled, settings.http_proxy_enabled),
        "socks" => (SettingKey::SocksProxyEnabled, settings.socks_proxy_enabled),
        "system" => (
            SettingKey::SystemProxyEnabled,
            settings.system_proxy_enabled,
        ),
        _ => (SettingKey::VpnEnabled, settings.vpn_enabled),
    };
    host.settings
        .apply(vec![Edit {
            key,
            value_json: (!enabled).to_string(),
        }])
        .await?;
    Ok(())
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "尚未连接", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "show", "打开 ShuVPN", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)?;
    let connect = MenuItem::with_id(app, "connect", "连接校园网络", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, "disconnect", "断开连接并清理", false, None::<&str>)?;
    let http = CheckMenuItem::with_id(app, "http", "HTTP 代理", true, false, None::<&str>)?;
    let socks = CheckMenuItem::with_id(app, "socks", "SOCKS5 代理", true, false, None::<&str>)?;
    let system = CheckMenuItem::with_id(
        app,
        "system",
        "连接时启用系统代理",
        true,
        false,
        None::<&str>,
    )?;
    let tun = CheckMenuItem::with_id(app, "tun", "连接时启用 TUN", true, false, None::<&str>)?;
    let proxies = Submenu::with_items(app, "下次连接配置", true, &[&http, &socks, &system, &tun])?;
    let account = MenuItem::with_id(app, "account", "校园账户", true, None::<&str>)?;
    let routing = MenuItem::with_id(app, "routing", "规则分流", true, None::<&str>)?;
    let records = MenuItem::with_id(app, "records", "连接记录", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置与外观", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "开机自启动",
        true,
        crate::startup::enabled().unwrap_or(false),
        None::<&str>,
    )?;
    let exit = MenuItem::with_id(app, "exit", "退出 ShuVPN", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &hide,
            &connect,
            &disconnect,
            &proxies,
            &PredefinedMenuItem::separator(app)?,
            &account,
            &routing,
            &records,
            &settings,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &exit,
        ],
    )?;
    let tray = TrayIconBuilder::with_id("main")
        .tooltip("ShuVPN · 尚未连接")
        .icon(icon(OwnerPhase::Disconnected))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "show" => show(app),
                "hide" => {
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(error) = window.hide() {
                            report(app, error);
                        }
                    }
                }
                "exit" => app.exit(0),
                "account" | "routing" | "records" | "settings" => {
                    show(app);
                    if let Err(error) = app.emit("navigate", id) {
                        report(app, error);
                    }
                }
                "autostart" => {
                    let result =
                        crate::startup::enabled().and_then(|enabled| crate::startup::set(!enabled));
                    if let Err(error) = result {
                        report(app, error);
                    }
                }
                "connect" | "disconnect" | "http" | "socks" | "system" | "tun" => {
                    let app = app.clone();
                    let id = id.to_owned();
                    let host = app.state::<Arc<DesktopHost>>().inner().clone();
                    tauri::async_runtime::spawn(async move {
                        let result = async {
                            if id == "connect" {
                                if !host.account.snapshot()?.signed_in {
                                    show(&app);
                                    app.emit("navigate", "account").map_err(|error| {
                                        crate::error::CommandError::new("tray", &error.to_string())
                                    })?;
                                    return Ok(());
                                }
                                let _permit = host.enter().await?;
                                host.connect(String::new(), None).await
                            } else if id == "disconnect" {
                                let _permit = host.enter().await?;
                                if let Some(diagnostic) = host.disconnect().await? {
                                    report(&app, diagnostic);
                                }
                                Ok(())
                            } else {
                                toggle_setting(&host, &id).await
                            }
                        }
                        .await;
                        if let Err(error) = result {
                            report(&app, error);
                        }
                    });
                }
                _ => {}
            }
        })
        .build(app)?;
    let host = app.state::<Arc<DesktopHost>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let mut events = host.owner.events();
        let mut last_phase = None;
        loop {
            let state: OwnerSnapshot = events.borrow_and_update().clone();
            if state.closed {
                break;
            }
            let update = || -> Result<(), Box<dyn std::error::Error>> {
                if last_phase != Some(state.phase) {
                    tray.set_icon(Some(icon(state.phase)))?;
                    tray.set_tooltip(Some(format!("ShuVPN · {}", phase_label(state.phase))))?;
                    status.set_text(phase_label(state.phase))?;
                    disconnect.set_text(if state.phase == OwnerPhase::Connecting {
                        "取消连接"
                    } else {
                        "断开连接并清理"
                    })?;
                }
                let idle = matches!(state.phase, OwnerPhase::Disconnected | OwnerPhase::Failed)
                    && !state.cleanup_pending;
                connect.set_enabled(idle)?;
                disconnect.set_enabled(!idle && state.phase != OwnerPhase::Disconnecting)?;
                let current = host.settings.snapshot()?;
                for (item, checked) in [
                    (&http, current.http_proxy_enabled),
                    (&socks, current.socks_proxy_enabled),
                    (&system, current.system_proxy_enabled),
                    (&tun, current.vpn_enabled),
                ] {
                    item.set_checked(checked)?;
                    item.set_enabled(idle)?;
                }
                if let Ok(enabled) = crate::startup::enabled() {
                    autostart.set_checked(enabled)?;
                }
                Ok(())
            };
            if let Err(error) = update() {
                eprintln!("托盘状态更新失败：{error}");
            }
            last_phase = Some(state.phase);
            tokio::select! {
                changed = events.changed() => if changed.is_err() { break; },
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taskbar_icons_have_opaque_glyphs_and_distinct_states() {
        let disconnected = icon(OwnerPhase::Disconnected);
        for phase in [
            OwnerPhase::Connected,
            OwnerPhase::Connecting,
            OwnerPhase::Failed,
        ] {
            let image = icon(phase);
            assert_ne!(image.rgba(), disconnected.rgba());
            assert!(
                image
                    .rgba()
                    .chunks_exact(4)
                    .filter(|pixel| pixel[3] == 255)
                    .count()
                    > 450
            );
            assert!(image
                .rgba()
                .chunks_exact(4)
                .any(|pixel| pixel == [255, 255, 255, 255]));
        }
    }
}
