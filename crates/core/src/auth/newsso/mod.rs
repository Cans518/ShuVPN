use super::{cookies::Jar, model::AuthFailure};
use reqwest::{Method, Url};
use serde_json::{json, Value};
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub(crate) mod account;
pub(crate) mod connection;
mod jwxt;
pub(crate) mod model;
mod oauth;
mod otp;
mod password;
mod probe;
mod qr;
mod store;
#[cfg(test)]
mod tests;
mod transport;
use model::{LoginResult, SsoSnapshot, VerificationTarget};

const LOGIN_PATH: &str = "/oauth2/login/";

pub(crate) struct Client {
    config: transport::Config,
    state: Mutex<State>,
    operation: tokio::sync::Mutex<()>,
}

struct State {
    generation: u64,
    cancelled: CancellationToken,
    jar: Jar,
    challenge: Option<Challenge>,
    legacy_imported: bool,
    import_open: bool,
    store: Option<store::Store>,
    closed: bool,
    qr: Option<qr::Pending>,
    oauth_states: std::collections::BTreeMap<String, String>,
    device_id: Option<String>,
    account: Option<Value>,
}

#[derive(Clone)]
struct Challenge {
    uri: Url,
    params: String,
    username: String,
    password: String,
    methods: Vec<VerificationTarget>,
}

struct Transaction {
    generation: u64,
    jar: Jar,
    challenge: Option<Challenge>,
    qr: Option<qr::Pending>,
    oauth_states: std::collections::BTreeMap<String, String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            generation: 0,
            cancelled: CancellationToken::new(),
            jar: Jar::default(),
            challenge: None,
            legacy_imported: false,
            import_open: true,
            store: None,
            closed: false,
            qr: None,
            oauth_states: std::collections::BTreeMap::new(),
            device_id: None,
            account: None,
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(state) = self.state.lock() {
            state.cancelled.cancel();
        }
    }
}

impl Client {
    pub(crate) fn new() -> Result<Self, AuthFailure> {
        Self::configured(transport::Config::production(password::public_key()?)?)
    }

    pub(crate) fn open(directory: String) -> Result<Self, AuthFailure> {
        Self::open_configured(
            transport::Config::production(password::public_key()?)?,
            directory,
        )
    }

    fn open_configured(config: transport::Config, directory: String) -> Result<Self, AuthFailure> {
        let client = Self::configured(config)?;
        let store = store::Store::open(directory)?;
        let loaded = store.load()?;
        {
            let mut state = client.state()?;
            if let Some(record) = loaded {
                if record.origin != client.config.sso.origin().ascii_serialization() {
                    return Err(AuthFailure::new("ssoStorage", "会话文件来源不匹配"));
                }
                state.jar = Jar::restore_session(&client.config.sso, &record.cookies)?;
                state.legacy_imported = record.legacy_imported;
                state.import_open = record.import_open;
                state.device_id = record.device_id;
                state.account = record.account;
            }
            state.store = Some(store);
        }
        Ok(client)
    }

    fn configured(config: transport::Config) -> Result<Self, AuthFailure> {
        config.validate(&config.entry)?;
        config.validate(&config.sso)?;
        Ok(Self {
            config,
            state: Mutex::new(State::default()),
            operation: tokio::sync::Mutex::new(()),
        })
    }

    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>, AuthFailure> {
        self.state
            .lock()
            .map_err(|_| AuthFailure::new("ssoState", "认证会话状态不可用"))
    }

    pub(crate) fn clear(&self) -> Result<(), AuthFailure> {
        self.reset().map(|_| ())
    }

    fn reset(&self) -> Result<(u64, CancellationToken), AuthFailure> {
        let mut state = self.state()?;
        live(&state)?;
        let generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| AuthFailure::new("ssoState", "认证会话代次超过限制"))?;
        state.cancelled.cancel();
        state.generation = generation;
        state.cancelled = CancellationToken::new();
        state.jar = Jar::default();
        state.challenge = None;
        state.qr = None;
        state.oauth_states.clear();
        state.account = None;
        state.import_open = false;
        self.persist(&state, Some(&state.jar), state.legacy_imported, false)?;
        Ok((state.generation, state.cancelled.clone()))
    }

    pub(crate) fn close(&self) -> Result<(), AuthFailure> {
        let mut state = self.state()?;
        if state.closed {
            return Ok(());
        }
        state.cancelled.cancel();
        state.closed = true;
        state.jar = Jar::default();
        state.challenge = None;
        state.qr = None;
        state.oauth_states.clear();
        state.store.take();
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Result<SsoSnapshot, AuthFailure> {
        let state = self.state()?;
        live(&state)?;
        let destination = self
            .config
            .sso
            .join("/oauth/authorize")
            .map_err(|_| invalid())?;
        let names = state.jar.names(&destination);
        Ok(SsoSnapshot {
            has_session: names.iter().any(|name| name == "SHU_OAUTH2"),
            challenge_methods: state
                .challenge
                .as_ref()
                .map(|challenge| challenge.methods.clone())
                .unwrap_or_default(),
            cookie_names: names,
            legacy_imported: state.legacy_imported,
            persistent: state.store.is_some(),
            legacy_import_allowed: state.import_open,
            qr_status: state.qr.as_ref().map(|qr| qr.status),
        })
    }

    pub(crate) fn import_legacy(&self, document: &str) -> Result<usize, AuthFailure> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| AuthFailure::new("ssoState", "认证进行中，不能导入旧会话"))?;
        let generation = {
            let state = self.state()?;
            live(&state)?;
            state.generation
        };
        let jar = self.prepare_legacy(document)?;
        self.commit_legacy(generation, jar)
    }

    fn prepare_legacy(&self, document: &str) -> Result<Jar, AuthFailure> {
        if document.len() > 64 * 1024 {
            return Err(invalid());
        }
        let values: Value = serde_json::from_str(document).map_err(|_| invalid())?;
        let values = values
            .as_array()
            .filter(|values| values.len() <= 256)
            .ok_or_else(invalid)?;
        let mut jar = Jar::default();
        for value in values {
            if value["name"] != "SHU_OAUTH2" {
                continue;
            }
            let Some(secret) = value["value"].as_str().filter(|value| !value.is_empty()) else {
                continue;
            };
            let path = value["path"]
                .as_str()
                .filter(|path| path.starts_with('/'))
                .unwrap_or("/");
            let mut cookie = cookie::Cookie::new("SHU_OAUTH2", secret.to_owned());
            cookie.set_path(path.to_owned());
            cookie.set_secure(true);
            cookie.set_http_only(true);
            if let Some(expires) = value["expires"].as_str() {
                let Ok(expires) = cookie::time::OffsetDateTime::parse(
                    expires,
                    &cookie::time::format_description::well_known::Rfc3339,
                ) else {
                    continue;
                };
                if expires <= cookie::time::OffsetDateTime::now_utc() {
                    continue;
                }
                cookie.set_expires(expires);
            }
            jar.insert(&self.config.sso, cookie)?;
        }
        Ok(jar)
    }

    fn commit_legacy(&self, generation: u64, jar: Jar) -> Result<usize, AuthFailure> {
        let mut state = self.state()?;
        live(&state)?;
        if state.generation != generation {
            return Err(cancelled_failure());
        }
        if state.legacy_imported {
            return Err(AuthFailure::new("alreadyImported", "旧会话已经导入"));
        }
        if !state.import_open {
            return Err(AuthFailure::new("importClosed", "当前会话已关闭旧数据导入"));
        }
        if state.challenge.is_some() || !state.jar.names(&self.config.sso).is_empty() {
            return Err(AuthFailure::new("ssoState", "不能覆盖当前认证会话"));
        }
        let next = state
            .generation
            .checked_add(1)
            .ok_or_else(|| AuthFailure::new("ssoState", "认证会话代次超过限制"))?;
        self.persist(&state, Some(&jar), true, false)?;
        state.cancelled.cancel();
        state.generation = next;
        state.cancelled = CancellationToken::new();
        let imported = jar.count();
        state.jar = jar;
        state.legacy_imported = true;
        state.import_open = false;
        Ok(imported)
    }

    pub(crate) async fn login(
        &self,
        username: String,
        password: String,
    ) -> Result<LoginResult, AuthFailure> {
        if username.is_empty() || username.len() > 256 || password.is_empty() {
            return Err(AuthFailure::new("credentials", "请输入校园账号和密码"));
        }
        let password = zeroize::Zeroizing::new(password);
        let encrypted = password::encrypt(&self.config.key, &password)?;
        drop(password);
        let (generation, cancelled) = self.reset()?;
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                let result = self.login_inner(&mut transaction, username, encrypted).await;
                self.finish(transaction, result)
            } => result,
        }
    }

    async fn login_inner(
        &self,
        transaction: &mut Transaction,
        username: String,
        password: String,
    ) -> Result<LoginResult, AuthFailure> {
        let uri = self.discover(transaction).await?;
        let params = uri
            .path()
            .strip_prefix(LOGIN_PATH)
            .filter(|params| !params.is_empty() && params.len() <= 8192 && !params.contains('/'))
            .ok_or_else(invalid)?
            .to_owned();
        let response = self.json(transaction, uri.join("/oauth/userLogin").map_err(|_| invalid())?,
            json!({"username":username,"password":password,"tenantId":"上海大学","params":params}), &uri).await?;
        success(&response)?;
        if response["twoStepRequired"] == true {
            let methods = methods(&response["twoStepMethods"])?;
            transaction.challenge = Some(Challenge {
                uri,
                params,
                username,
                password,
                methods: methods.clone(),
            });
            Ok(LoginResult {
                methods,
                callback_uri: None,
            })
        } else {
            let callback = self.callback(&uri, &response)?;
            transaction.challenge = None;
            Ok(LoginResult {
                methods: vec![],
                callback_uri: Some(callback),
            })
        }
    }

    pub(crate) async fn send_code(&self, method: String) -> Result<(), AuthFailure> {
        let (generation, cancelled) = {
            let state = self.state()?;
            (state.generation, state.cancelled.clone())
        };
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                let result = async {
                    let challenge = require_challenge(&transaction, &method)?;
                    let response = self.json(&mut transaction, challenge.uri.join("/oauth/twoStep/send").map_err(|_| invalid())?,
                        json!({"method":method}), &challenge.uri).await?;
                    success(&response)
                }.await;
                self.finish(transaction, result)
            } => result,
        }
    }

    pub(crate) async fn verify_code(
        &self,
        method: String,
        code: String,
    ) -> Result<LoginResult, AuthFailure> {
        if code.is_empty() || code.len() > 256 {
            return Err(AuthFailure::new("invalidCode", "验证码错误或已失效"));
        }
        let (generation, cancelled) = {
            let state = self.state()?;
            (state.generation, state.cancelled.clone())
        };
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                let result = async {
                    let challenge = require_challenge(&transaction, &method)?;
                    let response = self.json(&mut transaction, challenge.uri.join("/oauth/twoStep/verify").map_err(|_| invalid())?,
                        json!({"username":challenge.username,"password":challenge.password,
                            "tenantId":"上海大学","params":challenge.params,"method":method,"code":code}), &challenge.uri).await?;
                    success(&response)?;
                    let callback = self.callback(&challenge.uri, &response)?;
                    transaction.challenge = None;
                    Ok(LoginResult { methods: vec![], callback_uri: Some(callback) })
                }.await;
                self.finish(transaction, result)
            } => result,
        }
    }

    fn begin(&self, generation: u64) -> Result<Transaction, AuthFailure> {
        let state = self.state()?;
        live(&state)?;
        if state.generation != generation || state.cancelled.is_cancelled() {
            return Err(cancelled_failure());
        }
        Ok(Transaction {
            generation,
            jar: state.jar.clone(),
            challenge: state.challenge.clone(),
            qr: state.qr.clone(),
            oauth_states: state.oauth_states.clone(),
        })
    }

    fn finish<T>(
        &self,
        transaction: Transaction,
        result: Result<T, AuthFailure>,
    ) -> Result<T, AuthFailure> {
        let mut state = self.state()?;
        live(&state)?;
        if state.generation != transaction.generation || state.cancelled.is_cancelled() {
            return Err(cancelled_failure());
        }
        // Valid two-step errors may rotate challenge cookies. Protocol,
        // transport and redirect failures must not publish staged credentials.
        if result.is_ok()
            || result.as_ref().err().is_some_and(|error| {
                matches!(
                    error.code.as_str(),
                    "invalidCode"
                        | "sendError"
                        | "senderror"
                        | "ipLimitExceeded"
                        | "userLocked"
                        | "badPassword"
                        | "userNotFound"
                        | "userNotAllowed"
                        | "badRequestParams"
                )
            })
        {
            self.persist(
                &state,
                if transaction.challenge.is_none() && transaction.qr.is_none() {
                    Some(&transaction.jar)
                } else {
                    None
                },
                state.legacy_imported,
                state.import_open,
            )?;
            state.jar = transaction.jar;
            state.challenge = transaction.challenge;
            state.qr = transaction.qr;
            state.oauth_states = transaction.oauth_states;
        }
        result
    }

    fn persist(
        &self,
        state: &State,
        jar: Option<&Jar>,
        legacy_imported: bool,
        import_open: bool,
    ) -> Result<(), AuthFailure> {
        if let Some(store) = &state.store {
            store.save(&store::Record {
                version: if state.device_id.is_some() || state.account.is_some() {
                    2
                } else {
                    1
                },
                origin: self.config.sso.origin().ascii_serialization(),
                legacy_imported,
                import_open,
                cookies: jar
                    .map(|jar| jar.session_records(&self.config.sso))
                    .transpose()?
                    .unwrap_or_default(),
                device_id: state.device_id.clone(),
                account: state.account.clone(),
            })?;
        }
        Ok(())
    }

    async fn discover(&self, transaction: &mut Transaction) -> Result<Url, AuthFailure> {
        let mut uri = self.config.entry.clone();
        for _ in 0..16 {
            let response = self
                .config
                .request(&mut transaction.jar, Method::GET, uri.clone(), None, None)
                .await?;
            if response.status.is_redirection() {
                let location = response.location.as_deref().ok_or_else(invalid)?;
                uri = self.config.redirect(&uri, location)?;
            } else if uri.origin() == self.config.sso.origin()
                && uri.path().starts_with(LOGIN_PATH)
                && response.status.is_success()
            {
                return Ok(uri);
            } else {
                return Err(AuthFailure::new(
                    "loginPageNotFound",
                    "无法取得统一认证登录入口",
                ));
            }
        }
        Err(AuthFailure::new("tooManyRedirects", "认证入口跳转次数过多"))
    }

    async fn json(
        &self,
        transaction: &mut Transaction,
        uri: Url,
        body: Value,
        referer: &Url,
    ) -> Result<Value, AuthFailure> {
        let response = self
            .config
            .request(
                &mut transaction.jar,
                Method::POST,
                uri,
                Some(body),
                Some(referer),
            )
            .await?;
        let value: Value = serde_json::from_slice(&response.body).map_err(|_| invalid())?;
        if !value.is_object() {
            return Err(invalid());
        }
        if !response.status.is_success() {
            success(&value)?;
            return Err(AuthFailure::new("http", "学校认证服务请求失败"));
        }
        Ok(value)
    }

    fn callback(&self, current: &Url, response: &Value) -> Result<String, AuthFailure> {
        let raw = response["redirectUri"]
            .as_str()
            .filter(|raw| !raw.is_empty())
            .ok_or_else(|| AuthFailure::new("missingRedirect", "学校未返回授权地址"))?;
        let uri = current.join(raw).map_err(|_| invalid())?;
        self.config.validate(&uri)?;
        Ok(uri.into())
    }
}

fn require_challenge(transaction: &Transaction, method: &str) -> Result<Challenge, AuthFailure> {
    let challenge = transaction.challenge.as_ref().ok_or_else(|| {
        AuthFailure::new("challengeExpired", "登录状态已失效，请重新输入账号密码")
    })?;
    if !challenge
        .methods
        .iter()
        .any(|target| target.method == method)
    {
        return Err(AuthFailure::new("verificationMethod", "验证方式不可用"));
    }
    Ok(challenge.clone())
}

fn methods(value: &Value) -> Result<Vec<VerificationTarget>, AuthFailure> {
    let Some(value) = value.as_object() else {
        return Err(AuthFailure::new(
            "noTwoStepMethod",
            "学校未返回可用的验证方式",
        ));
    };
    let methods: Vec<_> = ["wecom", "sms"]
        .into_iter()
        .filter_map(|method| {
            value
                .get(method)
                .and_then(|value| match value {
                    Value::Null => Some(String::new()),
                    Value::String(value) => Some(value.clone()),
                    Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
                    _ => None,
                })
                .filter(|target| target.len() <= 256)
                .map(|target| VerificationTarget {
                    method: method.to_owned(),
                    target,
                })
        })
        .collect();
    if methods.is_empty() {
        return Err(AuthFailure::new(
            "noTwoStepMethod",
            "学校未返回可用的验证方式",
        ));
    }
    Ok(methods)
}

fn success(response: &Value) -> Result<(), AuthFailure> {
    match response["message"].as_str() {
        Some("success") => Ok(()),
        Some(code) => {
            let message = match code {
                "badPassword" => "学号或密码错误",
                "userNotFound" => "未找到该校园账户",
                "invalidCode" => "验证码错误或已失效",
                "userLocked" => "账户已被锁定，请稍后重试",
                "ipLimitExceeded" => "登录请求过于频繁，请稍后重试",
                "sendError" | "senderror" => "验证码发送过于频繁，请切换验证方式或稍后再试",
                "userNotAllowed" => "该账户暂时无法登录此服务",
                "internalServerError" => "学校认证服务暂时不可用",
                "badRequestParams" => "登录参数无效，请升级应用后重试",
                _ => "登录失败，请稍后重试",
            };
            let code = if matches!(
                code,
                "badPassword"
                    | "userNotFound"
                    | "invalidCode"
                    | "userLocked"
                    | "ipLimitExceeded"
                    | "sendError"
                    | "senderror"
                    | "userNotAllowed"
                    | "internalServerError"
                    | "badRequestParams"
            ) {
                code
            } else {
                "unknown"
            };
            Err(AuthFailure::new(code, message))
        }
        None => Err(invalid()),
    }
}

fn invalid() -> AuthFailure {
    AuthFailure::new("invalidResponse", "学校认证服务返回了无法识别的内容")
}
fn cancelled_failure() -> AuthFailure {
    AuthFailure::new("cancelled", "认证已取消")
}

fn live(state: &State) -> Result<(), AuthFailure> {
    if state.closed {
        Err(AuthFailure::new("closed", "认证会话已经关闭"))
    } else {
        Ok(())
    }
}
