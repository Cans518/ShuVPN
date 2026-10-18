pub use crate::announcements::model::{
    AnnouncementFailure, Block, Detail, Inline, Item, Page, Preview,
};
use crate::{announcements::prefetch::Prefetch, frb_generated::StreamSink};
use std::sync::Arc;

#[flutter_rust_bridge::frb(opaque)]
pub struct AnnouncementClient {
    inner: Arc<crate::announcements::Client>,
}
impl Drop for AnnouncementClient {
    fn drop(&mut self) {
        self.inner.close();
    }
}

impl Default for AnnouncementClient {
    fn default() -> Self {
        Self::new()
    }
}
impl AnnouncementClient {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(crate::announcements::Client::new()),
        }
    }
    pub async fn fetch_list(&self, page: Option<String>) -> Result<Page, AnnouncementFailure> {
        self.inner.list(page).await
    }
    pub async fn fetch_detail(&self, item: Item) -> Result<Detail, AnnouncementFailure> {
        self.inner.detail(item).await
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn cached_detail(&self, url: String) -> Result<Option<Detail>, AnnouncementFailure> {
        self.inner.cached(url)
    }
    #[flutter_rust_bridge::frb(sync)]
    pub fn close(&self) {
        self.inner.close();
    }
    pub async fn prefetch(
        &self,
        items: Vec<Item>,
    ) -> Result<AnnouncementPrefetch, AnnouncementFailure> {
        Ok(AnnouncementPrefetch {
            inner: Prefetch::new(Arc::clone(&self.inner), items)?,
        })
    }
}

#[flutter_rust_bridge::frb(opaque)]
pub struct AnnouncementPrefetch {
    inner: Arc<Prefetch>,
}

impl Drop for AnnouncementPrefetch {
    fn drop(&mut self) {
        self.inner.cancel();
    }
}
impl AnnouncementPrefetch {
    #[flutter_rust_bridge::frb(sync)]
    pub fn cancel(&self) {
        self.inner.cancel();
    }
    pub async fn events(&self, sink: StreamSink<Preview>) {
        if let Err(error) = self.inner.run(|preview| sink.add(preview).is_ok()).await {
            let _ = sink.add_error(anyhow::anyhow!("[{}] {}", error.code, error.message));
        }
    }
}
