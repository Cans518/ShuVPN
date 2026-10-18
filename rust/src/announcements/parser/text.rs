use super::{clean, content, Block, ElementRef};
use regex::Regex;
use std::sync::LazyLock;

pub(super) fn normalize(raw: &str) -> String {
    let mut output = String::new();
    let mut whitespace = false;
    for character in raw.chars() {
        if character.is_whitespace() {
            if !whitespace {
                output.push(' ');
            }
            whitespace = true;
        } else {
            output.push(character);
            whitespace = false;
        }
    }
    output
}

pub(super) fn metadata(spans: &[ElementRef<'_>]) -> (Option<String>, String) {
    let mut date = None;
    for (index, span) in spans.iter().enumerate() {
        let text = content(*span);
        if let Some(raw) = text.strip_prefix("创建时间：") {
            let raw = raw.trim();
            if !raw.is_empty() {
                date = Some(raw.to_owned());
            } else {
                let mut next = span.next_sibling();
                while let Some(node) = next {
                    if let Some(element) = ElementRef::wrap(node) {
                        let value = content(element);
                        if !value.is_empty() {
                            date = Some(value);
                        }
                        break;
                    }
                    next = node.next_sibling();
                }
            }
            if date.as_ref().is_some_and(|value| !value.is_empty()) {
                break;
            }
        }
        if index > 50_000 {
            break;
        }
    }
    let mut author = String::new();
    if let Some(date) = &date {
        if let Some(index) = spans.iter().position(|span| content(*span) == *date) {
            for span in spans.iter().skip(index + 1) {
                let raw = content(*span);
                if raw.contains("次数") {
                    break;
                }
                if !raw.is_empty() {
                    author = raw;
                    break;
                }
            }
        }
    }
    (date, author)
}

pub(super) fn date(raw: &str) -> Option<String> {
    static PATTERN: LazyLock<Result<Regex, regex::Error>> =
        LazyLock::new(|| Regex::new(r"([0-9]{4})[.\-/年]([0-9]{1,2})[.\-/月]([0-9]{1,2})"));
    let capture = PATTERN.as_ref().ok()?.captures(raw)?;
    let year = capture.get(1)?.as_str().parse::<i32>().ok()?;
    let month = capture.get(2)?.as_str().parse::<u8>().ok()?;
    let day = capture.get(3)?.as_str().parse::<u8>().ok()?;
    let date = cookie::time::Date::from_calendar_date(
        year,
        cookie::time::Month::try_from(month).ok()?,
        day,
    )
    .ok()?;
    Some(format!("{date}"))
}

pub(super) fn preview(blocks: &[Block]) -> String {
    let mut text = String::new();
    for block in blocks {
        if let Block::Text { inlines } = block {
            for inline in inlines {
                text.push_str(&inline.text);
            }
            if text.encode_utf16().count() >= 70 {
                break;
            }
        }
    }
    let text = clean(&text);
    let utf16: Vec<_> = text.encode_utf16().collect();
    if utf16.len() <= 70 {
        text
    } else {
        format!("{}…", String::from_utf16_lossy(&utf16[..70]))
    }
}
