use super::*;
use crate::auth::newsso::{password, transport};
use anyhow::{ensure, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex, Notify},
};

#[derive(Clone, Copy)]
pub(in crate::auth::newsso) enum Scenario {
    Success,
    MissingCookie,
    UnsafeCallback,
    SlowPoll,
    SlowRedeem,
    Expired,
    MalformedPage,
    BadImage,
    SlowImage,
    SlowCredentials,
    FailedCredentials,
}

struct Request {
    path: String,
    query: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
}
pub(in crate::auth::newsso) struct Fixture {
    pub(in crate::auth::newsso) client: Arc<Client>,
    requests: Arc<Mutex<Vec<Request>>>,
    pub(in crate::auth::newsso) entered: Arc<Notify>,
    pub(in crate::auth::newsso) release: Arc<Notify>,
    job: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.job.abort();
    }
}

impl Fixture {
    pub(in crate::auth::newsso) async fn count(&self, path: &str) -> usize {
        self.requests
            .lock()
            .await
            .iter()
            .filter(|request| request.path == path)
            .count()
    }
    pub(in crate::auth::newsso) async fn new(
        scenario: Scenario,
        directory: Option<String>,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let certificate = rcgen::generate_simple_self_signed(vec![
            "sso.fixture.test".to_owned(),
            "entry.fixture.test".to_owned(),
            "wecom.fixture.test".to_owned(),
            "jwxt.fixture.test".to_owned(),
            "otp.fixture.test".to_owned(),
            "atrust.fixture.test".to_owned(),
        ])?;
        let tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der())
                    .into(),
            )?;
        let tls = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
        let config = transport::Config {
            sso: Url::parse(&format!("https://sso.fixture.test:{}/", address.port()))?,
            entry: Url::parse(&format!("https://entry.fixture.test:{}/", address.port()))?,
            key: password::public_key()?,
            fixture: Some((Some(certificate.cert.der().to_vec()), address)),
        };
        let client = Arc::new(match directory {
            Some(directory) => Client::open_configured(config, directory)?,
            None => Client::configured(config)?,
        });
        let sso = client.config.sso.clone();
        let requests = Arc::new(Mutex::new(vec![]));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let seen = requests.clone();
        let observed = entered.clone();
        let resume = release.clone();
        let polls = Arc::new(AtomicUsize::new(0));
        let job = tokio::spawn(async move {
            let mut children = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = children.join_next(), if !children.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break; };
                        let tls = tls.clone();
                        let seen = seen.clone();
                        let observed = observed.clone();
                        let resume = resume.clone();
                        let polls = polls.clone();
                        let sso = sso.clone();
                        children.spawn(async move {
                            let mut stream = tls.accept(stream).await?;
                            let mut header = vec![];
                            while !header.ends_with(b"\r\n\r\n") {
                                ensure!(header.len() < 32 * 1024, "fixture header exceeded limit");
                                header.push(stream.read_u8().await?);
                            }
                            let text = String::from_utf8(header)?;
                            let mut lines = text.split("\r\n");
                            let first = lines.next().context("missing request")?;
                            let target = first.split_whitespace().nth(1).context("missing target")?;
                            let uri = Url::parse(&format!("https://fixture.test{target}"))?;
                            let query = uri.query_pairs().map(|(name, value)| (name.into_owned(), value.into_owned())).collect();
                            let headers = lines.filter_map(|line| line.split_once(':')).map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned())).collect();
                            let path = uri.path().to_owned();
                            seen.lock().await.push(Request { path: path.clone(), query, headers });
                            let mut status = "200 OK";
                            let mut extra = String::new();
                            let body: Vec<u8> = match path.as_str() {
                                "/wwopen/sso/qrConnect" => {
                                    extra = "Set-Cookie: provider=private; Path=/; Secure; HttpOnly\r\nSet-Cookie: invalid=comma,value; Path=/\r\n".to_owned();
                                    if matches!(scenario, Scenario::MalformedPage) { b"<html>missing key</html>".to_vec() }
                                    else { b"<img src='qrImg?key=0123456789abcdef'>".to_vec() }
                                },
                                "/wwopen/sso/qrImg" => {
                                    if matches!(scenario, Scenario::SlowImage) { observed.notify_one(); resume.notified().await; }
                                    if matches!(scenario, Scenario::BadImage) { b"<html>not image</html>".to_vec() }
                                    else { STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/lS8AAAAASUVORK5CYII=")? }
                                },
                                "/wwopen/sso/l/qrConnect" => {
                                    if matches!(scenario, Scenario::SlowPoll) { observed.notify_one(); resume.notified().await; }
                                    let index = polls.fetch_add(1, Ordering::AcqRel);
                                    if matches!(scenario, Scenario::Expired) { b"jsonpCallback({\"status\":\"QRCODE_SCAN_OVERDUE\"});".to_vec() }
                                    else if index == 0 && !matches!(scenario, Scenario::SlowPoll) { b"jsonpCallback({\"status\":\"QRCODE_SCAN_ING\"});".to_vec() }
                                    else { b"jsonpCallback({\"status\":\"QRCODE_SCAN_SUCC\",\"auth_code\":\"one-time-fixture-code\"});".to_vec() }
                                },
                                "/oauth/wecom/qrcode" => {
                                    if matches!(scenario, Scenario::SlowRedeem) { observed.notify_one(); resume.notified().await; }
                                    status = "302 Found";
                                    let location = if matches!(scenario, Scenario::UnsafeCallback) { "https://attacker.example.net/callback".to_owned() }
                                        else { sso.join("/oauth/authorize?code=fixture")?.to_string() };
                                    extra = format!("Location: {location}\r\n");
                                    if !matches!(scenario, Scenario::MissingCookie) { extra.push_str("Set-Cookie: SHU_OAUTH2=qr-fixture-session; Path=/; Secure; HttpOnly\r\n"); }
                                    vec![]
                                },
                                "/" => {
                                    extra = "Set-Cookie: ASP.NET_SessionId=otp-session; Path=/; Secure\r\n".into();
                                    b"<input type='hidden' name='state' value='fixture-state'>".to_vec()
                                },
                                "/oauth/authorize" => {
                                    let parameters: BTreeMap<_, _> = uri.query_pairs().collect();
                                    let callback = parameters.get("redirect_uri").context("missing callback")?;
                                    let mut callback = Url::parse(callback)?;
                                    if callback.host_str() == Some("otp.fixture.test") {
                                        callback.query_pairs_mut().append_pair("code", "otp-once");
                                        if let Some(state) = parameters.get("state") {
                                            callback.query_pairs_mut().append_pair("state", state);
                                        }
                                        status = "302 Found";
                                        extra = format!("Location: {callback}\r\n");
                                    } else { status = "503 Unavailable"; }
                                    vec![]
                                },
                                "//Callback.aspx" => {
                                    extra = "Refresh: 0;url=Default.aspx\r\n".into();
                                    b"<html>refresh</html>".to_vec()
                                },
                                "/Default.aspx" => {
                                    if matches!(scenario, Scenario::SlowCredentials) { observed.notify_one(); resume.notified().await; }
                                    if matches!(scenario, Scenario::FailedCredentials) { b"no usable code".to_vec() }
                                    else {
                                        b"<span id='DataList1_LabelNum_0'>012345</span><span id='DataList1_LabelAccount_0'>fixture-account</span>remainingSeconds=9; totalSeconds=60;".to_vec()
                                    }
                                },
                                "/passport/v1/public/authConfig" => { status = "503 Unavailable"; vec![] },
                                _ => anyhow::bail!("unexpected QR fixture path: {path}"),
                            };
                            stream.write_all(format!("HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await?;
                            stream.write_all(&body).await?;
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
            job,
        })
    }
}

#[test]
fn state_and_jsonp_parser_preserve_registered_params_without_executing_script() -> anyhow::Result<()>
{
    let encoded = protocol::state();
    ensure!(!encoded.contains('='), "state contains padding");
    let decoded: serde_json::Value =
        serde_json::from_slice(&base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded)?)?;
    ensure!(
        decoded["clientId"] == "Km5t225E8KECKQ6ZDm5K2P6aS2459Cua"
            && decoded["clientName"] == "本科生教务系统"
            && decoded["redirectUri"] == "https://jwxt.shu.edu.cn/sso/shulogin",
        "wrong registration"
    );
    ensure!(
        protocol::scan(b"jsonpCallback({\"status\":\"QRCODE_SCAN_SUCC\"});").0
            == QrStatus::ConfirmedPending,
        "empty code accepted"
    );
    ensure!(
        protocol::scan(b"evil({\"status\":\"QRCODE_SCAN_SUCC\",\"auth_code\":\"code\"});").0
            == QrStatus::Waiting,
        "wrong callback parsed"
    );
    ensure!(
        protocol::scan(b"jsonpCallback({\"status\":\"QRCODE_SCAN_CANCEL\"});").0
            == QrStatus::Expired,
        "cancel not expired"
    );
    Ok(())
}

#[tokio::test]
async fn actual_https_qr_image_poll_redeem_share_cookies_and_persist_only_sso() -> anyhow::Result<()>
{
    let root = tempfile::tempdir()?;
    let fixture = Fixture::new(
        Scenario::Success,
        Some(root.path().to_string_lossy().into_owned()),
    )
    .await?;
    let qr = fixture.client.start_qr().await?;
    ensure!(
        qr.image_bytes.starts_with(b"\x89PNG")
            && qr.confirm_url.contains("k=0123456789abcdef")
            && qr.wx_work_scheme_url.starts_with("wxwork://"),
        "wrong UI data"
    );
    ensure!(
        fixture.client.poll_qr().await? == QrStatus::ConfirmedPending,
        "wrong pending status"
    );
    ensure!(
        fixture.client.poll_qr().await? == QrStatus::Succeeded,
        "confirmation missing"
    );
    let result = fixture.client.redeem_qr().await?;
    ensure!(
        result.callback_uri.is_some()
            && fixture.client.snapshot()?.has_session
            && fixture.client.snapshot()?.qr_status.is_none(),
        "QR session not adopted"
    );
    ensure!(fixture.client.redeem_qr().await.is_err(), "code reused");
    let requests = fixture.requests.lock().await;
    let create = requests
        .iter()
        .find(|request| request.path == "/wwopen/sso/qrConnect")
        .context("missing create")?;
    ensure!(
        create.query["appid"] == protocol::APP_ID
            && create.query["agentid"] == protocol::AGENT_ID
            && create.query["state"] == protocol::state(),
        "wrong create parameters"
    );
    let poll = requests
        .iter()
        .find(|request| request.path == "/wwopen/sso/l/qrConnect")
        .context("missing poll")?;
    ensure!(
        poll.headers
            .get("cookie")
            .is_some_and(|value| value.contains("provider=private") && !value.contains("invalid=")),
        "provider cookie lost"
    );
    ensure!(
        poll.headers
            .get("x-requested-with")
            .is_some_and(|value| value == "XMLHttpRequest"),
        "AJAX header missing"
    );
    let redeem = requests
        .iter()
        .find(|request| request.path == "/oauth/wecom/qrcode")
        .context("missing redeem")?;
    ensure!(
        redeem.query["code"] == "one-time-fixture-code"
            && redeem.query["state"] == protocol::state()
            && redeem.query["appid"] == protocol::APP_ID,
        "wrong redeem parameters"
    );
    ensure!(
        !redeem
            .headers
            .get("cookie")
            .is_some_and(|value| value.contains("provider=")),
        "provider cookie leaked to SSO"
    );
    drop(requests);
    let state = fixture.client.state()?;
    let saved = state
        .store
        .as_ref()
        .context("missing store")?
        .load()?
        .context("missing state")?;
    ensure!(
        saved.cookies.len() == 1 && saved.cookies[0].starts_with("SHU_OAUTH2="),
        "provider secrets persisted"
    );
    drop(state);
    let config = super::super::transport::Config {
        sso: fixture.client.config.sso.clone(),
        entry: fixture.client.config.entry.clone(),
        key: password::public_key()?,
        fixture: fixture.client.config.fixture.clone(),
    };
    fixture.client.close()?;
    ensure!(
        Client::open_configured(config, root.path().to_string_lossy().into_owned())?
            .snapshot()?
            .has_session,
        "QR credential did not restore"
    );
    Ok(())
}

#[tokio::test]
async fn redeem_needs_real_sso_cookie_safe_callback_and_single_use_code() -> anyhow::Result<()> {
    for (scenario, code) in [
        (Scenario::MissingCookie, "redeemFailed"),
        (Scenario::UnsafeCallback, "unsafeRedirect"),
    ] {
        let fixture = Fixture::new(scenario, None).await?;
        fixture.client.start_qr().await?;
        fixture.client.poll_qr().await?;
        fixture.client.poll_qr().await?;
        ensure!(
            fixture
                .client
                .redeem_qr()
                .await
                .err()
                .is_some_and(|error| error.code == code),
            "unsafe redeem accepted"
        );
        ensure!(
            !fixture.client.snapshot()?.has_session,
            "failed redeem published cookie"
        );
        let before = fixture.requests.lock().await.len();
        ensure!(
            fixture.client.redeem_qr().await.is_err(),
            "failed code reused"
        );
        ensure!(
            fixture.requests.lock().await.len() == before,
            "code replay made HTTP request"
        );
    }
    Ok(())
}

#[tokio::test]
async fn clear_during_long_poll_rejects_late_confirmation_and_wait_deadline_expires(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::SlowPoll, None).await?;
    fixture.client.start_qr().await?;
    let client = fixture.client.clone();
    let pending = tokio::spawn(async move { client.wait_qr(180).await });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    fixture.client.clear()?;
    ensure!(
        tokio::time::timeout(Duration::from_secs(2), pending)
            .await??
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late confirmation accepted"
    );
    fixture.release.notify_one();
    ensure!(
        fixture.client.snapshot()?.qr_status.is_none() && !fixture.client.snapshot()?.has_session,
        "cancel left QR state"
    );

    let timed = Fixture::new(Scenario::SlowPoll, None).await?;
    timed.client.start_qr().await?;
    ensure!(
        timed.client.wait_qr(1).await? == QrStatus::Expired
            && timed.client.snapshot()?.qr_status == Some(QrStatus::Expired),
        "wait deadline not enforced"
    );
    timed.release.notify_one();
    Ok(())
}

#[tokio::test]
async fn logout_during_redeem_does_not_publish_late_cookie() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let fixture = Fixture::new(
        Scenario::SlowRedeem,
        Some(root.path().to_string_lossy().into_owned()),
    )
    .await?;
    fixture.client.start_qr().await?;
    fixture.client.poll_qr().await?;
    fixture.client.poll_qr().await?;
    let client = fixture.client.clone();
    let pending = tokio::spawn(async move { client.redeem_qr().await });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    fixture.client.clear()?;
    ensure!(
        tokio::time::timeout(Duration::from_secs(2), pending)
            .await??
            .is_err(),
        "late cookie accepted"
    );
    fixture.release.notify_one();
    ensure!(
        !fixture.client.snapshot()?.has_session,
        "late redeem restored logout"
    );
    let state = fixture.client.state()?;
    ensure!(
        state
            .store
            .as_ref()
            .context("missing store")?
            .load()?
            .context("missing state")?
            .cookies
            .is_empty(),
        "late cookie persisted"
    );
    Ok(())
}

#[tokio::test]
async fn invalid_page_image_and_expired_scan_remain_non_authenticated() -> anyhow::Result<()> {
    for scenario in [Scenario::MalformedPage, Scenario::BadImage] {
        let fixture = Fixture::new(scenario, None).await?;
        ensure!(
            fixture.client.start_qr().await.is_err()
                && fixture.client.snapshot()?.qr_status.is_none(),
            "bad QR creation published"
        );
    }
    let fixture = Fixture::new(Scenario::Expired, None).await?;
    fixture.client.start_qr().await?;
    ensure!(
        fixture.client.poll_qr().await? == QrStatus::Expired
            && fixture.client.redeem_qr().await.is_err(),
        "expired QR accepted"
    );
    Ok(())
}

#[tokio::test]
async fn wait_timeout_invalidates_an_independent_poll_in_the_same_generation() -> anyhow::Result<()>
{
    let fixture = Fixture::new(Scenario::SlowPoll, None).await?;
    fixture.client.start_qr().await?;
    let client = fixture.client.clone();
    let poll = tokio::spawn(async move { client.poll_qr().await });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    ensure!(
        fixture.client.wait_qr(1).await? == QrStatus::Expired,
        "queued wait did not expire"
    );
    fixture.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(2), poll).await??;
    ensure!(
        result.err().is_some_and(|error| error.code == "cancelled"),
        "late independent poll republished auth code"
    );
    ensure!(
        fixture.client.snapshot()?.qr_status == Some(QrStatus::Expired),
        "expiration overwritten"
    );
    ensure!(
        fixture.client.redeem_qr().await.is_err(),
        "expired code redeemed"
    );
    Ok(())
}
