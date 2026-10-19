mod appearance;
mod commands;
mod error;
pub mod host;
mod installation;
mod settings;
mod startup;
mod tray;

pub use commands::handler as command_handler;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Emitter, Manager};

struct TrayAvailable(AtomicBool);

fn hide_window<R: tauri::Runtime>(window: &tauri::Window<R>) -> tauri::Result<()> {
    if window.state::<TrayAvailable>().0.load(Ordering::Acquire) {
        window.hide()
    } else {
        // Retain a taskbar restore path when a tray host could not be created.
        window.minimize()
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = tauri::generate_context!();
    // Isolated profiles must not forward test or development launches to a
    // user's normal running instance.
    let arguments: Vec<_> = std::env::args_os().collect();
    if let Some(profile) = arguments.windows(2).find(|pair| pair[0] == "--data-dir") {
        use std::hash::{Hash, Hasher};
        let mut hash = std::hash::DefaultHasher::new();
        profile[1].hash(&mut hash);
        context
            .config_mut()
            .identifier
            .push_str(&format!(".profile{:016x}", hash.finish()));
    }
    let _installation = installation::InstallationGuard::acquire()?;
    let exiting = Arc::new(AtomicBool::new(false));
    let cleaned = Arc::new(AtomicBool::new(false));
    let retry = exiting.clone();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            if !arguments.iter().any(|argument| argument == "--hidden") {
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(error) = window
                        .show()
                        .and_then(|_| window.unminimize())
                        .and_then(|_| window.set_focus())
                    {
                        eprintln!("恢复窗口失败：{error}");
                    }
                }
            }
        }))
        .manage(TrayAvailable(AtomicBool::new(false)));
    let app = builder
        .setup(|app| {
            let arguments: Vec<_> = std::env::args_os().skip(1).collect();
            let hidden = arguments.iter().any(|arg| arg == "--hidden");
            let arguments: Vec<_> = arguments
                .into_iter()
                .filter(|arg| arg != "--hidden")
                .collect();
            let directory = match arguments.as_slice() {
                [] => app.path().app_data_dir()?,
                [flag, directory] if flag == "--data-dir" => std::path::PathBuf::from(directory),
                _ => return Err("用法：shuvpn [--hidden] [--data-dir ABSOLUTE_DIRECTORY]".into()),
            };
            let host = tauri::async_runtime::block_on(host::DesktopHost::open(&directory))?;
            app.manage(Arc::new(host));
            let configuration = app.config().app.windows.first().ok_or("主窗口配置缺失")?;
            if let Err(error) = tauri::WebviewWindowBuilder::from_config(app, configuration)?
                .data_directory(directory.join("webview"))
                .visible(!hidden)
                .build()
            {
                let host = app.state::<Arc<host::DesktopHost>>();
                tauri::async_runtime::block_on(host.shutdown())?;
                return Err(error.into());
            }
            if let Err(error) = tray::install(app) {
                // The window remains usable on desktops without a tray host.
                eprintln!("托盘不可用：{error}");
                if let Some(window) = app.get_webview_window("main") {
                    window.show()?;
                }
            } else {
                app.state::<TrayAvailable>()
                    .0
                    .store(true, Ordering::Release);
            }
            Ok(())
        })
        .invoke_handler(commands::handler())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = hide_window(window) {
                    eprintln!("隐藏窗口失败：{error}");
                }
            }
        })
        .build(context)?;
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if cleaned.load(Ordering::Acquire) {
                return;
            }
            api.prevent_exit();
            if exiting.swap(true, Ordering::AcqRel) {
                return;
            }
            let host = app.state::<Arc<host::DesktopHost>>().inner().clone();
            let app = app.clone();
            let retry = retry.clone();
            let cleaned = cleaned.clone();
            tauri::async_runtime::spawn(async move {
                match host.shutdown().await {
                    Ok(_) => {
                        cleaned.store(true, Ordering::Release);
                        app.exit(0);
                    }
                    Err(error) => {
                        retry.store(false, Ordering::Release);
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        if let Err(report_error) = app.emit("shutdown-error", &error) {
                            eprintln!("退出清理失败：{error}；错误通知失败：{report_error}");
                        }
                    }
                }
            });
        }
    });
    Ok(())
}
