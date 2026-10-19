use std::fmt;

pub struct AuthConfig {
    pub gateway_base: String,
    pub sso_base: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub fallback_login_domain: String,
    pub user_agent: String,
}

#[derive(Clone)]
pub struct AuthCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires_unix_seconds: Option<i64>,
}

pub struct AuthSeed {
    pub gateway_cookies: Vec<AuthCookie>,
    pub sso_cookies: Vec<AuthCookie>,
    pub device_id: String,
}

pub struct AuthSession {
    pub username: String,
    pub sid: String,
    pub ticket: String,
    pub login_domain: String,
    pub gateway_cookies: Vec<AuthCookie>,
    pub sso_cookies: Vec<AuthCookie>,
}

pub struct TunnelOptions {
    pub connection_id: String,
    pub sign_key: Vec<u8>,
    pub process_platform: String,
    pub strict_anti_mitm: bool,
}
pub struct TunnelPreparation {
    pub session: AuthSession,
    pub plan_json: String,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct AuthFailure {
    pub code: String,
    pub message: String,
}

impl AuthFailure {
    pub(crate) fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

impl fmt::Display for AuthFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AuthFailure {}
