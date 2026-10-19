//! The aTrust tunnel data plane: protocol framing, signing, packet codec,
//! conntrack, route matching, and userspace TCP termination.
//!
//! This crate performs **no I/O and has no platform dependencies**. Bytes go in
//! and bytes come out; transports, timers, and TUN devices live in
//! the calling application.
//!
//! Wire behaviour is pinned by
//! `tests/fixtures/native_atrust.json`. The frozen reference vectors are
//! consumed directly by Rust tests without another language toolchain.

pub mod crypto;
pub mod error;
pub mod flow;
pub mod json;
pub mod l3;
pub mod packet;
pub mod plan;
pub mod plane;
pub mod relay;
pub mod route;
pub mod tcp_tunnel;
pub mod terminator;

pub use error::{Error, Result};

/// Protocol version byte shared by the L3 and TCP-tunnel framings.
pub const PROTOCOL_VERSION: u8 = 0x05;
