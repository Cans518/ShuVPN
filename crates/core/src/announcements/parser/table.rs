use super::{blocks, failure, selector, AnnouncementFailure, Block, ElementRef, Inline};
use reqwest::Url;

pub(super) fn parse(
    table: ElementRef<'_>,
    base: &Url,
    depth: usize,
) -> Result<Option<Block>, AnnouncementFailure> {
    let mut rows = Vec::new();
    let mut has_header_row = false;
    let mut total = 0;
    let mut occupied = Vec::<usize>::new();
    for row in table.select(&selector("tr")?) {
        let nearest = row
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find(|element| element.value().name() == "table");
        if nearest.is_none_or(|parent| parent.id() != table.id()) {
            continue;
        }
        let mut cells = Vec::new();
        let mut next_occupied = occupied
            .iter()
            .map(|remaining| remaining.saturating_sub(1))
            .collect::<Vec<_>>();
        for cell in row.children().filter_map(ElementRef::wrap) {
            let name = cell.value().name();
            if !matches!(name, "td" | "th") {
                continue;
            }
            while occupied
                .get(cells.len())
                .is_some_and(|remaining| *remaining > 0)
            {
                cells.push(Vec::new());
            }
            let columns = span(cell, "colspan", 256)?;
            let remaining_rows = span(cell, "rowspan", 2048)?;
            let start = cells.len();
            let end = start + columns;
            if end > 256 {
                return Err(failure("size", "公告表格超过列数限制"));
            }
            if occupied
                .get(start..end.min(occupied.len()))
                .is_some_and(|slots| slots.iter().any(|remaining| *remaining > 0))
            {
                return Err(failure("parser", "公告表格合并单元格重叠"));
            }
            next_occupied.resize(next_occupied.len().max(end), 0);
            next_occupied[start..end].fill(remaining_rows - 1);
            let mut parts = Vec::new();
            blocks::collect(cell, &mut parts, base, depth + 1)?;
            let mut inlines = Vec::new();
            for part in parts {
                append_part(part, &mut inlines);
            }
            cells.push(inlines);
            cells.extend((1..columns).map(|_| Vec::new()));
            if name == "th" {
                has_header_row = true;
            }
        }
        cells.resize_with(cells.len().max(occupied.len()), Vec::new);
        occupied = next_occupied;
        if cells.is_empty() {
            continue;
        }
        if rows.is_empty() && row.value().classes().any(|class| class == "firstRow") {
            has_header_row = true;
        }
        total += cells.len();
        if rows.len() >= 2048 || cells.len() > 256 || total > 16_384 {
            return Err(failure("size", "公告表格超过大小限制"));
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Ok(None);
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width.saturating_mul(rows.len()) > 16_384 {
        return Err(failure("size", "公告表格补位超过大小限制"));
    }
    for row in &mut rows {
        row.resize_with(width, Vec::new);
    }
    Ok(Some(Block::Table {
        rows,
        has_header_row,
    }))
}

fn span(cell: ElementRef<'_>, attribute: &str, limit: usize) -> Result<usize, AnnouncementFailure> {
    let raw = cell.attr(attribute).unwrap_or("").trim();
    let value = match raw.parse::<usize>() {
        Ok(value) => value.max(1),
        Err(_) if !raw.is_empty() && raw.bytes().all(|byte| byte.is_ascii_digit()) => {
            return Err(failure("size", "公告表格合并单元格超过限制"));
        }
        Err(_) => 1,
    };
    if value > limit {
        return Err(failure("size", "公告表格合并单元格超过限制"));
    }
    Ok(value)
}

// The display grid holds inlines; retain nested content and expose images as links.
fn append_part(part: Block, output: &mut Vec<Inline>) {
    match part {
        Block::Text { inlines } => append_inlines(inlines, output),
        Block::Table { rows, .. } => {
            for row in rows {
                for cell in row {
                    append_inlines(cell, output);
                }
            }
        }
        Block::Image { url, alt } => append_inlines(
            vec![Inline {
                text: if alt.is_empty() { "图片".into() } else { alt },
                url: Some(url),
            }],
            output,
        ),
        Block::Attachment { name, url } => append_inlines(
            vec![Inline {
                text: name,
                url: Some(url),
            }],
            output,
        ),
    }
}

fn append_inlines(inlines: Vec<Inline>, output: &mut Vec<Inline>) {
    if inlines.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push(Inline {
            text: " ".into(),
            url: None,
        });
    }
    output.extend(inlines);
}
