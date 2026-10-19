use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

fn location() -> io::Result<PathBuf> {
    let directory = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".config"))
        })
        .ok_or_else(|| io::Error::other("无法取得用户配置目录"))?;
    Ok(directory.join("autostart/ShuVPN.desktop"))
}

fn entry(executable: &Path) -> io::Result<String> {
    let path = executable
        .to_str()
        .ok_or_else(|| io::Error::other("启动路径无法编码"))?;
    let mut quoted = String::from("\"");
    for character in path.chars() {
        match character {
            // Desktop Entry string escaping precedes Exec argument unquoting.
            '\\' => quoted.push_str(r"\\\\"),
            '"' | '$' | '`' => {
                quoted.push_str(r"\\");
                quoted.push(character);
            }
            '%' => quoted.push_str("%%"),
            '=' => {
                return Err(io::Error::other(
                    "自启动路径不能包含等号，请移动程序后重新启用",
                ))
            }
            value if value.is_control() => {
                return Err(io::Error::other("自启动路径不能包含控制字符"))
            }
            value => quoted.push(value),
        }
    }
    quoted.push('"');
    Ok(format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=ShuVPN\nExec={quoted} --hidden\nTerminal=false\nStartupNotify=false\n"
    ))
}

fn enabled_at(path: &Path) -> io::Result<bool> {
    let entry = match std::fs::read_to_string(path) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(entry.lines().any(|line| line.starts_with("Exec="))
        && !entry.lines().any(|line| {
            matches!(
                line.trim(),
                "Hidden=true" | "X-GNOME-Autostart-enabled=false"
            )
        }))
}

fn set_at(path: &Path, executable: &Path, enabled: bool) -> io::Result<()> {
    if !enabled {
        return match std::fs::remove_file(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
    }
    let contents = entry(executable)?;
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("启动配置目录无效"))?;
    std::fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(contents.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub(super) fn enabled() -> io::Result<bool> {
    enabled_at(&location()?)
}

pub(super) fn set(enabled: bool) -> io::Result<()> {
    set_at(&location()?, &std::env::current_exe()?, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_handles_spaced_paths_desktop_disabling_and_idempotent_removal() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("config/autostart/ShuVPN.desktop");
        let executable = directory.path().join("Campus VPN/shuvpn");
        assert!(!enabled_at(&path)?);
        set_at(&path, &executable, true)?;
        assert!(enabled_at(&path)?);
        assert!(std::fs::read_to_string(&path)?
            .contains(&format!("Exec=\"{}\" --hidden", executable.display())));
        std::fs::write(&path, format!("{}Hidden=true\n", entry(&executable)?))?;
        assert!(!enabled_at(&path)?);
        set_at(&path, &executable, false)?;
        set_at(&path, &executable, false)?;
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn exec_quotes_reserved_characters_without_accepting_new_fields() -> io::Result<()> {
        assert!(entry(Path::new("/tmp/a\nExec=other")).is_err());
        let encoded = entry(Path::new("/tmp/100%/$campus\"`\\vpn"))?;
        assert!(encoded.contains(r#"Exec="/tmp/100%%/\\$campus\\"\\`\\\\vpn" --hidden"#));
        Ok(())
    }
}
