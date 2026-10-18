use super::super::model::QrStatus;
use crate::auth::model::AuthFailure;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

pub(super) const APP_ID: &str = "wxa8dea949443de641";
pub(super) const AGENT_ID: &str = "1000059";

pub(super) fn state() -> String {
    URL_SAFE_NO_PAD.encode(
        json!({
            "responseType":"code", "clientId":"Km5t225E8KECKQ6ZDm5K2P6aS2459Cua",
            "clientName":"本科生教务系统","scope":"jw",
            "redirectUri":"https://jwxt.shu.edu.cn/sso/shulogin","state":"",
        })
        .to_string(),
    )
}

pub(super) fn key(bytes: &[u8]) -> Result<String, AuthFailure> {
    let html = std::str::from_utf8(bytes)
        .map_err(|_| failure("qrcodeKeyNotFound", "二维码页面编码无效"))?;
    let tail = html
        .split_once("qrImg?key=")
        .map(|(_, tail)| tail)
        .ok_or_else(|| failure("qrcodeKeyNotFound", "未能获取企业微信登录二维码"))?;
    let key: String = tail.chars().take_while(char::is_ascii_hexdigit).collect();
    if key.is_empty() || key.len() > 256 {
        return Err(failure("qrcodeKeyNotFound", "二维码标识无效"));
    }
    Ok(key)
}

pub(super) fn scan(bytes: &[u8]) -> (QrStatus, Option<zeroize::Zeroizing<String>>) {
    let Some(value) = std::str::from_utf8(bytes)
        .ok()
        .and_then(|body| {
            body.trim()
                .strip_suffix(';')
                .unwrap_or(body.trim())
                .trim()
                .strip_prefix("jsonpCallback(")?
                .strip_suffix(')')
        })
        .and_then(|json| serde_json::from_str::<Value>(json).ok())
    else {
        return (QrStatus::Waiting, None);
    };
    match value["status"].as_str().unwrap_or("") {
        "QRCODE_SCAN_SUCC" => {
            let code = value["auth_code"].as_str().unwrap_or("");
            if !code.is_empty()
                && code.len() <= 2048
                && code.bytes().all(|byte| byte.is_ascii_graphic())
            {
                (
                    QrStatus::Succeeded,
                    Some(zeroize::Zeroizing::new(code.to_owned())),
                )
            } else {
                (QrStatus::ConfirmedPending, None)
            }
        }
        "QRCODE_SCAN_ING" => (QrStatus::ConfirmedPending, None),
        "QRCODE_SCAN_ERR" | "QRCODE_SCAN_OVERDUE" | "QRCODE_SCAN_CANCEL" => {
            (QrStatus::Expired, None)
        }
        _ => (QrStatus::Waiting, None),
    }
}

pub(super) fn image(bytes: &[u8]) -> Result<(), AuthFailure> {
    let raster = bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP";
    if bytes.len() > 256 * 1024 || !raster {
        return Err(failure("qrcodeImage", "二维码图片格式或大小无效"));
    }
    Ok(())
}

pub(super) fn failure(code: &str, message: &str) -> AuthFailure {
    AuthFailure::new(code, message)
}
