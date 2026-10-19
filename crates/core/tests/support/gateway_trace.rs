//! Optional, bounded diagnostics for the test gateway, never the application.
use std::collections::VecDeque;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde_json::{json, Value};

pub(super) struct HeartbeatTrace {
    connection: u64,
    started: Instant,
    peer: SocketAddr,
    requests: u64,
    responses: u64,
    last_response: Instant,
    pending_request: Option<Instant>,
    max_response_gap_seconds: f64,
    max_write_seconds: f64,
    events: VecDeque<Value>,
}

impl HeartbeatTrace {
    pub(super) fn new(peer: SocketAddr) -> Self {
        let started = Instant::now();
        static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);
        Self {
            connection: NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed),
            started,
            peer,
            requests: 0,
            responses: 0,
            last_response: started,
            pending_request: None,
            max_response_gap_seconds: 0.0,
            max_write_seconds: 0.0,
            events: VecDeque::new(),
        }
    }

    fn event(&mut self, event: &str) {
        if self.events.len() == 64 {
            self.events.pop_front();
        }
        self.events.push_back(json!({
            "event": event,
            "elapsed_seconds": self.started.elapsed().as_secs_f64(),
            "requests": self.requests,
            "responses": self.responses,
        }));
    }

    pub(super) fn request(&mut self) {
        self.requests += 1;
        self.pending_request = Some(Instant::now());
        self.event("request_received");
    }

    pub(super) fn response_flushed(&mut self) {
        let now = Instant::now();
        self.responses += 1;
        self.max_response_gap_seconds = self
            .max_response_gap_seconds
            .max(now.duration_since(self.last_response).as_secs_f64());
        self.last_response = now;
        if let Some(requested) = self.pending_request.take() {
            self.max_write_seconds = self
                .max_write_seconds
                .max(now.duration_since(requested).as_secs_f64());
        }
        self.event("response_flushed");
    }

    pub(super) fn read_end(&mut self, error: &std::io::Error) {
        self.event(&format!("read_end: {error}"));
    }
}

impl Drop for HeartbeatTrace {
    fn drop(&mut self) {
        // Never panic while unwinding a connection. The verifier rejects missing
        // or truncated requested traces, so a failed write cannot become a pass.
        let _ = writeln!(
            std::io::stderr().lock(),
            "FIXTURE_HEARTBEAT_TRACE {}",
            json!({
                "connection": self.connection,
                "peer": self.peer.to_string(),
                "elapsed_seconds": self.started.elapsed().as_secs_f64(),
                "requests": self.requests,
                "responses": self.responses,
                "pending_request": self.pending_request.is_some(),
                "max_response_gap_seconds": self.max_response_gap_seconds,
                "max_write_seconds": self.max_write_seconds,
                "last_response_age_seconds": self.last_response.elapsed().as_secs_f64(),
                "last_events": self.events,
            })
        );
    }
}
