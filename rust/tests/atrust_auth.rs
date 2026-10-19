use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};
use shuvpn_core::api::atrust_auth::{
    ATrustAuthClient, AuthConfig, AuthCookie, AuthSeed, TunnelOptions,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{timeout, Duration};

#[derive(Clone, Copy)]
enum Scenario {
    Login,
    Reuse,
    MissingSid,
    Challenge,
    Loop,
    BadTicketOrigin,
    BadJson,
    HttpFailure,
    Slow,
    FastLogin,
    FastWithoutCode,
    BadFastOrigin,
    BadFastPath,
    CsrfDisappears,
    CookiePaths,
    Oversized,
    Enhanced,
    TunnelVerified,
    TunnelTampered,
    TunnelExpired,
}

#[derive(Clone)]
struct Request {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

struct Fixture {
    gateway: String,
    sso: String,
    requests: Arc<Mutex<Vec<Request>>>,
    entered: Arc<Notify>,
    jobs: Vec<JoinHandle<()>>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for job in &self.jobs {
            job.abort();
        }
    }
}

impl Fixture {
    async fn new(scenario: Scenario) -> anyhow::Result<Self> {
        let gateway = TcpListener::bind("127.0.0.1:0").await?;
        let sso = TcpListener::bind("127.0.0.1:0").await?;
        let gateway_url = format!("http://{}/", gateway.local_addr()?);
        let sso_url = format!("http://{}/", sso.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let entered = Arc::new(Notify::new());
        let mut jobs = vec![];
        for listener in [gateway, sso] {
            let requests = Arc::clone(&requests);
            let entered = Arc::clone(&entered);
            let gateway = gateway_url.clone();
            jobs.push(tokio::spawn(async move {
                let mut connections = JoinSet::new();
                loop {
                    tokio::select! {
                        accepted = listener.accept() => {
                            let Ok((stream, _)) = accepted else { break };
                            let requests = Arc::clone(&requests);
                            let entered = Arc::clone(&entered);
                            let gateway = gateway.clone();
                            connections.spawn(async move {
                                if let Err(error) = serve(stream, scenario, &gateway, requests, entered).await {
                                    // Client cancellation may close the fixture connection.
                                    if error.downcast_ref::<std::io::Error>().is_none() {
                                        panic!("fixture failed: {error}");
                                    }
                                }
                            });
                        }
                        Some(completed) = connections.join_next(), if !connections.is_empty() => {
                            if let Err(error) = completed { panic!("fixture task failed: {error}"); }
                        }
                    }
                }
            }));
        }
        Ok(Self {
            gateway: gateway_url,
            sso: sso_url,
            requests,
            entered,
            jobs,
        })
    }

    fn config(&self) -> AuthConfig {
        AuthConfig {
            gateway_base: self.gateway.clone(),
            sso_base: self.sso.clone(),
            client_id: "test-client".to_owned(),
            redirect_uri: format!("{}passport/v1/auth/httpsOauth2", self.gateway),
            scope: String::new(),
            fallback_login_domain: "fallback".to_owned(),
            user_agent: "ShuVPN test".to_owned(),
        }
    }

    fn seed(&self) -> AuthSeed {
        AuthSeed {
            device_id: "00112233445566778899aabbccddeeff".to_owned(),
            gateway_cookies: vec![cookie("gateway_seed", "gateway-only")],
            sso_cookies: vec![cookie("SHU_OAUTH2", "sso-only")],
        }
    }
}

fn cookie(name: &str, value: &str) -> AuthCookie {
    AuthCookie {
        name: name.to_owned(),
        value: value.to_owned(),
        domain: "127.0.0.1".to_owned(),
        path: "/".to_owned(),
        expires_unix_seconds: None,
    }
}

fn tunnel_options(strict: bool) -> TunnelOptions {
    TunnelOptions {
        connection_id: "fixture-connection".to_owned(),
        sign_key: (1..=32).collect(),
        process_platform: "Windows".to_owned(),
        strict_anti_mitm: strict,
    }
}

#[tokio::test]
async fn preparation_verifies_auth_fetches_resources_and_uses_rotated_sid() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::TunnelVerified).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    let prepared = client
        .prepare_tunnel(fixture.seed(), tunnel_options(true))
        .await?;
    let plan = sangfor_core::plan::SessionPlan::decode(prepared.plan_json.as_bytes())?;
    assert!(prepared.warnings.is_empty());
    assert_eq!(prepared.session.sid, "resource-sid");
    assert_eq!(plan.sid, prepared.session.sid);
    assert_eq!(plan.connection_id, "fixture-connection");
    assert_eq!(plan.process_platform, "Windows");
    assert_eq!(plan.process_name, "shuvpn");
    assert_eq!(
        std::path::PathBuf::from(&plan.process_path),
        std::env::current_exe()?
    );
    assert_eq!(plan.routes[0].host, "fixture.shu.edu.cn");
    assert_eq!(plan.routes[0].port_max, 445);
    assert_eq!(plan.nodes["campus"], ["127.0.0.1:441"]);
    assert_eq!(plan.dns_servers, ["10.99.0.53"]);
    assert!(!plan.accept_any_certificate);
    assert_eq!(
        plan.certificate_digests,
        ["514177A58F877B521EA2F2CB04C3E5172F518A52CE6A2EFBFE5160208A886E7E"]
    );
    let requests = fixture.requests.lock().await;
    assert_eq!(requests.len(), 4);
    let resource = requests
        .last()
        .ok_or_else(|| anyhow::anyhow!("resource request was not recorded"))?;
    assert_eq!(resource.method, "POST");
    assert_eq!(
        resource.headers.get("x-csrf-token").map(String::as_str),
        Some("fixture-csrf")
    );
    assert!(resource.headers["cookie"].contains("sid=final-sid"));
    assert!(!resource.headers["cookie"].contains("SHU_OAUTH2"));
    let body: Value = serde_json::from_slice(&resource.body)?;
    assert_eq!(
        body,
        json!({"resourceType":{
            "sdpPolicy":{},"appList":{},"favoriteAppList":{},"featureCenter":{},
            "uemSpace":{"params":{"action":"login"}}
        }})
    );
    Ok(())
}

#[tokio::test]
async fn report_only_keeps_advertised_certificate_pins_and_strict_rejects_tampering(
) -> anyhow::Result<()> {
    for strict in [false, true] {
        let fixture = Fixture::new(Scenario::TunnelTampered).await?;
        let client = ATrustAuthClient::new(fixture.config())?;
        let result = client
            .prepare_tunnel(fixture.seed(), tunnel_options(strict))
            .await;
        if strict {
            assert!(result.is_err());
            assert_eq!(fixture.requests.lock().await.len(), 3);
        } else {
            let prepared = result?;
            assert_eq!(prepared.warnings.len(), 1);
            let plan = sangfor_core::plan::SessionPlan::decode(prepared.plan_json.as_bytes())?;
            assert!(!plan.accept_any_certificate);
            assert_eq!(plan.certificate_digests.len(), 1);
            assert_eq!(fixture.requests.lock().await.len(), 4);
        }
    }
    Ok(())
}

#[tokio::test]
async fn expired_session_and_invalid_identity_stop_before_resource_requests() -> anyhow::Result<()>
{
    let fixture = Fixture::new(Scenario::TunnelExpired).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    let failure = client
        .prepare_tunnel(fixture.seed(), tunnel_options(false))
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("expired session was accepted"))?;
    assert_eq!(failure.code, "atrustNotLoggedIn");
    assert_eq!(fixture.requests.lock().await.len(), 3);
    let fixture = Fixture::new(Scenario::TunnelVerified).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    let mut options = tunnel_options(true);
    options.sign_key.pop();
    assert!(client
        .prepare_tunnel(fixture.seed(), options)
        .await
        .is_err());
    assert!(fixture.requests.lock().await.is_empty());
    Ok(())
}

async fn read_request(stream: &mut TcpStream) -> anyhow::Result<Request> {
    let mut bytes = vec![];
    let header_end = loop {
        let mut chunk = [0; 4096];
        let read = stream.read(&mut chunk).await?;
        anyhow::ensure!(read != 0, "request ended before headers");
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(offset) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            break offset + 4;
        }
        anyhow::ensure!(bytes.len() < 64 * 1024, "fixture header limit");
    };
    let text = String::from_utf8(bytes[..header_end].to_vec())?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("").to_owned();
    let target = first.next().unwrap_or("").to_owned();
    let headers: BTreeMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_lowercase(), value.trim().to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(0);
    while bytes.len() < header_end + length {
        let mut chunk = [0; 4096];
        let read = stream.read(&mut chunk).await?;
        anyhow::ensure!(read != 0, "request ended before body");
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(Request {
        method,
        target,
        headers,
        body: bytes[header_end..header_end + length].to_vec(),
    })
}

async fn serve(
    mut stream: TcpStream,
    scenario: Scenario,
    gateway: &str,
    requests: Arc<Mutex<Vec<Request>>>,
    entered: Arc<Notify>,
) -> anyhow::Result<()> {
    let request = read_request(&mut stream).await?;
    let url = reqwest::Url::parse(gateway)?.join(&request.target)?;
    let query: BTreeMap<_, _> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    requests.lock().await.push(request.clone());
    if matches!(scenario, Scenario::Slow) {
        entered.notify_one();
        std::future::pending::<()>().await;
    }
    let mut status = "200 OK";
    let mut headers = vec![];
    let body = match url.path() {
        "/passport/v1/public/authConfig" => {
            if matches!(scenario, Scenario::BadJson) {
                b"not json".to_vec()
            } else if matches!(scenario, Scenario::HttpFailure) {
                status = "403 Forbidden";
                b"not json".to_vec()
            } else {
                headers.push("Set-Cookie: gateway_hop=bootstrap; Path=/".to_owned());
                headers.push("Set-Cookie: scoped=passport-only; Path=/passport".to_owned());
                let after = query.get("mod").is_some_and(|value| value == "1");
                let config_count = requests
                    .lock()
                    .await
                    .iter()
                    .filter(|request| request.target.starts_with("/passport/v1/public/authConfig"))
                    .count();
                if matches!(
                    scenario,
                    Scenario::TunnelVerified | Scenario::TunnelTampered | Scenario::TunnelExpired
                ) {
                    let mut envelope: Value =
                        serde_json::from_str(include_str!("fixtures/anti_mitm.json"))?;
                    if config_count > 1 {
                        match scenario {
                            Scenario::TunnelTampered => {
                                envelope["data"]["antiMITMAttackData"]["mitmSig"] = json!("BAD");
                            }
                            Scenario::TunnelExpired => envelope["data"]["isLogin"] = json!(0),
                            _ => {}
                        }
                    }
                    envelope.to_string().into_bytes()
                } else {
                    let csrf = if matches!(scenario, Scenario::CsrfDisappears) && config_count > 1 {
                        Value::Null
                    } else {
                        json!(if after { "csrf-after" } else { "csrf-before" })
                    };
                    let login_url = if matches!(
                        scenario,
                        Scenario::FastLogin
                            | Scenario::FastWithoutCode
                            | Scenario::BadFastOrigin
                            | Scenario::BadFastPath
                    ) {
                        format!("{gateway}fast")
                    } else {
                        String::new()
                    };
                    json!({"code":0, "data": {
                    "isLogin": if matches!(scenario, Scenario::Reuse | Scenario::CsrfDisappears) || after {1} else {0},
                    "csrfToken": csrf,
                    "authServerInfoList": [{"authType":"auth/httpsOauth2","loginDomain":"server-domain","loginUrl":login_url}]
                }}).to_string().into_bytes()
                }
            }
        }
        "/oauth/authorize" => {
            status = "302 Found";
            headers.push(format!(
                "Location: {gateway}passport/v1/auth/httpsOauth2?code=one-time-code"
            ));
            headers.push("Set-Cookie: SHU_OAUTH2=sso-refreshed; Path=/".to_owned());
            if matches!(scenario, Scenario::CookiePaths) {
                headers.push("Set-Cookie: session=deleted; Path=/oauth; Max-Age=0".to_owned());
            }
            vec![]
        }
        "/fast" => {
            status = "302 Found";
            let destination = match scenario {
                Scenario::BadFastOrigin => {
                    "https://foreign.invalid/callback?code=wrong-code".to_owned()
                }
                Scenario::BadFastPath => format!("{gateway}wrong?code=wrong-code"),
                Scenario::FastWithoutCode => format!("{gateway}landing"),
                _ => format!("{gateway}passport/v1/auth/httpsOauth2?code=one-time-code"),
            };
            headers.push(format!("Location: {destination}"));
            vec![]
        }
        "/passport/v1/auth/httpsOauth2" => {
            status = "302 Found";
            let base = if matches!(scenario, Scenario::BadTicketOrigin) {
                "https://foreign.invalid/"
            } else {
                gateway
            };
            let mut location = reqwest::Url::parse(base)?.join("/portal/shortcut.html")?;
            location
                .query_pairs_mut()
                .append_pair("data", r#"{"ticket":"test-ticket"}"#);
            headers.push(format!("Location: {location}"));
            vec![]
        }
        "/controller/v1/public/reportEnv" => {
            if matches!(scenario, Scenario::Oversized) {
                vec![b' '; 1024 * 1024 + 1]
            } else {
                json!({"code":0,"data":{}}).to_string().into_bytes()
            }
        }
        "/passport/v1/auth/authCheck" => {
            if matches!(scenario, Scenario::Enhanced) {
                json!({"code":0,"data":{"nextService":"auth/accessCheck",
                    "nextServiceList":[{"authType":"auth/accessCheck","authId":"not-for-access"}]}})
                .to_string()
                .into_bytes()
            } else {
                let next = match scenario {
                    Scenario::Challenge => "auth/sendSms",
                    Scenario::Loop => "auth/authCheck",
                    _ => "",
                };
                json!({"code":0,"data":{"nextService":next}})
                    .to_string()
                    .into_bytes()
            }
        }
        "/passport/v1/auth/accessCheck"
        | "/passport/v1/auth/preEnhancedAuth"
        | "/passport/v1/auth/enhancedConfirm"
        | "/passport/v1/auth/enhancedDone" => {
            let next = match url.path() {
                "/passport/v1/auth/accessCheck" => "auth/preEnhancedAuth",
                "/passport/v1/auth/preEnhancedAuth" => "auth/enhancedConfirm",
                "/passport/v1/auth/enhancedConfirm" => "auth/enhancedDone",
                _ => "auth/bindAuthDevice",
            };
            json!({"code":0,"data":{"nextService":next,
                "nextServiceList":[{"authType":next,"authId":"step-id"}]}})
            .to_string()
            .into_bytes()
        }
        "/passport/v1/auth/bindAuthDevice" => json!({"code":0,"data":{}}).to_string().into_bytes(),
        "/passport/v1/user/onlineInfo" => {
            if !matches!(scenario, Scenario::MissingSid) {
                headers.push("Set-Cookie: sid=final-sid; Path=/; Max-Age=3600".to_owned());
            }
            json!({"code":0,"data":{"username":"test-account"}})
                .to_string()
                .into_bytes()
        }
        "/controller/v1/user/clientResource" => {
            headers.push("Set-Cookie: sid=resource-sid; Path=/; Max-Age=3600".to_owned());
            json!({"code":0,"data":{
                "appList":{"data":{
                    "appInfo":[{"apps":[{"id":"campus-app","nodeGroupId":"campus",
                        "accessModel":"L3VPN","addressList":[
                            {"host":"fixture.shu.edu.cn","protocol":"TCP","port":"443-445"}
                        ]}]}],
                    "config":{"nodeGroupConf":{
                        "majorNodeGroup":"campus",
                        "nodeGroupList":[{"id":"campus","addressInfo":[
                            {"address":"{{sdpcHost}}","type":"wan"}
                        ]}]
                    }}
                }},
                "sdpPolicy":{"data":{"clientOption":{"dnsOption":{"firstDNS":"10.99.0.53"}}}}
            }})
            .to_string()
            .into_bytes()
        }
        _ => {
            status = "404 Not Found";
            vec![]
        }
    };
    let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{}\r\n", body.len(),
        headers.iter().map(|header| format!("{header}\r\n")).collect::<String>());
    stream.write_all(response.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn enhanced_steps_forward_auth_id_only_to_the_defined_services() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Enhanced).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    client.sign_in(fixture.seed()).await?;
    let requests = fixture.requests.lock().await;
    for request in requests
        .iter()
        .filter(|request| request.target.starts_with("/passport/v1/auth/"))
    {
        let url = reqwest::Url::parse(&fixture.gateway)?.join(&request.target)?;
        let query: BTreeMap<_, _> = url.query_pairs().collect();
        match url.path() {
            "/passport/v1/auth/authCheck" | "/passport/v1/auth/accessCheck" => {
                assert!(!query.contains_key("authId"));
            }
            "/passport/v1/auth/preEnhancedAuth"
            | "/passport/v1/auth/enhancedConfirm"
            | "/passport/v1/auth/enhancedDone"
            | "/passport/v1/auth/bindAuthDevice" => {
                assert_eq!(query.get("authId").map(|id| id.as_ref()), Some("step-id"));
                if url.path().ends_with("bindAuthDevice") {
                    assert_eq!(
                        query.get("deviceId").map(|id| id.as_ref()),
                        Some("00112233445566778899aabbccddeeff")
                    );
                }
            }
            _ => {}
        }
    }
    assert_eq!(requests.len(), 12);
    Ok(())
}

#[tokio::test]
async fn oauth_ticket_csrf_and_cookies_roundtrip_without_redirect_following() -> anyhow::Result<()>
{
    let fixture = Fixture::new(Scenario::Login).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    let session = client.sign_in(fixture.seed()).await?;
    assert_eq!(session.username, "test-account");
    assert_eq!(session.sid, "final-sid");
    assert_eq!(session.ticket, "test-ticket");
    assert_eq!(session.login_domain, "server-domain");
    assert!(session
        .gateway_cookies
        .iter()
        .any(|cookie| cookie.name == "sid" && cookie.expires_unix_seconds.is_some()));
    assert!(session
        .sso_cookies
        .iter()
        .any(|cookie| cookie.name == "SHU_OAUTH2" && cookie.value == "sso-refreshed"));
    let requests = fixture.requests.lock().await;
    assert_eq!(requests.len(), 7);
    let paths: Vec<_> = requests
        .iter()
        .map(|request| request.target.split('?').next().unwrap_or(""))
        .collect();
    assert_eq!(
        paths,
        [
            "/passport/v1/public/authConfig",
            "/oauth/authorize",
            "/passport/v1/auth/httpsOauth2",
            "/passport/v1/public/authConfig",
            "/controller/v1/public/reportEnv",
            "/passport/v1/auth/authCheck",
            "/passport/v1/user/onlineInfo"
        ]
    );
    let sso = &requests[1];
    assert_eq!(
        sso.headers.get("cookie").map(String::as_str),
        Some("SHU_OAUTH2=sso-only")
    );
    assert!(!sso.headers.contains_key("x-csrf-token"));
    for request in requests
        .iter()
        .filter(|request| !request.target.starts_with("/oauth/"))
    {
        assert!(!request
            .headers
            .get("cookie")
            .is_some_and(|cookie| cookie.contains("SHU_OAUTH2")));
        assert_eq!(
            request.headers.get("x-sdp-traceid").map(String::len),
            Some(8)
        );
    }
    let callback = reqwest::Url::parse(&fixture.gateway)?.join(&requests[2].target)?;
    let query: BTreeMap<_, _> = callback.query_pairs().collect();
    assert_eq!(query.get("state").map(|value| value.as_ref()), Some("null"));
    assert_eq!(
        query.get("sfDomain").map(|value| value.as_ref()),
        Some("server-domain")
    );
    assert_eq!(
        requests[2].headers.get("x-csrf-token").map(String::as_str),
        Some("csrf-before")
    );
    let report = &requests[4];
    assert_eq!(report.method, "POST");
    assert_eq!(
        report.headers.get("x-csrf-token").map(String::as_str),
        Some("csrf-after")
    );
    assert!(report
        .headers
        .get("cookie")
        .is_some_and(|cookie| cookie.contains("gateway_hop=bootstrap")));
    assert!(!report
        .headers
        .get("cookie")
        .is_some_and(|cookie| cookie.contains("scoped=")));
    assert!(session
        .gateway_cookies
        .iter()
        .any(|cookie| cookie.name == "scoped" && cookie.path == "/passport"));
    let body: Value = serde_json::from_slice(&report.body)?;
    assert_eq!(body["ticket"], "test-ticket");
    assert_eq!(body["deviceId"], body["env"]["endpoint"]["device_id"]);
    Ok(())
}

#[test]
fn device_id_is_32_lowercase_hex_characters() -> anyhow::Result<()> {
    let device = shuvpn_core::api::atrust_auth::generate_device_id()?;
    assert_eq!(device.len(), 32);
    assert!(device
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    Ok(())
}

#[tokio::test]
async fn logged_in_gateway_skips_ticket_environment_and_auth_steps() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Reuse).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    let session = client.sign_in(fixture.seed()).await?;
    assert!(session.ticket.is_empty());
    assert_eq!(fixture.requests.lock().await.len(), 2);
    Ok(())
}

#[tokio::test]
async fn rejects_protocol_failures_with_typed_codes() -> anyhow::Result<()> {
    for (scenario, expected) in [
        (Scenario::MissingSid, "atrustNoSid"),
        (Scenario::Challenge, "atrustChallenge"),
        (Scenario::Loop, "atrustAuthChain"),
        (Scenario::BadTicketOrigin, "atrustTicketFailed"),
        (Scenario::BadJson, "atrustBadResponse"),
        (Scenario::HttpFailure, "atrustHttp403"),
        (Scenario::BadFastOrigin, "authorizeFailed"),
        (Scenario::BadFastPath, "authorizeFailed"),
        (Scenario::Oversized, "atrustBadResponse"),
    ] {
        let fixture = Fixture::new(scenario).await?;
        let client = ATrustAuthClient::new(fixture.config())?;
        let failure = client
            .sign_in(fixture.seed())
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("scenario unexpectedly succeeded"))?;
        assert_eq!(failure.code, expected);
        assert!(!failure.message.contains("one-time-code"));
        if matches!(scenario, Scenario::Loop) {
            assert_eq!(
                fixture
                    .requests
                    .lock()
                    .await
                    .iter()
                    .filter(|request| request.target.starts_with("/passport/v1/auth/authCheck"))
                    .count(),
                8
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn validated_fast_login_consumes_only_the_expected_callback() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::FastLogin).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    let session = client.sign_in(fixture.seed()).await?;
    assert_eq!(session.ticket, "test-ticket");
    let requests = fixture.requests.lock().await;
    assert_eq!(requests.len(), 7);
    assert_eq!(requests[1].target, "/fast");
    assert!(!requests
        .iter()
        .any(|request| request.target.starts_with("/oauth/authorize")));
    Ok(())
}

#[tokio::test]
async fn gateway_login_url_without_code_falls_back_to_the_registered_authorizer(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::FastWithoutCode).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    assert_eq!(client.sign_in(fixture.seed()).await?.ticket, "test-ticket");
    let requests = fixture.requests.lock().await;
    assert_eq!(requests.len(), 8);
    assert_eq!(requests[1].target, "/fast");
    assert!(requests[2].target.starts_with("/oauth/authorize"));
    Ok(())
}
#[tokio::test]
async fn repeated_client_does_not_reuse_a_previous_csrf_token() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::CsrfDisappears).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    client.sign_in(fixture.seed()).await?;
    let failure = client
        .sign_in(fixture.seed())
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("missing CSRF was accepted"))?;
    assert_eq!(failure.code, "atrustNoCsrf");
    assert!(!fixture.requests.lock().await[2]
        .headers
        .contains_key("x-csrf-token"));
    Ok(())
}

#[tokio::test]
async fn same_name_cookies_preserve_path_order_and_path_local_deletion() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::CookiePaths).await?;
    let client = ATrustAuthClient::new(fixture.config())?;
    let mut seed = fixture.seed();
    seed.sso_cookies.push(cookie("session", "root-value"));
    let mut scoped = cookie("session", "oauth-value");
    scoped.path = "/oauth".to_owned();
    seed.sso_cookies.push(scoped);
    let session = client.sign_in(seed).await?;
    let requests = fixture.requests.lock().await;
    let sso = requests[1]
        .headers
        .get("cookie")
        .ok_or_else(|| anyhow::anyhow!("no SSO cookie header"))?;
    assert!(sso.starts_with("session=oauth-value; "));
    assert!(sso.contains("session=root-value"));
    let same_name: Vec<_> = session
        .sso_cookies
        .iter()
        .filter(|cookie| cookie.name == "session")
        .collect();
    assert_eq!(same_name.len(), 1);
    assert_eq!(same_name[0].path, "/");
    assert_eq!(same_name[0].value, "root-value");
    Ok(())
}

#[tokio::test]
async fn cancelling_in_flight_authentication_also_cancels_queued_call() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Slow).await?;
    let client = Arc::new(ATrustAuthClient::new(fixture.config())?);
    let first_client = Arc::clone(&client);
    let seed = fixture.seed();
    let first = tokio::spawn(async move { first_client.sign_in(seed).await });
    timeout(Duration::from_secs(2), fixture.entered.notified()).await?;
    let second_client = Arc::clone(&client);
    let seed = fixture.seed();
    let second = tokio::spawn(async move { second_client.sign_in(seed).await });
    client.cancel();
    for task in [first, second] {
        let failure = timeout(Duration::from_secs(1), task)
            .await??
            .err()
            .ok_or_else(|| anyhow::anyhow!("cancelled call succeeded"))?;
        assert_eq!(failure.code, "cancelled");
    }
    assert_eq!(fixture.requests.lock().await.len(), 1);
    Ok(())
}

#[tokio::test]
async fn invalid_cookie_and_non_https_remote_configuration_do_not_send_requests(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Login).await?;
    let mut config = fixture.config();
    config.gateway_base = "http://example.com/".to_owned();
    assert!(ATrustAuthClient::new(config).is_err());
    let client = ATrustAuthClient::new(fixture.config())?;
    let mut seed = fixture.seed();
    seed.sso_cookies
        .push(cookie("SHU_OAUTH2", "bad\r\nInjected: true"));
    assert!(client.sign_in(seed).await.is_err());
    assert!(fixture.requests.lock().await.is_empty());
    Ok(())
}
