#[derive(Clone)]
pub struct Inline {
    pub text: String,
    pub url: Option<String>,
}

#[derive(Clone)]
pub struct Item {
    pub title: String,
    pub url: String,
    pub date_text: String,
    pub published_at: Option<String>,
}

pub struct Page {
    pub items: Vec<Item>,
    pub next_page: Option<String>,
}

pub struct Preview {
    pub url: String,
    pub text: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub enum Block {
    Text {
        inlines: Vec<Inline>,
    },
    Image {
        url: String,
        alt: String,
    },
    Table {
        rows: Vec<Vec<Vec<Inline>>>,
        has_header_row: bool,
    },
    Attachment {
        name: String,
        url: String,
    },
}

#[derive(Clone)]
pub struct Detail {
    pub title: String,
    pub url: String,
    pub date_text: String,
    pub published_at: Option<String>,
    pub author: String,
    pub blocks: Vec<Block>,
    pub preview: String,
}

#[derive(Debug)]
pub struct AnnouncementFailure {
    pub code: String,
    pub message: String,
    pub status_code: Option<u16>,
}
impl std::fmt::Display for AnnouncementFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for AnnouncementFailure {}
pub(super) fn failure(code: &str, message: &str) -> AnnouncementFailure {
    AnnouncementFailure {
        code: code.to_owned(),
        message: message.to_owned(),
        status_code: None,
    }
}
