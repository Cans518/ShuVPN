use super::*;
use tokio::sync::oneshot;

async fn ready_peer(
    pipe: &mut tokio::net::windows::named_pipe::NamedPipeClient,
) -> anyhow::Result<()> {
    let request: Request = wire::receive(pipe).await?.context("missing hello")?;
    ensure!(
        request.id == 0 && matches!(request.command, Command::Hello { .. }),
        "wrong hello"
    );
    wire::send(
        pipe,
        &Response {
            id: 0,
            result: Reply::Ready {
                version: VERSION,
                elevated: false,
            },
        },
    )
    .await
}

#[tokio::test]
async fn cancelled_exchange_drops_pipe_and_does_not_claim_confirmed_cleanup() -> anyhow::Result<()>
{
    let endpoint = Endpoint::new()?;
    let arguments = endpoint.arguments();
    #[cfg(feature = "tun-probe")]
    let expected_identity = (std::process::id(), endpoint.created);
    let (observed, received) = oneshot::channel();
    let peer = tokio::spawn(async move {
        let mut pipe = ClientOptions::new().open(&arguments[0])?;
        ready_peer(&mut pipe).await?;
        let request: Request = wire::receive(&mut pipe).await?.context("missing request")?;
        ensure!(matches!(request.command, Command::Info {}), "wrong request");
        let _ = observed.send(());
        ensure!(
            wire::receive::<Request>(&mut pipe).await?.is_none(),
            "cancelled connection remained open"
        );
        Ok::<_, anyhow::Error>(())
    });
    let client = endpoint.accept(std::process::id()).await?;
    #[cfg(feature = "tun-probe")]
    assert_eq!(client.probe_identity()?, expected_identity);
    tokio::select! {
        result = client.info() => anyhow::bail!("request completed without a reply: {result:?}"),
        observed = received => observed?,
    }
    assert!(client.info().await.is_err());
    assert!(client.close().await.is_err());
    tokio::time::timeout(Duration::from_secs(2), peer).await???;
    Ok(())
}

#[tokio::test]
async fn mismatched_response_sequence_permanently_invalidates_client() -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let arguments = endpoint.arguments();
    let peer = tokio::spawn(async move {
        let mut pipe = ClientOptions::new().open(&arguments[0])?;
        ready_peer(&mut pipe).await?;
        let request: Request = wire::receive(&mut pipe).await?.context("missing request")?;
        wire::send(
            &mut pipe,
            &Response {
                id: request.id + 1,
                result: Reply::Info {
                    document: "{}".to_owned(),
                },
            },
        )
        .await?;
        ensure!(
            wire::receive::<Request>(&mut pipe).await?.is_none(),
            "invalid response left pipe open"
        );
        Ok::<_, anyhow::Error>(())
    });
    let client = endpoint.accept(std::process::id()).await?;
    assert!(client.info().await.is_err());
    assert!(client.info().await.is_err());
    tokio::time::timeout(Duration::from_secs(2), peer).await???;
    Ok(())
}

#[tokio::test]
async fn parent_generation_and_invalid_nonce_fail_before_session_creation() -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let mut arguments = endpoint.arguments();
    arguments[2] = (endpoint.created + 1).to_string();
    assert!(run(arguments.to_vec()).await.is_err());

    let arguments = endpoint.arguments();
    let helper = tokio::spawn(run(arguments.to_vec()));
    tokio::time::timeout(Duration::from_secs(2), endpoint.pipe.connect()).await??;
    let mut pipe = endpoint.pipe;
    process::verify_peer(&pipe, std::process::id(), Side::Client)?;
    assert!(process::verify_peer(&pipe, 0, Side::Client).is_err());
    assert!(process::verify_peer(&pipe, std::process::id().wrapping_add(1), Side::Client).is_err());
    wire::send(
        &mut pipe,
        &Request {
            id: 0,
            command: Command::Hello {
                version: VERSION,
                nonce: "incorrect".to_owned(),
            },
        },
    )
    .await?;
    let result = tokio::time::timeout(Duration::from_secs(2), helper).await??;
    assert!(result.is_err());
    assert!(wire::receive::<Response>(&mut pipe).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn helper_rejects_replayed_request_and_finishes_when_client_drops() -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let helper = tokio::spawn(run(endpoint.arguments().to_vec()));
    let client = endpoint.accept(std::process::id()).await?;
    let mut channel = client.channel.lock().await;
    let mut pipe = channel.pipe.take().context("missing pipe")?;
    wire::send(
        &mut pipe,
        &Request {
            id: 0,
            command: Command::Info {},
        },
    )
    .await?;
    assert!(tokio::time::timeout(Duration::from_secs(2), helper)
        .await??
        .is_err());
    drop(channel);
    drop(pipe);
    drop(client);

    let endpoint = Endpoint::new()?;
    let helper = tokio::spawn(run(endpoint.arguments().to_vec()));
    let client = endpoint.accept(std::process::id()).await?;
    drop(client);
    tokio::time::timeout(Duration::from_secs(2), helper).await???;
    Ok(())
}

#[tokio::test]
async fn matching_id_with_wrong_reply_type_invalidates_connection() -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let arguments = endpoint.arguments();
    let peer = tokio::spawn(async move {
        let mut pipe = ClientOptions::new().open(&arguments[0])?;
        ready_peer(&mut pipe).await?;
        let request: Request = wire::receive(&mut pipe).await?.context("missing request")?;
        wire::send(
            &mut pipe,
            &Response {
                id: request.id,
                result: Reply::Done {},
            },
        )
        .await?;
        ensure!(
            wire::receive::<Request>(&mut pipe).await?.is_none(),
            "wrong reply type left pipe open"
        );
        Ok::<_, anyhow::Error>(())
    });
    let client = endpoint.accept(std::process::id()).await?;
    assert!(client.info().await.is_err());
    assert!(client.close().await.is_err());
    tokio::time::timeout(Duration::from_secs(2), peer).await???;
    Ok(())
}

#[tokio::test]
async fn cancelled_close_without_done_does_not_cache_cleanup_success() -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let arguments = endpoint.arguments();
    let (observed, received) = oneshot::channel();
    let peer = tokio::spawn(async move {
        let mut pipe = ClientOptions::new().open(&arguments[0])?;
        ready_peer(&mut pipe).await?;
        let request: Request = wire::receive(&mut pipe).await?.context("missing close")?;
        ensure!(
            matches!(request.command, Command::Close {}),
            "wrong request"
        );
        let _ = observed.send(());
        ensure!(
            wire::receive::<Request>(&mut pipe).await?.is_none(),
            "cancelled close left pipe open"
        );
        Ok::<_, anyhow::Error>(())
    });
    let client = endpoint.accept(std::process::id()).await?;
    tokio::select! {
        result = client.close() => anyhow::bail!("close completed without Done: {result:?}"),
        observed = received => observed?,
    }
    assert!(client.close().await.is_err());
    tokio::time::timeout(Duration::from_secs(2), peer).await???;
    Ok(())
}

#[tokio::test]
async fn failed_close_keeps_the_channel_for_an_acknowledged_cleanup_retry() -> anyhow::Result<()> {
    let endpoint = Endpoint::new()?;
    let arguments = endpoint.arguments();
    let peer = tokio::spawn(async move {
        let mut pipe = ClientOptions::new().open(&arguments[0])?;
        ready_peer(&mut pipe).await?;
        for (id, reply) in [
            (
                1,
                Reply::Failure {
                    code: "cleanup".to_owned(),
                    message: "injected deletion failure".to_owned(),
                },
            ),
            (2, Reply::Done {}),
        ] {
            let request: Request = wire::receive(&mut pipe).await?.context("missing close")?;
            ensure!(request.id == id, "close retry lost sequence");
            ensure!(
                matches!(request.command, Command::Close {}),
                "wrong request"
            );
            wire::send(&mut pipe, &Response { id, result: reply }).await?;
        }
        Ok::<_, anyhow::Error>(())
    });
    let client = endpoint.accept(std::process::id()).await?;
    let error = client
        .close()
        .await
        .err()
        .context("failed close was accepted")?;
    assert_eq!(error.code, "cleanup");
    assert!(!client.channel.lock().await.close_confirmed);
    assert!(matches!(
        client.exchange(Command::Close {}).await?,
        Reply::Done {}
    ));
    assert!(client.channel.lock().await.close_confirmed);
    assert!(matches!(
        client.exchange(Command::Close {}).await?,
        Reply::Done {}
    ));
    tokio::time::timeout(Duration::from_secs(2), peer).await???;
    Ok(())
}
