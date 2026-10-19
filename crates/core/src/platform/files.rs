use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub(crate) fn try_lock(file: &File) -> Result<(), std::fs::TryLockError> {
    file.try_lock()
}

pub(crate) fn unlock(file: &File) -> std::io::Result<()> {
    file.unlock()
}

#[cfg(unix)]
pub(crate) fn directory(path: &Path) -> Result<File, std::io::Error> {
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| std::io::Error::other("无法核验存储目录"))?;
    #[cfg(target_os = "linux")]
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(std::io::Error::other("存储目录不属于当前用户"));
    }
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other("存储目录类型无效"));
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| std::io::Error::other("无法设置存储目录权限"))?;
    File::open(path).map_err(|_| std::io::Error::other("无法保留存储目录"))
}

#[cfg(target_os = "windows")]
pub(crate) fn directory(path: &Path) -> Result<File, std::io::Error> {
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::*;
    if std::fs::symlink_metadata(path)
        .map_err(|_| std::io::Error::other("无法核验存储目录"))?
        .file_attributes()
        & FILE_ATTRIBUTE_REPARSE_POINT
        != 0
    {
        return Err(std::io::Error::other("存储目录不能是重解析点"));
    }
    OpenOptions::new()
        .read(true)
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|_| std::io::Error::other("无法保留存储目录"))
}

pub(crate) fn check_directory(path: &Path, handle: &File) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual =
            std::fs::symlink_metadata(path).map_err(|_| std::io::Error::other("存储目录已丢失"))?;
        let owned = handle
            .metadata()
            .map_err(|_| std::io::Error::other("无法核验存储目录身份"))?;
        if !actual.is_dir()
            || actual.file_type().is_symlink()
            || actual.dev() != owned.dev()
            || actual.ino() != owned.ino()
        {
            return Err(std::io::Error::other("存储目录身份已改变"));
        }
    }
    #[cfg(not(unix))]
    let _ = (path, handle);
    Ok(())
}

#[cfg(not(any(unix, target_os = "windows")))]
pub(crate) fn directory(_path: &Path) -> Result<File, std::io::Error> {
    Err(std::io::Error::other("当前平台不支持存储目录"))
}

pub(crate) fn options() -> OpenOptions {
    let options = OpenOptions::new();
    #[cfg(unix)]
    let mut options = options;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
        #[cfg(target_os = "linux")]
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options
}

pub(crate) fn regular(path: &Path) -> Result<bool, std::io::Error> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(std::io::Error::other("无法核验存储文件")),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other("存储文件类型无效"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err(std::io::Error::other("存储文件权限或链接无效"));
        }
        #[cfg(target_os = "linux")]
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(std::io::Error::other("存储文件不属于当前用户"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err(std::io::Error::other("存储文件不能是重解析点"));
        }
    }
    Ok(true)
}

pub(crate) fn same_file(handle: &File, path: &Path) -> Result<(), std::io::Error> {
    if !regular(path)? {
        return Err(std::io::Error::other("存储目录锁已丢失"));
    }
    let current = options()
        .read(true)
        .open(path)
        .map_err(|_| std::io::Error::other("无法核验存储目录锁"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let owned = handle
            .metadata()
            .map_err(|_| std::io::Error::other("无法核验目录锁身份"))?;
        let actual = current
            .metadata()
            .map_err(|_| std::io::Error::other("无法核验目录锁身份"))?;
        if owned.dev() != actual.dev() || owned.ino() != actual.ino() {
            return Err(std::io::Error::other("存储目录锁身份已改变"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::*;
        let mut owned = BY_HANDLE_FILE_INFORMATION::default();
        let mut actual = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: both handles remain live; output structures are writable.
        if unsafe { GetFileInformationByHandle(handle.as_raw_handle().cast(), &mut owned) } == 0
            || unsafe { GetFileInformationByHandle(current.as_raw_handle().cast(), &mut actual) }
                == 0
            || (
                owned.dwVolumeSerialNumber,
                owned.nFileIndexHigh,
                owned.nFileIndexLow,
            ) != (
                actual.dwVolumeSerialNumber,
                actual.nFileIndexHigh,
                actual.nFileIndexLow,
            )
        {
            return Err(std::io::Error::other("存储目录锁身份已改变"));
        }
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    let _ = (handle, current);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn replace(source: &Path, target: &Path) -> Result<(), std::io::Error> {
    std::fs::rename(source, target).map_err(|_| std::io::Error::other("无法原子替换存储文件"))
}
#[cfg(target_os = "windows")]
pub(crate) fn replace(source: &Path, target: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::*;
    let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both terminated paths remain live, and only our owned files move.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(std::io::Error::other("无法原子替换存储文件"));
    }
    Ok(())
}

pub(crate) fn sync(directory: &File) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    directory
        .sync_all()
        .map_err(|_| std::io::Error::other("无法同步存储目录"))?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_exclusion_and_explicit_unlock_keep_file_handles_live() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("lock");
        let first = options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        let second = options().read(true).write(true).open(&path)?;
        try_lock(&first)?;
        anyhow::ensure!(
            matches!(try_lock(&second), Err(std::fs::TryLockError::WouldBlock)),
            "second owner acquired an existing lock"
        );
        unlock(&first)?;
        try_lock(&second)?;
        anyhow::ensure!(
            matches!(try_lock(&first), Err(std::fs::TryLockError::WouldBlock)),
            "first owner ignored the new lock"
        );
        unlock(&second)?;
        try_lock(&first)?;
        unlock(&first)?;
        Ok(())
    }
}
