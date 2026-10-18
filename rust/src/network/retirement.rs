//! Keep Linux drop-triggered restoration owned until an explicit close joins it.
use super::{failure, registry, Egress, Registry};
use crate::connection::model::VpnFailure;
use std::{
    collections::BTreeMap,
    sync::{atomic::Ordering, Arc, Mutex, Weak},
};
use tokio::{sync::Mutex as AsyncMutex, task::JoinHandle};

type Outcome = Result<(), VpnFailure>;
struct Retirement {
    owner: Arc<Egress>,
    task: AsyncMutex<Option<JoinHandle<Outcome>>>,
}
static PENDING: Mutex<BTreeMap<u64, Arc<Retirement>>> = Mutex::new(BTreeMap::new());
static SERIAL: AsyncMutex<()> = AsyncMutex::const_new(());

pub(super) fn request(owner: Arc<Egress>) {
    if owner.closed.load(Ordering::Acquire) {
        return;
    }
    owner.retire();
    let Ok(mut pending) = PENDING.lock() else {
        return;
    };
    if pending.contains_key(&owner.epoch) {
        return;
    }
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        let task = runtime.spawn(deactivate(Arc::clone(&owner)));
        pending.insert(
            owner.epoch,
            Arc::new(Retirement {
                owner,
                task: AsyncMutex::new(Some(task)),
            }),
        );
    }
}

pub(super) async fn drain() -> Outcome {
    let owners = PENDING
        .lock()
        .map_err(|_| failure())?
        .values()
        .map(|retirement| Arc::clone(&retirement.owner))
        .collect::<Vec<_>>();
    for owner in owners {
        close(owner).await?;
    }
    Ok(())
}

pub(super) async fn close(owner: Arc<Egress>) -> Outcome {
    let pending = PENDING
        .lock()
        .map_err(|_| failure())?
        .get(&owner.epoch)
        .cloned();
    if let Some(retirement) = pending {
        let mut task = retirement.task.lock().await;
        let result = if let Some(task) = task.as_mut() {
            task.await
                .map_err(|_| VpnFailure::new("worker", "物理出口清理线程退出失败"))
                .and_then(|outcome| outcome)
        } else {
            Ok(())
        };
        task.take();
        result?;
    }
    let epoch = owner.epoch;
    deactivate(owner).await?;
    PENDING.lock().map_err(|_| failure())?.remove(&epoch);
    Ok(())
}

async fn deactivate(owner: Arc<Egress>) -> Outcome {
    let _serial = SERIAL.lock().await;
    if owner.closed.load(Ordering::Acquire) {
        return Ok(());
    }
    owner.retire();
    owner.wait_idle().await;
    let epoch = owner.epoch;
    let active = registry()
        .lock()
        .map_err(|_| failure())?
        .active
        .clone()
        .filter(|active| active.epoch == epoch);
    let Some(active) = active else {
        if let Some(broker) = &owner.broker {
            broker
                .close()
                .await
                .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
        }
        owner.closed.store(true, Ordering::Release);
        return Ok(());
    };
    active.retire();
    active.wait_idle().await;
    // Re-snapshot after in-flight creation has registered or closed its socket.
    let mut snapshot = {
        let registry = registry().lock().map_err(|_| failure())?;
        Registry {
            active: Some(active),
            sockets: registry.sockets.clone(),
            epoch,
        }
    };
    deactivate_registry(&mut snapshot, epoch).await?;
    let mut registry = registry().lock().map_err(|_| failure())?;
    if registry
        .active
        .as_ref()
        .is_some_and(|active| active.epoch == epoch)
    {
        registry.active.take();
    }
    owner.closed.store(true, Ordering::Release);
    Ok(())
}

pub(super) async fn deactivate_registry(registry: &mut Registry, epoch: u64) -> Outcome {
    let Some(active) = registry
        .active
        .as_ref()
        .filter(|active| active.epoch == epoch)
    else {
        return Ok(());
    };
    active.retire();
    active.wait_idle().await;
    let records = registry
        .sockets
        .iter()
        .filter_map(Weak::upgrade)
        .collect::<Vec<_>>();
    let mut failed = None;
    for record in records {
        if let Err(error) = record.restore(active).await {
            failed = Some(error);
        }
    }
    if let Some(error) = failed {
        return Err(error);
    }
    if let Some(broker) = &active.broker {
        broker
            .close()
            .await
            .map_err(|error| VpnFailure::new(&error.code, &error.message))?;
    }
    registry.active.take();
    Ok(())
}
