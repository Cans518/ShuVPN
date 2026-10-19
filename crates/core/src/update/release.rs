use super::version;
use serde_json::Value;
use std::io::Read;

#[derive(Clone, Copy)]
enum Distribution {
    Portable,
    Inno,
    Debian,
    Unsupported,
}

fn distribution(marker: &str) -> Distribution {
    match marker.trim() {
        "inno" => Distribution::Inno,
        "dpkg" => Distribution::Debian,
        _ => Distribution::Unsupported,
    }
}

fn installed_distribution() -> Distribution {
    let Ok(executable) = std::env::current_exe() else {
        return Distribution::Unsupported;
    };
    let Some(directory) = executable.parent() else {
        return Distribution::Unsupported;
    };
    distribution_at(directory)
}

fn distribution_at(directory: &std::path::Path) -> Distribution {
    let mut marker = String::new();
    let file = match std::fs::File::open(directory.join(".package-manager")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Distribution::Portable
        }
        Err(_) => return Distribution::Unsupported,
    };
    if file.take(33).read_to_string(&mut marker).is_ok() && marker.len() <= 32 {
        distribution(&marker)
    } else {
        Distribution::Unsupported
    }
}

pub struct UpdateInfo {
    pub latest_version: String,
    pub download_url: String,
    pub release_page_url: String,
    pub target_url: String,
    pub published_at: Option<String>,
}

pub(super) fn from_release(release: &Value, current: &str) -> Option<UpdateInfo> {
    let tag = release.get("tag_name").and_then(Value::as_str)?.trim();
    let latest = tag.strip_prefix(['v', 'V']).unwrap_or(tag);
    if latest.is_empty() || version::compare(latest, current) != std::cmp::Ordering::Greater {
        return None;
    }
    let download_url = pick_desktop(
        &release["assets"],
        std::env::consts::OS,
        installed_distribution(),
    );
    let release_page_url = release["html_url"].as_str().unwrap_or("").to_owned();
    let published_at = release["published_at"]
        .as_str()
        .and_then(|raw| {
            cookie::time::OffsetDateTime::parse(
                raw,
                &cookie::time::format_description::well_known::Rfc3339,
            )
            .ok()
        })
        .and_then(|date| {
            date.format(&cookie::time::format_description::well_known::Rfc3339)
                .ok()
        });
    Some(UpdateInfo {
        latest_version: latest.to_owned(),
        target_url: if download_url.trim().is_empty() {
            release_page_url.clone()
        } else {
            download_url.trim().to_owned()
        },
        download_url,
        release_page_url,
        published_at,
    })
}

fn pick_desktop(assets: &Value, platform: &str, distribution: Distribution) -> String {
    let suffix = match (platform, distribution) {
        ("windows", Distribution::Portable) => "-windows-x64.zip",
        ("windows", Distribution::Inno) => "-windows-x64-setup.exe",
        ("linux", Distribution::Portable) => "-linux-x64.tar.gz",
        ("linux", Distribution::Debian) => "-linux-x64.deb",
        _ => return String::new(),
    };
    let Some(assets) = assets.as_array() else {
        return String::new();
    };
    assets
        .iter()
        .find_map(|asset| {
            let name = asset["name"].as_str()?.to_lowercase();
            let url = asset["browser_download_url"].as_str()?.trim();
            (name.ends_with(suffix) && !url.is_empty()).then(|| url.to_owned())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn desktop_selection_skips_checksums_and_other_platforms() -> anyhow::Result<()> {
        let mut release = json!({"tag_name":"v0.4.0","html_url":"https://github.com/example/release",
        "published_at":"2026-10-03T19:12:25Z","assets":[
            {"name":"app-windows-x64.zip.sha256","browser_download_url":"checksum"},
            {"name":"app-linux-x64.tar.gz","browser_download_url":"linux"},
            {"name":"APP-WINDOWS-X64.ZIP","browser_download_url":"windows"},
            {"name":"app.apk","browser_download_url":"legacy"}
        ]});
        assert_eq!(
            pick_desktop(&release["assets"], "windows", Distribution::Portable),
            "windows"
        );
        assert_eq!(
            pick_desktop(&release["assets"], "linux", Distribution::Portable),
            "linux"
        );
        assert!(pick_desktop(&release["assets"], "unsupported", Distribution::Portable).is_empty());
        let info =
            from_release(&release, "0.3.0").ok_or_else(|| anyhow::anyhow!("missing release"))?;
        assert_eq!(info.download_url, std::env::consts::OS);
        assert_eq!(info.published_at.as_deref(), Some("2026-10-03T19:12:25Z"));
        release["assets"] = json!([]);
        let info =
            from_release(&release, "0.3.0").ok_or_else(|| anyhow::anyhow!("missing fallback"))?;
        assert_eq!(info.target_url, "https://github.com/example/release");
        assert!(from_release(&release, "0.4.0").is_none());
        release["tag_name"] = json!("latest");
        assert!(from_release(&release, "0.3.0").is_none());
        Ok(())
    }

    #[test]
    fn installed_updates_use_their_manager_and_never_a_portable_overwrite() -> anyhow::Result<()> {
        let assets = json!([
            {"name":"app-windows-x64.zip","browser_download_url":"zip"},
            {"name":"app-linux-x64.tar.gz","browser_download_url":"tar"},
            {"name":"app-windows-x64-setup.exe.sha256","browser_download_url":"checksum"},
            {"name":"app-linux-x64.deb.sha256","browser_download_url":"checksum"},
            {"name":"app-windows-x64-setup.exe","browser_download_url":"setup"},
            {"name":"app-linux-x64.deb","browser_download_url":"deb"}
        ]);
        assert_eq!(
            pick_desktop(&assets, "windows", distribution("inno\n")),
            "setup"
        );
        assert_eq!(
            pick_desktop(&assets, "linux", distribution("dpkg\n")),
            "deb"
        );
        assert!(pick_desktop(&assets, "linux", distribution("unknown")).is_empty());
        let portable = json!([{"name":"app-linux-x64.tar.gz","browser_download_url":"tar"}]);
        assert!(pick_desktop(&portable, "linux", Distribution::Debian).is_empty());
        let directory = tempfile::tempdir()?;
        assert_eq!(
            pick_desktop(&assets, "linux", distribution_at(directory.path())),
            "tar"
        );
        std::fs::write(directory.path().join(".package-manager"), "dpkg\n")?;
        assert_eq!(
            pick_desktop(&assets, "linux", distribution_at(directory.path())),
            "deb"
        );
        std::fs::write(
            directory.path().join(".package-manager"),
            format!("dpkg{}", " ".repeat(100)),
        )?;
        assert!(pick_desktop(&assets, "linux", distribution_at(directory.path())).is_empty());
        Ok(())
    }

    #[test]
    fn absent_null_and_non_string_release_tags_do_not_offer_an_update() {
        for release in [
            json!({}),
            json!({"tag_name":null}),
            json!({"tag_name":123}),
            json!({"tag_name":""}),
            json!({"tag_name":"  "}),
        ] {
            assert!(from_release(&release, "0.3.0").is_none());
        }
    }
}
