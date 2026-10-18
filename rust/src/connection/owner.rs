use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot, watch, Notify};
use tokio_util::sync::CancellationToken;

use super::{
    desktop_services::{self, DesktopServicesReport, ProxyStartup, TunStartup},
    model::{VpnFailure, VpnState},
    session::Engine,
};

#[derive(Clone)]
pub(crate) struct Session {
    pub engine: Arc<Engine>,
    pub run_id: u64,
    pub warnings: Vec<String>,
    pub startup: Option<DesktopServicesReport>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerPhase {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

#[derive(Clone)]
pub struct OwnerSnapshot {
    pub generation: u64,
    pub phase: OwnerPhase,
    pub run_id: Option<u64>,
    pub cleanup_pending: bool,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub closed: bool,
}

type ConnectFuture = Pin<Box<dyn Future<Output = Result<Session, VpnFailure>> + Send>>;
pub(crate) type Connect = Box<dyn FnOnce(CancellationToken) -> ConnectFuture + Send>;
type CloseResult = Result<Option<String>, VpnFailure>;

struct CloseFlight {
    result: Mutex<Option<CloseResult>>,
    done: Notify,
}

impl CloseFlight {
    async fn wait(&self) -> CloseResult {
        loop {
            let notified = self.done.notified();
            if let Some(result) = self.result.lock().map_err(|_| failed())?.clone() {
                return result;
            }
            notified.await;
        }
    }
    fn complete(&self, result: CloseResult) {
        if let Ok(mut saved) = self.result.lock() {
            *saved = Some(result);
        }
        self.done.notify_waiters();
    }
}

struct State {
    generation: u64,
    control_generation: u64,
    current: Option<Session>,
    pending: Option<(u64, CancellationToken)>,
    closing: Option<Arc<CloseFlight>>,
    terminal_close: bool,
    clear_error: bool,
    closed: bool,
    worker_failed: bool,
    error: Option<VpnFailure>,
    failed_generation: Option<u64>,
}

struct Shared {
    state: Mutex<State>,
    changed: watch::Sender<OwnerSnapshot>,
    wake: Notify,
}

impl Shared {
    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>, VpnFailure> {
        self.state.lock().map_err(|_| failed())
    }
    fn snapshot(state: &State) -> OwnerSnapshot {
        let event = state.current.as_ref().map(|session| {
            session
                .engine
                .shared
                .snapshot(super::model::VpnEventKind::State, 0.0, 0.0)
        });
        let phase = if state.worker_failed {
            OwnerPhase::Failed
        } else if state.closing.is_some() {
            OwnerPhase::Disconnecting
        } else if state.pending.is_some() {
            OwnerPhase::Connecting
        } else if state.error.is_some() {
            OwnerPhase::Failed
        } else {
            match event.as_ref().map(|event| event.state) {
                Some(VpnState::Connected) => OwnerPhase::Connected,
                Some(VpnState::Failed) => OwnerPhase::Failed,
                _ => OwnerPhase::Disconnected,
            }
        };
        OwnerSnapshot {
            generation: state.generation,
            phase,
            run_id: state.current.as_ref().map(|session| session.run_id),
            cleanup_pending: state.worker_failed
                || (state.closing.is_some()
                    && (state.current.is_some() || state.pending.is_some()))
                || (state.current.is_some()
                    && (state.error.is_some()
                        || event
                            .as_ref()
                            .is_some_and(|event| event.state != VpnState::Connected))),
            error_message: state
                .error
                .as_ref()
                .map(|error| error.message.clone())
                .or_else(|| event.and_then(|event| event.message)),
            error_code: state.error.as_ref().map(|error| error.code.clone()),
            closed: state.closed,
        }
    }
    fn publish(&self, state: &State) {
        self.changed.send_replace(Self::snapshot(state));
    }

    fn close(&self, terminal: bool, clear_error: bool) -> Result<Arc<CloseFlight>, VpnFailure> {
        self.close_control(None, terminal, clear_error)
    }
    fn close_control(
        &self,
        control: Option<u64>,
        terminal: bool,
        clear_error: bool,
    ) -> Result<Arc<CloseFlight>, VpnFailure> {
        let mut state = self.state()?;
        check_control(&state, control)?;
        self.close_locked(&mut state, terminal, clear_error)
    }
    fn close_generation(&self, generation: u64) -> Result<(), VpnFailure> {
        let mut state = self.state()?;
        if state.generation == generation {
            self.close_locked(&mut state, false, false)?;
        }
        Ok(())
    }
    fn close_locked(
        &self,
        state: &mut State,
        terminal: bool,
        clear_error: bool,
    ) -> Result<Arc<CloseFlight>, VpnFailure> {
        if state.worker_failed {
            return Err(failed());
        }
        if state.closed {
            return Ok(Arc::new(CloseFlight {
                result: Mutex::new(Some(Ok(None))),
                done: Notify::new(),
            }));
        }
        if let Some((_, token)) = &state.pending {
            token.cancel();
        }
        if let Some(session) = &state.current {
            session.engine.shared.cancellation.cancel();
        }
        state.terminal_close |= terminal;
        state.clear_error |= clear_error;
        let flight = state
            .closing
            .get_or_insert_with(|| {
                Arc::new(CloseFlight {
                    result: Mutex::new(None),
                    done: Notify::new(),
                })
            })
            .clone();
        self.publish(state);
        self.wake.notify_one();
        Ok(flight)
    }
}

struct Request {
    generation: u64,
    token: CancellationToken,
    connect: Connect,
    proxies: ProxyStartup,
    tun: Option<TunStartup>,
    directory: Option<String>,
    reply: oneshot::Sender<Result<Session, VpnFailure>>,
}

struct Waiting {
    shared: Arc<Shared>,
    generation: u64,
    token: CancellationToken,
    armed: bool,
}
impl Drop for Waiting {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.token.cancel();
        let _ = self.shared.close_generation(self.generation);
    }
}

pub(crate) struct Owner {
    shared: Arc<Shared>,
    requests: mpsc::Sender<Request>,
    worker: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Owner {
    pub(crate) fn spawn() -> Arc<Self> {
        let state = State {
            generation: 0,
            control_generation: 0,
            current: None,
            pending: None,
            closing: None,
            terminal_close: false,
            clear_error: false,
            closed: false,
            worker_failed: false,
            error: None,
            failed_generation: None,
        };
        let (changed, _) = watch::channel(Shared::snapshot(&state));
        let shared = Arc::new(Shared {
            state: Mutex::new(state),
            changed,
            wake: Notify::new(),
        });
        let (requests, receiver) = mpsc::channel(32);
        let worker = tokio::spawn(run(Arc::clone(&shared), receiver));
        Arc::new(Self {
            shared,
            requests,
            worker: tokio::sync::Mutex::new(Some(worker)),
        })
    }

    pub(crate) fn snapshot(&self) -> Result<OwnerSnapshot, VpnFailure> {
        let state = self.shared.state()?;
        Ok(Shared::snapshot(&state))
    }
    pub(crate) fn events(&self) -> watch::Receiver<OwnerSnapshot> {
        self.shared.changed.subscribe()
    }

    pub(crate) async fn connect(
        &self,
        control: Option<u64>,
        connect: Connect,
        proxies: ProxyStartup,
        tun: Option<TunStartup>,
        directory: Option<String>,
    ) -> Result<Session, VpnFailure> {
        let token = CancellationToken::new();
        let generation = {
            let mut state = self.shared.state()?;
            check_control(&state, control)?;
            if state.closed {
                return Err(VpnFailure::new("closed", "连接所有者已经关闭"));
            }
            if state.worker_failed {
                return Err(failed());
            }
            if state.closing.is_some() {
                return Err(VpnFailure::new("busy", "连接清理仍在进行"));
            }
            state.generation = state.generation.checked_add(1).ok_or_else(failed)?;
            if let Some((_, previous)) = &state.pending {
                previous.cancel();
            }
            if let Some(session) = &state.current {
                session.engine.shared.cancellation.cancel();
            }
            state.pending = Some((state.generation, token.clone()));
            self.shared.publish(&state);
            state.generation
        };
        let mut waiting = Waiting {
            shared: Arc::clone(&self.shared),
            generation,
            token: token.clone(),
            armed: true,
        };
        let (reply, response) = oneshot::channel();
        let request = Request {
            generation,
            token: token.clone(),
            connect,
            proxies,
            tun,
            directory,
            reply,
        };
        tokio::select! {
            biased;
            _ = token.cancelled() => return Err(cancelled()),
            sent = self.requests.send(request) => sent.map_err(|_| failed())?,
        }
        let result = response.await.map_err(|_| failed())?;
        waiting.armed = false;
        if token.is_cancelled() {
            let closing = self.shared.state()?.closing.clone();
            if let Some(closing) = closing {
                closing.wait().await?;
            }
            return Err(cancelled());
        }
        result
    }
    pub(crate) fn reserve_control(&self) -> Result<u64, VpnFailure> {
        let mut state = self.shared.state()?;
        if state.worker_failed {
            return Err(failed());
        }
        if state.closed {
            return Err(VpnFailure::new("closed", "连接所有者已经关闭"));
        }
        let control = state.control_generation.checked_add(1).ok_or_else(failed)?;
        let generation = state.generation.checked_add(1).ok_or_else(failed)?;
        state.control_generation = control;
        state.generation = generation;
        self.shared.close_locked(&mut state, false, true)?;
        Ok(control)
    }
    pub(crate) async fn await_control(&self, generation: u64) -> Result<(), VpnFailure> {
        let closing = {
            let state = self.shared.state()?;
            check_control(&state, Some(generation))?;
            state.closing.clone()
        };
        if let Some(closing) = closing {
            closing.wait().await?;
        }
        let state = self.shared.state()?;
        if state.worker_failed {
            return Err(failed());
        }
        if state.closed {
            return Err(VpnFailure::new("closed", "连接所有者已经关闭"));
        }
        check_control(&state, Some(generation))?;
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        Ok(())
    }
    pub(crate) fn cancel(&self, control: Option<u64>) -> Result<(), VpnFailure> {
        self.shared.close_control(control, false, false).map(|_| ())
    }
    pub(crate) async fn disconnect(&self, control: Option<u64>) -> CloseResult {
        self.shared
            .close_control(control, false, true)?
            .wait()
            .await
    }
    pub(crate) async fn shutdown(&self, control: Option<u64>) -> CloseResult {
        let result = self.shared.close_control(control, true, true)?.wait().await;
        if result.is_ok() {
            let mut worker = self.worker.lock().await;
            if let Some(task) = worker.as_mut() {
                task.await.map_err(|_| failed())?;
                worker.take();
            }
        }
        result
    }
}

struct WorkerCompletion {
    shared: Arc<Shared>,
    finished: bool,
}
impl Drop for WorkerCompletion {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.worker_failed = true;
            state.error = Some(failed());
            if let Some((_, token)) = &state.pending {
                token.cancel();
            }
            if let Some(session) = &state.current {
                session.engine.shared.cancellation.cancel();
            }
            if let Some(flight) = state.closing.take() {
                flight.complete(Err(failed()));
            }
            self.shared.publish(&state);
        }
    }
}

async fn cleanup(shared: &Shared) -> CloseResult {
    let session = shared.state()?.current.clone();
    let Some(session) = session else {
        return Ok(None);
    };
    match session.engine.finish_stop().await {
        Ok(diagnostic) => {
            let mut state = shared.state()?;
            if state
                .current
                .as_ref()
                .is_some_and(|current| current.run_id == session.run_id)
            {
                state.current = None;
            }
            Ok(diagnostic)
        }
        Err(error) => {
            let mut state = shared.state()?;
            state.error = Some(error.clone());
            state.failed_generation = Some(state.generation);
            shared.publish(&state);
            Err(error)
        }
    }
}

async fn close_flight(
    shared: &Shared,
    receiver: &mut mpsc::Receiver<Request>,
) -> Result<bool, VpnFailure> {
    let flight = shared.state()?.closing.clone();
    let Some(flight) = flight else {
        return Ok(false);
    };
    let result = cleanup(shared).await;
    while let Ok(request) = receiver.try_recv() {
        request.token.cancel();
        let _ = request.reply.send(Err(cancelled()));
    }
    let closed = {
        let mut state = shared.state()?;
        state.pending = None;
        state.closing = None;
        if result.is_ok() {
            state.current = None;
            state.failed_generation = None;
            if state.clear_error {
                state.error = None;
            }
            state.closed = state.terminal_close;
        }
        state.terminal_close = false;
        state.clear_error = false;
        shared.publish(&state);
        state.closed
    };
    flight.complete(result);
    Ok(closed)
}

fn clear_pending(shared: &Shared, generation: u64) -> Result<(), VpnFailure> {
    let mut state = shared.state()?;
    if state
        .pending
        .as_ref()
        .is_some_and(|(current, _)| *current == generation)
    {
        state.pending = None;
    }
    shared.publish(&state);
    Ok(())
}

async fn connect_request(shared: &Shared, request: Request) -> Result<(), VpnFailure> {
    let Request {
        generation,
        token,
        connect,
        proxies,
        tun,
        directory,
        reply,
    } = request;
    let rejected = {
        let state = shared.state()?;
        state
            .failed_generation
            .filter(|failed| generation <= *failed)
            .and(state.error.clone())
    };
    let result = if token.is_cancelled() || reply.is_closed() {
        Err(cancelled())
    } else if let Some(error) = rejected {
        Err(error)
    } else if let Err(error) = cleanup(shared).await {
        Err(error)
    } else if token.is_cancelled() || reply.is_closed() {
        Err(cancelled())
    } else {
        {
            let mut state = shared.state()?;
            state.error = None;
            state.failed_generation = None;
            shared.publish(&state);
        }
        match (connect)(token.clone()).await {
            Err(error) => Err(error),
            Ok(mut session) => {
                session.engine.bind_authority(token.clone());
                {
                    let mut state = shared.state()?;
                    state.current = Some(session.clone());
                    shared.publish(&state);
                }
                let started = if token.is_cancelled() || reply.is_closed() {
                    Err(cancelled())
                } else {
                    desktop_services::start(&session.engine, proxies, tun, directory).await
                };
                match started {
                    Ok(report) if !token.is_cancelled() && !reply.is_closed() => {
                        session.startup = Some(report);
                        shared.state()?.current = Some(session.clone());
                        Ok(session)
                    }
                    outcome => {
                        let event = session.engine.shared.snapshot(
                            super::model::VpnEventKind::State,
                            0.0,
                            0.0,
                        );
                        let error = if event.state == VpnState::Failed {
                            VpnFailure::new(
                                "connection",
                                event.message.as_deref().unwrap_or("连接已失败"),
                            )
                        } else {
                            outcome.err().unwrap_or_else(cancelled)
                        };
                        session.engine.shared.cancellation.cancel();
                        if shared.state()?.closing.is_none() {
                            match cleanup(shared).await {
                                Ok(_) => Err(error),
                                Err(cleanup) => Err(cleanup),
                            }
                        } else {
                            Err(error)
                        }
                    }
                }
            }
        }
    };
    if let Err(error) = &result {
        let mut state = shared.state()?;
        if error.code != "cancelled" && state.error.is_none() {
            state.error = Some(error.clone());
        }
    }
    clear_pending(shared, generation)?;
    if let Err(undelivered) = reply.send(result) {
        if undelivered.is_ok() {
            if let Some(session) = shared.state()?.current.as_ref() {
                session.engine.shared.cancellation.cancel();
            }
            let _ = shared.close(false, false)?;
        }
    }
    Ok(())
}

async fn run(shared: Arc<Shared>, mut receiver: mpsc::Receiver<Request>) {
    let mut completion = WorkerCompletion {
        shared: Arc::clone(&shared),
        finished: false,
    };
    let outcome: Result<(), VpnFailure> = async {
        loop {
            if close_flight(&shared, &mut receiver).await? { break; }
            let cancelled_session = {
                let state = shared.state()?;
                if state.pending.is_none() && state.error.is_none() {
                    state.current.as_ref().map(|session| session.engine.shared.cancellation.clone())
                } else { None }
            }.unwrap_or_default();
            tokio::select! {
                biased;
                request = receiver.recv() => match request {
                    Some(request) => connect_request(&shared, request).await?,
                    None => { shared.close(true, true)?; }
                },
                _ = shared.wake.notified() => {},
                _ = cancelled_session.cancelled() => {
                    {
                        let mut state = shared.state()?;
                        if let Some(session) = &state.current {
                            let event = session.engine.shared.snapshot(super::model::VpnEventKind::State, 0.0, 0.0);
                            if event.state == VpnState::Failed {
                                state.error = Some(VpnFailure::new("connection", event.message.as_deref().unwrap_or("连接已失败")));
                            }
                        }
                    }
                    shared.close(false, false)?;
                }
            }
        }
        Ok(())
    }.await;
    if outcome.is_ok() {
        completion.finished = true;
    }
}

fn cancelled() -> VpnFailure {
    VpnFailure::new("cancelled", "连接已取消")
}
fn check_control(state: &State, expected: Option<u64>) -> Result<(), VpnFailure> {
    if expected.is_some_and(|generation| generation != state.control_generation) {
        return Err(VpnFailure::new("stale", "连接控制权已移交给新的前端"));
    }
    Ok(())
}
fn failed() -> VpnFailure {
    VpnFailure::new("worker", "原生连接所有者异常，资源清理尚未确认")
}

#[cfg(test)]
mod tests;
