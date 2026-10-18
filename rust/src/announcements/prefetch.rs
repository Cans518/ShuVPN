use super::{
    model::{failure, AnnouncementFailure, Item, Preview},
    Client,
};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub(crate) struct Prefetch {
    client: Arc<Client>,
    items: Vec<Item>,
    cancelled: CancellationToken,
    started: AtomicBool,
}

impl Prefetch {
    pub(crate) fn new(
        client: Arc<Client>,
        items: Vec<Item>,
    ) -> Result<Arc<Self>, AnnouncementFailure> {
        client.live()?;
        let bytes = items.iter().try_fold(0usize, |sum, item| {
            sum.checked_add(item.title.len())?
                .checked_add(item.url.len())?
                .checked_add(item.date_text.len())?
                .checked_add(item.published_at.as_deref().unwrap_or("").len())
        });
        if items.len() > 2048 || bytes.is_none_or(|bytes| bytes > 4 * 1024 * 1024) {
            return Err(failure("size", "公告预取批次超过限制"));
        }
        let mut seen = HashSet::new();
        let items = items
            .into_iter()
            .filter_map(|item| {
                client.target(&item.url).ok()?;
                seen.insert(item.url.clone()).then_some(item)
            })
            .collect();
        Ok(Arc::new(Self {
            cancelled: client.cancelled.child_token(),
            client,
            items,
            started: AtomicBool::new(false),
        }))
    }
    pub(crate) fn cancel(&self) {
        self.cancelled.cancel();
    }

    pub(crate) async fn run(
        &self,
        mut emit: impl FnMut(Preview) -> bool,
    ) -> Result<(), AnnouncementFailure> {
        if self.started.swap(true, Ordering::AcqRel) {
            return Err(failure("state", "公告预取批次已经开始"));
        }
        let mut pending = self.items.iter().cloned();
        let mut workers = JoinSet::new();
        loop {
            while workers.len() < 3 && !self.cancelled.is_cancelled() {
                let Some(item) = pending.next() else {
                    break;
                };
                let client = Arc::clone(&self.client);
                let cancelled = self.cancelled.clone();
                workers.spawn(async move {
                    let url = item.url.clone();
                    let result = tokio::select! {
                        biased;
                        _ = cancelled.cancelled() => return None,
                        result = async {
                            if let Some(cached) = client.cached(item.url.clone())? { return Ok(cached); }
                            let _permit = client.preview_slots.clone().acquire_owned().await
                                .map_err(|_| failure("closed", "公告预取队列已经关闭"))?;
                            client.detail(item).await
                        } => result,
                    };
                    if cancelled.is_cancelled() { return None; }
                    Some(match result {
                        Ok(detail) => Preview { url, text: Some(detail.preview), error: None },
                        Err(error) => Preview { url, text: None, error: Some(error.message) },
                    })
                });
            }
            if workers.is_empty() {
                break;
            }
            tokio::select! {
                biased;
                _ = self.cancelled.cancelled() => break,
                completed = workers.join_next() => match completed {
                    Some(Ok(Some(preview))) if !self.cancelled.is_cancelled() => {
                        if !emit(preview) { self.cancel(); break; }
                    }
                    Some(Err(_)) => {
                        self.cancel();
                        workers.shutdown().await;
                        return Err(failure("worker", "公告预取服务异常结束"));
                    }
                    _ => {}
                },
            }
        }
        workers.shutdown().await;
        Ok(())
    }
}
