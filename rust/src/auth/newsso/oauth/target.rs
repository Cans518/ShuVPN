use super::super::transport::Config;
use crate::auth::model::AuthFailure;
use reqwest::Url;

pub(in crate::auth::newsso) struct Target {
    pub id: &'static str,
    pub client_id: &'static str,
    pub scope: &'static str,
    pub base: Url,
    pub callback: Url,
}

impl Target {
    pub(in crate::auth::newsso) fn new(config: &Config, id: &str) -> Result<Self, AuthFailure> {
        let base = config.system_base(id)?;
        let (id, client_id, scope, path) = match id {
            "jwxt" => (
                "jwxt",
                "Km5t225E8KECKQ6ZDm5K2P6aS2459Cua",
                "jw",
                "/sso/shulogin",
            ),
            "otp" => (
                "otp",
                "05Q1L8woQK5350aK1U5o5GKh411ar3h1",
                "read write",
                "//Callback.aspx",
            ),
            _ => return Err(AuthFailure::new("unknownSystem", "该系统需要原生协议认证")),
        };
        // Registered OTP path has two slashes: URL::join would treat it as a host.
        let mut callback = base.clone();
        callback.set_path(path);
        Ok(Self {
            id,
            client_id,
            scope,
            base,
            callback,
        })
    }

    pub(super) fn validate_callback(&self, uri: &Url, state: &str) -> Result<(), AuthFailure> {
        if uri.origin() != self.callback.origin()
            || uri.path() != self.callback.path()
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.fragment().is_some()
        {
            return Err(AuthFailure::new(
                "unexpectedRedirect",
                "统一认证返回了未预期的回调",
            ));
        }
        let codes: Vec<_> = uri
            .query_pairs()
            .filter(|(name, _)| name == "code")
            .collect();
        let states: Vec<_> = uri
            .query_pairs()
            .filter(|(name, _)| name == "state")
            .collect();
        if codes.len() != 1 || codes[0].1.is_empty() || codes[0].1.len() > 4096 {
            return Err(AuthFailure::new(
                "authorizeFailed",
                "统一认证未返回有效授权码",
            ));
        }
        if !state.is_empty() && (states.len() != 1 || states[0].1 != state)
            || state.is_empty() && states.len() > 1
        {
            return Err(AuthFailure::new("oauthState", "授权回调 state 校验失败"));
        }
        Ok(())
    }
}
