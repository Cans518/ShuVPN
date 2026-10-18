use super::model::VpnFailure;
use crate::{
    platform::process::{self, Query},
    routing::{
        model::{Flow, RoutingFailure},
        store::Snapshot,
    },
};

pub(crate) async fn attach(snapshot: &Snapshot, flow: &mut Flow, query: Query) -> Option<String> {
    if !snapshot
        .compiled
        .as_ref()
        .is_some_and(|compiled| compiled.uses_process)
    {
        return None;
    }
    match process::lookup(query).await {
        Ok(identity) => {
            flow.process = Some(identity);
            None
        }
        Err(error) => Some(error.to_string()),
    }
}

pub(crate) fn failure(error: RoutingFailure, diagnostic: Option<&str>) -> VpnFailure {
    if error.code == "process" {
        if let Some(diagnostic) = diagnostic {
            return VpnFailure::new("process", &format!("{}：{diagnostic}", error.message));
        }
    }
    VpnFailure::new(&error.code, &error.message)
}
