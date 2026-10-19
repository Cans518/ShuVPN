use super::model::{failure, AnnouncementFailure, Block, Detail, Inline, Item, Page};
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};
mod blocks;
mod table;
#[cfg(test)]
mod tests;
mod text;

fn selector(raw: &str) -> Result<Selector, AnnouncementFailure> {
    Selector::parse(raw).map_err(|_| failure("parser", "公告解析选择器无效"))
}
fn document(bytes: &[u8]) -> Result<Html, AnnouncementFailure> {
    if bytes.len() > 1024 * 1024 {
        return Err(failure("size", "公告页面超过大小限制"));
    }
    let html = Html::parse_document(&String::from_utf8_lossy(bytes));
    if html.tree.nodes().count() > 50_000 {
        return Err(failure("size", "公告页面节点过多"));
    }
    let mut depth = 0usize;
    for edge in html.tree.root().traverse() {
        match edge {
            ego_tree::iter::Edge::Open(_) => {
                depth += 1;
                if depth > 128 {
                    return Err(failure("size", "公告页面嵌套过深"));
                }
            }
            ego_tree::iter::Edge::Close(_) => depth = depth.saturating_sub(1),
        }
    }
    Ok(html)
}
fn link(base: &Url, raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let uri = base.join(raw).ok()?;
    matches!(uri.scheme(), "http" | "https" | "mailto").then(|| uri.to_string())
}
fn clean(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn content(element: ElementRef<'_>) -> String {
    clean(&element.text().collect::<String>())
}

pub(super) fn list(bytes: &[u8], base: &Url) -> Result<Page, AnnouncementFailure> {
    let html = document(bytes)?;
    let root = html
        .select(&selector(".only-list1")?)
        .next()
        .unwrap_or(html.root_element());
    let mut items = Vec::new();
    for node in root.select(&selector("li[id^=line_u4]")?) {
        if items.len() >= 2048 {
            return Err(failure("size", "公告列表条目过多"));
        }
        let Some(anchor) = node.select(&selector("a")?).next() else {
            continue;
        };
        let title = content(anchor);
        let Some(url) = link(base, anchor.attr("href")) else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let date_text = node
            .select(&selector("span")?)
            .next()
            .map(content)
            .unwrap_or_default();
        items.push(Item {
            title,
            url,
            published_at: text::date(&date_text),
            date_text,
        });
    }
    let next_page = html
        .select(&selector("span.p_next > a")?)
        .next()
        .and_then(|anchor| link(base, anchor.attr("href")));
    Ok(Page { items, next_page })
}

pub(super) fn detail(bytes: &[u8], item: &Item) -> Result<Detail, AnnouncementFailure> {
    let base = Url::parse(&item.url).map_err(|_| failure("url", "公告地址无效"))?;
    let html = document(bytes)?;
    let title = html
        .select(&selector("span.Head")?)
        .map(content)
        .find(|title| !title.is_empty())
        .unwrap_or_else(|| item.title.clone());
    let spans: Vec<_> = html.select(&selector("span")?).collect();
    let date = text::metadata(&spans);
    let date_text = date.0.unwrap_or_else(|| item.date_text.clone());
    let mut blocks = Vec::new();
    if let Some(root) = html.select(&selector(".v_news_content")?).next() {
        blocks::collect(root, &mut blocks, &base, 0)?;
        if blocks.is_empty() {
            let raw = content(root);
            if !raw.is_empty() {
                blocks.push(Block::Text {
                    inlines: vec![Inline {
                        text: raw,
                        url: None,
                    }],
                });
            }
        }
        for anchor in html.select(&selector("a")?) {
            let Some(url) = link(&base, anchor.attr("href")) else {
                continue;
            };
            let uri = Url::parse(&url).map_err(|_| failure("url", "附件地址无效"))?;
            let name = content(anchor);
            if uri.path().contains("download.jsp") && !name.is_empty() {
                if blocks.len() >= 4096 {
                    return Err(failure("size", "公告正文块过多"));
                }
                blocks.push(Block::Attachment { name, url });
            }
        }
    }
    let preview = text::preview(&blocks);
    Ok(Detail {
        title,
        url: item.url.clone(),
        published_at: text::date(&date_text),
        date_text,
        author: date.1,
        blocks,
        preview,
    })
}
