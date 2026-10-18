use super::*;
use anyhow::{ensure, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{rand_core::OsRng, Pkcs1v15Encrypt, RsaPrivateKey};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};

#[derive(Clone, Copy, Debug)]
pub(in crate::auth::newsso) enum Scenario {
    Direct,
    Challenge,
    UnsafeDiscovery,
    UnsafeCallback,
    Oversized,
    SlowFirst,
    Loop,
    EmptyTargetChallenge,
    SendRejected,
    SlowSend,
    NoCredentials,
    SlowCredentials,
    SlowVerification,
    ChallengeSlowCredentials,
    ChallengeNoCredentials,
}

pub(in crate::auth::newsso) struct Recorded {
    pub(in crate::auth::newsso) path: String,
    headers: BTreeMap<String, String>,
    body: Value,
}

pub(in crate::auth::newsso) struct Fixture {
    pub(in crate::auth::newsso) client: Arc<Client>,
    pub(in crate::auth::newsso) requests: Arc<tokio::sync::Mutex<Vec<Recorded>>>,
    pub(in crate::auth::newsso) entered: Arc<Notify>,
    pub(in crate::auth::newsso) release: Arc<Notify>,
    private: RsaPrivateKey,
    job: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.job.abort();
    }
}

impl Fixture {
    pub(in crate::auth::newsso) async fn new(scenario: Scenario) -> anyhow::Result<Self> {
        let private = RsaPrivateKey::new(&mut OsRng, 1024)?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let sso = format!("https://sso.fixture.test:{}/", address.port());
        let entry = format!("https://entry.fixture.test:{}/", address.port());
        let certificate = rcgen::generate_simple_self_signed(vec![
            "sso.fixture.test".to_owned(),
            "entry.fixture.test".to_owned(),
            "jwxt.fixture.test".to_owned(),
            "otp.fixture.test".to_owned(),
            "atrust.fixture.test".to_owned(),
        ])?;
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der())
                    .into(),
            )?;
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let client = Arc::new(Client::configured(transport::Config {
            sso: Url::parse(&sso)?,
            entry: Url::parse(&entry)?,
            key: private.to_public_key(),
            fixture: Some((Some(certificate.cert.der().to_vec()), address)),
        })?);
        let requests = Arc::new(tokio::sync::Mutex::new(vec![]));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let seen = requests.clone();
        let slow = entered.clone();
        let resumed = release.clone();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let job = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = clients.join_next(), if !clients.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break; };
                        let tls = acceptor.clone();
                        let seen = seen.clone();
                        let slow = slow.clone();
                        let resumed = resumed.clone();
                        let count = count.clone();
                        let sso = sso.clone();
                        clients.spawn(async move {
                            let mut stream = tls.accept(stream).await?;
                            let mut bytes = vec![];
                            while !bytes.ends_with(b"\r\n\r\n") {
                                ensure!(bytes.len() < 32 * 1024, "fixture header exceeded limit");
                                bytes.push(stream.read_u8().await?);
                            }
                            let header = String::from_utf8(bytes)?;
                            let mut lines = header.split("\r\n");
                            let start = lines.next().context("missing request")?;
                            let path = start.split_whitespace().nth(1).context("missing path")?.to_owned();
                            let headers: BTreeMap<_, _> = lines.filter_map(|line| line.split_once(':'))
                                .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned())).collect();
                            let length = headers.get("content-length").map(|length| length.parse()).transpose()?.unwrap_or(0);
                            let mut body = vec![0; length];
                            stream.read_exact(&mut body).await?;
                            let body: Value = if body.is_empty() { Value::Null } else { serde_json::from_slice(&body)? };
                            seen.lock().await.push(Recorded { path: path.clone(), headers: headers.clone(), body: body.clone() });
                            let mut extra = String::new();
                            let mut status = "200 OK";
                            let response = if headers.get("host").is_some_and(|host| host.starts_with("entry.")) {
                                status = "302 Found";
                                let location = if matches!(scenario, Scenario::UnsafeDiscovery) { "https://attacker.example.net/steal".to_owned() } else { format!("{sso}oauth2/login/bootstrap") };
                                extra = format!("Location: {location}\r\nSet-Cookie: entry_only=entry; Path=/; Secure\r\n");
                                String::new()
                            } else if path == "/oauth2/login/bootstrap" {
                                status = "302 Found";
                                let location = if matches!(scenario, Scenario::Loop) { "/oauth2/login/bootstrap" } else { "/oauth2/login/server-opaque-params" };
                                extra = format!("Location: {location}\r\nSet-Cookie: discovery=ready; Path=/; Secure\r\n");
                                String::new()
                            } else if path.starts_with("/oauth2/login/") {
                                "<html>fixture login</html>".to_owned()
                            } else if path == "/oauth/userLogin" {
                                if matches!(scenario, Scenario::SlowFirst) && count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                                    slow.notify_one();
                                    resumed.notified().await;
                                }
                                if matches!(scenario, Scenario::Oversized) { "x".repeat(1024 * 1024 + 1) }
                                else if matches!(scenario, Scenario::Challenge | Scenario::EmptyTargetChallenge | Scenario::SendRejected | Scenario::SlowSend | Scenario::SlowVerification | Scenario::ChallengeSlowCredentials | Scenario::ChallengeNoCredentials) {
                                    extra = "Set-Cookie: challenge=initial; Path=/; Secure; HttpOnly\r\n".to_owned();
                                    let methods = if matches!(scenario, Scenario::EmptyTargetChallenge) { json!({"sms":null}) }
                                        else { json!({"sms":"138****0000","wecom":"Student","unknown":"ignored"}) };
                                    json!({"message":"success","twoStepRequired":true,"twoStepMethods":methods}).to_string()
                                } else {
                                    extra = format!("Set-Cookie: SHU_OAUTH2=fixture-{}; Path=/; Secure; HttpOnly\r\n", body["username"].as_str().unwrap_or("user"));
                                    let callback = if matches!(scenario, Scenario::UnsafeCallback) { "https://attacker.example.net/steal".to_owned() } else { format!("{sso}oauth/authorize?code=fixture") };
                                    json!({"message":"success","redirectUri":callback}).to_string()
                                }
                            } else if path == "/oauth/twoStep/send" {
                                if matches!(scenario, Scenario::SlowSend) {
                                    slow.notify_one();
                                    resumed.notified().await;
                                }
                                json!({"message": if matches!(scenario, Scenario::SendRejected) { "senderror" } else { "success" }}).to_string()
                            } else if path == "/oauth/twoStep/verify" && body["code"] == "bad" {
                                status = "400 Bad Request";
                                extra = "Set-Cookie: attempt=renewed; Path=/; Secure\r\n".to_owned();
                                json!({"message":"invalidCode"}).to_string()
                            } else if path == "/oauth/twoStep/verify" {
                                if matches!(scenario, Scenario::SlowVerification) {
                                    slow.notify_one();
                                    resumed.notified().await;
                                }
                                extra = "Set-Cookie: SHU_OAUTH2=fixture-session; Path=/; Secure; HttpOnly\r\n".to_owned();
                                json!({"message":"success","redirectUri":format!("{sso}oauth/authorize?code=verified")}).to_string()
                            } else if path.starts_with("/Default.aspx") {
                                if matches!(scenario, Scenario::SlowCredentials | Scenario::ChallengeSlowCredentials) {
                                    slow.notify_one();
                                    resumed.notified().await;
                                }
                                if matches!(scenario, Scenario::NoCredentials | Scenario::ChallengeNoCredentials) {
                                    status = "503 Unavailable";
                                    "no credentials".to_owned()
                                } else {
                                    "<span id='DataList1_LabelNum_0'>012345</span><span id='DataList1_LabelAccount_0'>fixture-account</span>remainingSeconds=9; totalSeconds=60;".to_owned()
                                }
                            } else if path.starts_with("/oauth/authorize?") || path.starts_with("/passport/v1/public/authConfig") {
                                status = "503 Unavailable";
                                String::new()
                            } else { anyhow::bail!("unexpected fixture path"); };
                            stream.write_all(format!("HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", response.len()).as_bytes()).await?;
                            stream.write_all(response.as_bytes()).await?;
                            // Finish TLS records before dropping the socket; a truncated
                            // fixture body must not masquerade as a production network error.
                            stream.shutdown().await?;
                            Ok::<_, anyhow::Error>(())
                        });
                    }
                }
            }
        });
        Ok(Self {
            client,
            requests,
            entered,
            release,
            private,
            job,
        })
    }
}

#[tokio::test]
async fn actual_https_discovers_server_params_encrypts_password_and_keeps_cookie_hosts_separate(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Direct).await?;
    let result = fixture
        .client
        .login("student".to_owned(), "密码-fixture".to_owned())
        .await?;
    ensure!(
        result.callback_uri.is_some() && result.methods.is_empty(),
        "wrong login result"
    );
    let requests = fixture.requests.lock().await;
    let login = requests
        .iter()
        .find(|request| request.path == "/oauth/userLogin")
        .context("missing login")?;
    ensure!(
        login.body["params"] == "server-opaque-params" && login.body["tenantId"] == "上海大学",
        "wrong opaque params"
    );
    let encrypted = STANDARD.decode(
        login.body["password"]
            .as_str()
            .context("missing encrypted password")?,
    )?;
    ensure!(
        fixture.private.decrypt(Pkcs1v15Encrypt, &encrypted)? == "密码-fixture".as_bytes(),
        "wrong plaintext"
    );
    let cookies = login.headers.get("cookie").context("missing cookies")?;
    ensure!(
        cookies.contains("discovery=ready") && !cookies.contains("entry_only"),
        "host cookies leaked"
    );
    ensure!(
        login
            .headers
            .get("origin")
            .is_some_and(|value| value.contains("sso.fixture.test")),
        "wrong origin"
    );
    ensure!(
        fixture.client.snapshot()?.has_session,
        "session cookie not adopted"
    );
    fixture.client.clear()?;
    ensure!(
        !fixture.client.snapshot()?.has_session,
        "clear retained cookie"
    );
    Ok(())
}

#[tokio::test]
async fn two_step_retries_keep_challenge_context_and_updated_cookies_until_success(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Challenge).await?;
    let challenge = fixture
        .client
        .login("student".to_owned(), "password".to_owned())
        .await?;
    ensure!(
        challenge.methods.len() == 2 && challenge.callback_uri.is_none(),
        "wrong challenge"
    );
    let before = fixture.requests.lock().await.len();
    ensure!(
        fixture
            .client
            .send_code("unknown".to_owned())
            .await
            .is_err(),
        "unknown method accepted"
    );
    ensure!(
        fixture.requests.lock().await.len() == before,
        "unknown method made a request"
    );
    fixture.client.send_code("sms".to_owned()).await?;
    let failed = fixture
        .client
        .verify_code("sms".to_owned(), "bad".to_owned())
        .await
        .err()
        .context("wrong code accepted")?;
    ensure!(
        failed.code == "invalidCode" && fixture.client.snapshot()?.challenge_methods.len() == 2,
        "challenge lost on retry"
    );
    let result = fixture
        .client
        .verify_code("sms".to_owned(), "good".to_owned())
        .await?;
    ensure!(
        result.callback_uri.is_some() && fixture.client.snapshot()?.challenge_methods.is_empty(),
        "challenge not consumed"
    );
    ensure!(fixture.client.snapshot()?.has_session, "session missing");
    let requests = fixture.requests.lock().await;
    let login = requests
        .iter()
        .find(|request| request.path == "/oauth/userLogin")
        .context("missing login")?;
    let verifies: Vec<_> = requests
        .iter()
        .filter(|request| request.path == "/oauth/twoStep/verify")
        .collect();
    for request in &verifies {
        ensure!(
            request.body["password"] == login.body["password"]
                && request.body["params"] == login.body["params"],
            "challenge context changed"
        );
    }
    ensure!(
        verifies[1]
            .headers
            .get("cookie")
            .is_some_and(|value| value.contains("attempt=renewed")),
        "retry cookie rotation lost"
    );
    ensure!(
        fixture
            .client
            .verify_code("sms".to_owned(), "good".to_owned())
            .await
            .err()
            .is_some_and(|error| error.code == "challengeExpired"),
        "completed challenge reused"
    );
    Ok(())
}

#[tokio::test]
async fn unsafe_redirects_loops_and_oversize_bodies_do_not_publish_credentials(
) -> anyhow::Result<()> {
    for (scenario, code) in [
        (Scenario::UnsafeDiscovery, "unsafeRedirect"),
        (Scenario::UnsafeCallback, "unsafeRedirect"),
        (Scenario::Loop, "tooManyRedirects"),
        (Scenario::Oversized, "invalidResponse"),
    ] {
        let fixture = Fixture::new(scenario).await?;
        let failure = fixture
            .client
            .login("student".to_owned(), "password".to_owned())
            .await
            .err()
            .context("invalid flow accepted")?;
        ensure!(
            failure.code == code,
            "{scenario:?}: expected {code}, got {}: {}",
            failure.code,
            failure.message
        );
        ensure!(
            !fixture.client.snapshot()?.has_session,
            "failed flow published credentials"
        );
    }
    Ok(())
}

#[tokio::test]
async fn clear_cancels_inflight_response_and_new_login_can_use_the_same_handle(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::SlowFirst).await?;
    let client = fixture.client.clone();
    let pending =
        tokio::spawn(async move { client.login("old".to_owned(), "password".to_owned()).await });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    fixture.client.clear()?;
    let result = tokio::time::timeout(Duration::from_secs(2), pending).await??;
    ensure!(
        result.err().is_some_and(|error| error.code == "cancelled"),
        "clear did not cancel"
    );
    fixture.release.notify_one();
    ensure!(
        !fixture.client.snapshot()?.has_session,
        "late response restored session"
    );
    fixture
        .client
        .login("new".to_owned(), "password".to_owned())
        .await?;
    ensure!(
        fixture.client.snapshot()?.has_session,
        "new generation cannot login"
    );
    Ok(())
}

#[test]
fn legacy_cookie_import_is_host_only_once_per_handle_and_cannot_resurrect_after_clear(
) -> anyhow::Result<()> {
    let client = Client::new()?;
    ensure!(client.import_legacy(r#"[{"name":"SHU_OAUTH2","value":"legacy","path":"/"},{"name":"other","value":"ignored"},{"name":"SHU_OAUTH2","value":"expired","expires":"2000-01-01T00:00:00Z"}]"#)? == 1, "wrong import count");
    ensure!(
        client.snapshot()?.has_session && client.snapshot()?.legacy_imported,
        "legacy import missing"
    );
    client.clear()?;
    ensure!(
        !client.snapshot()?.has_session && client.snapshot()?.legacy_imported,
        "clear reset migration gate"
    );
    ensure!(
        client
            .import_legacy("[]")
            .err()
            .is_some_and(|error| error.code == "alreadyImported"),
        "old session reimported after logout"
    );
    ensure!(
        Client::new()?.import_legacy("{}").is_err(),
        "bad storage shape accepted"
    );
    Ok(())
}

#[test]
fn first_import_cannot_commit_across_clear_and_delayed_import_after_logout_is_closed(
) -> anyhow::Result<()> {
    let client = Arc::new(Client::new()?);
    let prepared = Arc::new(std::sync::Barrier::new(2));
    let resume = Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        let client_import = client.clone();
        let prepared_import = prepared.clone();
        let resume_import = resume.clone();
        let import = scope.spawn(move || -> Result<usize, AuthFailure> {
            let _operation = client_import.operation.try_lock().map_err(|_| invalid())?;
            let generation = client_import.state()?.generation;
            let jar = client_import.prepare_legacy(r#"[{"name":"SHU_OAUTH2","value":"old"}]"#)?;
            prepared_import.wait();
            resume_import.wait();
            client_import.commit_legacy(generation, jar)
        });
        prepared.wait();
        client.clear()?;
        resume.wait();
        let result = import
            .join()
            .map_err(|_| anyhow::anyhow!("import thread panicked"))?;
        ensure!(
            result.err().is_some_and(|error| error.code == "cancelled"),
            "old import crossed clear"
        );
        Ok::<_, anyhow::Error>(())
    })?;
    ensure!(
        !client.snapshot()?.has_session && !client.snapshot()?.legacy_imported,
        "clear revived import"
    );
    ensure!(
        client
            .import_legacy(r#"[{"name":"SHU_OAUTH2","value":"old"}]"#)
            .err()
            .is_some_and(|error| error.code == "importClosed"),
        "late import revived logged-out session"
    );
    Ok(())
}

#[tokio::test]
async fn new_login_supersedes_inflight_login_and_import_cannot_override_active_operation(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::SlowFirst).await?;
    let client = fixture.client.clone();
    let previous =
        tokio::spawn(async move { client.login("old".to_owned(), "password".to_owned()).await });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    ensure!(
        fixture
            .client
            .import_legacy("[]")
            .err()
            .is_some_and(|error| error.code == "ssoState"),
        "import replaced active login"
    );
    fixture
        .client
        .login("new".to_owned(), "password".to_owned())
        .await?;
    ensure!(
        tokio::time::timeout(Duration::from_secs(2), previous)
            .await??
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "older login completed"
    );
    fixture.release.notify_one();
    let state = fixture.client.state()?;
    let header = state
        .jar
        .header(&fixture.client.config.sso)?
        .context("missing session")?;
    ensure!(
        header.to_str()?.contains("SHU_OAUTH2=fixture-new")
            && !header.to_str()?.contains("fixture-old"),
        "old identity replaced new login"
    );
    Ok(())
}

#[tokio::test]
async fn untrusted_tls_and_oversized_password_never_send_http_credentials() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Direct).await?;
    let address = fixture
        .client
        .config
        .fixture
        .as_ref()
        .context("missing fixture")?
        .1;
    let untrusted = Client::configured(transport::Config {
        sso: fixture.client.config.sso.clone(),
        entry: fixture.client.config.entry.clone(),
        key: fixture.private.to_public_key(),
        fixture: Some((None, address)),
    })?;
    ensure!(
        untrusted
            .login("student".to_owned(), "password".to_owned())
            .await
            .err()
            .is_some_and(|error| error.code == "network"),
        "untrusted TLS accepted"
    );
    ensure!(
        fixture.requests.lock().await.is_empty(),
        "TLS rejection leaked HTTP"
    );
    ensure!(
        fixture
            .client
            .login("student".to_owned(), "x".repeat(118))
            .await
            .err()
            .is_some_and(|error| error.code == "passwordEncryption"),
        "oversized password accepted"
    );
    ensure!(
        fixture.requests.lock().await.is_empty(),
        "oversized password made HTTP requests"
    );
    Ok(())
}

#[tokio::test]
async fn two_step_method_without_a_masked_target_remains_usable() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::EmptyTargetChallenge).await?;
    let result = fixture
        .client
        .login("student".to_owned(), "password".to_owned())
        .await?;
    ensure!(
        result.methods.len() == 1
            && result.methods[0].method == "sms"
            && result.methods[0].target.is_empty(),
        "empty target removed valid method"
    );
    fixture.client.send_code("sms".to_owned()).await?;
    Ok(())
}

#[test]
fn persistent_import_close_reopen_and_logout_prevent_old_preference_resurrection(
) -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let old = r#"[{"name":"SHU_OAUTH2","value":"legacy","path":"/"}]"#;
    let client = Client::open(directory.clone())?;
    ensure!(
        client.snapshot()?.persistent && client.snapshot()?.legacy_import_allowed,
        "fresh store not importable"
    );
    ensure!(
        Client::open(directory.clone()).is_err(),
        "duplicate owner accepted"
    );
    client.import_legacy(old)?;
    client.close()?;
    ensure!(
        client
            .snapshot()
            .err()
            .is_some_and(|error| error.code == "closed"),
        "closed handle stayed live"
    );
    let reopened = Client::open(directory.clone())?;
    ensure!(
        reopened.snapshot()?.has_session && !reopened.snapshot()?.legacy_import_allowed,
        "committed import lost"
    );
    ensure!(
        reopened
            .import_legacy(old)
            .err()
            .is_some_and(|error| error.code == "alreadyImported"),
        "import repeated after restart"
    );
    reopened.clear()?;
    reopened.close()?;
    let logged_out = Client::open(directory)?;
    ensure!(
        !logged_out.snapshot()?.has_session && !logged_out.snapshot()?.legacy_import_allowed,
        "logout tombstone lost"
    );
    ensure!(
        logged_out.import_legacy(old).is_err(),
        "old preference resurrected logout"
    );
    Ok(())
}

#[test]
fn fresh_logout_and_failed_import_keep_durable_migration_contracts() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let client = Client::open(directory.clone())?;
    {
        let state = client.state()?;
        state
            .store
            .as_ref()
            .context("missing store")?
            .fail_writes
            .store(true, std::sync::atomic::Ordering::Release);
    }
    ensure!(
        client
            .import_legacy(r#"[{"name":"SHU_OAUTH2","value":"old"}]"#)
            .err()
            .is_some_and(|error| error.code == "ssoStorage"),
        "failed import reported success"
    );
    ensure!(
        !client.snapshot()?.has_session
            && !client.snapshot()?.legacy_imported
            && client.snapshot()?.legacy_import_allowed,
        "failed import published migration"
    );
    {
        let state = client.state()?;
        state
            .store
            .as_ref()
            .context("missing store")?
            .fail_writes
            .store(false, std::sync::atomic::Ordering::Release);
    }
    client.clear()?;
    client.close()?;
    let reopened = Client::open(directory)?;
    ensure!(
        !reopened.snapshot()?.legacy_imported && !reopened.snapshot()?.legacy_import_allowed,
        "fresh logout reopened import"
    );
    ensure!(
        reopened
            .import_legacy("[]")
            .err()
            .is_some_and(|error| error.code == "importClosed"),
        "fresh logout lost tombstone"
    );
    Ok(())
}

fn fixture_config(fixture: &Fixture) -> transport::Config {
    transport::Config {
        sso: fixture.client.config.sso.clone(),
        entry: fixture.client.config.entry.clone(),
        key: fixture.private.to_public_key(),
        fixture: fixture.client.config.fixture.clone(),
    }
}

#[tokio::test]
async fn successful_https_login_restores_cookie_but_incomplete_challenge_secrets_never_persist(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Direct).await?;
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let client = Client::open_configured(fixture_config(&fixture), directory.clone())?;
    client
        .login("student".to_owned(), "password".to_owned())
        .await?;
    client.close()?;
    let restored = Client::open_configured(fixture_config(&fixture), directory)?;
    ensure!(
        restored.snapshot()?.has_session && restored.snapshot()?.challenge_methods.is_empty(),
        "HTTPS credential not restored"
    );
    restored.close()?;

    let challenge_fixture = Fixture::new(Scenario::Challenge).await?;
    let challenge_root = tempfile::tempdir()?;
    let directory = challenge_root.path().to_string_lossy().into_owned();
    let pending = Client::open_configured(fixture_config(&challenge_fixture), directory.clone())?;
    pending
        .login("student".to_owned(), "password".to_owned())
        .await?;
    ensure!(
        pending.snapshot()?.challenge_methods.len() == 2,
        "challenge missing"
    );
    let state = pending.state()?;
    let saved = state
        .store
        .as_ref()
        .context("missing store")?
        .load()?
        .context("missing state")?;
    ensure!(saved.cookies.is_empty(), "challenge credentials persisted");
    drop(state);
    pending.close()?;
    let reopened = Client::open_configured(fixture_config(&challenge_fixture), directory)?;
    ensure!(
        reopened.snapshot()?.challenge_methods.is_empty() && !reopened.snapshot()?.has_session,
        "challenge restored across restart"
    );
    Ok(())
}

#[tokio::test]
async fn persistent_logout_cancels_late_response_before_reopening() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::SlowFirst).await?;
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let client = Arc::new(Client::open_configured(
        fixture_config(&fixture),
        directory.clone(),
    )?);
    let worker = client.clone();
    let login =
        tokio::spawn(async move { worker.login("old".to_owned(), "password".to_owned()).await });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    client.clear()?;
    ensure!(
        tokio::time::timeout(Duration::from_secs(2), login)
            .await??
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late login not cancelled"
    );
    fixture.release.notify_one();
    client.close()?;
    let reopened = Client::open_configured(fixture_config(&fixture), directory)?;
    ensure!(
        !reopened.snapshot()?.has_session && !reopened.snapshot()?.legacy_import_allowed,
        "late response revived persisted logout"
    );
    Ok(())
}

#[test]
fn failed_logout_is_reported_and_retry_is_required_before_restart_can_claim_logout(
) -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let client = Client::open(directory.clone())?;
    client.import_legacy(r#"[{"name":"SHU_OAUTH2","value":"committed"}]"#)?;
    client
        .state()?
        .store
        .as_ref()
        .context("missing store")?
        .fail_writes
        .store(true, std::sync::atomic::Ordering::Release);
    ensure!(
        client
            .clear()
            .err()
            .is_some_and(|error| error.code == "ssoStorage"),
        "failed logout reported success"
    );
    ensure!(
        !client.snapshot()?.has_session,
        "failed logout kept live memory credential"
    );
    client.close()?;
    let reopened = Client::open(directory.clone())?;
    ensure!(
        reopened.snapshot()?.has_session,
        "pre-replacement failure did not preserve known disk record"
    );
    reopened.clear()?;
    reopened.close()?;
    ensure!(
        !Client::open(directory)?.snapshot()?.has_session,
        "successful retry did not persist logout"
    );
    Ok(())
}

#[test]
fn exhausted_generation_does_not_commit_import_or_cancel_previous_state() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let client = Client::open(root.path().to_string_lossy().into_owned())?;
    client.state()?.generation = u64::MAX;
    ensure!(
        client
            .import_legacy(r#"[{"name":"SHU_OAUTH2","value":"old"}]"#)
            .err()
            .is_some_and(|error| error.code == "ssoState"),
        "overflow committed"
    );
    ensure!(
        !client.snapshot()?.legacy_imported && client.snapshot()?.legacy_import_allowed,
        "overflow changed import state"
    );
    ensure!(
        client
            .clear()
            .err()
            .is_some_and(|error| error.code == "ssoState"),
        "overflow clear accepted"
    );
    ensure!(
        !client.state()?.cancelled.is_cancelled(),
        "overflow cancelled previous state"
    );
    ensure!(
        client
            .state()?
            .store
            .as_ref()
            .context("missing store")?
            .load()?
            .is_none(),
        "overflow wrote credentials"
    );
    Ok(())
}

#[tokio::test]
async fn persistent_mfa_write_failure_keeps_challenge_and_retry_survives_reopen(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Challenge).await?;
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let client = Client::open_configured(fixture_config(&fixture), directory.clone())?;
    client
        .login("student".to_owned(), "password".to_owned())
        .await?;
    client
        .state()?
        .store
        .as_ref()
        .context("missing store")?
        .fail_writes
        .store(true, std::sync::atomic::Ordering::Release);
    ensure!(
        client
            .verify_code("sms".to_owned(), "good".to_owned())
            .await
            .err()
            .is_some_and(|error| error.code == "ssoStorage"),
        "failed MFA persistence reported success"
    );
    ensure!(
        !client.snapshot()?.has_session && client.snapshot()?.challenge_methods.len() == 2,
        "failed MFA published staged session"
    );
    client
        .state()?
        .store
        .as_ref()
        .context("missing store")?
        .fail_writes
        .store(false, std::sync::atomic::Ordering::Release);
    client
        .verify_code("sms".to_owned(), "good".to_owned())
        .await?;
    client.close()?;
    let restored = Client::open_configured(fixture_config(&fixture), directory)?;
    ensure!(
        restored.snapshot()?.has_session && restored.snapshot()?.challenge_methods.is_empty(),
        "successful MFA retry not restored"
    );
    Ok(())
}
