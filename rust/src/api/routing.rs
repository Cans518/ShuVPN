pub use crate::routing::model::RoutingFailure;
pub use crate::routing::model::RoutingStatus;
use crate::routing::{
    engine::Evaluation,
    model::{ConfigurationInfo, Flow},
    store::RoutingStore,
};
use std::sync::Arc;

pub struct RoutingManager {
    pub(crate) store: Arc<RoutingStore>,
}

pub struct RoutingPreview {
    store: Arc<RoutingStore>,
    prepared: crate::routing::store::Prepared,
}

impl RoutingPreview {
    pub fn info(&self) -> Result<String, RoutingFailure> {
        encode_info(&RoutingStore::preview(&self.prepared))
    }
    pub async fn commit(&self) -> Result<String, RoutingFailure> {
        encode_info(&self.store.apply_prepared(self.prepared.clone()).await?)
    }
}

impl RoutingManager {
    pub async fn prepare(
        &self,
        profile: String,
        overrides: String,
        expected_version: u64,
    ) -> Result<RoutingPreview, RoutingFailure> {
        Ok(RoutingPreview {
            store: Arc::clone(&self.store),
            prepared: self
                .store
                .prepare(profile, overrides, expected_version)
                .await?,
        })
    }
    pub async fn open(directory: String) -> Result<Self, RoutingFailure> {
        let store = tokio::task::spawn_blocking(move || RoutingStore::open(directory))
            .await
            .map_err(|_| RoutingFailure {
                code: "worker".to_owned(),
                message: "打开规则配置失败".to_owned(),
            })??;
        crate::routing::refresh::spawn(Arc::downgrade(&store), store.version_updates());
        Ok(Self { store })
    }

    pub fn current(&self) -> Result<String, RoutingFailure> {
        encode_info(&self.store.snapshot()?.info)
    }

    pub fn status(&self) -> Result<String, RoutingFailure> {
        encode_status(&self.store.status())
    }

    pub async fn refresh(&self, expected_version: u64) -> Result<String, RoutingFailure> {
        encode_info(&self.store.refresh(expected_version).await?)
    }

    pub fn updates(&self) -> tokio::sync::watch::Receiver<RoutingStatus> {
        self.store.updates()
    }

    pub async fn preview(
        &self,
        profile: String,
        overrides: String,
        expected_version: u64,
    ) -> Result<String, RoutingFailure> {
        let prepared = self
            .store
            .prepare(profile, overrides, expected_version)
            .await?;
        encode_info(&RoutingStore::preview(&prepared))
    }

    pub async fn apply(
        &self,
        profile: String,
        overrides: String,
        expected_version: u64,
    ) -> Result<String, RoutingFailure> {
        encode_info(
            &self
                .store
                .apply(profile, overrides, expected_version)
                .await?,
        )
    }

    pub async fn rollback(&self, expected_version: u64) -> Result<String, RoutingFailure> {
        encode_info(&self.store.rollback(expected_version).await?)
    }

    pub fn test(&self, flow_json: String, routes_json: String) -> Result<String, RoutingFailure> {
        if flow_json.len() > 16 * 1024 || routes_json.len() > 1024 * 1024 {
            return Err(RoutingFailure::new("size", "测试输入超过大小限制"));
        }
        let flow: Flow = serde_json::from_str(&flow_json)
            .map_err(|_| RoutingFailure::new("flow", "流元数据格式无效"))?;
        let routes: Vec<sangfor_core::route::Route> = serde_json::from_str(&routes_json)
            .map_err(|_| RoutingFailure::new("flow", "资源表格式无效"))?;
        let result = match self.store.snapshot()?.evaluate(&routes, &flow)? {
            Evaluation::Decided(decision) => {
                serde_json::json!({"decision":decision,"needsResolution":false})
            }
            Evaluation::NeedAddress => serde_json::json!({"needsResolution":true}),
        };
        serde_json::to_string(&result).map_err(|_| RoutingFailure::new("encoding", "决策编码失败"))
    }
}

pub fn profile_template(name: String) -> Result<String, RoutingFailure> {
    match name.as_str() {
        "campus-only" => Ok(crate::routing::store::suggested_profile(true)),
        "campus-direct" => Ok(crate::routing::store::suggested_profile(false)),
        _ => Err(RoutingFailure::new("profile", "未知内置配置档")),
    }
}

pub fn routing_capabilities() -> Result<String, RoutingFailure> {
    serde_json::to_string(&crate::routing::compiler::capabilities())
        .map_err(|_| RoutingFailure::new("encoding", "规则能力编码失败"))
}

pub fn routing_editor(profile: String, overrides: String) -> Result<String, RoutingFailure> {
    crate::routing::editor::inspect(&profile, &overrides)
}

pub fn edit_routing(
    profile: String,
    overrides: String,
    operation: String,
) -> Result<String, RoutingFailure> {
    crate::routing::editor::edit(&profile, &overrides, &operation)
}

fn encode_info(info: &ConfigurationInfo) -> Result<String, RoutingFailure> {
    serde_json::to_string(info).map_err(|_| RoutingFailure::new("encoding", "配置状态编码失败"))
}

fn encode_status(status: &crate::routing::model::RoutingStatus) -> Result<String, RoutingFailure> {
    serde_json::to_string(status).map_err(|_| RoutingFailure::new("encoding", "刷新状态编码失败"))
}
