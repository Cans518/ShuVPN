//! Own helper restart and socket-option exchanges independently of caller cancellation.
use super::Client;
use crate::platform::{linux_tun, model::PlatformFailure};
use socket2::Socket;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::{
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

type Outcome = Result<(), PlatformFailure>;
enum Job {
    Client(oneshot::Sender<Result<Client, PlatformFailure>>),
    Bind(Socket, Option<String>, oneshot::Sender<Outcome>),
    Stop(oneshot::Sender<Outcome>),
}
struct Worker {
    task: Option<JoinHandle<Outcome>>,
    closing: Option<oneshot::Receiver<Outcome>>,
    acknowledged: bool,
    outcome: Option<Outcome>,
}
pub(crate) struct Broker {
    jobs: mpsc::Sender<Job>,
    stopping: AtomicBool,
    cancellation: CancellationToken,
    worker: Mutex<Worker>,
}
impl Broker {
    pub(crate) async fn prepare() -> Result<Option<Arc<Self>>, PlatformFailure> {
        if linux_tun::privileges_ready()? {
            return Ok(None);
        }
        let client = Client::launch().await?;
        let (jobs, receiver) = mpsc::channel(32);
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(run(client, receiver, cancellation.clone()));
        Ok(Some(Arc::new(Self {
            jobs,
            stopping: AtomicBool::new(false),
            cancellation,
            worker: Mutex::new(Worker {
                task: Some(task),
                closing: None,
                acknowledged: false,
                outcome: None,
            }),
        })))
    }
    async fn send(&self, job: Job) -> Outcome {
        if self.stopping.load(Ordering::Acquire) {
            return Err(closed());
        }
        self.jobs.send(job).await.map_err(|_| closed())
    }
    pub(crate) async fn client(&self) -> Result<Client, PlatformFailure> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Client(reply)).await?;
        result.await.map_err(|_| closed())?
    }
    pub(crate) async fn bind(&self, socket: &Socket, interface: Option<String>) -> Outcome {
        let (reply, result) = oneshot::channel();
        self.send(Job::Bind(socket.try_clone()?, interface, reply))
            .await?;
        result.await.map_err(|_| closed())?
    }
    pub(crate) async fn close(&self) -> Outcome {
        self.stopping.store(true, Ordering::Release);
        let mut worker = self.worker.lock().await;
        if let Some(outcome) = &worker.outcome {
            return duplicate(outcome);
        }
        if !worker.acknowledged && worker.closing.is_none() {
            let (reply, result) = oneshot::channel();
            if self.jobs.send(Job::Stop(reply)).await.is_ok() {
                worker.closing = Some(result);
            }
        }
        if let Some(reply) = worker.closing.as_mut() {
            let outcome = reply.await.map_err(|_| closed());
            worker.closing.take();
            outcome??;
            worker.acknowledged = true;
        }
        let outcome = if let Some(task) = worker.task.as_mut() {
            task.await
                .map_err(|_| PlatformFailure::new("worker", "网络权限调度线程退出失败"))
                .and_then(|outcome| outcome)
        } else {
            Err(closed())
        };
        worker.task.take();
        worker.outcome = Some(outcome);
        duplicate(worker.outcome.as_ref().ok_or_else(closed)?)
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
async fn run(
    mut client: Client,
    mut jobs: mpsc::Receiver<Job>,
    cancellation: CancellationToken,
) -> Outcome {
    loop {
        let job = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return client.shutdown().await,
            job = jobs.recv() => match job {
                Some(job) => job, None => return client.shutdown().await,
            },
        };
        if matches!(job, Job::Stop(_)) {
            if let Job::Stop(reply) = job {
                let outcome = client.shutdown().await;
                let finished = outcome.is_ok();
                let _ = reply.send(outcome);
                if finished {
                    return Ok(());
                }
            }
            continue;
        }
        if client.is_closed() {
            let replacement = match client.shutdown().await {
                Ok(()) => Client::launch().await,
                Err(error) => Err(error),
            };
            match replacement {
                Ok(replacement) => {
                    client = replacement;
                    #[cfg(feature = "tun-probe")]
                    eprintln!("LINUX_TUN_HELPER_REPLACED {}", client.process_id());
                }
                Err(error) => {
                    match job {
                        Job::Client(reply) => {
                            let _ = reply.send(Err(error));
                        }
                        Job::Bind(_, _, reply) => {
                            let _ = reply.send(Err(error));
                        }
                        Job::Stop(_) => unreachable!(),
                    }
                    continue;
                }
            }
        }
        match job {
            Job::Client(reply) => {
                let _ = reply.send(Ok(client.clone()));
            }
            Job::Bind(socket, interface, reply) => {
                let _ = reply.send(client.bind_socket(&socket, interface).await);
            }
            Job::Stop(_) => unreachable!(),
        }
    }
}
fn duplicate(outcome: &Outcome) -> Outcome {
    outcome
        .as_ref()
        .map(|_| ())
        .map_err(|error| PlatformFailure::new(&error.code, &error.message))
}
fn closed() -> PlatformFailure {
    PlatformFailure::new("closed", "网络权限调度已停止")
}
