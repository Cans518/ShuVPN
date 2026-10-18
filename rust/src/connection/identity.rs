use super::model::VpnFailure;

pub(crate) struct Identity {
    pub connection_id: String,
    pub sign_key: Vec<u8>,
}

pub(crate) fn new(device_id: &str) -> Result<Identity, VpnFailure> {
    use md5::{Digest, Md5};
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| VpnFailure::new("clock", "系统时钟无效"))?
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    let previous = LAST
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |previous| {
            Some(now.max(previous.saturating_add(1)))
        })
        .map_err(|_| VpnFailure::new("clock", "无法生成连接标识"))?;
    let stamp = now.max(previous.saturating_add(1));
    let digest = Md5::digest(device_id.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    let mut key = vec![0; 32];
    getrandom::getrandom(&mut key)
        .map_err(|_| VpnFailure::new("random", "无法生成安全签名密钥"))?;
    Ok(Identity {
        connection_id: format!("{digest}-{stamp}"),
        sign_key: key,
    })
}
