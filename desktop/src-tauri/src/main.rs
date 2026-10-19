#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == "--version" {
        println!("ShuVPN {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if arguments.len() == 1 && arguments[0] == "--build-info" {
        println!(
            "{}",
            serde_json::json!({
                "schema": 1,
                "product": "ShuVPN",
                "version": env!("CARGO_PKG_VERSION"),
                "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "frontend": if tauri::is_dev() { "dev-server" } else { "embedded" },
                "platform": std::env::consts::OS,
                "architecture": std::env::consts::ARCH,
            })
        );
        return;
    }
    if let Err(error) = shuvpn_desktop::run() {
        eprintln!("ShuVPN 启动失败：{error}");
        std::process::exit(1);
    }
}
