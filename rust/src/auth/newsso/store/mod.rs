use crate::auth::model::AuthFailure;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

mod platform;
#[cfg(test)]
mod tests;

const MAGIC: &[u8] = b"ShuVPN-SSO-1\n";
const LIMIT: usize = 256 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub version: u32,
    pub origin: String,
    pub legacy_imported: bool,
    pub import_open: bool,
    pub cookies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<serde_json::Value>,
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
        // Unlock explicitly: a forked child may still hold an inherited fd.
        if self.creator == std::process::id() {
            let _ = crate::platform::files::unlock(&self.lock);
        }
    }
}

impl Store {
    pub(super) fn open(directory: String) -> Result<Self, AuthFailure> {
        let root = PathBuf::from(directory);
        if !root.is_absolute() {
            return Err(failure("会话目录必须是绝对路径"));
        }
        if std::fs::symlink_metadata(&root).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(failure("会话目录不能是符号链接"));
        }
        std::fs::create_dir_all(&root).map_err(|_| failure("无法创建会话目录"))?;
        let directory = platform::directory(&root)?;
        let root = root
            .canonicalize()
            .map_err(|_| failure("无法定位会话目录"))?;
        let path = root.join(".newsso.lock");
        platform::regular(&path)?;
        let lock = platform::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|_| failure("无法打开会话目录锁"))?;
        crate::platform::files::try_lock(&lock).map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => failure("会话目录正在被另一个实例使用"),
            std::fs::TryLockError::Error(error) => failure(&format!("无法锁定会话目录：{error}")),
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

    pub(super) fn load(&self) -> Result<Option<Record>, AuthFailure> {
        platform::check_directory(&self.root, &self.directory)?;
        platform::same_file(&self.lock, &self.root.join(".newsso.lock"))?;
        let path = self.root.join("newsso-state.bin");
        if !platform::regular(&path)? {
            return Ok(None);
        }
        let file = platform::options()
            .read(true)
            .open(path)
            .map_err(|_| failure("无法读取会话文件"))?;
        let mut bytes = vec![];
        file.take((LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| failure("无法读取会话文件"))?;
        if bytes.len() > LIMIT || !bytes.starts_with(MAGIC) {
            return Err(failure("会话文件格式无效或过大"));
        }
        let bytes = platform::unseal(&bytes[MAGIC.len()..])?;
        if bytes.len() > LIMIT {
            return Err(failure("恢复后的会话数据超过限制"));
        }
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|_| failure("会话文件格式无效"))?;
        if !matches!(record.version, 1 | 2)
            || record.version == 1 && (record.device_id.is_some() || record.account.is_some())
            || record.device_id.as_ref().is_some_and(|id| {
                id.is_empty() || id.len() > 256 || id.chars().any(char::is_control)
            })
            || record.cookies.len() > 256
            || record.legacy_imported && record.import_open
            || !record.cookies.is_empty() && record.import_open
        {
            return Err(failure("会话文件版本或迁移状态无效"));
        }
        Ok(Some(record))
    }

    pub(super) fn save(&self, record: &Record) -> Result<(), AuthFailure> {
        platform::check_directory(&self.root, &self.directory)?;
        platform::same_file(&self.lock, &self.root.join(".newsso.lock"))?;
        #[cfg(test)]
        if self.fail_writes.load(std::sync::atomic::Ordering::Acquire) {
            return Err(failure("注入的会话写入失败"));
        }
        let bytes = zeroize::Zeroizing::new(
            serde_json::to_vec(record).map_err(|_| failure("无法编码会话数据"))?,
        );
        if bytes.len() > LIMIT - MAGIC.len() {
            return Err(failure("会话数据超过大小限制"));
        }
        let sealed = platform::seal(&bytes)?;
        if sealed.len() > LIMIT - MAGIC.len() {
            return Err(failure("保护后的会话数据超过大小限制"));
        }
        let target = self.root.join("newsso-state.bin");
        platform::regular(&target)?;
        let mut nonce = [0; 8];
        getrandom::getrandom(&mut nonce).map_err(|_| failure("无法生成会话临时文件"))?;
        let name: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let temporary = self.root.join(format!(".newsso-{name}.tmp"));
        let result = (|| {
            let mut file = platform::options()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|_| failure("无法创建会话临时文件"))?;
            file.write_all(MAGIC)
                .and_then(|_| file.write_all(&sealed))
                .and_then(|_| file.sync_all())
                .map_err(|_| failure("无法同步会话数据"))?;
            drop(file);
            platform::replace(&temporary, &target)?;
            #[cfg(test)]
            if self
                .fail_after_replace
                .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err(failure("注入的替换后同步失败"));
            }
            platform::sync(&self.directory)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
}

fn failure(message: &str) -> AuthFailure {
    AuthFailure::new("ssoStorage", message)
}
