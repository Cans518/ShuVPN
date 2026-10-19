use std::collections::BTreeMap;
use std::sync::Weak;
use tokio::sync::watch;
use tokio::time::{Duration, Instant};

use super::{model::Provider, store::RoutingStore};

struct Scheduled {
    descriptor: Provider,
    next: Instant,
}

pub(crate) fn spawn(owner: Weak<RoutingStore>, mut versions: watch::Receiver<u64>) {
    tokio::spawn(async move {
        let mut schedule = BTreeMap::<String, Scheduled>::new();
        loop {
            let version = *versions.borrow_and_update();
            let Some(store) = owner.upgrade() else { return };
            let providers = match store.scheduled_providers() {
                Ok(providers) => providers,
                Err(error) => {
                    store.refresh_state(version, false, Some(error.to_string()), &[]);
                    return;
                }
            };
            drop(store);
            schedule.retain(|name, entry| providers.get(name) == Some(&entry.descriptor));
            let now = Instant::now();
            for (name, descriptor) in providers {
                let interval = Duration::from_secs(descriptor.interval.unwrap_or(1));
                schedule.entry(name).or_insert_with(|| Scheduled {
                    descriptor,
                    next: now + interval,
                });
            }
            let Some(deadline) = schedule.values().map(|entry| entry.next).min() else {
                if versions.changed().await.is_err() {
                    return;
                }
                continue;
            };
            if deadline > now {
                tokio::select! {
                    _ = tokio::time::sleep_until(deadline) => {}
                    changed = versions.changed() => {
                        if changed.is_err() { return; }
                    }
                }
                continue;
            }
            let mut due = Vec::new();
            for (name, entry) in &mut schedule {
                if entry.next <= now {
                    due.push(name.clone());
                    entry.next = now + Duration::from_secs(entry.descriptor.interval.unwrap_or(1));
                }
            }
            let Some(store) = owner.upgrade() else { return };
            let plan = match store.refresh_plan(due.clone(), version) {
                Ok(plan) => plan,
                Err(error) => {
                    store.refresh_state(version, false, Some(error.to_string()), &due);
                    continue;
                }
            };
            store.refresh_state(version, true, None, &due);
            drop(store);
            // The timer and download own no store; dropping its last owner closes
            // the watch channel and cancels pending I/O instead of retaining the lock.
            let result = tokio::select! {
                result = plan.prepare() => result,
                changed = versions.changed() => {
                    if changed.is_err() { return; }
                    continue;
                }
            };
            let Some(store) = owner.upgrade() else { return };
            let result = match result {
                Ok(Some(prepared)) => store
                    .apply_prepared(prepared)
                    .await
                    .map(|info| info.version),
                Ok(None) => Ok(version),
                Err(error) => Err(error),
            };
            let checked_version = result.as_ref().copied().unwrap_or(version);
            store.refresh_state(
                checked_version,
                false,
                result.err().map(|error| error.to_string()),
                &due,
            );
        }
    });
}
