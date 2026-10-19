use super::*;
use crate::auth::newsso::{password, transport};
use anyhow::{ensure, Context};
use base64::Engine as _;
use serde_json::json;
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
    Normal,
    WrongState,
    WrongPath,
    LoginRedirect,
    ForeignHop,
    Loop,
    SlowCallback,
    HiddenState,
    EmptyState,
    BadBody,
    ParentCookie,
    DuplicateCode,
    OtpDirect,
    OtpRetry,
    OtpLost,
    OtpNoCode,
    OtpForeign,
    OtpSlow,
    JwxtProfileMissing,
    JwxtTermMissing,
    JwxtSlow,
    JwxtPostFail,
    ResourceFail,
    ResourceSlow,
    ResourceScopedSid,
    ResourceVerified,
    ResourceTampered,
    ResourceWarning,
    ControlSlow,
    AllSystemsFail,
}
struct Request {
    host: String,
    path: String,
    query: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
}
pub(in crate::auth::newsso) struct Fixture {
    pub(in crate::auth::newsso) client: Arc<Client>,
    requests: Arc<Mutex<Vec<Request>>>,
    pub(in crate::auth::newsso) entered: Arc<Notify>,
    pub(in crate::auth::newsso) release: Arc<Notify>,
    closed: Arc<Notify>,
    job: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.job.abort();
    }
}

impl Fixture {
    pub(in crate::auth::newsso) async fn new(scenario: Scenario) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let certificate = rcgen::generate_simple_self_signed(vec![
            "sso.fixture.test".into(),
            "entry.fixture.test".into(),
            "jwxt.fixture.test".into(),
            "otp.fixture.test".into(),
            "atrust.fixture.test".into(),
        ])?;
        let node_certificate =
            base64::engine::general_purpose::STANDARD.encode(certificate.cert.der());
        let tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der())
                    .into(),
            )?;
        let tls = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
        let client = Arc::new(Client::configured(transport::Config {
            sso: Url::parse(&format!("https://sso.fixture.test:{}/", address.port()))?,
            entry: Url::parse(&format!("https://entry.fixture.test:{}/", address.port()))?,
            key: password::public_key()?,
            fixture: Some((Some(certificate.cert.der().to_vec()), address)),
        })?);
        client.import_legacy(r#"[{"name":"SHU_OAUTH2","value":"account-fixture","path":"/"}]"#)?;
        let requests = Arc::new(Mutex::new(vec![]));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let seen = requests.clone();
        let observed = entered.clone();
        let resumed = release.clone();
        let closed = Arc::new(Notify::new());
        let disconnected = closed.clone();
        let warmed = Arc::new(AtomicUsize::new(0));
        let ticketed = Arc::new(AtomicUsize::new(0));
        let otp_reads = Arc::new(AtomicUsize::new(0));
        let job = tokio::spawn(async move {
            let mut peers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = peers.join_next(), if !peers.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break; };
                        let tls = tls.clone();
                        let seen = seen.clone();
                        let observed = observed.clone();
                        let resumed = resumed.clone();
                        let warmed = warmed.clone();
                        let ticketed = ticketed.clone();
                        let otp_reads = otp_reads.clone();
                        let disconnected = disconnected.clone();
                        let node_certificate = node_certificate.clone();
                        peers.spawn(async move {
                            let mut stream = tls.accept(stream).await?;
                            let first = stream.read_u8().await?;
                            if first == 5 {
                                let mut opening = [0; 6];
                                stream.read_exact(&mut opening).await?;
                                ensure!(opening[1] == 0xd0, "not an L3 control request");
                                let mut payload = vec![0; usize::from(u16::from_be_bytes([opening[4], opening[5]]))];
                                stream.read_exact(&mut payload).await?;
                                let auth: serde_json::Value = serde_json::from_slice(&payload)?;
                                let expected = if matches!(scenario, Scenario::ResourceScopedSid) { "resource-legacy" } else { "resource-sid" };
                                ensure!(auth["sid"] == expected, "prepared SID not used on actual transport");
                                let mut trailer = [0; 10];
                                stream.read_exact(&mut trailer).await?;
                                ensure!(trailer == [5,4,0,1,0,0,0,0,0,0], "bad VIP request");
                                let reply = br#"{"code":0,"data":{"deviceId":"owned-device"}}"#;
                                if matches!(scenario, Scenario::ControlSlow) {
                                    observed.notify_one();
                                    resumed.notified().await;
                                }
                                let response = [vec![5,0xd0,0x53,0,0,reply.len() as u8],reply.to_vec(),vec![5,0,0,1,10,99,0,2,0,0]].concat();
                                stream.write_all(&response).await?;
                                stream.flush().await?;
                                let mut heartbeat = [0; 4];
                                while stream.read_exact(&mut heartbeat).await.is_ok() {
                                    ensure!(heartbeat == [5,0x15,0,0], "unexpected control packet");
                                    stream.write_all(&[5,0x95,0,0]).await?;
                                    stream.flush().await?;
                                }
                                disconnected.notify_one();
                                return Ok::<_, anyhow::Error>(());
                            }
                            let mut header = vec![first];
                            while !header.ends_with(b"\r\n\r\n") {
                                ensure!(header.len() < 32 * 1024, "fixture headers exceeded limit");
                                header.push(stream.read_u8().await?);
                            }
                            let text = String::from_utf8(header)?;
                            let mut lines = text.split("\r\n");
                            let target = lines.next().context("missing request")?.split_whitespace().nth(1).context("missing target")?;
                            let uri = Url::parse(&format!("https://fixture.test{target}"))?;
                            let query: BTreeMap<_, _> = uri.query_pairs().map(|(name, value)| (name.to_string(), value.to_string())).collect();
                            let headers: BTreeMap<_, _> = lines.filter_map(|line| line.split_once(':')).map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned())).collect();
                            let host = headers.get("host").context("missing host")?.split(':').next().unwrap_or("").to_owned();
                            let path = uri.path().to_owned();
                            let length: usize = headers.get("content-length").map(|length| length.parse()).transpose()?.unwrap_or(0);
                            let mut payload = vec![0; length];
                            stream.read_exact(&mut payload).await?;
                            seen.lock().await.push(Request { host: host.clone(), path: path.clone(), query: query.clone(), headers: headers.clone() });
                            let mut status = "200 OK";
                            let mut extra = String::new();
                            let body = if matches!(scenario, Scenario::AllSystemsFail) {
                                status = "503 Unavailable";
                                "unavailable".to_owned()
                            } else { match (host.as_str(), path.as_str()) {
                                ("otp.fixture.test", "/") => {
                                    let index = warmed.fetch_add(1, Ordering::AcqRel);
                                    extra = "Set-Cookie: ASP.NET_SessionId=otp-session; Path=/; Secure\r\n".to_owned();
                                    if matches!(scenario, Scenario::HiddenState) { "<input type='hidden' name='state' value='body-state'>".to_owned() }
                                    else if matches!(scenario, Scenario::EmptyState | Scenario::OtpNoCode) || index > 0 { "<html>no state</html>".to_owned() }
                                    else {
                                        status = "302 Found";
                                        extra.push_str(&format!("Location: https://sso.fixture.test:{}/oauth/authorize?state=bootstrap-state\r\n", address.port()));
                                        String::new()
                                    }
                                },
                                ("sso.fixture.test", "/oauth/authorize") => {
                                    ensure!(headers.get("cookie").is_some_and(|header| header.contains("SHU_OAUTH2=account-fixture") || header.contains("SHU_OAUTH2=renewed")), "SSO cookie missing");
                                    status = "302 Found";
                                    let callback = query.get("redirect_uri").context("missing redirect")?;
                                    let mut callback = Url::parse(callback)?;
                                    callback.query_pairs_mut().append_pair("code", "one-use-code");
                                    if matches!(scenario, Scenario::DuplicateCode) { callback.query_pairs_mut().append_pair("code", "duplicate"); }
                                    if let Some(state) = query.get("state") {
                                        callback.query_pairs_mut().append_pair("state", if matches!(scenario, Scenario::WrongState) { "wrong" } else { state });
                                    }
                                    if matches!(scenario, Scenario::WrongPath) { callback.set_path("/wrong"); }
                                    let location = if matches!(scenario, Scenario::LoginRedirect) {
                                        format!("https://sso.fixture.test:{}/oauth2/login/missing", address.port())
                                    } else { callback.into() };
                                    extra = format!("Location: {location}\r\nSet-Cookie: SHU_OAUTH2=renewed; Path=/; Secure; HttpOnly\r\n");
                                    String::new()
                                },
                                ("jwxt.fixture.test", "/sso/shulogin") => {
                                    if matches!(scenario, Scenario::SlowCallback) { observed.notify_one(); resumed.notified().await; }
                                    status = "302 Found";
                                    let location = if matches!(scenario, Scenario::ForeignHop) { "https://attacker.example.net/steal" }
                                        else if matches!(scenario, Scenario::Loop) { "/sso/shulogin?code=loop" }
                                        else if matches!(scenario, Scenario::BadBody) { "/unconfirmed" }
                                        else { "/jwglxt/index" };
                                    extra = format!("Location: {location}\r\nSet-Cookie: JSESSIONID=business-session; Path=/; Secure; HttpOnly\r\n");
                                    String::new()
                                },
                                ("jwxt.fixture.test", "/jwglxt/index") => {
                                    ensure!(headers.get("cookie").is_some_and(|cookie| cookie.contains("JSESSIONID=business-session") && !cookie.contains("SHU_OAUTH2")), "intermediate cookie lost or SSO leaked");
                                    "教学管理".to_owned()
                                },
                                ("jwxt.fixture.test", "/unconfirmed") => "login required".to_owned(),
                                ("jwxt.fixture.test", "/jwglxt/xtgl/index_cxYhxxIndex.html") => {
                                    ensure!(!headers.contains_key("x-requested-with") && headers.get("cookie").is_some_and(|cookie| cookie.contains("JSESSIONID=business-session")), "profile headers/cookies wrong");
                                    if matches!(scenario, Scenario::JwxtProfileMissing) {
                                        status = "503 Unavailable";
                                        extra = "Set-Cookie: JSESSIONID=failed-profile; Path=/; Secure\r\n".to_owned();
                                        "missing".to_owned()
                                    } else {
                                        "<h4 class='media-heading'>档案姓名&nbsp;学生</h4><p>示例学院 2025级示例专业</p>".to_owned()
                                    }
                                },
                                ("jwxt.fixture.test", "/jwglxt/kbcx/xskbcx_cxXskbcxIndex.html") => {
                                    ensure!(!headers.contains_key("x-requested-with"), "index unexpectedly AJAX");
                                    ensure!(headers.get("cookie").is_some_and(|cookie| cookie.contains("JSESSIONID=business-session")), "failed profile cookie was adopted");
                                    extra = "Set-Cookie: schedule=warmed; Path=/; Secure\r\n".to_owned();
                                    if matches!(scenario, Scenario::JwxtTermMissing) { "missing terms".to_owned() }
                                    else { "<input name='xnm' value='2025'><select id='xqm'><option value='12' selected>current</option></select>".to_owned() }
                                },
                                ("jwxt.fixture.test", "/jwglxt/kbcx/xskbcx_cxXsgrkb.html") => {
                                    if matches!(scenario, Scenario::JwxtSlow) {
                                        observed.notify_one();
                                        resumed.notified().await;
                                    }
                                    ensure!(headers.get("cookie").is_some_and(|cookie| cookie.contains("schedule=warmed")), "warmup cookie missing");
                                    ensure!(headers.get("x-requested-with").is_some_and(|value| value == "XMLHttpRequest") && headers.get("content-type").is_some_and(|value| value.starts_with("application/x-www-form-urlencoded")), "schedule POST headers wrong");
                                    let form = Url::parse(&format!("https://fixture.test/?{}", String::from_utf8(payload)?))?;
                                    let fields: BTreeMap<_, _> = form.query_pairs().collect();
                                    ensure!(fields.get("xnm").is_some_and(|value| value == "2025") && fields.get("xqm").is_some_and(|value| value == "12") && fields.get("kzlx").is_some_and(|value| value == "ck"), "term codes lost");
                                    if matches!(scenario, Scenario::JwxtPostFail) {
                                        status = "503 Unavailable";
                                        extra = "Set-Cookie: identity=failed; Path=/; Secure\r\n".to_owned();
                                        "unavailable".to_owned()
                                    } else {
                                        json!({"xsxx":{"XM":"课表姓名","XH":"25123456","BJMC":"示例班级"}}).to_string()
                                    }
                                },
                                ("otp.fixture.test", "//Callback.aspx") => {
                                    ensure!(headers.get("cookie").is_some_and(|cookie| cookie.contains("ASP.NET_SessionId=otp-session") && !cookie.contains("SHU_OAUTH2")), "OTP bootstrap session lost");
                                    extra = "Refresh: 0;url=Default.aspx\r\n".to_owned();
                                    "<html>refresh</html>".to_owned()
                                },
                                ("otp.fixture.test", "/Default.aspx") => {
                                    let read = otp_reads.fetch_add(1, Ordering::AcqRel);
                                    ensure!(headers.get("cache-control").is_some_and(|value| value == "no-cache") || !query.contains_key("_"), "OTP cache headers missing");
                                    if matches!(scenario, Scenario::OtpSlow) {
                                        observed.notify_one();
                                        resumed.notified().await;
                                    }
                                    if matches!(scenario, Scenario::OtpRetry) && read < 2 {
                                        status = "503 Unavailable";
                                        extra = "Set-Cookie: failed=retry-cookie; Path=/; Secure\r\n".to_owned();
                                        "temporarily unavailable".to_owned()
                                    } else if matches!(scenario, Scenario::OtpForeign) {
                                        status = "302 Found";
                                        extra = "Location: https://attacker.example.net/steal\r\n".to_owned();
                                        String::new()
                                    } else if matches!(scenario, Scenario::OtpLost) && !headers.get("cookie").is_some_and(|header| header.contains("ASP.NET_SessionId")) {
                                        status = "302 Found";
                                        extra = format!("Location: https://sso.fixture.test:{}/oauth2/login/expired\r\n", address.port());
                                        String::new()
                                    } else if matches!(scenario, Scenario::OtpDirect | Scenario::OtpRetry | Scenario::OtpLost | Scenario::OtpSlow) {
                                        ensure!(!headers.get("cookie").is_some_and(|value| value.contains("failed=retry-cookie")), "failed retry cookie survived");
                                        extra = "Refresh: 11;url=/Default.aspx\r\nSet-Cookie: otp=rotated; Path=/; Secure\r\n".to_owned();
                                        "<span id='DataList1_LabelNum_0'><b>012345</b></span><span id='DataList1_LabelAccount_0'>fixture-account</span>remainingSeconds=9; totalSeconds=60;".to_owned()
                                    } else { "账户名 fixture".to_owned() }
                                },
                                ("atrust.fixture.test", "/passport/v1/public/authConfig") => {
                                    if matches!(scenario, Scenario::ResourceVerified | Scenario::ResourceTampered | Scenario::ResourceWarning)
                                        && ticketed.load(Ordering::Acquire) == 1
                                        && query.get("needTicket").is_some_and(|value| value == "1") {
                                        let mut signed: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/fixtures/anti_mitm.json"))?;
                                        if matches!(scenario, Scenario::ResourceTampered | Scenario::ResourceWarning) {
                                            signed["data"]["antiMITMAttackData"]["mitmSig"] = json!("BAD");
                                        }
                                        if matches!(scenario, Scenario::ResourceWarning) {
                                            signed["data"]["antiMITMAttackData"]["rsaCert"] = json!(node_certificate);
                                        }
                                        signed.to_string()
                                    } else {
                                    if matches!(scenario, Scenario::ParentCookie) && ticketed.load(Ordering::Acquire) == 0 {
                                        extra = "Set-Cookie: shared=rotated; Domain=fixture.test; Path=/; Secure; HttpOnly; SameSite=Strict\r\n".to_owned();
                                    }
                                    json!({"code":0,"data":{"isLogin": if ticketed.load(Ordering::Acquire) == 0 {0} else {1},"csrfToken":"csrf-fixture"}}).to_string()
                                    }
                                },
                                ("atrust.fixture.test", "/passport/v1/auth/httpsOauth2") => {
                                    ensure!(query.get("sfDomain").is_some_and(|domain| domain == "customOAuth76881") && query.get("code").is_some_and(|code| code == "one-use-code"), "wrong gateway ticket request");
                                    ticketed.store(1, Ordering::Release);
                                    status = "302 Found";
                                    let mut destination = Url::parse(&format!("https://atrust.fixture.test:{}/portal/shortcut.html", address.port()))?;
                                    destination.query_pairs_mut().append_pair("data", &json!({"ticket":"ticket-fixture"}).to_string());
                                    extra = format!("Location: {destination}\r\nSet-Cookie: sid=gateway-sid; Path=/; Secure; HttpOnly\r\n");
                                    if matches!(scenario, Scenario::ParentCookie) { extra.push_str("Set-Cookie: shared=; Domain=fixture.test; Path=/; Max-Age=0; Secure\r\n"); }
                                    String::new()
                                },
                                ("atrust.fixture.test", "/controller/v1/public/reportEnv") => {
                                    let body: serde_json::Value = serde_json::from_slice(&payload)?;
                                    ensure!(body["deviceId"] == "owned-device" && body["ticket"] == "ticket-fixture", "wrong device report");
                                    json!({"code":0,"data":{}}).to_string()
                                },
                                ("atrust.fixture.test", "/passport/v1/auth/authCheck") => json!({"code":0,"data":{}}).to_string(),
                                ("atrust.fixture.test", "/passport/v1/user/onlineInfo") => {
                                    ensure!(headers.get("cookie").is_some_and(|cookie| (cookie.contains("sid=gateway-sid") || cookie.contains("sid=resource-sid") || cookie.contains("sid-legacy=resource-legacy")) && !cookie.contains("SHU_OAUTH2")), "gateway cookie isolation failed");
                                    json!({"code":0,"data":{"username":"fixture-account"}}).to_string()
                                },
                                ("atrust.fixture.test", "/controller/v1/user/clientResource") => {
                                    ensure!(headers.get("cookie").is_some_and(|header| (header.contains("sid=gateway-sid") || header.contains("sid=resource-sid") || header.contains("sid-legacy=resource-legacy")) && !header.contains("SHU_OAUTH2")), "resource request not shared gateway session");
                                    ensure!(headers.get("x-csrf-token").is_some_and(|value| value == "csrf-fixture" || value == "fixture-csrf"), "resource CSRF missing");
                                    if matches!(scenario, Scenario::ResourceSlow) {
                                        observed.notify_one();
                                        resumed.notified().await;
                                    }
                                    extra = "Set-Cookie: sid=resource-sid; Path=/; Secure; HttpOnly\r\n".to_owned();
                                    if matches!(scenario, Scenario::ResourceScopedSid) {
                                        extra = "Set-Cookie: sid=; Path=/; Max-Age=0; Secure\r\nSet-Cookie: sid=unrelated-sid; Path=/unrelated; Secure\r\nSet-Cookie: sid-legacy=resource-legacy; Path=/; Secure; HttpOnly\r\n".to_owned();
                                    }
                                    if matches!(scenario, Scenario::ResourceFail) {
                                        json!({"code":0,"data":{}}).to_string()
                                    } else {
                                        json!({"code":0,"data":{
                                            "appList":{"data":{"appInfo":[{"apps":[{"id":"campus-app","nodeGroupId":"campus","accessModel":"L3VPN","addressList":[{"host":"fixture.shu.edu.cn","protocol":"TCP","port":"443-445"}]}]}],
                                                "config":{"nodeGroupConf":{"majorNodeGroup":"campus","nodeGroupList":[{"id":"campus","addressInfo":[{"address":format!("127.0.0.1:{}", address.port()),"type":"wan"}]}]}}}},
                                            "sdpPolicy":{"data":{"clientOption":{"dnsOption":{"firstDNS":"10.99.0.53"}}}}
                                        }}).to_string()
                                    }
                                },
                                _ => anyhow::bail!("unexpected authorization fixture path"),
                            }};
                            stream.write_all(format!("HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await?;
                            stream.write_all(body.as_bytes()).await?;
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
            closed,
            job,
        })
    }
}

#[tokio::test]
async fn shared_sso_authorizes_jwxt_with_state_and_preserves_mid_hop_cookie() -> anyhow::Result<()>
{
    let fixture = Fixture::new(Scenario::Normal).await?;
    let receipt = fixture.client.authorize_system("jwxt".to_owned()).await?;
    ensure!(
        receipt.system_id == "jwxt"
            && receipt.landing_url.ends_with("/jwglxt/index")
            && !receipt.landing_url.contains("code="),
        "bad public receipt"
    );
    let requests = fixture.requests.lock().await;
    let authorize = requests
        .iter()
        .find(|request| request.host == "sso.fixture.test")
        .context("no SSO authorization")?;
    ensure!(
        authorize.query["state"].len() == 32 && authorize.query["scope"] == "jw",
        "JWXT state missing"
    );
    ensure!(
        authorize
            .headers
            .get("accept")
            .is_some_and(|value| value.starts_with("text/html"))
            && authorize
                .headers
                .get("cookie")
                .is_some_and(|value| value.contains("SHU_OAUTH2=account-fixture")),
        "authorization did not use shared session page headers"
    );
    ensure!(
        requests
            .iter()
            .any(|request| request.path == "/jwglxt/index"),
        "landing not requested"
    );
    Ok(())
}

#[tokio::test]
async fn otp_registered_double_slash_bootstrap_hidden_state_cache_and_followup_are_real(
) -> anyhow::Result<()> {
    for scenario in [
        Scenario::Normal,
        Scenario::HiddenState,
        Scenario::EmptyState,
    ] {
        let fixture = Fixture::new(scenario).await?;
        fixture.client.authorize_system("otp".to_owned()).await?;
        fixture.client.authorize_system("otp".to_owned()).await?;
        let requests = fixture.requests.lock().await;
        let authorizes: Vec<_> = requests
            .iter()
            .filter(|request| request.host == "sso.fixture.test")
            .collect();
        let expected = match scenario {
            Scenario::HiddenState => Some("body-state"),
            Scenario::EmptyState => None,
            _ => Some("bootstrap-state"),
        };
        for request in authorizes {
            ensure!(
                request.query["redirect_uri"].ends_with("//Callback.aspx"),
                "registered path changed"
            );
            ensure!(
                request.query.get("state").map(String::as_str) == expected,
                "bootstrap/cache state wrong"
            );
        }
        ensure!(
            requests
                .iter()
                .any(|request| request.path == "//Callback.aspx")
                && requests
                    .iter()
                    .any(|request| request.path == "/Default.aspx"),
            "OTP follow-up missing"
        );
    }
    Ok(())
}

#[tokio::test]
async fn state_callback_origin_hops_and_markers_fail_closed_without_publishing_cookies(
) -> anyhow::Result<()> {
    for (scenario, code) in [
        (Scenario::WrongState, "oauthState"),
        (Scenario::WrongPath, "unexpectedRedirect"),
        (Scenario::LoginRedirect, "sessionNotReused"),
        (Scenario::ForeignHop, "unsafeRedirect"),
        (Scenario::Loop, "tooManyRedirects"),
        (Scenario::BadBody, "systemSessionFailed"),
    ] {
        let fixture = Fixture::new(scenario).await?;
        ensure!(
            fixture
                .client
                .authorize_system("jwxt".to_owned())
                .await
                .err()
                .is_some_and(|error| error.code == code),
            "unsafe authorization accepted"
        );
        let state = fixture.client.state()?;
        ensure!(
            !state
                .jar
                .names(&fixture.client.config.system_base("jwxt")?)
                .iter()
                .any(|name| name == "JSESSIONID"),
            "failed transaction published cookie"
        );
    }
    Ok(())
}

#[tokio::test]
async fn shared_account_atrust_uses_native_ticket_csrf_chain() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Normal).await?;
    let receipt = fixture
        .client
        .sign_in_atrust("owned-device".to_owned())
        .await?;
    ensure!(
        receipt.system_id == "atrust" && receipt.username.as_deref() == Some("fixture-account"),
        "native gateway authentication missing"
    );
    fixture
        .client
        .sign_in_atrust("owned-device".to_owned())
        .await?;
    let requests = fixture.requests.lock().await;
    ensure!(
        requests
            .iter()
            .filter(|request| request.path == "/passport/v1/auth/httpsOauth2")
            .count()
            == 1,
        "gateway authentication not reused"
    );
    drop(requests);
    let state = fixture.client.state()?;
    ensure!(
        state
            .jar
            .names(&fixture.client.config.system_base("atrust")?)
            .iter()
            .any(|name| name == "sid"),
        "gateway cookie not owned by account"
    );
    Ok(())
}

#[tokio::test]
async fn logout_invalidates_callback_and_unknown_or_missing_session_never_sends_http(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::SlowCallback).await?;
    let client = fixture.client.clone();
    let pending = tokio::spawn(async move { client.authorize_system("jwxt".to_owned()).await });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    fixture.client.clear()?;
    ensure!(
        tokio::time::timeout(Duration::from_secs(2), pending)
            .await??
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late callback adopted"
    );
    fixture.release.notify_one();
    let before = fixture.requests.lock().await.len();
    ensure!(
        fixture
            .client
            .authorize_system("unknown".to_owned())
            .await
            .err()
            .is_some_and(|error| error.code == "unknownSystem"),
        "unknown system accepted"
    );
    ensure!(
        fixture
            .client
            .authorize_system("otp".to_owned())
            .await
            .err()
            .is_some_and(|error| error.code == "sessionMissing"),
        "missing session authorized"
    );
    ensure!(
        fixture.requests.lock().await.len() == before,
        "local rejection sent requests"
    );
    Ok(())
}

#[tokio::test]
async fn owned_atrust_parent_cookie_rotation_and_deletion_do_not_revive_stale_snapshots(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::ParentCookie).await?;
    fixture.client.state()?.jar.save(
        &fixture.client.config.sso,
        "shared=old; Domain=fixture.test; Path=/; Secure; HttpOnly; SameSite=Strict",
    )?;
    fixture
        .client
        .sign_in_atrust("owned-device".to_owned())
        .await?;
    let requests = fixture.requests.lock().await;
    let authorize = requests
        .iter()
        .find(|request| request.host == "sso.fixture.test")
        .context("missing SSO request")?;
    ensure!(
        authorize.headers.get("cookie").is_some_and(
            |header| header.contains("shared=rotated") && !header.contains("shared=old")
        ),
        "gateway update did not reach shared SSO jar"
    );
    let online = requests
        .iter()
        .find(|request| request.path == "/passport/v1/user/onlineInfo")
        .context("missing online info")?;
    ensure!(
        !online
            .headers
            .get("cookie")
            .is_some_and(|header| header.contains("shared=")),
        "deleted parent cookie reached gateway"
    );
    drop(requests);
    ensure!(
        !fixture
            .client
            .state()?
            .jar
            .names(&fixture.client.config.sso)
            .iter()
            .any(|name| name == "shared"),
        "deleted parent cookie resurrected"
    );
    Ok(())
}

#[tokio::test]
async fn duplicate_authorization_codes_are_rejected_before_redeem() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::DuplicateCode).await?;
    ensure!(
        fixture
            .client
            .authorize_system("jwxt".to_owned())
            .await
            .err()
            .is_some_and(|error| error.code == "authorizeFailed"),
        "duplicate business code accepted"
    );
    ensure!(
        fixture
            .client
            .sign_in_atrust("owned-device".to_owned())
            .await
            .err()
            .is_some_and(|error| error.code == "authorizeFailed"),
        "duplicate gateway code accepted"
    );
    ensure!(
        !fixture
            .requests
            .lock()
            .await
            .iter()
            .any(|request| request.path == "/passport/v1/auth/httpsOauth2"),
        "invalid gateway code redeemed"
    );
    Ok(())
}

#[tokio::test]
async fn otp_fetch_proves_code_retries_network_and_reauthorizes_only_lost_session(
) -> anyhow::Result<()> {
    for (scenario, count, authorizations) in [
        (Scenario::OtpDirect, 1, 0),
        (Scenario::OtpRetry, 3, 0),
        (Scenario::OtpLost, 3, 1),
    ] {
        let fixture = Fixture::new(scenario).await?;
        let code = fixture.client.fetch_otp().await?;
        ensure!(
            code.code == "012345"
                && code.account == "fixture-account"
                && code.remaining_seconds == 11
                && code.period_seconds == 60,
            "actual OTP/server clock missing"
        );
        let requests = fixture.requests.lock().await;
        ensure!(
            requests
                .iter()
                .filter(|request| request.path == "/Default.aspx")
                .count()
                == count,
            "wrong retry count"
        );
        ensure!(
            requests
                .iter()
                .filter(|request| request.host == "sso.fixture.test")
                .count()
                == authorizations,
            "network retry reauthorized"
        );
        let reads: Vec<_> = requests
            .iter()
            .filter(|request| request.path == "/Default.aspx" && request.query.contains_key("_"))
            .collect();
        ensure!(
            !reads.is_empty()
                && reads.iter().all(|request| request
                    .headers
                    .get("pragma")
                    .is_some_and(|value| value == "no-cache")),
            "fresh read headers missing"
        );
        if reads.len() > 1 {
            ensure!(
                reads[0].query["_"] != reads[1].query["_"],
                "cache bust reused"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn otp_landing_without_code_and_foreign_redirect_are_not_usable() -> anyhow::Result<()> {
    for (scenario, expected) in [
        (Scenario::OtpNoCode, "otpParseFailed"),
        (Scenario::OtpForeign, "otpUnsafeRedirect"),
    ] {
        let fixture = Fixture::new(scenario).await?;
        ensure!(
            fixture
                .client
                .fetch_otp()
                .await
                .err()
                .is_some_and(|error| error.code == expected),
            "invalid page became OTP"
        );
        let requests = fixture.requests.lock().await;
        let count = requests
            .iter()
            .filter(|request| request.host == "sso.fixture.test")
            .count();
        ensure!(
            count
                == if matches!(scenario, Scenario::OtpNoCode) {
                    1
                } else {
                    0
                },
            "unexpected authorization loop"
        );
    }
    Ok(())
}

#[tokio::test]
async fn logout_discards_late_otp_code_and_cookie() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::OtpSlow).await?;
    let client = fixture.client.clone();
    let request = tokio::spawn(async move { client.fetch_otp().await });
    fixture.entered.notified().await;
    fixture.client.clear()?;
    fixture.release.notify_one();
    ensure!(
        request
            .await?
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late OTP survived logout"
    );
    ensure!(
        !fixture.client.snapshot()?.has_session
            && fixture.client.snapshot()?.cookie_names.is_empty(),
        "logout Cookie restored"
    );
    Ok(())
}

#[tokio::test]
async fn jwxt_warmup_form_and_optional_profile_merge_keep_authorization_independent(
) -> anyhow::Result<()> {
    for scenario in [
        Scenario::Normal,
        Scenario::JwxtProfileMissing,
        Scenario::JwxtTermMissing,
        Scenario::JwxtPostFail,
    ] {
        let fixture = Fixture::new(scenario).await?;
        fixture.client.authorize_system("jwxt".into()).await?;
        let account = fixture.client.fetch_jwxt_account().await?;
        if matches!(scenario, Scenario::JwxtTermMissing | Scenario::JwxtPostFail) {
            if matches!(scenario, Scenario::JwxtTermMissing) {
                ensure!(
                    account.name.as_deref() == Some("档案姓名") && account.student_id.is_none(),
                    "missing terms invented identity"
                );
                ensure!(
                    !fixture
                        .requests
                        .lock()
                        .await
                        .iter()
                        .any(|request| request.path.ends_with("xskbcx_cxXsgrkb.html")),
                    "empty terms POSTed"
                );
            }
            let base = fixture.client.config.system_base("jwxt")?;
            let state = fixture.client.state()?;
            ensure!(
                !state
                    .jar
                    .names(&base)
                    .iter()
                    .any(|name| name == "schedule" || name == "identity"),
                "partial identity cookies committed"
            );
        } else {
            ensure!(
                account.name.as_deref() == Some("课表姓名")
                    && account.student_id.as_deref() == Some("25123456")
                    && account.class_name.as_deref() == Some("示例班级"),
                "identity did not override profile"
            );
            ensure!(
                account.college.is_some() != matches!(scenario, Scenario::JwxtProfileMissing),
                "optional profile failure not isolated"
            );
        }
        ensure!(
            fixture.client.snapshot()?.has_session,
            "optional data cleared authorization"
        );
    }
    Ok(())
}

#[tokio::test]
async fn logout_discards_late_jwxt_identity() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::JwxtSlow).await?;
    fixture.client.authorize_system("jwxt".into()).await?;
    let client = fixture.client.clone();
    let request = tokio::spawn(async move { client.fetch_jwxt_account().await });
    fixture.entered.notified().await;
    fixture.client.clear()?;
    fixture.release.notify_one();
    ensure!(
        request
            .await?
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late identity survived logout"
    );
    Ok(())
}

#[tokio::test]
async fn account_preparation_owns_plan_identity_resource_sid_and_one_use() -> anyhow::Result<()> {
    for scenario in [
        Scenario::Normal,
        Scenario::ResourceScopedSid,
        Scenario::ResourceVerified,
    ] {
        let fixture = Fixture::new(scenario).await?;
        let prepared = fixture
            .client
            .prepare_tunnel("owned-device".into(), true)
            .await?;
        let plan = sangfor_core::plan::SessionPlan::decode(prepared.take()?.as_bytes())?;
        let sid = if matches!(scenario, Scenario::ResourceScopedSid) {
            "resource-legacy"
        } else {
            "resource-sid"
        };
        ensure!(
            plan.sid == sid
                && plan.device_id == "owned-device"
                && plan.sign_key().is_some_and(|key| key.len() == 32)
                && !plan.connection_id.is_empty(),
            "native plan identity missing or SID scope lost"
        );
        ensure!(
            plan.routes.len() == 1
                && plan.routes[0].host == "fixture.shu.edu.cn"
                && plan.routes[0].port_max == 445
                && plan.dns_servers == ["10.99.0.53"],
            "resource plan missing"
        );
        ensure!(
            prepared
                .take()
                .err()
                .is_some_and(|error| error.code == "planUsed"),
            "plan replay allowed"
        );
        if matches!(scenario, Scenario::ResourceVerified) {
            ensure!(
                !plan.accept_any_certificate
                    && plan.certificate_digests.len() == 1
                    && prepared.warnings().is_empty(),
                "verified pins not preserved"
            );
        }
        let fresh = fixture
            .client
            .prepare_tunnel("owned-device".into(), false)
            .await?;
        let next = sangfor_core::plan::SessionPlan::decode(fresh.take()?.as_bytes())?;
        ensure!(
            plan.connection_id != next.connection_id && plan.sign_key() != next.sign_key(),
            "connection identity reused"
        );
    }
    Ok(())
}

#[tokio::test]
async fn opaque_prepared_plan_connects_real_tls_control_and_cannot_replay() -> anyhow::Result<()> {
    for scenario in [Scenario::Normal, Scenario::ResourceScopedSid] {
        let fixture = Fixture::new(scenario).await?;
        let prepared = crate::api::newsso::PreparedATrustTunnel {
            inner: fixture
                .client
                .prepare_tunnel("owned-device".into(), false)
                .await?,
        };
        let attempt = crate::api::vpn::VpnConnectAttempt::new();
        let session = attempt.connect_prepared(&prepared, 3000).await?;
        ensure!(
            session.snapshot().state == crate::connection::model::VpnState::Connected
                && session.snapshot().virtual_address.as_deref() == Some("10.99.0.2"),
            "opaque plan did not connect actual L3"
        );
        ensure!(
            attempt
                .connect_prepared(&prepared, 3000)
                .await
                .err()
                .is_some_and(|error| error.code == "planUsed"),
            "opaque connection replayed"
        );
        session.stop().await?;
    }
    Ok(())
}

#[tokio::test]
async fn failed_resource_or_strict_signature_never_publishes_gateway_cookies() -> anyhow::Result<()>
{
    for (scenario, expected) in [
        (Scenario::ResourceFail, "atrustResource"),
        (Scenario::ResourceTampered, "atrustAntiMitm"),
    ] {
        let fixture = Fixture::new(scenario).await?;
        ensure!(
            fixture
                .client
                .prepare_tunnel("owned-device".into(), true)
                .await
                .err()
                .is_some_and(|error| error.code == expected),
            "unsafe preparation succeeded"
        );
        let base = fixture.client.config.system_base("atrust")?;
        ensure!(
            !fixture
                .client
                .state()?
                .jar
                .names(&base)
                .iter()
                .any(|name| name == "sid"),
            "failed preparation published SID"
        );
    }
    let fixture = Fixture::new(Scenario::ResourceTampered).await?;
    let prepared = fixture
        .client
        .prepare_tunnel("owned-device".into(), false)
        .await?;
    let plan = sangfor_core::plan::SessionPlan::decode(prepared.take()?.as_bytes())?;
    ensure!(
        prepared.warnings().len() == 1
            && !plan.accept_any_certificate
            && plan.certificate_digests.len() == 1,
        "compatible signature failure lost pins"
    );
    Ok(())
}

#[tokio::test]
async fn logout_cancels_resource_request_and_invalidates_prepared_opaque_handle(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::ResourceSlow).await?;
    let client = fixture.client.clone();
    let job =
        tokio::spawn(async move { client.prepare_tunnel("owned-device".into(), false).await });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    fixture.client.clear()?;
    fixture.release.notify_one();
    ensure!(
        job.await?
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late resource plan survived logout"
    );
    let fixture = Fixture::new(Scenario::Normal).await?;
    let prepared = crate::api::newsso::PreparedATrustTunnel {
        inner: fixture
            .client
            .prepare_tunnel("owned-device".into(), false)
            .await?,
    };
    fixture.client.clear()?;
    let attempt = crate::api::vpn::VpnConnectAttempt::new();
    ensure!(
        attempt
            .connect_prepared(&prepared, 3000)
            .await
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "logged-out prepared plan connected"
    );
    Ok(())
}

#[tokio::test]
async fn pending_opaque_control_obeys_attempt_and_account_cancellation() -> anyhow::Result<()> {
    for cancel_account in [false, true] {
        let fixture = Fixture::new(Scenario::ControlSlow).await?;
        let prepared = Arc::new(crate::api::newsso::PreparedATrustTunnel {
            inner: fixture
                .client
                .prepare_tunnel("owned-device".into(), false)
                .await?,
        });
        let attempt = Arc::new(crate::api::vpn::VpnConnectAttempt::new());
        let caller = attempt.clone();
        let plan = prepared.clone();
        let job = tokio::spawn(async move { caller.connect_prepared(&plan, 3000).await });
        tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
        if cancel_account {
            fixture.client.clear()?;
        } else {
            attempt.cancel();
        }
        ensure!(
            tokio::time::timeout(Duration::from_secs(3), job)
                .await??
                .err()
                .is_some_and(|error| error.code == "cancelled"),
            "pending control ignored cancellation"
        );
        fixture.release.notify_one();
        ensure!(
            fixture.client.snapshot()?.has_session != cancel_account,
            "attempt cancellation logged out account"
        );
    }
    Ok(())
}

#[tokio::test]
async fn account_logout_or_close_disconnects_established_native_transport() -> anyhow::Result<()> {
    for close in [false, true] {
        let fixture = Fixture::new(Scenario::Normal).await?;
        let prepared = crate::api::newsso::PreparedATrustTunnel {
            inner: fixture
                .client
                .prepare_tunnel("owned-device".into(), false)
                .await?,
        };
        let session = crate::api::vpn::VpnConnectAttempt::new()
            .connect_prepared(&prepared, 3000)
            .await?;
        if close {
            fixture.client.close()?;
        } else {
            fixture.client.clear()?;
        }
        tokio::time::timeout(Duration::from_secs(3), fixture.closed.notified()).await?;
        tokio::time::timeout(Duration::from_secs(3), async {
            while session.snapshot().state == crate::connection::model::VpnState::Connected {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        ensure!(
            session.snapshot().state == crate::connection::model::VpnState::Disconnected,
            "account invalidation left established VPN alive"
        );
        session.stop().await?;
    }
    Ok(())
}

#[tokio::test]
async fn native_credential_probe_connects_and_closes_before_reporting_available(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Normal).await?;
    let verified = fixture
        .client
        .verify_atrust("owned-device".into(), false)
        .await?;
    tokio::time::timeout(Duration::from_secs(3), fixture.closed.notified()).await?;
    ensure!(
        verified.virtual_address.as_deref() == Some("10.99.0.2")
            && verified.dns_servers == ["10.99.0.53"],
        "probe did not use actual control response"
    );
    ensure!(
        fixture.client.snapshot()?.has_session,
        "temporary probe discarded reusable account"
    );
    Ok(())
}
