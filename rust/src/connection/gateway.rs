use super::model::VpnFailure;

pub(crate) fn normalize(raw: &str) -> Result<String, VpnFailure> {
    let invalid = || VpnFailure::new("gateway", "服务器地址无效，请填写校园网关主机或 HTTPS 地址");
    if raw.len() > 4096 || raw.chars().any(char::is_control) || raw.contains('\\') {
        return Err(invalid());
    }
    let text = raw.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return Err(invalid());
    }
    let text = match text.split_once("://") {
        Some((scheme, rest))
            if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") =>
        {
            format!("https://{rest}")
        }
        Some(_) => return Err(invalid()),
        None => format!("https://{text}"),
    };
    let url = reqwest::Url::parse(&text).map_err(|_| invalid())?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_normalization_is_https_only_and_preserves_explicit_ports() -> anyhow::Result<()> {
        for (input, expected) in [
            (" atrust.shu.edu.cn ", "https://atrust.shu.edu.cn/"),
            (
                "HTTP://ATRUST.SHU.EDU.CN:80/path?q=1",
                "https://atrust.shu.edu.cn:80/path?q=1",
            ),
            ("[2001:db8::1]:8443", "https://[2001:db8::1]:8443/"),
            (
                "https://atrust.shu.edu.cn:443",
                "https://atrust.shu.edu.cn/",
            ),
        ] {
            assert_eq!(normalize(input)?, expected);
        }
        for input in [
            "",
            " ",
            "https://[",
            "ftp://atrust.shu.edu.cn",
            "https://user:secret@atrust.shu.edu.cn",
            "https://atrust.shu.edu.cn/#token",
            "atrust.shu.edu.cn\n",
            "atrust .shu.edu.cn",
            "https:\\\\atrust.shu.edu.cn",
        ] {
            assert!(
                normalize(input).is_err_and(|error| error.code == "gateway"),
                "{input:?}"
            );
        }
        assert!(normalize(&"x".repeat(4097)).is_err());
        Ok(())
    }
}
