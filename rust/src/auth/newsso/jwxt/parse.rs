use super::JwxtAccount;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

struct Patterns {
    heading: Regex,
    paragraph: Regex,
    tags: Regex,
    grade: Regex,
}
static PATTERNS: LazyLock<Result<Patterns, regex::Error>> = LazyLock::new(|| {
    Ok(Patterns {
        heading: Regex::new(r#"(?s)class=["']media-heading["'][^>]*>(.*?)</h4>"#)?,
        paragraph: Regex::new(r"(?s)<p[^>]*>(.*?)</p>")?,
        tags: Regex::new(r"<[^>]*>")?,
        grade: Regex::new(r"^([0-9]{4}级)")?,
    })
});

fn plain(fragment: &str, tags: &Regex) -> String {
    tags.replace_all(fragment, " ")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn profile(bytes: &[u8]) -> Option<JwxtAccount> {
    let html = String::from_utf8_lossy(bytes);
    let patterns = PATTERNS.as_ref().ok()?;
    let heading = plain(
        patterns.heading.captures(&html)?.get(1)?.as_str(),
        &patterns.tags,
    );
    let mut words = heading.split_whitespace();
    let name = words.next()?.to_owned();
    let student_id = words
        .find(|word| word.bytes().all(|byte| byte.is_ascii_digit()))
        .map(str::to_owned);
    let mut account = JwxtAccount {
        name: Some(name),
        student_id,
        ..Default::default()
    };
    let info = patterns
        .paragraph
        .captures_iter(&html)
        .filter_map(|capture| {
            capture
                .get(1)
                .map(|value| plain(value.as_str(), &patterns.tags))
        })
        .find(|text| text.contains("学院") || text.contains('级'))
        .unwrap_or_default();
    for word in info.split_whitespace() {
        if account.college.is_none() && (word.contains("学院") || word.ends_with('系')) {
            account.college = Some(word.to_owned());
        } else if let Some(capture) = patterns
            .grade
            .captures(word)
            .filter(|_| account.grade.is_none())
        {
            let grade = capture.get(1)?;
            account.grade = Some(grade.as_str().to_owned());
            let rest = &word[grade.end()..];
            if account.major.is_none() && !rest.is_empty() {
                account.major = Some(rest.to_owned());
            }
        } else if account.major.is_none() {
            account.major = Some(word.to_owned());
        }
    }
    Some(account)
}

pub(super) fn identity(bytes: &[u8]) -> Option<JwxtAccount> {
    let json: Value = serde_json::from_slice(bytes).ok()?;
    let info = json.get("xsxx")?.as_object()?;
    let text = |key: &str| {
        info.get(key)?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let account = JwxtAccount {
        name: text("XM"),
        student_id: text("XH"),
        class_name: text("BJMC"),
        ..Default::default()
    };
    (account.name.is_some() || account.student_id.is_some() || account.class_name.is_some())
        .then_some(account)
}

pub(super) fn term(bytes: &[u8]) -> Option<(String, String)> {
    let html = String::from_utf8_lossy(bytes);
    let selected = |name: &str| -> Option<String> {
        let select = Regex::new(&format!(
            r#"(?is)<select[^>]*id=["']{name}["'][^>]*>(.*?)</select>"#
        ))
        .ok()?;
        let input = Regex::new(&format!(r#"(?i)<input[^>]*name=["']{name}["'][^>]*>"#)).ok()?;
        let option = Regex::new(r"(?is)<option[^>]*\bselected\b[^>]*>.*?</option>").ok()?;
        let value = Regex::new(r#"(?i)\bvalue\s*=\s*["']([^"']*)["']"#).ok()?;
        let from_select = select.captures(&html).and_then(|capture| {
            capture.get(1).and_then(|fragment| {
                option.find(fragment.as_str()).and_then(|option| {
                    value
                        .captures(option.as_str())
                        .and_then(|capture| capture.get(1).map(|value| value.as_str().to_owned()))
                })
            })
        });
        from_select.or_else(|| {
            input.find(&html).and_then(|input| {
                value
                    .captures(input.as_str())
                    .and_then(|capture| capture.get(1).map(|value| value.as_str().to_owned()))
            })
        })
    };
    let (year, term) = (selected("xnm")?, selected("xqm")?);
    (!year.is_empty() && !term.is_empty()).then_some((year, term))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_role_and_partial_fields_do_not_invent_student_id() -> anyhow::Result<()> {
        let account = profile(
            "<h4 class='media-heading'>张三&nbsp;学生</h4><p>示例学院 2025级示例专业</p>"
                .as_bytes(),
        )
        .ok_or_else(|| anyhow::anyhow!("no profile"))?;
        assert_eq!(
            (
                account.name.as_deref(),
                account.student_id.as_deref(),
                account.grade.as_deref(),
                account.college.as_deref(),
                account.major.as_deref()
            ),
            (
                Some("张三"),
                None,
                Some("2025级"),
                Some("示例学院"),
                Some("示例专业")
            )
        );
        assert!(profile(b"login").is_none());
        Ok(())
    }
    #[test]
    fn selected_and_hidden_terms_and_string_identity_are_preserved() -> anyhow::Result<()> {
        assert_eq!(term(br#"<select id='xnm'><option value='2025' selected>current</option></select><input name='xqm' value='12'>"#), Some(("2025".into(), "12".into())));
        assert!(term(b"login").is_none());
        let account = identity(br#"{"xsxx":{"XM":" name ","XH":12345,"BJMC":" class "}}"#)
            .ok_or_else(|| anyhow::anyhow!("no identity"))?;
        assert_eq!(
            (
                account.name.as_deref(),
                account.student_id.as_deref(),
                account.class_name.as_deref()
            ),
            (Some("name"), None, Some("class"))
        );
        assert!(identity(br#"{"xsxx":{}}"#).is_none());
        Ok(())
    }
}
