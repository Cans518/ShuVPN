fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri_build::build();
    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows" {
        // Tauri's resource includes Common Controls v6. Its build helper links
        // that manifest to applications, while the IPC test executable needs it too.
        let output = std::env::var("OUT_DIR")?;
        println!("cargo:rustc-link-arg-tests={output}/resource.lib");
    }
    Ok(())
}
