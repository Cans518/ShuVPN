use super::{
    model::{failure, SettingsFailure},
    schema::Values,
};
use crate::platform::files;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub format_version: u32,
    pub revision: u32,
    pub values: Values,
}
pub(super) struct Store {
    root: PathBuf,
    directory: File,
    lock: File,
    creator: u32,
    #[cfg(test)]
    pub fail_writes: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    pub fail_after_replace: std::sync::atomic::AtomicBool,
}
impl Drop for Store {
    fn drop(&mut self) {
        if self.creator == std::process::id() {
            let _ = files::unlock(&self.lock);
        }
    }
}
fn storage(error: std::io::Error) -> SettingsFailure {
    failure("storage", &error.to_string())
}
impl Store {
    pub(super) fn open(directory: String) -> Result<Self, SettingsFailure> {
        let root = PathBuf::from(directory);
        if !root.is_absolute() {
            return Err(failure("directory", "设置目录必须是绝对路径"));
        }
        std::fs::create_dir_all(&root).map_err(storage)?;
        let directory = files::directory(&root).map_err(storage)?;
        let root = root.canonicalize().map_err(storage)?;
        let path = root.join(".settings.lock");
        files::regular(&path).map_err(storage)?;
        let lock = files::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(storage)?;
        files::try_lock(&lock).map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => failure("locked", "设置目录正在被另一个实例使用"),
            std::fs::TryLockError::Error(error) => storage(error),
        })?;
        Ok(Self {
            root,
            directory,
            lock,
            creator: std::process::id(),
            #[cfg(test)]
            fail_writes: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_replace: std::sync::atomic::AtomicBool::new(false),
        })
    }
    fn check(&self) -> Result<(), SettingsFailure> {
        files::check_directory(&self.root, &self.directory).map_err(storage)?;
        files::same_file(&self.lock, &self.root.join(".settings.lock")).map_err(storage)
    }
    pub(super) fn load(&self) -> Result<Option<Record>, SettingsFailure> {
        self.check()?;
        let path = self.root.join("settings-state.json");
        if !files::regular(&path).map_err(storage)? {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        files::options()
            .read(true)
            .open(path)
            .map_err(storage)?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        if bytes.len() > 65536 {
            return Err(failure("size", "设置文件超过大小限制"));
        }
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|_| failure("format", "设置文件格式无效"))?;
        if record.format_version != 1
            || record.values.len() > 256
            || record
                .values
                .keys()
                .any(|key| !key.starts_with("settings."))
        {
            return Err(failure("format", "设置文件版本或内容无效"));
        }
        Ok(Some(record))
    }
    pub(super) fn save(&self, record: &Record) -> Result<(), SettingsFailure> {
        if record.values.len() > 256 {
            return Err(failure("size", "设置条目过多"));
        }
        self.check()?;
        #[cfg(test)]
        if self.fail_writes.load(std::sync::atomic::Ordering::Acquire) {
            return Err(failure("storage", "注入的设置写入失败"));
        }
        let bytes = serde_json::to_vec(record).map_err(|_| failure("format", "设置编码失败"))?;
        if bytes.len() > 65536 {
            return Err(failure("size", "设置文件超过大小限制"));
        }
        let target = self.root.join("settings-state.json");
        files::regular(&target).map_err(storage)?;
        let mut nonce = [0u8; 8];
        getrandom::getrandom(&mut nonce).map_err(|_| failure("storage", "设置临时文件创建失败"))?;
        let name: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let temporary = self.root.join(format!(".settings-{name}.tmp"));
        let mut replaced = false;
        let result = (|| {
            let mut file = files::options()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(storage)?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(storage)?;
            drop(file);
            files::replace(&temporary, &target).map_err(storage)?;
            replaced = true;
            #[cfg(test)]
            if self
                .fail_after_replace
                .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err(failure("uncertain", "无法确认设置保存结果，请重新启动应用"));
            }
            files::sync(&self.directory)
                .map_err(|_| failure("uncertain", "无法确认设置保存结果，请重新启动应用"))?;
            Ok(())
        })();
        if result.is_err() && !replaced {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
}
