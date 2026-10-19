use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

macro_rules! core_error {
    ($($source:ty),+ $(,)?) => {$(
        impl From<$source> for CommandError {
            fn from(error: $source) -> Self {
                Self { code: error.code, message: error.message }
            }
        }
    )+};
}

core_error!(
    shuvpn_core::api::account::AuthFailure,
    shuvpn_core::api::settings::SettingsFailure,
    shuvpn_core::api::routing::RoutingFailure,
    shuvpn_core::api::vpn::VpnFailure,
);

pub type CommandResult<T> = Result<T, CommandError>;
