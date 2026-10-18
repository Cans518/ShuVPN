mod announcements;
pub mod api;
mod auth;
mod connection;
#[cfg(feature = "egress-probe")]
pub mod egress_probe;
mod network;
mod platform;
mod settings;
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod tun_helper;
mod update;
#[cfg(all(feature = "tun-probe", target_os = "windows"))]
pub use platform::windows_tun::probe::verify as verify_windows_tun;
#[cfg(all(feature = "tun-probe", target_os = "windows"))]
pub use platform::windows_tun::probe::{
    verify_helper as verify_windows_tun_helper,
    verify_helper_disconnect as verify_windows_tun_helper_disconnect,
};
pub mod routing;
// FRB-generated serializers use checked-by-codec unwraps.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod frb_generated;
