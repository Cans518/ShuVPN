use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialState {
    Unknown,
    Refreshing,
    Available,
    Missing,
    Expired,
    Failing,
}

#[derive(Clone)]
pub struct Credential {
    pub system_id: String,
    pub state: CredentialState,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    pub name: Option<String>,
    pub student_id: Option<String>,
    pub grade: Option<String>,
    pub college: Option<String>,
    pub major: Option<String>,
    pub class_name: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct Persisted {
    pub account_id: Option<String>,
    pub profile: Option<Profile>,
    pub credentials: BTreeMap<String, CredentialState>,
    pub verified_at: Option<i64>,
}

pub struct Snapshot {
    pub signed_in: bool,
    pub account_id: Option<String>,
    pub profile: Option<Profile>,
    pub signed_in_at: Option<i64>,
    pub busy: bool,
    pub progress_label: Option<String>,
    pub error_message: Option<String>,
    pub credentials: Vec<Credential>,
    pub available_count: u32,
    pub total_count: u32,
    pub needs_reverification: bool,
    pub session_expired: bool,
    pub masked_device_id: String,
    pub qr_status: Option<super::super::model::QrStatus>,
}

pub(crate) const SYSTEMS: [&str; 3] = ["jwxt", "atrust", "otp"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SignInState {
    Completed,
    VerificationRequired,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QrSignInPhase {
    Waiting,
    Redeeming,
    Credentials,
    Completed,
    Unavailable,
    Failed,
}

pub struct QrSignInEvent {
    pub phase: QrSignInPhase,
    pub image: Option<super::super::model::QrSession>,
    pub error: Option<crate::auth::model::AuthFailure>,
}

impl Persisted {
    pub(crate) fn restore(value: serde_json::Value) -> Self {
        let mut restored: Self = serde_json::from_value(value).unwrap_or_default();
        restored
            .credentials
            .retain(|key, _| SYSTEMS.contains(&key.as_str()));
        for state in restored.credentials.values_mut() {
            if *state == CredentialState::Refreshing {
                *state = CredentialState::Unknown;
            }
        }
        restored
    }
    pub(crate) fn legacy(document: &str) -> Self {
        if document.len() > 64 * 1024 {
            return Self::default();
        }
        let value: serde_json::Value = match serde_json::from_str(document) {
            Ok(value) => value,
            _ => return Self::default(),
        };
        let text = |value: &serde_json::Value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty() && text.len() <= 2048)
                .map(str::to_owned)
        };
        let raw = &value["profile"];
        let name = text(&raw["name"]);
        let profile = name.map(|name| Profile {
            name: Some(name),
            student_id: text(&raw["studentId"]),
            grade: text(&raw["grade"]),
            college: text(&raw["college"]),
            major: text(&raw["major"]),
            class_name: text(&raw["className"]),
        });
        let credentials = SYSTEMS
            .into_iter()
            .filter_map(|id| {
                let state = serde_json::from_value(value["credentials"][id].clone()).ok()?;
                Some((
                    id.to_owned(),
                    if state == CredentialState::Refreshing {
                        CredentialState::Unknown
                    } else {
                        state
                    },
                ))
            })
            .collect();
        let verified_at = value["verifiedAt"]
            .as_str()
            .and_then(|raw| {
                cookie::time::OffsetDateTime::parse(
                    raw,
                    &cookie::time::format_description::well_known::Rfc3339,
                )
                .ok()
            })
            .map(|time| time.unix_timestamp());
        Self {
            account_id: text(&value["accountId"]),
            profile,
            credentials,
            verified_at,
        }
    }
}
