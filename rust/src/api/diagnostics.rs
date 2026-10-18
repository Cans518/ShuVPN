use crate::frb_generated::StreamSink;

/// This is bridge health, separate from VPN connection state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreEventKind {
    Ready,
    Log,
    Pong,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreEvent {
    pub kind: CoreEventKind,
    pub message: String,
}

#[flutter_rust_bridge::frb(init)]
pub fn init_core() {
    flutter_rust_bridge::setup_default_user_utils();
}

pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// A finite stream makes the first FFI check independent of a network or account.
/// The nonce is returned only as data, never written to the application log.
pub fn probe_core(nonce: String, sink: StreamSink<CoreEvent>) {
    for event in probe_events(nonce) {
        if sink.add(event).is_err() {
            break;
        }
    }
}

fn probe_events(nonce: String) -> [CoreEvent; 3] {
    [
        CoreEvent {
            kind: CoreEventKind::Ready,
            message: core_version(),
        },
        CoreEvent {
            kind: CoreEventKind::Log,
            message: "Rust core initialized".to_owned(),
        },
        CoreEvent {
            kind: CoreEventKind::Pong,
            message: nonce,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_emits_ready_log_and_correlated_reply() {
        let events = probe_events("ffi-roundtrip".to_owned());
        assert_eq!(events[0].kind, CoreEventKind::Ready);
        assert_eq!(events[0].message, env!("CARGO_PKG_VERSION"));
        assert_eq!(events[1].kind, CoreEventKind::Log);
        assert_eq!(events[2].kind, CoreEventKind::Pong);
        assert_eq!(events[2].message, "ffi-roundtrip");
        assert!(!events[1].message.contains("ffi-roundtrip"));
    }
}
