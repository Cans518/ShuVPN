#[derive(Clone)]
pub struct VerificationTarget {
    pub method: String,
    pub target: String,
}

pub struct LoginResult {
    pub methods: Vec<VerificationTarget>,
    pub callback_uri: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QrStatus {
    Waiting,
    ConfirmedPending,
    Succeeded,
    Expired,
}

pub struct QrSession {
    pub image_bytes: Vec<u8>,
    pub confirm_url: String,
    pub wx_work_scheme_url: String,
}

pub struct AuthorizationReceipt {
    pub system_id: String,
    pub landing_url: String,
    pub http_status: Option<u16>,
    pub username: Option<String>,
}

pub struct OtpCode {
    pub code: String,
    pub account: String,
    pub remaining_seconds: u32,
    pub period_seconds: u32,
}

pub struct ATrustVerification {
    pub virtual_address: Option<String>,
    pub dns_servers: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Default)]
pub struct JwxtAccount {
    pub name: Option<String>,
    pub student_id: Option<String>,
    pub grade: Option<String>,
    pub college: Option<String>,
    pub major: Option<String>,
    pub class_name: Option<String>,
}

pub struct SsoSnapshot {
    pub has_session: bool,
    pub challenge_methods: Vec<VerificationTarget>,
    pub cookie_names: Vec<String>,
    pub legacy_imported: bool,
    pub persistent: bool,
    pub legacy_import_allowed: bool,
    pub qr_status: Option<QrStatus>,
}
