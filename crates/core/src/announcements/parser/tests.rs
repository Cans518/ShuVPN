use super::*;
use anyhow::{ensure, Context};

const LIST: &[u8] = include_bytes!("../../../tests/fixtures/announcements/list.html");
const DETAIL: &[u8] = include_bytes!("../../../tests/fixtures/announcements/detail.html");
const RICH: &[u8] = include_bytes!("../../../tests/fixtures/announcements/rich.html");
const NESTED: &[u8] = include_bytes!("../../../tests/fixtures/announcements/nested.html");
fn item() -> Item {
    Item {
        title: "fallback".into(),
        url: "https://newits.shu.edu.cn/info/1095/6432.htm".into(),
        date_text: "2026-09-22".into(),
        published_at: Some("2026-09-22".into()),
    }
}

#[test]
fn frozen_list_uses_page_relative_links_and_pagination() -> anyhow::Result<()> {
    let base = Url::parse("https://newits.shu.edu.cn/index/tzgg.htm")?;
    let page = list(LIST, &base)?;
    ensure!(
        page.items.len() == 2 && page.items[0].url == item().url,
        "list links wrong"
    );
    ensure!(
        page.items[0].title == "关于2026年 上海大学计算机软件及数据类无形资产清查盘点的通知",
        "title changed"
    );
    ensure!(
        page.items[0].published_at.as_deref() == Some("2026-09-22"),
        "date changed"
    );
    ensure!(
        page.items[1].title == "关于启用新版本企业微信的通知",
        "NBSP not cleaned"
    );
    ensure!(
        page.next_page.as_deref() == Some("https://newits.shu.edu.cn/index/tzgg/31.htm"),
        "pagination changed"
    );
    let numbered = String::from_utf8(LIST.to_vec())?
        .replace("../info", "../../info")
        .replace("tzgg/31.htm", "29.htm");
    let page = list(
        numbered.as_bytes(),
        &Url::parse("https://newits.shu.edu.cn/index/tzgg/30.htm")?,
    )?;
    ensure!(
        page.items[0].url == item().url
            && page.next_page.as_deref() == Some("https://newits.shu.edu.cn/index/tzgg/29.htm"),
        "numbered relative base lost"
    );
    Ok(())
}

#[test]
fn last_page_and_detail_fallbacks_preserve_display_content() -> anyhow::Result<()> {
    let base = Url::parse("https://newits.shu.edu.cn/index/tzgg/1.htm")?;
    let page = list(
        r#"<li id="line_u4_0"><a href="/info/1.htm">one</a><span></span></li>
        <li id="line_u4_1"><a href="/info/2.htm">two</a><span>近期</span></li>
        <span class="p_next_d">下页</span>"#
            .as_bytes(),
        &base,
    )?;
    ensure!(
        page.items.len() == 2
            && page.next_page.is_none()
            && page.items.iter().all(|item| item.published_at.is_none()),
        "last page or absent/unreadable date changed"
    );
    let parsed = detail(
        r#"<span>创建时间：</span><span>2026/10/16</span>
        <span>浏览次数：</span><span>999</span>
        <div class="v_news_content"><p>first</p><img src="/image.png">
        <p>second <a href="javascript:void(0)">dead</a></p>
        <table><tr><td>not preview</td></tr></table><p>third</p></div>"#
            .as_bytes(),
        &item(),
    )?;
    ensure!(parsed.author.is_empty(), "click counter became the author");
    ensure!(
        parsed.preview == "firstsecond deadthird",
        "preview included images/table or lost text"
    );
    let dead = parsed
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Text { inlines } => inlines.iter().find(|inline| inline.text == "dead"),
            _ => None,
        })
        .context("unsafe link text was lost")?;
    ensure!(dead.url.is_none(), "unsafe link remained navigable");
    let image = detail(
        br#"<div class="v_news_content"><img src="/image.png"></div>"#,
        &item(),
    )?;
    ensure!(image.preview.is_empty(), "image-only preview became text");
    Ok(())
}

#[test]
fn frozen_detail_preserves_metadata_original_image_and_attachment_order() -> anyhow::Result<()> {
    let detail = detail(DETAIL, &item())?;
    ensure!(
        detail.title == "关于2026年资产清查盘点的通知"
            && detail.author == "吕露"
            && detail.date_text == "2026/09/22",
        "metadata changed"
    );
    let texts: Vec<_> = detail
        .blocks
        .iter()
        .filter_map(|block| {
            if let Block::Text { inlines } = block {
                Some(
                    inlines
                        .iter()
                        .map(|inline| inline.text.as_str())
                        .collect::<String>(),
                )
            } else {
                None
            }
        })
        .collect();
    ensure!(
        texts
            == [
                "各学院（部门）：",
                "本次清查盘点截止到 2026年8月31日。 请于 11月30日前完成自查。",
                "信息化工作办公室"
            ],
        "paragraphs changed"
    );
    ensure!(
        matches!(&detail.blocks[2], Block::Image {url,alt} if url == "https://newits.shu.edu.cn/img/notice.png" && alt == "盘点流程"),
        "original image order changed"
    );
    ensure!(
        matches!(detail.blocks.last(), Some(Block::Attachment {name,url}) if name == "上海大学计算机软件及数据类无形资产自查表.rar" && url.contains("urltype=news.DownloadAttachUrl&wbfileid=CB68")),
        "attachment lost"
    );
    Ok(())
}

#[test]
fn frozen_rich_inline_table_padding_and_nested_table_rows() -> anyhow::Result<()> {
    let parsed = detail(RICH, &item())?;
    let Block::Text { inlines } = &parsed.blocks[0] else {
        anyhow::bail!("no inline paragraph");
    };
    ensure!(
        inlines
            .iter()
            .map(|inline| inline.text.as_str())
            .collect::<Vec<_>>()
            == ["如需申请本校邮箱，请点击这里：", "上海大学邮箱申请", "。"],
        "inline boundaries changed"
    );
    ensure!(
        inlines[1].url.as_deref() == Some("https://newsso.shu.edu.cn/oauth2/mailRegister"),
        "inline URL lost"
    );
    let Block::Table {
        rows,
        has_header_row,
    } = &parsed.blocks[3]
    else {
        anyhow::bail!("no table in order");
    };
    ensure!(
        *has_header_row
            && rows.len() == 3
            && rows.iter().all(|row| row.len() == 2)
            && rows[2][1].is_empty(),
        "table padding changed"
    );
    let nested = detail(NESTED, &item())?;
    let Block::Table { rows, .. } = &nested.blocks[0] else {
        anyhow::bail!("no nested outer table");
    };
    ensure!(
        rows.len() == 2
            && rows[1][0][0].text == "链接"
            && rows[1][0][0].url.as_deref() == Some("https://www.shu.edu.cn/"),
        "nested rows included or link lost"
    );
    Ok(())
}

#[test]
fn missing_content_fallback_preview_and_structure_limits() -> anyhow::Result<()> {
    let empty = detail(b"<span class='Head'>only title</span>", &item())?;
    ensure!(
        empty.blocks.is_empty() && empty.title == "only title",
        "missing body not empty"
    );
    let fallback = detail(b"<div class='v_news_content'><p>body</p></div>", &item())?;
    ensure!(
        fallback.title == "fallback" && fallback.date_text == "2026-09-22",
        "fallback lost"
    );
    let long = format!(
        "<div class='v_news_content'><p>{}</p></div>",
        "字".repeat(200)
    );
    let parsed = detail(long.as_bytes(), &item())?;
    ensure!(
        parsed.preview.chars().count() == 71 && parsed.preview.ends_with('…'),
        "preview changed"
    );
    let oversized =
        b"<div class='v_news_content'><table><tr><td colspan='1000000'>x</td></tr></table></div>";
    ensure!(
        detail(oversized, &item())
            .err()
            .is_some_and(|error| error.code == "size"),
        "unbounded table accepted"
    );
    ensure!(
        text::date("2026.9.2").as_deref() == Some("2026-09-02"),
        "date variant lost"
    );
    ensure!(
        list(b"maintenance", &Url::parse(&item().url)?)
            .context("empty list")?
            .items
            .is_empty(),
        "empty list rejected"
    );
    Ok(())
}

#[test]
fn reviewed_limits_cover_overflow_nested_link_and_external_attachments() -> anyhow::Result<()> {
    let huge = b"<div class='v_news_content'><table><tr><td colspan='184467440737095516160'>x</td></tr></table></div>";
    ensure!(
        detail(huge, &item())
            .err()
            .is_some_and(|error| error.code == "size"),
        "overflow colspan bypassed limit"
    );
    let deep = format!(
        "<div class='v_news_content'><p><a href='/x'>{}text{}</a></p></div>",
        "<span>".repeat(140),
        "</span>".repeat(140)
    );
    ensure!(
        detail(deep.as_bytes(), &item())
            .err()
            .is_some_and(|error| error.code == "size"),
        "nested link bypassed depth limit"
    );
    let attachments = format!(
        "<div class='v_news_content'>{}</div><a href='/system/_content/download.jsp'>file</a>",
        "<p>x</p>".repeat(4096)
    );
    ensure!(
        detail(attachments.as_bytes(), &item())
            .err()
            .is_some_and(|error| error.code == "size"),
        "attachment bypassed block count"
    );
    Ok(())
}

#[test]
fn empty_date_sibling_uses_list_fallback() -> anyhow::Result<()> {
    let parsed = detail("<span>创建时间：</span><span></span>".as_bytes(), &item())?;
    ensure!(
        parsed.date_text == "2026-09-22",
        "empty sibling suppressed fallback date"
    );
    Ok(())
}

#[test]
fn table_preserves_rowspans_nested_text_and_image_links() -> anyhow::Result<()> {
    let parsed = detail(
        br#"<div class="v_news_content"><table>
      <tr><td rowspan="2">A</td><td>B</td></tr>
      <tr><td><table><tr><td>nested</td></tr></table><img src="/image.png" alt="diagram"></td></tr>
    </table></div>"#,
        &item(),
    )?;
    let Block::Table { rows, .. } = &parsed.blocks[0] else {
        anyhow::bail!("missing table")
    };
    ensure!(
        rows.len() == 2 && rows.iter().all(|row| row.len() == 2),
        "span shape wrong"
    );
    ensure!(rows[1][0].is_empty(), "rowspan shifted the next row");
    ensure!(
        rows[1][1].iter().any(|inline| inline.text == "nested"),
        "nested table text lost"
    );
    ensure!(
        rows[1][1].iter().any(|inline| inline.text == "diagram"
            && inline.url.as_deref() == Some("https://newits.shu.edu.cn/image.png")),
        "cell image lost"
    );
    let huge = br#"<div class="v_news_content"><table><tr><td rowspan="184467440737095516160">x</td></tr></table></div>"#;
    ensure!(
        detail(huge, &item())
            .err()
            .is_some_and(|error| error.code == "size"),
        "rowspan overflow accepted"
    );
    Ok(())
}
