use super::{codec::Tuple, tcp_stack::Stack};
use crate::connection::{model::VpnFailure, session::Shared, tunnel};
use crate::routing::{engine::Evaluation, model::Network, store::Snapshot};
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

const CHUNK: usize = 16 * 1024;
enum Write {
    Bytes(Vec<u8>),
    Finish,
}
enum Read {
    Bytes(Vec<u8>),
    Finished,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn serve(
    shared: &Shared,
    tuple: Tuple,
    snapshot: Arc<Snapshot>,
    mut incoming: mpsc::Receiver<Vec<u8>>,
    outgoing: mpsc::Sender<Vec<u8>>,
    interface: Option<&str>,
    cancellation: CancellationToken,
    binding: crate::connection::capture::Binding,
) -> Result<(), VpnFailure> {
    let mut stack = Stack::new(tuple, shared.plan.mtu)?;
    let first = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Ok(()),
        packet = incoming.recv() => packet,
    };
    let Some(first) = first else {
        return Ok(());
    };
    stack.accept(first);
    stack.poll();
    let host = binding
        .host
        .clone()
        .unwrap_or_else(|| tuple.destination.ip().to_string());
    let _lease = binding._lease;
    let record_snapshot = Arc::clone(&snapshot);
    let mut dns_server = None;
    let dns_client =
        if tuple.destination.port() == 53 && shared.dns_capture_full.load(Ordering::Acquire) {
            let (client, server) = tokio::io::duplex(16 * 1024);
            dns_server = Some(server);
            Some(client)
        } else {
            None
        };
    let dial = async {
        if let Some(client) = dns_client {
            Ok(tunnel::Dialed {
                io: Box::new(client) as Box<dyn tunnel::ProxyIo>,
                leftover: Vec::new(),
            })
        } else {
            tunnel::dial_packet(
                shared,
                &host,
                interface,
                snapshot,
                tunnel::PacketIngress {
                    source: tuple.source,
                    destination: tuple.destination,
                    domain_candidates: binding.domain_candidates,
                },
            )
            .await
        }
    };
    tokio::pin!(dial);
    let deadline = tokio::time::Instant::now() + shared.dial_timeout;
    let opened = loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Ok(()),
            _ = tokio::time::sleep_until(deadline) => break Err(VpnFailure::new("timeout", "TUN TCP 拨号超时")),
            result = &mut dial => break result,
            packet = incoming.recv() => {
                let Some(packet) = packet else { return Ok(()); };
                if !stack.accept(packet) { shared.stats.failures.fetch_add(1, Ordering::Relaxed); }
                stack.poll();
                if !stack.socket().is_open() { return Ok(()); }
            }
        }
    };
    let opened = match opened {
        Ok(opened) => opened,
        Err(error) => {
            if error.code == "timeout" {
                record_failure(shared, tuple, &host, &record_snapshot, &error);
            }
            stack.discard_output();
            stack.socket().abort();
            stack.poll();
            emit_cancellable(&mut stack, &outgoing, &cancellation).await?;
            if cancellation.is_cancelled() {
                return Ok(());
            }
            return Err(error);
        }
    };
    // Publish SYN-ACK only after the selected outbound has actually opened.
    // REJECT and failed campus authorization therefore reset the initial SYN.
    emit_cancellable(&mut stack, &outgoing, &cancellation).await?;
    if cancellation.is_cancelled() {
        return Ok(());
    }
    let token = CancellationToken::new();
    let (commands, writes) = mpsc::channel(1);
    let (write_results, mut written) = mpsc::channel(1);
    let (reads, mut received) = mpsc::channel(1);
    let (reader, writer) = tokio::io::split(opened.io);
    let mut tasks = JoinSet::new();
    let read_token = token.clone();
    tasks.spawn(async move {
        let result = tokio::select! {
            biased;
            _ = read_token.cancelled() => Ok(()),
            result = read_stream(reader, reads, opened.leftover) => result,
        };

        result
    });
    let write_token = token.clone();
    tasks.spawn(async move {
        let result = tokio::select! {
            biased;
            _ = write_token.cancelled() => Ok(()),
            result = write_stream(writer, writes, write_results) => result,
        };

        result
    });
    let service = async {
        if let Some(server) = dns_server {
            super::dns_service::tcp(shared, server).await?;
        }
        // Let the relay consume buffered answers and EOF before ending the
        // TCP stack; successful DNS-server completion is not an abort signal.
        std::future::pending::<Result<(), VpnFailure>>().await
    };
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => Ok(()),
        result = relay(
            shared, &mut stack, &mut incoming, &outgoing,
            &commands, &mut written, &mut received, &mut tasks,
        ) => result,
        result = service => result,
    };
    token.cancel();

    let mut result = result;
    while let Some(joined) = tasks.join_next().await {
        if joined.is_err() && result.is_ok() {
            result = Err(VpnFailure::new("worker", "TCP 清理期间工作线程异常退出"));
        }
    }

    if let Err(error) = &result {
        record_failure(shared, tuple, &host, &record_snapshot, error);
        stack.socket().abort();
        stack.poll();
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {},
            _ = emit(&mut stack, &outgoing) => {},
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn relay(
    shared: &Shared,
    stack: &mut Stack,
    incoming: &mut mpsc::Receiver<Vec<u8>>,
    outgoing: &mpsc::Sender<Vec<u8>>,
    commands: &mpsc::Sender<Write>,
    written: &mut mpsc::Receiver<Result<usize, VpnFailure>>,
    received: &mut mpsc::Receiver<Result<Read, VpnFailure>>,
    tasks: &mut JoinSet<Result<(), VpnFailure>>,
) -> Result<(), VpnFailure> {
    let mut timer = tokio::time::interval(Duration::from_millis(10));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut pending: Option<(Vec<u8>, usize)> = None;
    let mut writing = false;
    let mut read_finished = false;
    let mut write_finished = false;
    loop {
        stack.poll();
        emit(stack, outgoing).await?;
        if let Some((bytes, offset)) = pending.as_mut() {
            if stack.socket().can_send() {
                let length = stack
                    .socket()
                    .send_slice(&bytes[*offset..])
                    .map_err(|_| VpnFailure::new("packet", "TCP 发送缓冲已关闭"))?;
                *offset += length;
                shared
                    .stats
                    .download
                    .fetch_add(length as u64, Ordering::Relaxed);
                if *offset == bytes.len() {
                    pending = None;
                }
            }
        }
        if read_finished && pending.is_none() {
            stack.socket().close();
        }
        if !writing && !write_finished && stack.socket().can_recv() {
            let mut bytes = vec![0; CHUNK];
            let length = stack
                .socket()
                .recv_slice(&mut bytes)
                .map_err(|_| VpnFailure::new("packet", "TCP 接收缓冲已关闭"))?;
            bytes.truncate(length);
            commands
                .send(Write::Bytes(bytes))
                .await
                .map_err(|_| closed())?;
            writing = true;
        } else if !writing
            && !write_finished
            && !matches!(
                stack.socket().state(),
                smoltcp::socket::tcp::State::Listen
                    | smoltcp::socket::tcp::State::SynReceived
                    | smoltcp::socket::tcp::State::SynSent
            )
            && !stack.socket().may_recv()
        {
            commands.send(Write::Finish).await.map_err(|_| closed())?;
            writing = true;
            write_finished = true;
        }
        // Poll again after application reads/sends/FIN so window updates and
        // queued data don't wait for an unrelated network event.
        stack.poll();
        emit(stack, outgoing).await?;
        if !stack.socket().is_open() && !writing && write_finished && !stack.socket().can_recv() {
            return Ok(());
        }
        tokio::select! {
            packet = incoming.recv() => {
                let Some(packet) = packet else { return Ok(()); };
                if super::tcp_codec::parse(&packet).is_ok_and(|segment|
                    segment.flags & super::tcp_codec::RST != 0)
                {
                    return Ok(());
                }
                if !stack.accept(packet) { shared.stats.failures.fetch_add(1, Ordering::Relaxed); }
            }
            result = written.recv(), if writing => {
                let length = result.ok_or_else(closed)??;
                shared.stats.upload.fetch_add(length as u64, Ordering::Relaxed);
                writing = false;
            }
            result = received.recv(), if pending.is_none() && !read_finished => {
                match result.ok_or_else(closed)?? {
                    Read::Bytes(bytes) => pending = Some((bytes, 0)),
                    Read::Finished => read_finished = true,
                }
            }
            result = tasks.join_next(), if !tasks.is_empty() => {
                let result = result.ok_or_else(closed)?
                    .map_err(|_| VpnFailure::new("worker", "TCP 流读写线程退出失败"))?;
                result?;
            }
            _ = timer.tick() => {},
        }
    }
}

async fn read_stream(
    mut reader: tokio::io::ReadHalf<Box<dyn tunnel::ProxyIo>>,
    output: mpsc::Sender<Result<Read, VpnFailure>>,
    leftover: Vec<u8>,
) -> Result<(), VpnFailure> {
    if leftover.len() > tunnel::HANDSHAKE_LIMIT {
        return Err(VpnFailure::new("protocol", "TCP 握手剩余数据超过缓冲限制"));
    }
    for chunk in leftover.chunks(CHUNK) {
        output
            .send(Ok(Read::Bytes(chunk.to_vec())))
            .await
            .map_err(|_| closed())?;
    }
    loop {
        let mut bytes = vec![0; CHUNK];
        match reader.read(&mut bytes).await {
            Ok(0) => {
                output
                    .send(Ok(Read::Finished))
                    .await
                    .map_err(|_| closed())?;
                return Ok(());
            }
            Ok(length) => {
                bytes.truncate(length);
                output
                    .send(Ok(Read::Bytes(bytes)))
                    .await
                    .map_err(|_| closed())?;
            }
            Err(error) => {
                output.send(Err(error.into())).await.map_err(|_| closed())?;
                return Ok(());
            }
        }
    }
}

async fn write_stream(
    mut writer: tokio::io::WriteHalf<Box<dyn tunnel::ProxyIo>>,
    mut input: mpsc::Receiver<Write>,
    output: mpsc::Sender<Result<usize, VpnFailure>>,
) -> Result<(), VpnFailure> {
    while let Some(command) = input.recv().await {
        let finished = matches!(command, Write::Finish);
        let operation = async {
            match command {
                Write::Bytes(bytes) => writer.write_all(&bytes).await.map(|_| bytes.len()),
                Write::Finish => writer.shutdown().await.map(|_| 0),
            }
        };
        let result = tokio::time::timeout(Duration::from_secs(300), operation)
            .await
            .map_err(|_| VpnFailure::new("timeout", "TCP 写入或半关闭超过等待预算"))
            .and_then(|result| result.map_err(VpnFailure::from));
        let failed = result.is_err();
        output.send(result).await.map_err(|_| closed())?;
        if finished || failed {
            return Ok(());
        }
    }
    Ok(())
}
fn record_failure(
    shared: &Shared,
    tuple: Tuple,
    host: &str,
    snapshot: &Snapshot,
    error: &VpnFailure,
) {
    let mut flow = tuple.flow();
    flow.host = host.to_owned();
    flow.network = Network::Tcp;
    if let Ok(Evaluation::Decided(decision)) = snapshot.evaluate(&shared.plan.routes, &flow) {
        let failed: Result<(), _> = Err(VpnFailure::new(&error.code, &error.message));
        tunnel::record_decision(shared, &flow, decision, &[], &failed);
    } else {
        tunnel::record_setup_failure(shared, &flow, snapshot.info.version, error);
    }
}

async fn emit(stack: &mut Stack, outgoing: &mpsc::Sender<Vec<u8>>) -> Result<(), VpnFailure> {
    while let Some(packet) = stack.packet() {
        outgoing.send(packet).await.map_err(|_| closed())?;
    }
    Ok(())
}
async fn emit_cancellable(
    stack: &mut Stack,
    outgoing: &mpsc::Sender<Vec<u8>>,
    cancellation: &CancellationToken,
) -> Result<(), VpnFailure> {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Ok(()),
        result = emit(stack, outgoing) => result,
    }
}
fn closed() -> VpnFailure {
    VpnFailure::new("closed", "TUN TCP 流已关闭")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_permanently_stalled_writer_reports_timeout_and_releases_its_task(
    ) -> anyhow::Result<()> {
        let (local, _peer) = tokio::io::duplex(1);
        let (_reader, writer) = tokio::io::split(Box::new(local) as Box<dyn tunnel::ProxyIo>);
        let (commands, input) = mpsc::channel(1);
        let (output, mut results) = mpsc::channel(1);
        commands.send(Write::Bytes(vec![0; CHUNK])).await?;
        write_stream(writer, input, output).await?;
        let result = results
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("missing write result"))?;
        assert_eq!(
            result.err().map(|error| error.code),
            Some("timeout".to_owned())
        );
        Ok(())
    }
}
