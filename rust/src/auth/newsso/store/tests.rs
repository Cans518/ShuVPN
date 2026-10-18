use super::*;

fn record(secret: &str) -> Record {
    Record {
        version: 1,
        origin: "https://newsso.shu.edu.cn".to_owned(),
        legacy_imported: true,
        import_open: false,
        cookies: vec![format!("SHU_OAUTH2={secret}; Path=/; Secure; HttpOnly")],
        device_id: None,
        account: None,
    }
}

#[test]
fn repeated_atomic_replacement_and_exclusive_owner_survive_reopen() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let store = Store::open(directory.clone())?;
    assert!(store.load()?.is_none());
    assert!(Store::open(directory.clone()).is_err());
    store.save(&record("old"))?;
    store.save(&record("current"))?;
    let loaded = store
        .load()?
        .ok_or_else(|| anyhow::anyhow!("missing state"))?;
    assert_eq!(loaded.cookies, record("current").cookies);
    #[cfg(target_os = "windows")]
    assert!(!std::fs::read(root.path().join("newsso-state.bin"))?
        .windows(7)
        .any(|bytes| bytes == b"current"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.path())?.permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(root.path().join("newsso-state.bin"))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    drop(store);
    let reopened = Store::open(directory)?;
    assert!(
        !reopened
            .load()?
            .ok_or_else(|| anyhow::anyhow!("missing state"))?
            .import_open
    );
    Ok(())
}

#[test]
fn corrupt_or_incompatible_state_is_not_treated_as_fresh_and_pending_files_are_ignored(
) -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let store = Store::open(root.path().to_string_lossy().into_owned())?;
    store.save(&record("committed"))?;
    let path = root.path().join("newsso-state.bin");
    std::fs::write(root.path().join(".newsso-leftover.tmp"), b"uncommitted")?;
    assert_eq!(
        store
            .load()?
            .ok_or_else(|| anyhow::anyhow!("missing state"))?
            .cookies,
        record("committed").cookies
    );
    let mut incompatible = record("future");
    incompatible.version = 3;
    store.save(&incompatible)?;
    assert!(store.load().is_err());
    std::fs::write(&path, b"broken")?;
    assert!(store.load().is_err());
    std::fs::write(&path, vec![0; LIMIT + 1])?;
    assert!(store.load().is_err());
    Ok(())
}

#[test]
fn failed_write_keeps_old_commit_and_later_retry_can_close_migration() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let store = Store::open(root.path().to_string_lossy().into_owned())?;
    store.save(&record("committed"))?;
    store
        .fail_writes
        .store(true, std::sync::atomic::Ordering::Release);
    let mut tombstone = record("new");
    tombstone.cookies.clear();
    assert!(store.save(&tombstone).is_err());
    assert_eq!(
        store
            .load()?
            .ok_or_else(|| anyhow::anyhow!("missing state"))?
            .cookies,
        record("committed").cookies
    );
    store
        .fail_writes
        .store(false, std::sync::atomic::Ordering::Release);
    store.save(&tombstone)?;
    assert!(store
        .load()?
        .ok_or_else(|| anyhow::anyhow!("missing state"))?
        .cookies
        .is_empty());
    Ok(())
}

#[test]
fn frozen_version1_json_recovers_without_new_fields_and_upgrades_atomically() -> anyhow::Result<()>
{
    for original in [
        r#"{"version":1,"origin":"https://newsso.shu.edu.cn","legacy_imported":true,"import_open":false,"cookies":["SHU_OAUTH2=frozen; Path=/; Secure; HttpOnly"]}"#,
        r#"{"version":1,"origin":"https://newsso.shu.edu.cn","legacy_imported":false,"import_open":false,"cookies":[]}"#,
    ] {
        let root = tempfile::tempdir()?;
        let store = Store::open(root.path().to_string_lossy().into_owned())?;
        let sealed = platform::seal(original.as_bytes())?;
        std::fs::write(
            root.path().join("newsso-state.bin"),
            [MAGIC.to_vec(), sealed.to_vec()].concat(),
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                root.path().join("newsso-state.bin"),
                std::fs::Permissions::from_mode(0o600),
            )?;
        }
        let mut loaded = store
            .load()?
            .ok_or_else(|| anyhow::anyhow!("no frozen record"))?;
        assert!(loaded.device_id.is_none() && loaded.account.is_none() && !loaded.import_open);
        let cookies = loaded.cookies.clone();
        let imported = loaded.legacy_imported;
        loaded.version = 2;
        loaded.device_id = Some("stable".into());
        loaded.account = Some(serde_json::json!({"account_id":"fixture"}));
        store.save(&loaded)?;
        let upgraded = store
            .load()?
            .ok_or_else(|| anyhow::anyhow!("no upgraded record"))?;
        assert_eq!(upgraded.cookies, cookies);
        assert_eq!(upgraded.legacy_imported, imported);
        assert_eq!(upgraded.device_id.as_deref(), Some("stable"));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinks_and_replaced_directory_are_rejected_without_writing_foreign_files() -> anyhow::Result<()>
{
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir()?;
    let owned = root.path().join("owned");
    let store = Store::open(owned.to_string_lossy().into_owned())?;
    let foreign = root.path().join("foreign");
    std::fs::write(&foreign, b"foreign")?;
    symlink(&foreign, owned.join("newsso-state.bin"))?;
    assert!(store.save(&record("secret")).is_err());
    assert_eq!(std::fs::read(&foreign)?, b"foreign");
    std::fs::remove_file(owned.join("newsso-state.bin"))?;
    std::fs::rename(&owned, root.path().join("original"))?;
    std::fs::create_dir(&owned)?;
    assert!(store.save(&record("secret")).is_err());
    assert!(!owned.join("newsso-state.bin").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn replaced_lock_cannot_leave_two_authoritative_writers() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let first = Store::open(directory.clone())?;
    std::fs::remove_file(root.path().join(".newsso.lock"))?;
    let second = Store::open(directory)?;
    assert!(first.save(&record("obsolete")).is_err());
    second.save(&record("current"))?;
    assert_eq!(
        second
            .load()?
            .ok_or_else(|| anyhow::anyhow!("missing state"))?
            .cookies,
        record("current").cookies
    );
    Ok(())
}
