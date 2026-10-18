use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::connection::model::VpnFailure;

#[cfg(target_os = "linux")]
mod gnome;
#[cfg(target_os = "linux")]
mod kde;
#[cfg(target_os = "windows")]
mod windows;

type Settings = BTreeMap<String, Option<String>>;

#[derive(serde::Serialize, serde::Deserialize)]
struct Journal {
    schema: u32,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    preserve: BTreeSet<String>,
    backend: String,
    original: Settings,
    applied: Settings,
}

pub(crate) struct Lease {
    backend: Backend,
    directory: PathBuf,
    _lock: File,
    journal: Option<Journal>,
    enabled: bool,
}

enum Backend {
    #[cfg(target_os = "windows")]
    Windows,
    #[cfg(target_os = "linux")]
    Gnome,
    #[cfg(target_os = "linux")]
    Kde(kde::Tools),
    #[cfg(test)]
    Fixture(std::sync::Arc<std::sync::Mutex<Settings>>),
    #[cfg(test)]
    FaultFixture(
        std::sync::Arc<std::sync::Mutex<Settings>>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ),
}

fn failure(message: &str) -> VpnFailure {
    VpnFailure::new("systemProxy", message)
}

impl Backend {
    fn detect() -> Result<Self, VpnFailure> {
        #[cfg(target_os = "windows")]
        {
            Ok(Self::Windows)
        }
        #[cfg(target_os = "linux")]
        {
            let desktop = std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if desktop.split(':').any(|part| part == "kde") {
                Ok(Self::Kde(kde::Tools::detect()?))
            } else if desktop.split(':').any(|part| part == "gnome") {
                gnome::available()?;
                Ok(Self::Gnome)
            } else {
                Err(failure("当前桌面环境尚未接入系统代理"))
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err(failure("此平台尚未接入系统代理"))
        }
    }

    fn named(name: &str) -> Result<Self, VpnFailure> {
        match name {
            #[cfg(target_os = "windows")]
            "windows" => Ok(Self::Windows),
            #[cfg(target_os = "linux")]
            "gnome" => {
                gnome::available()?;
                Ok(Self::Gnome)
            }
            #[cfg(target_os = "linux")]
            "kde" => Ok(Self::Kde(kde::Tools::detect()?)),
            _ => Err(failure("恢复记录的平台不匹配")),
        }
    }

    fn name(&self) -> &str {
        match *self {
            #[cfg(target_os = "windows")]
            Self::Windows => "windows",
            #[cfg(target_os = "linux")]
            Self::Gnome => "gnome",
            #[cfg(target_os = "linux")]
            Self::Kde(_) => "kde",
            #[cfg(test)]
            Self::Fixture(_) | Self::FaultFixture(_, _) => "fixture",
        }
    }

    fn target(&self, port: u16) -> Settings {
        #[cfg(not(any(target_os = "windows", target_os = "linux", test)))]
        let _ = port;
        match *self {
            #[cfg(target_os = "windows")]
            Self::Windows => windows::target(port),
            #[cfg(target_os = "linux")]
            Self::Gnome => gnome::target(port),
            #[cfg(target_os = "linux")]
            Self::Kde(_) => kde::target(port),
            #[cfg(test)]
            Self::Fixture(_) | Self::FaultFixture(_, _) => BTreeMap::from([
                ("mode".to_owned(), Some("manual".to_owned())),
                ("server".to_owned(), Some(format!("127.0.0.1:{port}"))),
            ]),
        }
    }

    fn read(&self) -> Result<Settings, VpnFailure> {
        match *self {
            #[cfg(target_os = "windows")]
            Self::Windows => windows::read(),
            #[cfg(target_os = "linux")]
            Self::Gnome => gnome::read(),
            #[cfg(target_os = "linux")]
            Self::Kde(ref tools) => tools.read(),
            #[cfg(test)]
            Self::Fixture(ref settings) | Self::FaultFixture(ref settings, _) => settings
                .lock()
                .map(|settings| settings.clone())
                .map_err(|_| failure("测试设置锁失败")),
        }
    }

    fn write(&self, settings: &Settings) -> Result<(), VpnFailure> {
        #[cfg(not(any(target_os = "windows", target_os = "linux", test)))]
        let _ = settings;
        match *self {
            #[cfg(target_os = "windows")]
            Self::Windows => windows::write(settings),
            #[cfg(target_os = "linux")]
            Self::Gnome => gnome::write(settings),
            #[cfg(target_os = "linux")]
            Self::Kde(ref tools) => tools.write(settings),
            #[cfg(test)]
            Self::Fixture(ref current) => {
                current
                    .lock()
                    .map_err(|_| failure("测试设置锁失败"))?
                    .extend(settings.clone());
                Ok(())
            }
            #[cfg(test)]
            Self::FaultFixture(ref current, ref failures) => {
                let mut current = current.lock().map_err(|_| failure("测试设置锁失败"))?;
                let failed = failures
                    .fetch_update(
                        std::sync::atomic::Ordering::SeqCst,
                        std::sync::atomic::Ordering::SeqCst,
                        |remaining| remaining.checked_sub(1),
                    )
                    .is_ok();
                if failed {
                    if settings.get("mode").and_then(Option::as_deref) == Some("manual") {
                        if let Some((key, value)) = settings.first_key_value() {
                            current.insert(key.clone(), value.clone());
                        }
                    }
                    Err(failure("注入部分写入失败"))
                } else {
                    current.extend(settings.clone());
                    Ok(())
                }
            }
        }
    }

    fn preserve_external_endpoints(&self, restore: &mut Settings, conflicts: &BTreeSet<String>) {
        #[cfg(not(any(target_os = "windows", target_os = "linux", test)))]
        let _ = (restore, conflicts);
        match *self {
            #[cfg(target_os = "windows")]
            Self::Windows => {
                if conflicts.contains("server") {
                    restore.remove("flags");
                }
            }
            #[cfg(target_os = "linux")]
            Self::Gnome => {
                for protocol in ["http", "https"] {
                    let host = format!("org.gnome.system.proxy.{protocol}/host");
                    let port = format!("org.gnome.system.proxy.{protocol}/port");
                    if conflicts.contains(&host) || conflicts.contains(&port) {
                        restore.remove(&host);
                        restore.remove(&port);
                        restore.remove("org.gnome.system.proxy/mode");
                    }
                }
            }
            #[cfg(target_os = "linux")]
            Self::Kde(_) => {
                if conflicts.contains("httpProxy") || conflicts.contains("httpsProxy") {
                    restore.remove("ProxyType");
                }
            }
            #[cfg(test)]
            Self::Fixture(_) | Self::FaultFixture(_, _) => {
                if conflicts.contains("server") {
                    restore.remove("mode");
                }
            }
        }
    }

    fn is_endpoint(&self, key: &str) -> bool {
        #[cfg(not(any(target_os = "windows", target_os = "linux", test)))]
        let _ = key;
        match *self {
            #[cfg(target_os = "windows")]
            Self::Windows => key == "server",
            #[cfg(target_os = "linux")]
            Self::Gnome => matches!(
                key,
                "org.gnome.system.proxy.http/host"
                    | "org.gnome.system.proxy.http/port"
                    | "org.gnome.system.proxy.https/host"
                    | "org.gnome.system.proxy.https/port"
            ),
            #[cfg(target_os = "linux")]
            Self::Kde(_) => matches!(key, "httpProxy" | "httpsProxy"),
            #[cfg(test)]
            Self::Fixture(_) | Self::FaultFixture(_, _) => key == "server",
        }
    }
}

impl Lease {
    #[cfg(test)]
    pub(crate) fn fixture(
        directory: &str,
        settings: std::sync::Arc<std::sync::Mutex<Settings>>,
        failures: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> Result<Self, VpnFailure> {
        Self::open_with(directory, Some(Backend::FaultFixture(settings, failures)))
    }

    pub(crate) fn open(directory: &str) -> Result<Self, VpnFailure> {
        Self::open_with(directory, None)
    }

    fn open_with(directory: &str, backend: Option<Backend>) -> Result<Self, VpnFailure> {
        let directory = Path::new(directory);
        if !directory.is_absolute() {
            return Err(failure("系统代理恢复目录必须是绝对路径"));
        }
        std::fs::create_dir_all(directory).map_err(|_| failure("无法创建系统代理恢复目录"))?;
        let directory = directory
            .canonicalize()
            .map_err(|_| failure("系统代理恢复目录无效"))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(".system-proxy.lock"))
            .map_err(|_| failure("无法打开系统代理锁"))?;
        crate::platform::files::try_lock(&lock).map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => failure("系统代理恢复目录正在被另一个实例使用"),
            std::fs::TryLockError::Error(error) => {
                failure(&format!("无法锁定系统代理恢复目录：{error}"))
            }
        })?;
        let path = directory.join("system-proxy.json");
        let journal = if path.exists() {
            let actual = path
                .canonicalize()
                .map_err(|_| failure("系统代理恢复记录路径无效"))?;
            if !actual.starts_with(&directory)
                || std::fs::metadata(&actual)
                    .map_err(|_| failure("无法读取系统代理恢复记录"))?
                    .len()
                    > 256 * 1024
            {
                return Err(failure("系统代理恢复记录越界或超限"));
            }
            let bytes = std::fs::read(actual).map_err(|_| failure("无法读取系统代理恢复记录"))?;
            let journal: Journal =
                serde_json::from_slice(&bytes).map_err(|_| failure("系统代理恢复记录损坏"))?;
            if journal.schema != 1 {
                return Err(failure("系统代理恢复记录版本不支持"));
            }
            Some(journal)
        } else {
            None
        };
        let backend = match backend {
            Some(backend) => backend,
            None => match &journal {
                Some(journal) => Backend::named(&journal.backend)?,
                None => Backend::detect()?,
            },
        };
        let mut lease = Self {
            backend,
            directory,
            _lock: lock,
            journal,
            enabled: false,
        };
        lease.validate_journal()?;
        lease.restore()?;
        Ok(lease)
    }

    fn validate_journal(&self) -> Result<(), VpnFailure> {
        if let Some(journal) = &self.journal {
            self.validate_document(journal)?;
        }
        Ok(())
    }

    fn validate_document(&self, journal: &Journal) -> Result<(), VpnFailure> {
        let keys = self.backend.target(1);
        if journal.backend != self.backend.name()
            || journal.original.keys().ne(keys.keys())
            || journal.applied.keys().ne(keys.keys())
            || journal.preserve.iter().any(|key| !keys.contains_key(key))
            || journal
                .original
                .values()
                .chain(journal.applied.values())
                .flatten()
                .any(|value| value.len() > 65536 || value.contains('\0'))
        {
            return Err(failure("系统代理恢复字段无效"));
        }
        Ok(())
    }

    pub(crate) fn enable(&mut self, port: u16) -> Result<(), VpnFailure> {
        if port == 0 || self.journal.is_some() {
            return Err(failure("系统代理端口无效或仍有待恢复设置"));
        }
        let journal = Journal {
            schema: 1,
            active: false,
            preserve: BTreeSet::new(),
            backend: self.backend.name().to_owned(),
            original: self.backend.read()?,
            applied: self.backend.target(port),
        };
        self.validate_document(&journal)?;
        self.persist_document(&journal)?;
        self.journal = Some(journal);
        let result = self
            .backend
            .write(
                &self
                    .journal
                    .as_ref()
                    .ok_or_else(|| failure("恢复记录缺失"))?
                    .applied,
            )
            .and_then(|_| {
                let current = self.backend.read()?;
                if current
                    == self
                        .journal
                        .as_ref()
                        .ok_or_else(|| failure("恢复记录缺失"))?
                        .applied
                {
                    Ok(())
                } else {
                    Err(failure("系统代理设置未生效"))
                }
            })
            .and_then(|_| self.set_active(true));
        if let Err(error) = result {
            return match self.restore() {
                Ok(_) => Err(error),
                Err(restore) => Err(failure(&format!(
                    "{}；恢复也失败：{}",
                    error.message, restore.message
                ))),
            };
        }
        self.enabled = true;
        Ok(())
    }

    fn set_active(&mut self, active: bool) -> Result<(), VpnFailure> {
        let journal = self
            .journal
            .as_mut()
            .ok_or_else(|| failure("恢复记录缺失"))?;
        let previous = journal.active;
        if previous == active {
            return Ok(());
        }
        journal.active = active;
        let result = self.persist_document(
            self.journal
                .as_ref()
                .ok_or_else(|| failure("恢复记录缺失"))?,
        );
        if result.is_err() {
            if let Some(journal) = &mut self.journal {
                journal.active = previous;
            }
        }
        result
    }

    fn persist_document(&self, journal: &Journal) -> Result<(), VpnFailure> {
        let bytes = serde_json::to_vec(journal).map_err(|_| failure("无法编码系统代理恢复记录"))?;
        if bytes.len() > 256 * 1024 {
            return Err(failure("系统代理原设置超出恢复记录预算"));
        }
        let pending = self.directory.join("system-proxy.pending");
        // A previous interrupted save never reached the OS write.
        if pending.exists() {
            std::fs::remove_file(&pending).map_err(|_| failure("无法清理未提交的代理记录"))?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&pending)
            .map_err(|_| failure("无法创建系统代理恢复记录"))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| failure("无法保存系统代理恢复记录"))?;
        drop(file);
        std::fs::rename(pending, self.directory.join("system-proxy.json"))
            .map_err(|_| failure("无法提交系统代理恢复记录"))?;
        Ok(())
    }

    fn save_restore_plan(&mut self, preserve: BTreeSet<String>) -> Result<(), VpnFailure> {
        let journal = self
            .journal
            .as_mut()
            .ok_or_else(|| failure("恢复记录缺失"))?;
        let previous_active = journal.active;
        let previous_preserve = journal.preserve.clone();
        journal.active = false;
        journal.preserve.extend(preserve);
        let result = self.persist_document(
            self.journal
                .as_ref()
                .ok_or_else(|| failure("恢复记录缺失"))?,
        );
        if result.is_err() {
            if let Some(journal) = &mut self.journal {
                journal.active = previous_active;
                journal.preserve = previous_preserve;
            }
        }
        result
    }

    pub(crate) fn is_enabled(&self) -> Result<bool, VpnFailure> {
        if !self.enabled {
            return Ok(false);
        }
        match &self.journal {
            Some(journal) => Ok(self.backend.read()? == journal.applied),
            None => Ok(false),
        }
    }

    /// Only undo values still owned by this lease. External changes survive.
    pub(crate) fn restore(&mut self) -> Result<bool, VpnFailure> {
        let Some(was_active) = self.journal.as_ref().map(|journal| journal.active) else {
            return Ok(false);
        };
        // A successfully active lease has not started rollback yet. An endpoint
        // reverted externally is still an external choice, even if it equals
        // the saved value. Retry/replay must also allow partially restored fields.
        let journal = self
            .journal
            .as_ref()
            .ok_or_else(|| failure("恢复记录缺失"))?;
        let current = self.backend.read()?;
        let mut restore = Settings::new();
        let mut conflicts = BTreeSet::new();
        for (key, applied) in &journal.applied {
            if journal.preserve.contains(key) {
                continue;
            }
            let original = journal
                .original
                .get(key)
                .ok_or_else(|| failure("恢复记录字段缺失"))?;
            match current.get(key) {
                Some(value) if value == applied => {
                    restore.insert(key.clone(), original.clone());
                }
                Some(value) if value == original => {
                    if was_active && value != applied && self.backend.is_endpoint(key) {
                        conflicts.insert(key.clone());
                    }
                }
                _ => {
                    conflicts.insert(key.clone());
                }
            }
        }
        // A new external endpoint keeps its mode and endpoint pair. Changes
        // to bypass rules alone must not leave our stopped listener selected.
        let before_protection: BTreeSet<_> = restore.keys().cloned().collect();
        self.backend
            .preserve_external_endpoints(&mut restore, &conflicts);
        let mut preserve = conflicts.clone();
        preserve.extend(
            before_protection
                .into_iter()
                .filter(|key| !restore.contains_key(key)),
        );
        self.save_restore_plan(preserve)?;
        self.enabled = false;
        if !restore.is_empty() {
            self.backend.write(&restore)?;
            let current = self.backend.read()?;
            if restore
                .iter()
                .any(|(key, value)| current.get(key) != Some(value))
            {
                return Err(failure("系统代理恢复未生效，已保留恢复记录"));
            }
        }
        std::fs::remove_file(self.directory.join("system-proxy.json"))
            .map_err(|_| failure("无法清理已恢复的系统代理记录"))?;
        self.journal = None;
        self.enabled = false;
        Ok(!conflicts.is_empty())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        // A failed restore leaves the durable record for the next startup.
        let _ = self.restore();
        // A fork can temporarily inherit the same open file description before
        // CLOEXEC runs. Release ownership explicitly instead of waiting for all
        // inherited descriptors to close.
        let _ = crate::platform::files::unlock(&self._lock);
    }
}

pub(crate) fn recover(directory: &str) -> Result<(), VpnFailure> {
    // Do not require a desktop backend when there is nothing to recover.
    if !Path::new(directory).join("system-proxy.json").exists() {
        return Ok(());
    }
    Lease::open(directory).map(|_| ())
}

pub(crate) fn backend_name() -> Result<String, VpnFailure> {
    Backend::detect().map(|backend| backend.name().to_owned())
}

pub(crate) fn uses_port(port: u16) -> Result<bool, VpnFailure> {
    if port == 0 {
        return Ok(false);
    }
    let backend = Backend::detect()?;
    let current = backend.read()?;
    Ok(backend
        .target(port)
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "bypass"
                    | "NoProxyFor"
                    | "ReversedException"
                    | "org.gnome.system.proxy/ignore-hosts"
                    | "org.gnome.system.proxy.http/use-authentication"
            )
        })
        .all(|(key, value)| current.get(key) == Some(value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_a_lease_releases_the_lock_even_with_an_inherited_descriptor() -> anyhow::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().to_string_lossy().into_owned();
        let settings = std::sync::Arc::new(std::sync::Mutex::new(BTreeMap::from([
            ("mode".to_owned(), Some("auto".to_owned())),
            ("server".to_owned(), None),
        ])));
        let lease = Lease::open_with(&path, Some(Backend::Fixture(settings.clone())))?;
        let inherited = lease._lock.try_clone()?;
        drop(lease);
        let reopened = Lease::open_with(&path, Some(Backend::Fixture(settings)))?;
        drop(inherited);
        drop(reopened);
        Ok(())
    }

    #[test]
    fn an_external_endpoint_reverted_to_the_saved_value_keeps_its_active_mode() -> anyhow::Result<()>
    {
        for replay in 0..3 {
            let directory = tempfile::tempdir()?;
            let original = BTreeMap::from([
                ("mode".to_owned(), Some("none".to_owned())),
                ("server".to_owned(), Some("external:8080".to_owned())),
            ]);
            let settings = std::sync::Arc::new(std::sync::Mutex::new(original));
            let mut lease = Lease::open_with(
                &directory.path().to_string_lossy(),
                Some(Backend::Fixture(settings.clone())),
            )?;
            lease.enable(3322)?;
            settings
                .lock()
                .map_err(|_| anyhow::anyhow!("lock"))?
                .insert("server".to_owned(), Some("external:8080".to_owned()));
            if replay != 0 {
                if replay == 2 {
                    lease.save_restore_plan(BTreeSet::from([
                        "mode".to_owned(),
                        "server".to_owned(),
                    ]))?;
                }
                let path = directory.path().to_string_lossy().into_owned();
                lease.journal.take();
                drop(lease);
                let recovered = Lease::open_with(&path, Some(Backend::Fixture(settings.clone())))?;
                assert!(recovered.journal.is_none());
            } else {
                assert!(lease.restore()?);
            }
            assert_eq!(
                settings
                    .lock()
                    .map_err(|_| anyhow::anyhow!("lock"))?
                    .get("mode"),
                Some(&Some("manual".to_owned()))
            );
        }
        Ok(())
    }

    #[test]
    fn oversized_original_settings_fail_before_any_os_write() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let original = BTreeMap::from([
            ("mode".to_owned(), Some("auto".to_owned())),
            ("server".to_owned(), Some("x".repeat(65537))),
        ]);
        let settings = std::sync::Arc::new(std::sync::Mutex::new(original.clone()));
        let mut lease = Lease::open_with(
            &directory.path().to_string_lossy(),
            Some(Backend::Fixture(settings.clone())),
        )?;
        assert!(lease.enable(3322).is_err());
        assert_eq!(
            *settings.lock().map_err(|_| anyhow::anyhow!("lock"))?,
            original
        );
        assert!(!directory.path().join("system-proxy.json").exists());
        Ok(())
    }

    #[test]
    fn partial_write_failure_rolls_back_and_failed_rollback_retains_recovery() -> anyhow::Result<()>
    {
        for failure_count in [1, 3] {
            let directory = tempfile::tempdir()?;
            let path = directory.path().to_string_lossy().into_owned();
            let original = BTreeMap::from([
                ("mode".to_owned(), Some("auto".to_owned())),
                ("server".to_owned(), None),
            ]);
            let settings = std::sync::Arc::new(std::sync::Mutex::new(original.clone()));
            let failures = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(failure_count));
            let mut lease = Lease::open_with(
                &path,
                Some(Backend::FaultFixture(settings.clone(), failures)),
            )?;
            assert!(lease.enable(3322).is_err());
            assert!(!lease.is_enabled()?);
            assert_eq!(
                directory.path().join("system-proxy.json").exists(),
                failure_count > 1
            );
            drop(lease);
            assert_eq!(
                directory.path().join("system-proxy.json").exists(),
                failure_count > 1
            );
            let recovered = Lease::open_with(&path, Some(Backend::Fixture(settings.clone())))?;
            assert!(recovered.journal.is_none());
            assert_eq!(
                *settings.lock().map_err(|_| anyhow::anyhow!("lock"))?,
                original
            );
        }
        Ok(())
    }

    #[test]
    fn restore_preserves_external_changes_and_recovery_replays_partial_writes() -> anyhow::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().to_string_lossy().into_owned();
        let original = BTreeMap::from([
            ("mode".to_owned(), Some("auto".to_owned())),
            ("server".to_owned(), None),
        ]);
        let settings = std::sync::Arc::new(std::sync::Mutex::new(original.clone()));
        let mut lease = Lease::open_with(&path, Some(Backend::Fixture(settings.clone())))?;
        assert!(Lease::open_with(&path, Some(Backend::Fixture(settings.clone()))).is_err());
        lease.enable(3322)?;
        {
            let mut values = settings.lock().map_err(|_| anyhow::anyhow!("lock"))?;
            values.insert("mode".to_owned(), Some("external".to_owned()));
        }
        assert!(!lease.is_enabled()?);
        assert!(lease.restore()?);
        assert_eq!(
            settings
                .lock()
                .map_err(|_| anyhow::anyhow!("lock"))?
                .get("server"),
            Some(&None)
        );
        assert!(!directory.path().join("system-proxy.json").exists());
        *settings.lock().map_err(|_| anyhow::anyhow!("lock"))? = original.clone();
        lease.enable(3324)?;
        settings
            .lock()
            .map_err(|_| anyhow::anyhow!("lock"))?
            .insert("server".to_owned(), Some("external:8888".to_owned()));
        assert!(lease.restore()?);
        assert_eq!(
            settings
                .lock()
                .map_err(|_| anyhow::anyhow!("lock"))?
                .get("mode"),
            Some(&Some("manual".to_owned()))
        );
        assert_eq!(
            settings
                .lock()
                .map_err(|_| anyhow::anyhow!("lock"))?
                .get("server"),
            Some(&Some("external:8888".to_owned()))
        );
        *settings.lock().map_err(|_| anyhow::anyhow!("lock"))? = original;
        lease.enable(3323)?;
        // Simulate a crash after the journal and one OS field were committed.
        lease.set_active(false)?;
        settings
            .lock()
            .map_err(|_| anyhow::anyhow!("lock"))?
            .insert("server".to_owned(), None);
        lease.journal.take();
        drop(lease);
        let recovered = Lease::open_with(&path, Some(Backend::Fixture(settings.clone())))?;
        assert!(recovered.journal.is_none());
        assert_eq!(
            settings
                .lock()
                .map_err(|_| anyhow::anyhow!("lock"))?
                .get("mode"),
            Some(&Some("auto".to_owned()))
        );
        Ok(())
    }
}
