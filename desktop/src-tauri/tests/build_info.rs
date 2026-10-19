use std::process::Command;

#[test]
fn build_info_identifies_version_and_profile_without_a_display(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_shuvpn"))
        .arg("--build-info")
        .env("DISPLAY", "")
        .output()?;
    assert!(output.status.success());
    let info: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        info["profile"],
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    assert_eq!(info["platform"], std::env::consts::OS);
    assert_eq!(info["architecture"], std::env::consts::ARCH);
    assert_eq!(
        info["frontend"],
        if tauri::is_dev() {
            "dev-server"
        } else {
            "embedded"
        }
    );
    Ok(())
}

#[test]
fn version_command_does_not_start_the_gui() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_shuvpn"))
        .arg("--version")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?.trim(),
        format!("ShuVPN {}", env!("CARGO_PKG_VERSION"))
    );
    Ok(())
}
