use super::OtpCode;
use regex::Regex;
use std::sync::LazyLock;

struct Patterns {
    code: Regex,
    value: Regex,
    account: Regex,
    remaining: Regex,
    period: Regex,
    refresh: Regex,
}
static PATTERNS: LazyLock<Result<Patterns, regex::Error>> = LazyLock::new(|| {
    Ok(Patterns {
        code: Regex::new(
            r#"id=["']DataList1_LabelNum_[0-9]+["'][^>]*>(?:<[^>]+>|&nbsp;|&#160;|[\s\u{00A0}])*([0-9]{4,8})(?:[^0-9]|$)"#,
        )?,
        value: Regex::new(
            r#"id=["']DataList1_LabelNum_[0-9]+["'][^>]*value=["']([0-9]{4,8})["']"#,
        )?,
        account: Regex::new(
            r#"id=["']DataList1_LabelAccount_[0-9]+["'][^>]*>(?:<[^>]+>|&nbsp;|&#160;|[\s\u{00A0}])*([^<]*?)\s*<"#,
        )?,
        remaining: Regex::new(r"remainingSeconds\s*=\s*([0-9]+)\s*[;,)]")?,
        period: Regex::new(r"totalSeconds\s*=\s*([0-9]+)\s*[;,)]")?,
        refresh: Regex::new(r#"^\s*"?\s*([0-9]{1,5})(?:[^0-9]|$)"#)?,
    })
});

pub(super) fn page(bytes: &[u8], refresh: Option<&str>) -> Option<OtpCode> {
    let html = String::from_utf8_lossy(bytes);
    let patterns = PATTERNS.as_ref().ok()?;
    let capture = patterns
        .code
        .captures(&html)
        .or_else(|| patterns.value.captures(&html))?;
    let code = capture.get(1)?.as_str().to_owned();
    let account = patterns
        .account
        .captures(&html)
        .and_then(|capture| capture.get(1).map(|value| value.as_str().trim().to_owned()))
        .unwrap_or_default();
    let seconds = |pattern: &Regex, source: &str| {
        pattern
            .captures(source)
            .and_then(|capture| capture.get(1)?.as_str().parse::<u32>().ok())
    };
    let remaining_seconds = refresh
        .and_then(|raw| seconds(&patterns.refresh, raw))
        .filter(|value| *value > 0)
        .or_else(|| seconds(&patterns.remaining, &html))
        .unwrap_or(30);
    Some(OtpCode {
        code,
        account,
        remaining_seconds,
        period_seconds: seconds(&patterns.period, &html).unwrap_or(30),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_markup_value_account_and_server_clock() -> anyhow::Result<()> {
        let code = page(br#"<span id='DataList1_LabelNum_0'><b>&nbsp;123456</b></span><span id='DataList1_LabelAccount_0'> account </span>remainingSeconds = 14; totalSeconds = 60;"#, Some("  \"12;url=/Default.aspx")).ok_or_else(|| anyhow::anyhow!("no OTP"))?;
        assert_eq!(
            (
                code.code.as_str(),
                code.account.as_str(),
                code.remaining_seconds,
                code.period_seconds
            ),
            ("123456", "account", 12, 60)
        );
        let code = page(
            br#"<input id="DataList1_LabelNum_9" value="0123">remainingSeconds=7);"#,
            Some("0;url=/"),
        )
        .ok_or_else(|| anyhow::anyhow!("no value OTP"))?;
        assert_eq!(
            (
                code.code.as_str(),
                code.remaining_seconds,
                code.period_seconds
            ),
            ("0123", 7, 30)
        );
        Ok(())
    }
    #[test]
    fn missing_invalid_overlong_and_lossy_utf8_are_distinguished() {
        for raw in [
            "login",
            "<span id='DataList1_LabelNum_0'>123</span>",
            "<span id='DataList1_LabelNum_0'>123456789</span>",
        ] {
            assert!(page(raw.as_bytes(), None).is_none());
        }
        assert_eq!(
            page(
                b"\xff<span id='DataList1_LabelNum_0'>123456</span>",
                Some("999999")
            )
            .map(|code| code.remaining_seconds),
            Some(30)
        );
    }
}
