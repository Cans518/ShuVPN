use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::watch;

use super::{
    compiler::{self, Compiled},
    engine::{self, Evaluation},
    model::*,
    providers::{fetch_all, CachedProvider, ProviderLoader},
};
use sangfor_core::route::Route;
const MAX_STATE_BYTES: usize = 192 * 1024 * 1024;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Documents {
    profile: String,
    overrides: String,
    providers: BTreeMap<String, CachedProvider>,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Saved {
    version: u64,
    current: Option<Documents>,
    previous: Option<Documents>,
    #[serde(default)]
    has_previous: bool,
}

pub(crate) struct Snapshot {
    pub info: ConfigurationInfo,
    pub compiled: Option<Compiled>,
    documents: Option<Documents>,
}

#[derive(Clone)]
pub(crate) struct Prepared {
    expected_version: u64,
    snapshot: Arc<Snapshot>,
    source: PreparedSource,
}

#[derive(Clone, Copy)]
enum PreparedSource {
    User,
    Refresh,
}
struct Versions {
    current: Arc<Snapshot>,
    previous: Option<Arc<Snapshot>>,
}

pub(crate) struct RoutingStore {
    versions: RwLock<Versions>,
    directory: Option<PathBuf>,
    _directory_lock: Option<std::fs::File>,
    version_changes: watch::Sender<u64>,
    status: watch::Sender<RoutingStatus>,
}

pub(crate) struct RefreshPlan {
    expected_version: u64,
    documents: Documents,
    names: Vec<String>,
    directory: PathBuf,
}

impl RefreshPlan {
    pub(crate) async fn prepare(mut self) -> Result<Option<Prepared>, RoutingFailure> {
        let loader = ProviderLoader::new(&self.directory)?;
        let mut changed = false;
        for name in self.names {
            let cached = self
                .documents
                .providers
                .get_mut(&name)
                .ok_or_else(|| RoutingFailure::new("provider", "规则集已不存在"))?;
            let payload = loader.load(&name, &cached.descriptor).await?;
            changed |= payload != cached.payload;
            cached.payload = payload;
            let warning = format!("规则集 {name} 更新失败，保留有效版本");
            let before = self.documents.warnings.len();
            self.documents.warnings.retain(|value| value != &warning);
            changed |= before != self.documents.warnings.len();
        }
        let version = self
            .expected_version
            .checked_add(1)
            .ok_or_else(|| RoutingFailure::new("version", "规则版本溢出"))?;
        let snapshot = compile_documents(self.documents, version)?;
        Ok(changed.then_some(Prepared {
            expected_version: self.expected_version,
            snapshot,
            source: PreparedSource::Refresh,
        }))
    }
}

impl Snapshot {
    fn legacy() -> Self {
        Self {
            info: ConfigurationInfo {
                version: 0,
                name: "已有严格校园代理".to_owned(),
                profile_document: String::new(),
                overrides_document: String::new(),
                rule_count: 1,
                notices: vec![],
                warnings: vec![],
            },
            compiled: None,
            documents: None,
        }
    }

    pub(crate) fn evaluate(
        &self,
        routes: &[Route],
        flow: &Flow,
    ) -> Result<Evaluation, RoutingFailure> {
        engine::evaluate(self.compiled.as_ref(), routes, self.info.version, flow)
    }
}

impl RoutingStore {
    pub(crate) fn legacy() -> Arc<Self> {
        Self::from_versions(
            Versions {
                current: Arc::new(Snapshot::legacy()),
                previous: None,
            },
            None,
            None,
        )
    }

    pub(crate) fn open(directory: String) -> Result<Arc<Self>, RoutingFailure> {
        let directory = PathBuf::from(directory);
        if !directory.is_absolute() {
            return Err(RoutingFailure::new("directory", "规则目录必须是绝对路径"));
        }
        std::fs::create_dir_all(&directory)
            .map_err(|_| RoutingFailure::new("directory", "无法创建规则目录"))?;
        let directory = directory
            .canonicalize()
            .map_err(|_| RoutingFailure::new("directory", "无法定位规则目录"))?;
        let directory_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join(".routing.lock"))
            .map_err(|_| RoutingFailure::new("storage", "无法锁定规则目录"))?;
        crate::platform::files::try_lock(&directory_lock).map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => {
                RoutingFailure::new("storage", "规则目录正在被另一个实例使用")
            }
            std::fs::TryLockError::Error(error) => {
                RoutingFailure::new("storage", &format!("无法锁定规则目录：{error}"))
            }
        })?;
        let path = directory.join("routing-state.json");
        let versions = if path.exists() {
            if !path
                .canonicalize()
                .map_err(|_| RoutingFailure::new("storage", "配置缓存路径无效"))?
                .starts_with(&directory)
            {
                return Err(RoutingFailure::new("storage", "配置缓存越出规则目录"));
            }
            let file = std::fs::File::open(path)
                .map_err(|_| RoutingFailure::new("storage", "无法读取规则配置"))?;
            use std::io::Read;
            let mut bytes = vec![];
            file.take((MAX_STATE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| RoutingFailure::new("storage", "无法读取规则配置"))?;
            if bytes.len() > MAX_STATE_BYTES {
                return Err(RoutingFailure::new("storage", "规则配置缓存过大"));
            }
            let saved: Saved = serde_json::from_slice(&bytes)
                .map_err(|_| RoutingFailure::new("storage", "规则配置缓存无效"))?;
            let current = match saved.current {
                Some(documents) => compile_documents(documents, saved.version)?,
                None => {
                    let mut legacy = Snapshot::legacy();
                    legacy.info.version = saved.version;
                    Arc::new(legacy)
                }
            };
            let previous = if saved.has_previous {
                Some(match saved.previous {
                    Some(documents) => {
                        compile_documents(documents, saved.version.saturating_sub(1))?
                    }
                    None => {
                        let mut legacy = Snapshot::legacy();
                        legacy.info.version = saved.version.saturating_sub(1);
                        Arc::new(legacy)
                    }
                })
            } else {
                None
            };
            Versions { current, previous }
        } else {
            Versions {
                current: Arc::new(Snapshot::legacy()),
                previous: None,
            }
        };
        Ok(Self::from_versions(
            versions,
            Some(directory),
            Some(directory_lock),
        ))
    }

    fn from_versions(
        versions: Versions,
        directory: Option<PathBuf>,
        directory_lock: Option<std::fs::File>,
    ) -> Arc<Self> {
        let (version_changes, _) = watch::channel(versions.current.info.version);
        let (status, _) = watch::channel(RoutingStatus {
            configuration: versions.current.info.clone(),
            refreshing: false,
            refresh_error: None,
            provider_errors: BTreeMap::new(),
            last_checked_unix_ms: None,
        });
        Arc::new(Self {
            versions: RwLock::new(versions),
            directory,
            _directory_lock: directory_lock,
            version_changes,
            status,
        })
    }

    pub(crate) fn updates(&self) -> watch::Receiver<RoutingStatus> {
        self.status.subscribe()
    }

    pub(crate) fn status(&self) -> RoutingStatus {
        self.status.borrow().clone()
    }

    pub(crate) fn version_updates(&self) -> watch::Receiver<u64> {
        self.version_changes.subscribe()
    }

    pub(crate) fn scheduled_providers(&self) -> Result<BTreeMap<String, Provider>, RoutingFailure> {
        let snapshot = self.snapshot()?;
        Ok(snapshot
            .documents
            .as_ref()
            .map(|documents| {
                documents
                    .providers
                    .iter()
                    .filter(|(_, cached)| {
                        cached.descriptor.kind != "inline" && cached.descriptor.interval.is_some()
                    })
                    .map(|(name, cached)| (name.clone(), cached.descriptor.clone()))
                    .collect()
            })
            .unwrap_or_default())
    }

    pub(crate) fn refresh_plan(
        &self,
        names: Vec<String>,
        expected_version: u64,
    ) -> Result<RefreshPlan, RoutingFailure> {
        let current = self.snapshot()?;
        if current.info.version != expected_version {
            return Err(conflict());
        }
        Ok(RefreshPlan {
            expected_version,
            documents: current
                .documents
                .clone()
                .ok_or_else(|| RoutingFailure::new("provider", "当前配置没有可更新规则集"))?,
            names,
            directory: self
                .directory
                .clone()
                .ok_or_else(|| RoutingFailure::new("directory", "缺少规则目录"))?,
        })
    }

    pub(crate) fn refresh_state(
        &self,
        version: u64,
        refreshing: bool,
        error: Option<String>,
        names: &[String],
    ) {
        self.status.send_modify(|status| {
            if status.configuration.version != version {
                return;
            }
            status.refreshing = refreshing;
            if !refreshing {
                if names.is_empty() {
                    match &error {
                        Some(error) => {
                            status
                                .provider_errors
                                .insert("配置".to_owned(), error.clone());
                        }
                        None => {
                            status.provider_errors.remove("配置");
                        }
                    }
                }
                for name in names {
                    match &error {
                        Some(error) => {
                            status.provider_errors.insert(name.clone(), error.clone());
                        }
                        None => {
                            status.provider_errors.remove(name);
                        }
                    }
                }
                status.refresh_error = (!status.provider_errors.is_empty()).then(|| {
                    status
                        .provider_errors
                        .iter()
                        .map(|(name, error)| format!("{name}：{error}"))
                        .collect::<Vec<_>>()
                        .join("；")
                });
                status.last_checked_unix_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);
            }
        });
    }

    pub(crate) async fn refresh(
        self: &Arc<Self>,
        expected_version: u64,
    ) -> Result<ConfigurationInfo, RoutingFailure> {
        let snapshot = self.snapshot()?;
        let names: Vec<String> = snapshot
            .documents
            .as_ref()
            .map(|documents| {
                documents
                    .providers
                    .iter()
                    .filter(|(_, cached)| cached.descriptor.kind != "inline")
                    .map(|(name, _)| name.clone())
                    .collect()
            })
            .unwrap_or_default();
        let plan = self.refresh_plan(names.clone(), expected_version)?;
        self.refresh_state(expected_version, true, None, &names);
        let result = async {
            match plan.prepare().await? {
                Some(prepared) => self.apply_prepared(prepared).await,
                None => {
                    let current = self.snapshot()?;
                    if current.info.version != expected_version {
                        return Err(conflict());
                    }
                    Ok(current.info.clone())
                }
            }
        }
        .await;
        let version = result
            .as_ref()
            .map(|info| info.version)
            .unwrap_or(expected_version);
        self.refresh_state(
            version,
            false,
            result.as_ref().err().map(ToString::to_string),
            &names,
        );
        result
    }

    pub(crate) fn snapshot(&self) -> Result<Arc<Snapshot>, RoutingFailure> {
        Ok(Arc::clone(
            &self.versions.read().map_err(|_| lock_failure())?.current,
        ))
    }

    pub(crate) async fn prepare(
        &self,
        profile: String,
        overrides: String,
        expected_version: u64,
    ) -> Result<Prepared, RoutingFailure> {
        let current = self.snapshot()?;
        if current.info.version != expected_version {
            return Err(conflict());
        }
        let parsed: Profile = compiler::parse_document(&profile)?;
        let edits: Overrides = compiler::parse_document(if overrides.trim().is_empty() {
            "{}"
        } else {
            &overrides
        })?;
        let root = self
            .directory
            .as_ref()
            .ok_or_else(|| RoutingFailure::new("directory", "配置应用需要规则目录"))?;
        let loader = ProviderLoader::new(root)?;
        let cached = current
            .documents
            .as_ref()
            .map(|documents| &documents.providers);
        let (providers, warnings) =
            fetch_all(&loader, &parsed, cached.unwrap_or(&BTreeMap::new())).await?;
        let documents = Documents {
            profile,
            overrides,
            providers,
            warnings: warnings.clone(),
        };
        let payloads = documents
            .providers
            .iter()
            .map(|(name, provider)| (name.clone(), provider.payload.clone()))
            .collect();
        let compiled = compiler::compile(&parsed, &edits, &payloads)?;
        let version = expected_version
            .checked_add(1)
            .ok_or_else(|| RoutingFailure::new("version", "规则版本溢出"))?;
        let info = ConfigurationInfo {
            version,
            name: compiled.name.clone(),
            profile_document: documents.profile.clone(),
            overrides_document: documents.overrides.clone(),
            rule_count: compiled.rules.len(),
            notices: compiled.notices.clone(),
            warnings,
        };
        Ok(Prepared {
            expected_version,
            snapshot: Arc::new(Snapshot {
                info,
                compiled: Some(compiled),
                documents: Some(documents),
            }),
            source: PreparedSource::User,
        })
    }

    pub(crate) fn preview(prepared: &Prepared) -> ConfigurationInfo {
        prepared.snapshot.info.clone()
    }

    pub(crate) async fn apply(
        self: &Arc<Self>,
        profile: String,
        overrides: String,
        expected_version: u64,
    ) -> Result<ConfigurationInfo, RoutingFailure> {
        let prepared = self.prepare(profile, overrides, expected_version).await?;
        self.apply_prepared(prepared).await
    }

    pub(crate) async fn apply_prepared(
        self: &Arc<Self>,
        prepared: Prepared,
    ) -> Result<ConfigurationInfo, RoutingFailure> {
        let owner = Arc::clone(self);
        tokio::task::spawn_blocking(move || owner.commit(prepared))
            .await
            .map_err(|_| RoutingFailure::new("worker", "应用规则配置失败"))?
    }

    fn commit(&self, prepared: Prepared) -> Result<ConfigurationInfo, RoutingFailure> {
        let mut versions = self.versions.write().map_err(|_| lock_failure())?;
        if versions.current.info.version != prepared.expected_version {
            return Err(conflict());
        }
        self.persist(&Saved {
            version: prepared.snapshot.info.version,
            current: prepared.snapshot.documents.clone(),
            previous: versions.current.documents.clone(),
            has_previous: true,
        })?;
        versions.previous = Some(Arc::clone(&versions.current));
        versions.current = prepared.snapshot;
        self.publish(&versions.current.info, prepared.source);
        Ok(versions.current.info.clone())
    }

    pub(crate) async fn rollback(
        self: &Arc<Self>,
        expected_version: u64,
    ) -> Result<ConfigurationInfo, RoutingFailure> {
        let owner = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            let mut versions = owner.versions.write().map_err(|_| lock_failure())?;
            if versions.current.info.version != expected_version {
                return Err(conflict());
            }
            let previous = versions
                .previous
                .as_ref()
                .ok_or_else(|| RoutingFailure::new("rollback", "没有可回退版本"))?;
            let mut info = previous.info.clone();
            info.version = expected_version
                .checked_add(1)
                .ok_or_else(|| RoutingFailure::new("version", "规则版本溢出"))?;
            let restored = Arc::new(Snapshot {
                info,
                compiled: previous.compiled.clone(),
                documents: previous.documents.clone(),
            });
            owner.persist(&Saved {
                version: restored.info.version,
                current: restored.documents.clone(),
                previous: versions.current.documents.clone(),
                has_previous: true,
            })?;
            versions.previous = Some(Arc::clone(&versions.current));
            versions.current = restored;
            owner.publish(&versions.current.info, PreparedSource::User);
            Ok(versions.current.info.clone())
        })
        .await
        .map_err(|_| RoutingFailure::new("worker", "回退规则配置失败"))?
    }

    fn persist(&self, saved: &Saved) -> Result<(), RoutingFailure> {
        let root = self
            .directory
            .as_ref()
            .ok_or_else(|| RoutingFailure::new("directory", "缺少规则目录"))?;
        let bytes = serde_json::to_vec(saved)
            .map_err(|_| RoutingFailure::new("storage", "规则缓存编码失败"))?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err(RoutingFailure::new("size", "规则缓存超过大小限制"));
        }
        let mut random = [0; 8];
        getrandom::getrandom(&mut random)
            .map_err(|_| RoutingFailure::new("storage", "无法生成临时配置文件"))?;
        let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let temporary = root.join(format!(".routing-{name}.tmp"));
        let write = || -> std::io::Result<()> {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temporary, root.join("routing-state.json"))?;
            Ok(())
        };
        if write().is_err() {
            let _ = std::fs::remove_file(&temporary);
            return Err(RoutingFailure::new(
                "storage",
                "规则配置写入失败，旧版本继续运行",
            ));
        }
        Ok(())
    }

    fn publish(&self, info: &ConfigurationInfo, source: PreparedSource) {
        self.status.send_modify(|status| {
            status.configuration = info.clone();
            status.refreshing = false;
            if matches!(source, PreparedSource::User) {
                status.refresh_error = None;
                status.provider_errors.clear();
                status.last_checked_unix_ms = None;
            }
        });
        self.version_changes.send_replace(info.version);
    }
}

fn compile_documents(documents: Documents, version: u64) -> Result<Arc<Snapshot>, RoutingFailure> {
    let profile: Profile = compiler::parse_document(&documents.profile)?;
    if documents.providers.len() != profile.providers.len() {
        return Err(RoutingFailure::new("storage", "规则集缓存与配置档不一致"));
    }
    for (name, cached) in &documents.providers {
        if profile.providers.get(name) != Some(&cached.descriptor) {
            return Err(RoutingFailure::new(
                "storage",
                "规则集缓存描述符与配置档不一致",
            ));
        }
        super::providers::validate_descriptor(&cached.descriptor)?;
    }
    let overrides: Overrides =
        compiler::parse_document(if documents.overrides.trim().is_empty() {
            "{}"
        } else {
            &documents.overrides
        })?;
    let payloads = documents
        .providers
        .iter()
        .map(|(name, provider)| (name.clone(), provider.payload.clone()))
        .collect();
    let compiled = compiler::compile(&profile, &overrides, &payloads)?;
    let info = ConfigurationInfo {
        version,
        name: compiled.name.clone(),
        profile_document: documents.profile.clone(),
        overrides_document: documents.overrides.clone(),
        rule_count: compiled.rules.len(),
        notices: compiled.notices.clone(),
        warnings: documents.warnings.clone(),
    };
    Ok(Arc::new(Snapshot {
        info,
        compiled: Some(compiled),
        documents: Some(documents),
    }))
}
fn conflict() -> RoutingFailure {
    RoutingFailure::new("conflict", "规则版本已更新，请重新预览")
}
fn lock_failure() -> RoutingFailure {
    RoutingFailure::new("lock", "无法访问规则快照")
}

pub(crate) fn suggested_profile(campus_only: bool) -> String {
    format!(
        "name: {}\nrules:\n  - MATCH,{}\n",
        if campus_only {
            "仅校园"
        } else {
            "校园优先"
        },
        if campus_only { "REJECT" } else { "DIRECT" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_provider_metadata_must_match_the_profile_and_have_a_valid_interval(
    ) -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        for (declared, cached_interval) in [(0, 0), (u64::MAX, u64::MAX), (1, 2)] {
            let profile = format!("rules: ['RULE-SET,p,REJECT', 'MATCH,DIRECT']\nrule-providers:\n p:\n  type: file\n  behavior: domain\n  path: domains.yaml\n  interval: {declared}\n");
            let parsed: Profile = compiler::parse_document(&profile)?;
            let providers = parsed
                .providers
                .into_iter()
                .map(|(name, mut descriptor)| {
                    descriptor.interval = Some(cached_interval);
                    (
                        name,
                        CachedProvider {
                            descriptor,
                            payload: vec!["old.example".to_owned()],
                        },
                    )
                })
                .collect();
            let saved = Saved {
                version: 1,
                current: Some(Documents {
                    profile,
                    overrides: "{}".to_owned(),
                    providers,
                    warnings: vec![],
                }),
                previous: None,
                has_previous: false,
            };
            std::fs::write(
                directory.path().join("routing-state.json"),
                serde_json::to_vec(&saved)?,
            )?;
            assert!(RoutingStore::open(directory.path().to_string_lossy().into_owned()).is_err());
        }
        Ok(())
    }

    #[tokio::test]
    async fn dropping_the_last_owner_cancels_an_in_flight_https_refresh() -> anyhow::Result<()> {
        use tokio::io::AsyncReadExt;
        use tokio::time::{timeout, Duration};
        let directory = tempfile::tempdir()?;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let url = format!("https://127.0.0.1:{}/rules", listener.local_addr()?.port());
        let profile = format!("rules: ['RULE-SET,p,REJECT', 'MATCH,DIRECT']\nrule-providers:\n p:\n  type: http\n  behavior: domain\n  url: {url}\n  interval: 1\n");
        let parsed: Profile = compiler::parse_document(&profile)?;
        let providers = parsed
            .providers
            .into_iter()
            .map(|(name, descriptor)| {
                (
                    name,
                    CachedProvider {
                        descriptor,
                        payload: vec!["old.example".to_owned()],
                    },
                )
            })
            .collect();
        let saved = Saved {
            version: 1,
            current: Some(Documents {
                profile,
                overrides: "{}".to_owned(),
                providers,
                warnings: vec![],
            }),
            previous: None,
            has_previous: false,
        };
        std::fs::write(
            directory.path().join("routing-state.json"),
            serde_json::to_vec(&saved)?,
        )?;
        let (accepted, arrived) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            let _ = accepted.send(());
            let mut buffer = [0; 4096];
            while socket.read(&mut buffer).await? != 0 {}
            Ok::<_, std::io::Error>(())
        });
        let store = RoutingStore::open(directory.path().to_string_lossy().into_owned())?;
        super::super::refresh::spawn(Arc::downgrade(&store), store.version_updates());
        timeout(Duration::from_secs(4), arrived).await??;
        let weak = Arc::downgrade(&store);
        drop(store);
        assert!(weak.upgrade().is_none());
        let reopened = RoutingStore::open(directory.path().to_string_lossy().into_owned())?;
        assert_eq!(reopened.snapshot()?.info.version, 1);
        timeout(Duration::from_secs(2), server).await???;
        Ok(())
    }

    #[tokio::test]
    async fn an_old_refresh_result_cannot_replace_a_newer_user_configuration() -> anyhow::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let source = directory.path().join("domains.yaml");
        std::fs::write(&source, "payload: ['old.example']")?;
        let store = RoutingStore::open(directory.path().to_string_lossy().into_owned())?;
        let profile = "rules: ['RULE-SET,p,REJECT', 'MATCH,DIRECT']\nrule-providers:\n p:\n  type: file\n  behavior: domain\n  path: domains.yaml\n";
        store.apply(profile.to_owned(), "{}".to_owned(), 0).await?;
        std::fs::write(&source, "payload: ['new.example']")?;
        let prepared = store
            .refresh_plan(vec!["p".to_owned()], 1)?
            .prepare()
            .await?
            .ok_or_else(|| anyhow::anyhow!("changed provider was not prepared"))?;
        store
            .apply(
                "name: newer\nrules: ['MATCH,REJECT']".to_owned(),
                "{}".to_owned(),
                1,
            )
            .await?;
        let error = store
            .apply_prepared(prepared)
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("stale refresh unexpectedly committed"))?;
        assert_eq!(error.code, "conflict");
        assert_eq!(store.snapshot()?.info.version, 2);
        assert_eq!(store.snapshot()?.info.name, "newer");
        Ok(())
    }
}
