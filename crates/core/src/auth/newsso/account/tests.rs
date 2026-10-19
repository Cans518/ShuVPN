use super::*;
use crate::auth::newsso::oauth::tests::{Fixture, Scenario};
use anyhow::{ensure, Context};
use std::sync::atomic::Ordering;

const COOKIES: &str = r#"[{"name":"SHU_OAUTH2","value":"legacy-secret","path":"/"}]"#;
const LEGACY: &str = r#"{"accountId":"25123456","profile":{"name":"legacy-name","studentId":"25123456","college":"college","sid":"not-copied","password":"not-copied"},"credentials":{"jwxt":"available","atrust":"refreshing","otp":"missing","unknown":"available"},"verifiedAt":"2026-01-01T00:00:00Z","code":"not-copied"}"#;

#[tokio::test]
async fn password_sign_in_completes_credentials_once_and_stops_at_real_verification(
) -> anyhow::Result<()> {
    use crate::auth::newsso::tests::{Fixture as LoginFixture, Scenario as LoginScenario};
    for (scenario, challenge) in [
        (LoginScenario::Direct, false),
        (LoginScenario::Challenge, true),
    ] {
        let fixture = LoginFixture::new(scenario).await?;
        let account = Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?;
        let result = account
            .sign_in_password("student".into(), "password".into())
            .await?;
        if challenge {
            ensure!(result == model::SignInState::VerificationRequired);
            ensure!(!account.snapshot()?.signed_in && !account.snapshot()?.busy);
            ensure!(!fixture
                .requests
                .lock()
                .await
                .iter()
                .any(|request| request.path.starts_with("/Default.aspx")));
            account.send_code("sms".into()).await?;
            ensure!(account
                .sign_in_code("sms".into(), "bad".into())
                .await
                .is_err());
            ensure!(account.verification_state()?.is_some());
            ensure!(!fixture
                .requests
                .lock()
                .await
                .iter()
                .any(|request| request.path.starts_with("/Default.aspx")));
            account.sign_in_code("sms".into(), "good".into()).await?;
            ensure!(account
                .sign_in_code("sms".into(), "good".into())
                .await
                .is_err());
        } else {
            ensure!(result == model::SignInState::Completed);
        }
        let snapshot = account.snapshot()?;
        ensure!(snapshot.signed_in && snapshot.available_count == 1 && !snapshot.busy);
        ensure!(account.verify_if_stale().await?);
        ensure!(
            fixture
                .requests
                .lock()
                .await
                .iter()
                .filter(|request| request.path.starts_with("/Default.aspx"))
                .count()
                == 1
        );
    }
    Ok(())
}

#[tokio::test]
async fn unified_sign_in_reports_no_credentials_without_discarding_adopted_sso(
) -> anyhow::Result<()> {
    use crate::auth::newsso::tests::{Fixture as LoginFixture, Scenario as LoginScenario};
    for (scenario, challenge) in [
        (LoginScenario::NoCredentials, false),
        (LoginScenario::ChallengeNoCredentials, true),
    ] {
        let fixture = LoginFixture::new(scenario).await?;
        let account = Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?;
        let result = if challenge {
            account
                .sign_in_password("student".into(), "password".into())
                .await?;
            account.sign_in_code("sms".into(), "good".into()).await
        } else {
            account
                .sign_in_password("student".into(), "password".into())
                .await
                .map(|_| ())
        };
        ensure!(result
            .err()
            .is_some_and(|error| error.code == "credentialsUnavailable"));
        let snapshot = account.snapshot()?;
        ensure!(snapshot.signed_in && snapshot.available_count == 0 && !snapshot.busy);
        ensure!(snapshot.error_message.as_deref() == Some("凭据交换失败，请重新登录"));
        ensure!(account.verification_state()?.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn unified_sign_in_holds_one_operation_and_cancels_identity_or_credentials(
) -> anyhow::Result<()> {
    use crate::auth::newsso::tests::{Fixture as LoginFixture, Scenario as LoginScenario};
    for (scenario, challenge, adopted) in [
        (LoginScenario::SlowFirst, false, false),
        (LoginScenario::SlowCredentials, false, true),
        (LoginScenario::SlowVerification, true, false),
        (LoginScenario::ChallengeSlowCredentials, true, true),
    ] {
        let fixture = LoginFixture::new(scenario).await?;
        let account = Arc::new(Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?);
        if challenge {
            ensure!(
                account
                    .sign_in_password("student".into(), "password".into())
                    .await?
                    == model::SignInState::VerificationRequired
            );
        }
        let caller = account.clone();
        let job = tokio::spawn(async move {
            if challenge {
                caller.sign_in_code("sms".into(), "good".into()).await
            } else {
                caller
                    .sign_in_password("student".into(), "password".into())
                    .await
                    .map(|_| ())
            }
        });
        tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
        ensure!(account
            .complete_sign_in()
            .await
            .err()
            .is_some_and(|error| error.code == "busy"));
        account.cancel_work()?;
        fixture.release.notify_one();
        ensure!(job
            .await?
            .err()
            .is_some_and(|error| error.code == "cancelled"));
        let snapshot = account.snapshot()?;
        ensure!(!snapshot.busy && snapshot.signed_in == adopted && snapshot.available_count == 0);
    }
    Ok(())
}

#[tokio::test]
async fn qr_sign_in_owns_all_stages_and_accepts_partial_credentials_once() -> anyhow::Result<()> {
    use crate::auth::newsso::qr::tests::{Fixture as QrFixture, Scenario as QrScenario};
    let fixture = QrFixture::new(QrScenario::Success, None).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    let mut phases = Vec::new();
    let mut completion_locked = false;
    ensure!(
        account
            .sign_in_qr(|event| {
                phases.push(event.phase);
                if event.phase == model::QrSignInPhase::Completed {
                    completion_locked =
                        account.view.try_lock().is_err() && account.operation.try_lock().is_err();
                }
                event.image.is_some() == (event.phase == model::QrSignInPhase::Waiting)
            })
            .await?
    );
    ensure!(
        completion_locked,
        "completion escaped the account operation"
    );
    ensure!(
        phases
            == vec![
                model::QrSignInPhase::Waiting,
                model::QrSignInPhase::Redeeming,
                model::QrSignInPhase::Credentials,
                model::QrSignInPhase::Completed
            ]
    );
    ensure!(fixture.count("/oauth/wecom/qrcode").await == 1);
    ensure!(fixture.count("/Default.aspx").await == 1);
    let snapshot = account.snapshot()?;
    ensure!(snapshot.signed_in && snapshot.available_count == 1 && !snapshot.busy);
    account.cancel_qr()?;
    ensure!(account.snapshot()?.signed_in);
    Ok(())
}

#[tokio::test]
async fn qr_sign_in_cancellation_at_each_stage_rejects_late_commit() -> anyhow::Result<()> {
    use crate::auth::newsso::qr::tests::{Fixture as QrFixture, Scenario as QrScenario};
    for (scenario, adopted) in [
        (QrScenario::SlowImage, false),
        (QrScenario::SlowPoll, false),
        (QrScenario::SlowRedeem, false),
        (QrScenario::SlowCredentials, true),
    ] {
        let fixture = QrFixture::new(scenario, None).await?;
        let account = Arc::new(Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?);
        let caller = account.clone();
        let job = tokio::spawn(async move { caller.sign_in_qr(|_| true).await });
        tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
        ensure!(account.snapshot()?.busy);
        ensure!(account
            .complete_sign_in()
            .await
            .err()
            .is_some_and(|error| error.code == "busy"));
        account.cancel_qr()?;
        fixture.release.notify_one();
        ensure!(job
            .await?
            .err()
            .is_some_and(|error| error.code == "cancelled"));
        let snapshot = account.snapshot()?;
        ensure!(snapshot.signed_in == adopted && !snapshot.busy && snapshot.available_count == 0);
        ensure!(snapshot.qr_status.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn qr_sign_in_stops_on_expiry_and_preserves_login_when_all_credentials_fail(
) -> anyhow::Result<()> {
    use crate::auth::newsso::qr::tests::{Fixture as QrFixture, Scenario as QrScenario};
    let expired = QrFixture::new(QrScenario::Expired, None).await?;
    let account = Account::adopt(
        expired.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    ensure!(account
        .sign_in_qr(|_| true)
        .await
        .err()
        .is_some_and(|error| error.code == "qrExpired"));
    ensure!(expired.count("/oauth/wecom/qrcode").await == 0);
    let failed = QrFixture::new(QrScenario::FailedCredentials, None).await?;
    let account = Account::adopt(
        failed.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    ensure!(!account.sign_in_qr(|_| true).await?);
    account.cancel_qr()?;
    ensure!(account.snapshot()?.signed_in && account.snapshot()?.available_count == 0);
    Ok(())
}

#[tokio::test]
async fn qr_sign_in_closed_consumer_cancels_before_redeeming() -> anyhow::Result<()> {
    use crate::auth::newsso::qr::tests::{Fixture as QrFixture, Scenario as QrScenario};
    let fixture = QrFixture::new(QrScenario::Success, None).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    ensure!(account
        .sign_in_qr(|_| false)
        .await
        .err()
        .is_some_and(|error| error.code == "cancelled"));
    ensure!(fixture.count("/wwopen/sso/l/qrConnect").await == 0);
    ensure!(!account.snapshot()?.signed_in && !account.snapshot()?.busy);
    Ok(())
}

#[tokio::test]
async fn qr_sign_in_cancel_preserves_redeem_committed_before_view_marker() -> anyhow::Result<()> {
    use crate::auth::newsso::qr::tests::{Fixture as QrFixture, Scenario as QrScenario};
    let fixture = QrFixture::new(QrScenario::Success, None).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    account.start_qr().await?;
    account.view()?.qr_flow = true;
    fixture.client.poll_qr().await?;
    fixture.client.poll_qr().await?;
    fixture.client.redeem_qr().await?;
    ensure!(account.view()?.qr_active);
    account.cancel_qr()?;
    ensure!(account.snapshot()?.signed_in && !account.snapshot()?.busy);
    ensure!(!account.view()?.qr_active && !account.view()?.qr_flow);
    Ok(())
}

#[tokio::test]
async fn verification_delivery_enforces_cooldown_across_method_changes_and_relogin(
) -> anyhow::Result<()> {
    use crate::auth::newsso::tests::{Fixture as LoginFixture, Scenario as LoginScenario};
    let fixture = LoginFixture::new(LoginScenario::Challenge).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    account.login("student".into(), "password".into()).await?;
    let state = account
        .verification_state()?
        .context("missing verification")?;
    ensure!(state.selected_method == "wecom" && state.resend_after_seconds == 0);
    account.send_code("wecom".into()).await?;
    account.select_verification_method("sms".into())?;
    ensure!(
        account
            .verification_state()?
            .context("missing state")?
            .resend_after_seconds
            == 0
    );
    account.select_verification_method("wecom".into())?;
    ensure!(
        matches!(account.send_code("wecom".into()).await, Err(error) if error.code == "sendCooldown")
    );
    account.cancel_work()?;
    account.login("student".into(), "password".into()).await?;
    ensure!(
        account
            .verification_state()?
            .context("missing state")?
            .resend_after_seconds
            > 0
    );
    ensure!(
        matches!(account.send_code("wecom".into()).await, Err(error) if error.code == "sendCooldown")
    );
    ensure!(
        fixture
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request.path == "/oauth/twoStep/send")
            .count()
            == 1
    );
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(60)).await;
    ensure!(
        account
            .verification_state()?
            .context("missing state")?
            .resend_after_seconds
            == 0
    );
    tokio::time::resume();
    account.send_code("wecom".into()).await?;
    ensure!(
        fixture
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request.path == "/oauth/twoStep/send")
            .count()
            == 2
    );
    account.verify_code("wecom".into(), "good".into()).await?;
    ensure!(account.verification_state()?.is_none());
    Ok(())
}

#[tokio::test]
async fn rejected_verification_delivery_selects_alternate_but_never_automatically_sends(
) -> anyhow::Result<()> {
    use crate::auth::newsso::tests::{Fixture as LoginFixture, Scenario as LoginScenario};
    let fixture = LoginFixture::new(LoginScenario::SendRejected).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    account.login("student".into(), "password".into()).await?;
    ensure!(
        matches!(account.send_code("wecom".into()).await, Err(error) if error.code == "senderror")
    );
    let state = account
        .verification_state()?
        .context("missing verification")?;
    ensure!(state.selected_method == "sms" && state.resend_after_seconds == 0);
    ensure!(
        fixture
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request.path == "/oauth/twoStep/send")
            .count()
            == 1
    );
    account.select_verification_method("wecom".into())?;
    ensure!(
        matches!(account.send_code("wecom".into()).await, Err(error) if error.code == "sendCooldown")
    );
    Ok(())
}

#[tokio::test]
async fn cancelling_an_inflight_delivery_keeps_its_reservation_and_rejects_concurrent_selection(
) -> anyhow::Result<()> {
    use crate::auth::newsso::tests::{Fixture as LoginFixture, Scenario as LoginScenario};
    let fixture = LoginFixture::new(LoginScenario::SlowSend).await?;
    let account = Arc::new(Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?);
    account.login("student".into(), "password".into()).await?;
    let task = tokio::spawn({
        let account = account.clone();
        async move { account.send_code("wecom".into()).await }
    });
    tokio::time::timeout(Duration::from_secs(5), fixture.entered.notified()).await?;
    ensure!(
        matches!(account.select_verification_method("sms".into()), Err(error) if error.code == "busy")
    );
    account.cancel_work()?;
    ensure!(matches!(task.await?, Err(error) if error.code == "cancelled"));
    ensure!(
        account
            .verification_state()?
            .context("missing state")?
            .resend_after_seconds
            > 0
    );
    ensure!(
        matches!(account.send_code("wecom".into()).await, Err(error) if error.code == "sendCooldown")
    );
    fixture.release.notify_one();
    ensure!(
        fixture
            .requests
            .lock()
            .await
            .iter()
            .filter(|request| request.path == "/oauth/twoStep/send")
            .count()
            == 1
    );
    Ok(())
}

#[tokio::test]
async fn credential_deadline_clears_pending_state_keeps_login_and_allows_retry(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::OtpDirect).await?;
    let account = Arc::new(Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?);
    let gate = fixture.client.operation.lock().await;
    let caller = account.clone();
    let job = tokio::spawn(async move { caller.complete_sign_in().await });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if account
                .snapshot()?
                .credentials
                .iter()
                .any(|item| item.state == CredentialState::Refreshing)
            {
                break Ok::<_, AuthFailure>(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(90)).await;
    let failure = job
        .await?
        .err()
        .context("stalled credential round succeeded")?;
    tokio::time::resume();
    ensure!(
        failure.code == "timeout",
        "native deadline returned the wrong failure"
    );
    let snapshot = account.snapshot()?;
    ensure!(
        snapshot.signed_in && !snapshot.busy && snapshot.progress_label.is_none(),
        "deadline damaged login or retained busy state"
    );
    ensure!(
        snapshot
            .credentials
            .iter()
            .all(|item| item.state != CredentialState::Refreshing),
        "deadline retained a refreshing credential"
    );
    ensure!(
        snapshot
            .credentials
            .iter()
            .any(|item| item.error_code.as_deref() == Some("timeout")),
        "interrupted credential lost its failure"
    );
    drop(gate);
    ensure!(
        account.complete_sign_in().await?,
        "timed-out round retained its operation lease"
    );
    ensure!(
        account.snapshot()?.available_count == 3,
        "retry did not verify all systems"
    );
    Ok(())
}

#[test]
fn bundle_adoption_restores_identity_cookie_and_display_then_logout_keeps_device(
) -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let account = Account::open(
        directory.clone(),
        COOKIES,
        Some("stable-device".into()),
        LEGACY,
    )?;
    let snapshot = account.snapshot()?;
    ensure!(
        snapshot.signed_in
            && snapshot.account_id.as_deref() == Some("25123456")
            && snapshot.profile.is_some(),
        "migration display missing"
    );
    ensure!(
        snapshot.available_count == 1 && snapshot.credentials[1].state == CredentialState::Unknown,
        "legacy state handling wrong"
    );
    let stored = account
        .client
        .state()?
        .store
        .as_ref()
        .context("missing store")?
        .load()?
        .context("missing record")?;
    ensure!(
        stored.version == 2
            && stored.device_id.as_deref() == Some("stable-device")
            && stored.legacy_imported
            && !stored.import_open,
        "bundle not committed"
    );
    ensure!(
        !serde_json::to_string(&stored.account)?.contains("not-copied"),
        "legacy secrets persisted as display"
    );
    account.close()?;
    let reopened = Account::open(
        directory.clone(),
        "bad ignored",
        Some("wrong-device".into()),
        "{}",
    )?;
    ensure!(
        reopened.device_id == "stable-device"
            && reopened.snapshot()?.account_id == snapshot.account_id,
        "reopen changed identity or display"
    );
    reopened.sign_out()?;
    reopened.close()?;
    let logged_out = Account::open(directory, COOKIES, Some("wrong-device".into()), LEGACY)?;
    ensure!(
        !logged_out.snapshot()?.signed_in
            && logged_out.snapshot()?.profile.is_none()
            && logged_out.device_id == "stable-device",
        "logout resurrected old bundle"
    );
    Ok(())
}

#[test]
fn failed_bundle_adoption_publishes_nothing_and_can_retry_atomically() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let client = Arc::new(Client::open(root.path().to_string_lossy().into_owned())?);
    client
        .state()?
        .store
        .as_ref()
        .context("missing store")?
        .fail_writes
        .store(true, Ordering::Release);
    ensure!(
        client
            .initialize_account(COOKIES, Some("old-device".into()), LEGACY)
            .err()
            .is_some_and(|error| error.code == "ssoStorage"),
        "write failure hidden"
    );
    {
        let state = client.state()?;
        ensure!(
            state.device_id.is_none()
                && state.account.is_none()
                && state.import_open
                && !state.legacy_imported
                && state.jar.count() == 0,
            "partial adoption published"
        );
        ensure!(
            state
                .store
                .as_ref()
                .context("missing store")?
                .load()?
                .is_none(),
            "failed bundle changed disk"
        );
        state
            .store
            .as_ref()
            .context("missing store")?
            .fail_writes
            .store(false, Ordering::Release);
    }
    let account = Account::adopt(client, COOKIES, Some("old-device".into()), LEGACY)?;
    ensure!(
        account.snapshot()?.signed_in && account.device_id == "old-device",
        "retry did not adopt complete bundle"
    );
    Ok(())
}

#[test]
fn failed_logout_keeps_old_disk_bundle_and_successful_retry_closes_it() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let account = Account::open(directory.clone(), COOKIES, Some("stable".into()), LEGACY)?;
    account
        .client
        .state()?
        .store
        .as_ref()
        .context("missing store")?
        .fail_writes
        .store(true, Ordering::Release);
    ensure!(
        account
            .sign_out()
            .err()
            .is_some_and(|error| error.code == "ssoStorage")
            && !account.snapshot()?.signed_in,
        "failed logout falsely succeeded"
    );
    {
        let state = account.client.state()?;
        let disk = state
            .store
            .as_ref()
            .context("missing store")?
            .load()?
            .context("missing record")?;
        ensure!(
            !disk.cookies.is_empty()
                && disk.account.is_some()
                && disk.device_id.as_deref() == Some("stable"),
            "failed logout partially changed disk bundle"
        );
        state
            .store
            .as_ref()
            .context("missing store")?
            .fail_writes
            .store(false, Ordering::Release);
    }
    account.sign_out()?;
    account.close()?;
    let reopened = Account::open(directory, COOKIES, None, LEGACY)?;
    ensure!(
        !reopened.snapshot()?.signed_in && reopened.device_id == "stable",
        "logout retry not durable"
    );
    Ok(())
}

#[tokio::test]
async fn native_account_verifies_all_systems_and_caches_only_display() -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::OtpDirect).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    ensure!(account.complete_sign_in().await?, "credential round failed");
    let snapshot = account.snapshot()?;
    ensure!(
        snapshot.available_count == 3
            && !snapshot.busy
            && !snapshot.needs_reverification
            && snapshot.account_id.as_deref() == Some("25123456"),
        "native account state incomplete"
    );
    let value = account
        .client
        .state()?
        .account
        .clone()
        .context("missing cached display")?;
    let raw = serde_json::to_string(&value)?;
    ensure!(
        !raw.contains("resource-sid") && !raw.contains("012345") && !raw.contains("sign_key"),
        "secrets cached in account display"
    );
    ensure!(
        account.verify_if_stale().await?,
        "fresh cached round rejected"
    );
    Ok(())
}

#[tokio::test]
async fn logout_prevents_late_verification_from_publishing_profile_or_busy_state(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::SlowCallback).await?;
    let account = Arc::new(Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?);
    let caller = account.clone();
    let job = tokio::spawn(async move { caller.complete_sign_in().await });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    account.sign_out()?;
    fixture.release.notify_one();
    ensure!(
        job.await?
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late account round survived"
    );
    let snapshot = account.snapshot()?;
    ensure!(
        !snapshot.signed_in
            && !snapshot.busy
            && snapshot.profile.is_none()
            && snapshot.available_count == 0,
        "late round republished identity"
    );
    Ok(())
}

#[test]
fn stale_legacy_profile_is_not_used_without_matching_session_and_device_is_generated_once(
) -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let account = Account::open(directory.clone(), "[]", None, LEGACY)?;
    let id = account.device_id.clone();
    ensure!(
        id.len() == 32 && !account.snapshot()?.signed_in && account.snapshot()?.profile.is_none(),
        "fresh account adopted orphan profile"
    );
    account.close()?;
    let reopened = Account::open(directory, COOKIES, None, LEGACY)?;
    ensure!(
        reopened.device_id == id && !reopened.snapshot()?.signed_in,
        "generated identity changed or old key resurrected"
    );
    Ok(())
}

#[tokio::test]
async fn review_regression_zero_available_credentials_never_become_cached_success(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::AllSystemsFail).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    ensure!(
        !account.complete_sign_in().await?,
        "failed round reported success"
    );
    ensure!(
        !account.verify_if_stale().await?,
        "zero-available round became cached success"
    );
    Ok(())
}

#[tokio::test]
async fn review_regression_cancelling_qr_creation_discards_late_image_and_qr() -> anyhow::Result<()>
{
    use crate::auth::newsso::qr::tests::{Fixture as QrFixture, Scenario as QrScenario};
    let fixture = QrFixture::new(QrScenario::SlowImage, None).await?;
    let account = Arc::new(Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?);
    let caller = account.clone();
    let job = tokio::spawn(async move { caller.start_qr().await });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    account.cancel_qr()?;
    fixture.release.notify_one();
    ensure!(
        job.await?
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "late QR survived cancellation during creation"
    );
    ensure!(
        account.client.snapshot()?.qr_status.is_none() && !account.snapshot()?.busy,
        "late QR published state"
    );
    Ok(())
}

#[test]
fn replacement_then_sync_error_is_reported_and_reopen_recovers_whole_new_bundle(
) -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let directory = root.path().to_string_lossy().into_owned();
    let client = Client::open(directory.clone())?;
    client
        .state()?
        .store
        .as_ref()
        .context("missing store")?
        .fail_after_replace
        .store(true, Ordering::Release);
    ensure!(
        client
            .initialize_account(COOKIES, Some("stable".into()), LEGACY)
            .err()
            .is_some_and(|error| error.code == "ssoStorage"),
        "post-replacement error hidden"
    );
    {
        let state = client.state()?;
        ensure!(
            state.device_id.is_none() && state.account.is_none() && state.import_open,
            "failed adoption published in memory"
        );
        let committed = state
            .store
            .as_ref()
            .context("missing store")?
            .load()?
            .context("missing new record")?;
        ensure!(
            committed.device_id.as_deref() == Some("stable")
                && committed.account.is_some()
                && !committed.cookies.is_empty()
                && !committed.import_open,
            "post-replacement disk bundle was partial"
        );
    }
    client.close()?;
    let recovered = Account::open(
        directory,
        "bad ignored",
        Some("must-not-override".into()),
        "{}",
    )?;
    ensure!(
        recovered.device_id == "stable"
            && recovered.snapshot()?.account_id.as_deref() == Some("25123456")
            && recovered.snapshot()?.signed_in,
        "whole committed bundle not recoverable"
    );
    Ok(())
}

#[tokio::test]
async fn disposing_successful_qr_page_does_not_log_out_adopted_account() -> anyhow::Result<()> {
    use crate::auth::newsso::qr::tests::{Fixture as QrFixture, Scenario as QrScenario};
    let fixture = QrFixture::new(QrScenario::Success, None).await?;
    let account = Account::adopt(
        fixture.client.clone(),
        "[]",
        Some("owned-device".into()),
        "{}",
    )?;
    account.start_qr().await?;
    account.poll_qr().await?;
    account.poll_qr().await?;
    account.redeem_qr().await?;
    account.cancel_qr()?;
    ensure!(
        account.snapshot()?.signed_in,
        "QR page disposal logged out successful account"
    );
    Ok(())
}

#[tokio::test]
async fn connection_attempt_prepares_from_account_owned_device_and_validates_origin(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::Normal).await?;
    let origin = fixture.client.config.system_base("atrust")?.to_string();
    let account = crate::api::account::AccountSession {
        inner: Arc::new(Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?),
    };
    let attempt = crate::api::vpn::VpnConnectAttempt::new();
    ensure!(
        attempt
            .prepare_account(&account, "https://foreign.example/".into(), false)
            .await
            .err()
            .is_some_and(|error| error.code == "atrustOrigin"),
        "foreign gateway accepted"
    );
    let plan = attempt.prepare_account(&account, origin, false).await?;
    let session = attempt.connect_prepared(&plan, 3000).await?;
    ensure!(
        session.snapshot().virtual_address.as_deref() == Some("10.99.0.2"),
        "account plan did not connect"
    );
    session.stop().await?;
    Ok(())
}

#[tokio::test]
async fn connection_attempt_cancel_during_account_resource_preparation_keeps_login(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::ResourceSlow).await?;
    let origin = fixture.client.config.system_base("atrust")?.to_string();
    let account = Arc::new(crate::api::account::AccountSession {
        inner: Arc::new(Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?),
    });
    let attempt = Arc::new(crate::api::vpn::VpnConnectAttempt::new());
    let caller = attempt.clone();
    let owner = account.clone();
    let job = tokio::spawn(async move { caller.prepare_account(&owner, origin, false).await });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    attempt.cancel();
    ensure!(
        job.await?
            .err()
            .is_some_and(|error| error.code == "cancelled"),
        "preparation ignored attempt cancellation"
    );
    fixture.release.notify_one();
    ensure!(
        account.snapshot()?.signed_in && !account.snapshot()?.busy,
        "connection cancellation changed account lifecycle"
    );
    Ok(())
}

#[tokio::test]
async fn account_connection_establishes_the_tunnel_and_preserves_origin_checks(
) -> anyhow::Result<()> {
    use crate::api::desktop_connection::{ConnectionStartup, DesktopConnectionOwner};
    let fixture = Fixture::new(Scenario::ResourceWarning).await?;
    let origin = fixture.client.config.system_base("atrust")?.to_string();
    let account = crate::api::account::AccountSession {
        inner: Arc::new(Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?),
    };
    let directory = tempfile::tempdir()?;
    let routing =
        crate::api::routing::RoutingManager::open(directory.path().to_string_lossy().into())
            .await?;
    routing
        .apply(
            "automatic_campus: false\nrules: [\"MATCH,REJECT\"]".into(),
            "{}".into(),
            0,
        )
        .await?;
    let owner = DesktopConnectionOwner::isolated();
    owner.claim_frontend()?;
    owner.await_frontend().await?;
    let startup = || ConnectionStartup {
        proxies: crate::api::vpn::ProxyStartup {
            http_enabled: true,
            http_address: "127.0.0.1".into(),
            http_port: 0,
            ..Default::default()
        },
        tun: None,
        recovery_directory: None,
    };
    ensure!(owner
        .connect_account(
            &account,
            &routing,
            "https://foreign.example/".into(),
            3000,
            startup(),
        )
        .await
        .err()
        .is_some_and(|error| error.code == "atrustOrigin"));
    let session = owner
        .connect_account(
            &account,
            &routing,
            origin.trim_start_matches("https://").into(),
            3000,
            startup(),
        )
        .await?;
    ensure!(
        session.snapshot().virtual_address.as_deref() == Some("10.99.0.2"),
        "account connection did not establish the tunnel"
    );
    ensure!(
        !session.preparation_warnings().is_empty(),
        "preparation warnings were lost"
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = session
        .start_proxy(crate::api::vpn::ProxyKind::Http, "127.0.0.1".into(), 0)
        .await?;
    let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", listener.port)).await?;
    socket
        .write_all(b"CONNECT blocked.fixture:443 HTTP/1.1\r\nHost: blocked.fixture:443\r\n\r\n")
        .await?;
    let mut response = vec![0; 1024];
    let read = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut response)).await??;
    ensure!(read > 0);
    let decisions: serde_json::Value = serde_json::from_str(&session.decisions()?)?;
    ensure!(
        decisions
            .as_array()
            .context("missing decisions")?
            .iter()
            .any(|event| event["decision"]["action"] == "REJECT"
                && event["decision"]["version"] == 1),
        "first proxy request did not use the supplied routing owner"
    );
    owner.shutdown().await?;
    ensure!(owner.snapshot()?.closed && !owner.snapshot()?.cleanup_pending);
    Ok(())
}

#[tokio::test]
async fn account_connection_cancels_during_resource_preparation_and_control_handshake(
) -> anyhow::Result<()> {
    for scenario in [Scenario::ResourceSlow, Scenario::ControlSlow] {
        let fixture = Fixture::new(scenario).await?;
        let origin = fixture.client.config.system_base("atrust")?.to_string();
        let account = Arc::new(crate::api::account::AccountSession {
            inner: Arc::new(Account::adopt(
                fixture.client.clone(),
                "[]",
                Some("owned-device".into()),
                "{}",
            )?),
        });
        let attempt = Arc::new(crate::api::vpn::VpnConnectAttempt::new());
        let caller = attempt.clone();
        let owner = account.clone();
        let job =
            tokio::spawn(
                async move { caller.connect_account(&owner, origin, false, 10_000).await },
            );
        tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
        attempt.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), job).await??;
        ensure!(
            result
                .err()
                .is_some_and(|failure| failure.code == "cancelled"),
            "combined account connection ignored cancellation"
        );
        fixture.release.notify_one();
        ensure!(
            account.snapshot()?.signed_in && !account.snapshot()?.busy,
            "connection cancellation changed account login"
        );
    }
    Ok(())
}

#[tokio::test]
async fn account_connection_native_deadline_cancels_resource_preparation_without_logout(
) -> anyhow::Result<()> {
    let fixture = Fixture::new(Scenario::ResourceSlow).await?;
    let origin = fixture.client.config.system_base("atrust")?.to_string();
    let account = Arc::new(crate::api::account::AccountSession {
        inner: Arc::new(Account::adopt(
            fixture.client.clone(),
            "[]",
            Some("owned-device".into()),
            "{}",
        )?),
    });
    let attempt = Arc::new(crate::api::vpn::VpnConnectAttempt::new());
    let caller = attempt.clone();
    let owner = account.clone();
    let job =
        tokio::spawn(async move { caller.connect_account(&owner, origin, false, 1000).await });
    tokio::time::timeout(Duration::from_secs(3), fixture.entered.notified()).await?;
    let result = tokio::time::timeout(Duration::from_secs(2), job).await??;
    ensure!(
        result
            .err()
            .is_some_and(|failure| failure.code == "timeout"),
        "stalled resource preparation ignored the native deadline"
    );
    fixture.release.notify_one();
    ensure!(
        account.snapshot()?.signed_in && !account.snapshot()?.busy,
        "connection timeout changed account login"
    );
    ensure!(
        attempt
            .connect_account(&account, "https://foreign.example/".into(), false, 1000)
            .await
            .err()
            .is_some_and(|failure| failure.code == "cancelled"),
        "timed-out attempt remained reusable"
    );
    Ok(())
}
