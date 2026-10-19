//! Delivery boundary for the standalone desktop network helper.
#[cfg(target_os = "linux")]
pub use crate::platform::linux_tun::helper::{run, Client, Lease};
#[cfg(target_os = "windows")]
pub use crate::platform::windows_tun::helper::{run, Client, Endpoint};
