use super::{
    clean, failure, link, selector, table, text, AnnouncementFailure, Block, ElementRef, Inline,
};
use ego_tree::NodeRef;
use reqwest::Url;
use scraper::Node;

const BLOCKS: &[&str] = &[
    "p",
    "div",
    "table",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "tr",
    "td",
    "th",
    "tbody",
    "thead",
    "tfoot",
    "section",
    "article",
    "blockquote",
    "pre",
    "hr",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
];
const IGNORED: &[&str] = &[
    "script", "style", "noscript", "iframe", "meta", "link", "hr",
];

fn budget(blocks: &[Block], depth: usize) -> Result<(), AnnouncementFailure> {
    if depth > 128 || blocks.len() > 4096 {
        return Err(failure("size", "公告正文结构超过限制"));
    }
    Ok(())
}

pub(super) fn collect(
    container: ElementRef<'_>,
    blocks: &mut Vec<Block>,
    base: &Url,
    depth: usize,
) -> Result<(), AnnouncementFailure> {
    budget(blocks, depth)?;
    for node in container.children() {
        match node.value() {
            Node::Text(value) => {
                let text = clean(value);
                if !text.is_empty() {
                    blocks.push(Block::Text {
                        inlines: vec![Inline { text, url: None }],
                    });
                }
            }
            Node::Element(_) => {
                let Some(element) = ElementRef::wrap(node) else {
                    continue;
                };
                let name = element.value().name();
                if IGNORED.contains(&name) {
                    continue;
                }
                if name == "img" {
                    image(blocks, element, base);
                } else if name == "table" {
                    if let Some(table) = table::parse(element, base, depth + 1)? {
                        blocks.push(table);
                    }
                } else if element
                    .children()
                    .filter_map(ElementRef::wrap)
                    .any(|child| BLOCKS.contains(&child.value().name()))
                {
                    collect(element, blocks, base, depth + 1)?;
                } else {
                    let mut inlines = Vec::new();
                    inline(node, &mut inlines, blocks, base, depth + 1)?;
                    flush(&mut inlines, blocks);
                }
            }
            _ => {}
        }
        budget(blocks, depth)?;
    }
    Ok(())
}

fn inline(
    node: NodeRef<'_, Node>,
    inlines: &mut Vec<Inline>,
    blocks: &mut Vec<Block>,
    base: &Url,
    depth: usize,
) -> Result<(), AnnouncementFailure> {
    budget(blocks, depth)?;
    if let Node::Text(value) = node.value() {
        let text = text::normalize(value);
        if !text.is_empty() {
            inlines.push(Inline { text, url: None });
        }
        return Ok(());
    }
    let Some(element) = ElementRef::wrap(node) else {
        return Ok(());
    };
    let name = element.value().name();
    if IGNORED.contains(&name) {
        return Ok(());
    }
    match name {
        "img" => {
            flush(inlines, blocks);
            image(blocks, element, base);
        }
        "table" => {
            flush(inlines, blocks);
            if let Some(table) = table::parse(element, base, depth + 1)? {
                blocks.push(table);
            }
        }
        "a" if element.select(&selector("img")?).next().is_none() => {
            inlines.push(Inline {
                text: text::normalize(&element.text().collect::<String>()),
                url: link(base, element.attr("href")),
            });
        }
        _ => {
            for child in node.children() {
                inline(child, inlines, blocks, base, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn flush(inlines: &mut Vec<Inline>, blocks: &mut Vec<Block>) {
    while let Some(first) = inlines.first_mut() {
        first.text = first.text.trim_start().to_owned();
        if first.text.is_empty() {
            inlines.remove(0);
        } else {
            break;
        }
    }
    while let Some(last) = inlines.last_mut() {
        last.text = last.text.trim_end().to_owned();
        if last.text.is_empty() {
            inlines.pop();
        } else {
            break;
        }
    }
    if !inlines.is_empty() {
        blocks.push(Block::Text {
            inlines: std::mem::take(inlines),
        });
    }
}

fn image(blocks: &mut Vec<Block>, element: ElementRef<'_>, base: &Url) {
    let source = element
        .attr("orisrc")
        .or(element.attr("src"))
        .or(element.attr("vurl"));
    if let Some(url) = link(base, source) {
        blocks.push(Block::Image {
            url,
            alt: clean(element.attr("alt").unwrap_or("")),
        });
    }
}
