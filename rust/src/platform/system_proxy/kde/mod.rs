use super::{failure, Settings};
use crate::connection::model::VpnFailure;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

mod command;

#[derive(Clone)]
pub(super) struct Tools {
    reader: PathBuf,
    writer: PathBuf,
    config: PathBuf,
}

impl Tools {
    pub(super) fn detect() -> Result<Self, VpnFailure> {
        if std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .unwrap_or_default()
            .is_empty()
        {
            return Err(failure("当前 KDE 会话没有可用的桌面 D-Bus"));
        }
        let config_home =
            match std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
                Some(path) => PathBuf::from(path),
                None => PathBuf::from(
                    std::env::var_os("HOME").ok_or_else(|| failure("无法定位 KDE 用户配置目录"))?,
                )
                .join(".config"),
            };
        if !config_home.is_absolute() {
            return Err(failure("KDE 用户配置目录必须是绝对路径"));
        }
        let (reader, writer) = [6, 5]
            .into_iter()
            .find_map(|version| {
                let reader = PathBuf::from(format!("/usr/bin/kreadconfig{version}"));
                let writer = PathBuf::from(format!("/usr/bin/kwriteconfig{version}"));
                (reader.is_file() && writer.is_file()).then_some((reader, writer))
            })
            .ok_or_else(|| failure("KDE 系统代理需要 KConfig 5/6 读写工具"))?;
        if !Path::new("/usr/bin/dbus-send").is_file() {
            return Err(failure("KDE 系统代理需要 dbus-send"));
        }
        command::run(&reader, &["--help".into()])?;
        command::run(&writer, &["--help".into()])?;
        Ok(Self {
            reader,
            writer,
            config: config_home.join("kioslaverc"),
        })
    }

    fn args(&self, file: &Path, key: &str) -> Vec<OsString> {
        vec![
            "--file".into(),
            file.as_os_str().to_owned(),
            "--group".into(),
            "Proxy Settings".into(),
            "--key".into(),
            key.into(),
        ]
    }

    fn check_metadata(&self) -> Result<(), VpnFailure> {
        if !self.config.exists() {
            return Ok(());
        }
        if std::fs::metadata(&self.config)
            .map_err(|_| failure("无法读取 KDE 配置元数据"))?
            .len()
            > 1024 * 1024
        {
            return Err(failure("KDE 配置文件超过读取预算"));
        }
        let text =
            std::fs::read_to_string(&self.config).map_err(|_| failure("无法读取 KDE 配置文件"))?;
        let mut proxy_group = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                proxy_group = line == "[Proxy Settings]" || line == "[Proxy Settings][$i]";
            } else if proxy_group {
                if let Some((key, _)) = line.split_once('=') {
                    let bare = key.split('[').next().unwrap_or("").trim();
                    if target(1).contains_key(bare)
                        && (key.contains("[$e]") || key.contains("[$d]"))
                    {
                        return Err(failure("KDE 代理字段含展开或删除标记，无法安全恢复"));
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn read(&self) -> Result<Settings, VpnFailure> {
        self.check_metadata()?;
        let mut random = [0; 16];
        getrandom::getrandom(&mut random).map_err(|_| failure("无法生成 KDE 配置读取标识"))?;
        let sentinel = format!("__SHUVPN_ABSENT_{random:02x?}__");
        let mut settings = Settings::new();
        for key in target(1).keys() {
            let mut args = self.args(&self.config, key);
            args.extend(["--default".into(), sentinel.clone().into()]);
            let output = command::run(&self.reader, &args)?;
            let output = output
                .strip_suffix('\n')
                .ok_or_else(|| failure("KDE 配置输出格式无效"))?;
            settings.insert(key.clone(), (output != sentinel).then(|| output.to_owned()));
        }
        Ok(settings)
    }

    pub(super) fn write(&self, settings: &Settings) -> Result<(), VpnFailure> {
        self.check_metadata()?;
        let mut keys: Vec<_> = settings.keys().collect();
        let enabling = settings.get("ProxyType").and_then(Option::as_deref) == Some("1");
        keys.sort_by_key(|key| {
            if key.as_str() == "ProxyType" {
                if enabling {
                    2
                } else {
                    0
                }
            } else {
                1
            }
        });
        for key in keys {
            if !target(1).contains_key(key) {
                return Err(failure("KDE 代理字段不支持"));
            }
            let value = &settings[key];
            // Relative writes obey the same cascading immutability policy as KIO.
            // Local deletes reveal inherited values instead of masking them.
            let file = if value.is_some() {
                Path::new("kioslaverc")
            } else {
                &self.config
            };
            let mut args = self.args(file, key);
            match value {
                Some(value) => {
                    args.extend(["--".into(), value.clone().into()]);
                }
                None => args.push("--delete".into()),
            }
            command::run(&self.writer, &args)?;
        }
        command::run(
            Path::new("/usr/bin/dbus-send"),
            &[
                "--session".into(),
                "--type=signal".into(),
                "/KIO/Scheduler".into(),
                "org.kde.KIO.Scheduler.reparseSlaveConfiguration".into(),
                "string:".into(),
            ],
        )?;
        Ok(())
    }
}

pub(super) fn target(port: u16) -> Settings {
    Settings::from([
        ("ProxyType".to_owned(), Some("1".to_owned())),
        (
            "httpProxy".to_owned(),
            Some(format!("http://127.0.0.1 {port}")),
        ),
        (
            "httpsProxy".to_owned(),
            Some(format!("http://127.0.0.1 {port}")),
        ),
        (
            "NoProxyFor".to_owned(),
            Some("localhost,127.0.0.1,::1".to_owned()),
        ),
        ("ReversedException".to_owned(), Some("false".to_owned())),
    ])
}
