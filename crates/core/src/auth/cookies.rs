use cookie::Cookie;
use cookie_store::{CookieDomain, CookieStore};
use reqwest::{header::HeaderValue, Url};
use std::collections::{BTreeMap, BTreeSet};

use super::model::AuthFailure;

type Key = (String, String, String);

/// HTTP cookie semantics stay here; account state owns this jar and its epoch.
#[derive(Clone, Default)]
pub(super) struct Jar {
    store: CookieStore,
    order: BTreeMap<Key, u64>,
    next: u64,
}

impl Jar {
    pub(super) fn save(&mut self, source: &Url, raw: &str) -> Result<(), AuthFailure> {
        if raw.len() > 8192 {
            return Err(failure("Cookie 超过大小限制"));
        }
        let cookie = match Cookie::parse(raw.to_owned()) {
            Ok(cookie) => cookie,
            Err(_) => return Ok(()),
        };
        self.insert(source, cookie)
    }

    pub(super) fn insert(
        &mut self,
        source: &Url,
        mut raw: Cookie<'static>,
    ) -> Result<(), AuthFailure> {
        if raw.name().len()
            + raw.value().len()
            + raw.path().map_or(0, str::len)
            + raw.domain().map_or(0, str::len)
            > 8192
        {
            return Err(failure("Cookie 超过大小限制"));
        }
        if !valid_name(raw.name()) || !valid_value(raw.value()) {
            return Ok(());
        }
        // Existing SHU storage treats an empty authentication cookie as a
        // scoped deletion, even when the server omits Max-Age.
        if raw.value().is_empty() {
            raw.set_max_age(cookie::time::Duration::ZERO);
        }
        if raw.secure() == Some(true) && source.scheme() != "https"
            || raw.name().starts_with("__Secure-")
                && (raw.secure() != Some(true) || source.scheme() != "https")
            || raw.name().starts_with("__Host-")
                && (raw.secure() != Some(true)
                    || source.scheme() != "https"
                    || raw.domain().is_some()
                    || raw.path() != Some("/"))
        {
            return Ok(());
        }
        let parsed = match cookie_store::Cookie::try_from_raw_cookie(&raw, source) {
            Ok(cookie) => cookie.into_owned(),
            Err(_) => return Ok(()),
        };
        if let CookieDomain::Suffix(domain) = &parsed.domain {
            if psl::suffix_str(domain).is_some_and(|suffix| suffix == domain) {
                return Ok(());
            }
        }
        self.purge();
        let key = key(&parsed);
        if self.order.len() >= 256 && !self.order.contains_key(&key) {
            return Err(failure("Cookie 数量超过限制"));
        }
        // insert() does not log cookie values, unlike store_response_cookies().
        if self.store.insert(parsed, source).is_ok() && !self.order.contains_key(&key) {
            self.order.insert(key, self.next);
            self.next = self
                .next
                .checked_add(1)
                .ok_or_else(|| failure("Cookie 次序超过限制"))?;
        }
        self.purge();
        Ok(())
    }

    pub(super) fn header(&self, destination: &Url) -> Result<Option<HeaderValue>, AuthFailure> {
        let mut cookies = self.store.matches(destination);
        cookies.sort_by_key(|cookie| {
            (
                std::cmp::Reverse(cookie.path.len()),
                self.order.get(&key(cookie)).copied().unwrap_or(u64::MAX),
            )
        });
        let header = cookies
            .into_iter()
            .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
            .collect::<Vec<_>>()
            .join("; ");
        if header.len() > 32 * 1024 {
            return Err(failure("Cookie 请求头超过限制"));
        }
        if header.is_empty() {
            return Ok(None);
        }
        let mut value =
            HeaderValue::from_str(&header).map_err(|_| failure("Cookie 请求头格式无效"))?;
        value.set_sensitive(true);
        Ok(Some(value))
    }

    pub(super) fn names(&self, destination: &Url) -> Vec<String> {
        self.store
            .matches(destination)
            .iter()
            .map(|cookie| cookie.name().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(super) fn value(
        &self,
        destination: &Url,
        name: &str,
    ) -> Result<Option<String>, AuthFailure> {
        let Some(header) = self.header(destination)? else {
            return Ok(None);
        };
        let raw = header
            .to_str()
            .map_err(|_| failure("Cookie 请求头格式无效"))?;
        Ok(raw
            .split("; ")
            .filter_map(|entry| entry.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.to_owned()))
    }

    pub(super) fn count(&self) -> usize {
        self.store.iter_unexpired().count()
    }

    pub(super) fn auth_seed(&self, source: &Url) -> Vec<super::model::AuthCookie> {
        self.store
            .iter_unexpired()
            .filter(|cookie| cookie.domain.matches(source))
            .map(|cookie| super::model::AuthCookie {
                name: cookie.name().to_owned(),
                value: cookie.value().to_owned(),
                domain: String::from(&cookie.domain),
                path: String::from(&cookie.path),
                expires_unix_seconds: match cookie.expires {
                    cookie_store::CookieExpiration::AtUtc(time) => Some(time.unix_timestamp()),
                    cookie_store::CookieExpiration::SessionEnd => None,
                },
            })
            .collect()
    }

    pub(super) fn session_records(&self, source: &Url) -> Result<Vec<String>, AuthFailure> {
        let mut cookies: Vec<_> = self
            .store
            .iter_unexpired()
            .filter(|cookie| cookie.name() == "SHU_OAUTH2" && cookie.domain.matches(source))
            .collect();
        cookies.sort_by_key(|cookie| self.order.get(&key(cookie)).copied().unwrap_or(u64::MAX));
        cookies
            .into_iter()
            .map(|cookie| {
                let mut raw: Cookie<'static> = std::ops::Deref::deref(cookie).clone();
                raw.set_path(String::from(&cookie.path));
                match &cookie.domain {
                    CookieDomain::HostOnly(_) => raw.unset_domain(),
                    CookieDomain::Suffix(domain) => raw.set_domain(domain.clone()),
                    _ => {}
                }
                // Replaying Max-Age would restart its TTL on every process boot.
                raw.set_max_age(None);
                match cookie.expires {
                    cookie_store::CookieExpiration::AtUtc(time) => raw.set_expires(time),
                    cookie_store::CookieExpiration::SessionEnd => raw.unset_expires(),
                }
                let record = raw.to_string();
                if record.len() > 8192 {
                    return Err(failure("会话 Cookie 超过大小限制"));
                }
                Ok(record)
            })
            .collect()
    }

    pub(super) fn restore_session(source: &Url, records: &[String]) -> Result<Self, AuthFailure> {
        if records.len() > 256 {
            return Err(failure("会话 Cookie 数量超过限制"));
        }
        let mut jar = Self::default();
        for record in records {
            let cookie =
                Cookie::parse(record.clone()).map_err(|_| failure("会话 Cookie 格式无效"))?;
            if record.len() > 8192
                || cookie.name() != "SHU_OAUTH2"
                || cookie.max_age().is_some()
                || !valid_value(cookie.value())
                || cookie.value().is_empty()
                || !cookie.path().is_some_and(|path| path.starts_with('/'))
            {
                return Err(failure("会话 Cookie 格式无效"));
            }
            let parsed = cookie_store::Cookie::try_from_raw_cookie(&cookie, source)
                .map_err(|_| failure("会话 Cookie 格式无效"))?;
            if !parsed.domain.matches(source)
                || matches!(&parsed.domain, CookieDomain::Suffix(domain) if psl::suffix_str(domain).is_some_and(|suffix| suffix == domain))
            {
                return Err(failure("会话 Cookie 来源无效"));
            }
            jar.insert(source, cookie)?;
        }
        Ok(jar)
    }

    fn purge(&mut self) {
        let expired: Vec<_> = self
            .store
            .iter_any()
            .filter(|cookie| cookie.is_expired())
            .map(key)
            .collect();
        for (domain, path, name) in expired {
            self.store.remove(&domain, &path, &name);
            self.order.remove(&(domain, path, name));
        }
    }
}

fn key(cookie: &cookie_store::Cookie<'_>) -> Key {
    (
        String::from(&cookie.domain),
        String::from(&cookie.path),
        cookie.name().to_owned(),
    )
}

pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn valid_value(value: &str) -> bool {
    let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        &value[1..value.len() - 1]
    } else {
        value
    };
    value
        .bytes()
        .all(|byte| matches!(byte, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}

fn failure(message: &str) -> AuthFailure {
    AuthFailure::new("ssoCookie", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cookie_scope_security_expiry_prefixes_and_path_order_are_preserved() -> anyhow::Result<()> {
        let source = Url::parse("https://newsso.shu.edu.cn/oauth/login")?;
        let mut jar = Jar::default();
        for cookie in [
            "session=root; Path=/; Secure; HttpOnly",
            "session=oauth; Path=/oauth; Secure",
            "default=path",
            "wide=parent; Domain=shu.edu.cn; Path=/; Secure",
            "suffix=forbidden; Domain=edu.cn; Path=/",
            "foreign=forbidden; Domain=other.shu.edu.cn; Path=/",
            "__Host-bad=forbidden; Secure; Domain=shu.edu.cn; Path=/",
            "__Secure-bad=forbidden; Path=/",
        ] {
            jar.save(&source, cookie)?;
        }
        let same = Url::parse("https://newsso.shu.edu.cn/oauth/token")?;
        let header = jar
            .header(&same)?
            .ok_or_else(|| anyhow::anyhow!("missing header"))?;
        assert!(header
            .to_str()?
            .starts_with("session=oauth; default=path; session=root"));
        assert!(!header.to_str()?.contains("forbidden"));
        let other = Url::parse("https://other.newsso.shu.edu.cn/oauth/token")?;
        assert_eq!(
            jar.header(&other)?
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("wide=parent".to_owned())
        );
        assert!(jar
            .header(&Url::parse("http://newsso.shu.edu.cn/oauth/token")?)?
            .is_some_and(|value| value == "default=path"));
        jar.save(&source, "session=deleted; Path=/oauth; Max-Age=0")?;
        assert!(!jar
            .header(&same)?
            .ok_or_else(|| anyhow::anyhow!("missing header"))?
            .to_str()?
            .contains("session=oauth"));
        jar.save(
            &source,
            "session=expired; Path=/; Expires=Thu, 01 Jan 1970 00:00:01 GMT",
        )?;
        assert!(!jar.names(&same).contains(&"session".to_owned()));
        assert!(!jar
            .names(&Url::parse("https://newsso.shu.edu.cn/oauth2")?)
            .contains(&"default".to_owned()));
        jar.save(&source, "default=; Path=/oauth")?;
        assert!(jar
            .header(&Url::parse("http://newsso.shu.edu.cn/oauth/token")?)?
            .is_none());
        Ok(())
    }

    #[test]
    fn persisted_session_keeps_scope_attributes_and_absolute_expiry_without_extending_max_age(
    ) -> anyhow::Result<()> {
        let source = Url::parse("https://newsso.shu.edu.cn/oauth/login")?;
        let mut jar = Jar::default();
        jar.save(
            &source,
            "SHU_OAUTH2=host; Secure; HttpOnly; SameSite=Lax; Max-Age=3600",
        )?;
        jar.save(
            &source,
            "SHU_OAUTH2=parent; Domain=shu.edu.cn; Path=/; Secure",
        )?;
        jar.save(&source, "business=discard; Path=/; Secure")?;
        let saved = jar.session_records(&source)?;
        assert_eq!(saved.len(), 2);
        let host = Cookie::parse(saved[0].clone())?;
        assert_eq!(host.domain(), None);
        assert_eq!(host.path(), Some("/oauth"));
        assert_eq!(host.secure(), Some(true));
        assert_eq!(host.http_only(), Some(true));
        assert_eq!(host.same_site(), Some(cookie::SameSite::Lax));
        assert!(host.max_age().is_none() && host.expires_datetime().is_some());
        let restored = Jar::restore_session(&source, &saved)?;
        let second_save = restored.session_records(&source)?;
        let second_host = Cookie::parse(second_save[0].clone())?;
        assert_eq!(host.expires_datetime(), second_host.expires_datetime());
        let sibling = Url::parse("https://other.newsso.shu.edu.cn/oauth/login")?;
        assert_eq!(
            restored
                .header(&sibling)?
                .ok_or_else(|| anyhow::anyhow!("missing parent cookie"))?,
            "SHU_OAUTH2=parent"
        );
        assert!(!restored.names(&source).contains(&"business".to_owned()));
        Ok(())
    }

    #[test]
    fn persisted_invalid_scope_is_an_error_and_expired_cookie_is_omitted() -> anyhow::Result<()> {
        let source = Url::parse("https://newsso.shu.edu.cn/")?;
        for record in [
            "SHU_OAUTH2=secret; Domain=other.example; Path=/",
            "SHU_OAUTH2=secret; Domain=edu.cn; Path=/",
            "SHU_OAUTH2=bad\\value; Path=/",
            "SHU_OAUTH2=secret; Path=relative",
        ] {
            assert!(Jar::restore_session(&source, &[record.to_owned()]).is_err());
        }
        let expired = Jar::restore_session(
            &source,
            &["SHU_OAUTH2=expired; Path=/; Expires=Thu, 01 Jan 1970 00:00:01 GMT".to_owned()],
        )?;
        assert_eq!(expired.count(), 0);
        Ok(())
    }
}
