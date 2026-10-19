use super::model::AuthFailure;
use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use sangfor_core::crypto::{hex_upper, hmac_sha256, sha256_str};
use serde_json::Value;
use std::collections::BTreeMap;

const CHALLENGE_SALT: &str = "OrHWuJz7gku5awmVb5w1sKTmfeCWHmzokBxmn0sn0faIcv1G10PdrbbRGKBrrZ3m";
const SIGNATURE_SALT: &str = "3uW5IEy8KwDaOMK8uw1TmNr50U3aK1Qdu8b6vopXxGstzan3AJXxVNR6piuKi5Nq";

pub(crate) fn verify(envelope: &Value) -> Result<Vec<String>, AuthFailure> {
    let data = &envelope["data"]["antiMITMAttackData"];
    if integer(&data["enable"]) != Some(1) {
        return Ok(vec![]);
    }
    let modulus = text(&data["devicePubKeyMod"]);
    let exponent = text(&data["devicePubKeyExp"]);
    let challenge = text(&data["challenge"]);
    if modulus.is_empty()
        || exponent.is_empty()
        || challenge.is_empty()
        || modulus.len() > 8192
        || exponent.len() > 64
        || challenge.len() > 65536
    {
        return Err(bad("anti-MITM 挑战字段缺失或超限"));
    }
    let material = sha256_str(&format!("{modulus}{exponent}{CHALLENGE_SALT}"));
    let encrypted =
        cbc::Encryptor::<aes::Aes128>::new_from_slices(&material[..16], &material[16..])
            .map_err(|_| bad("anti-MITM AES 配置无效"))?
            .encrypt_padded_vec_mut::<Pkcs7>(challenge.as_bytes());
    if !hex_upper(&encrypted).eq_ignore_ascii_case(text(&data["encryptedChallenge"])) {
        return Err(bad("anti-MITM 挑战不匹配"));
    }
    let first = sha256_str(&format!("{modulus}{exponent}{SIGNATURE_SALT}"));
    let second = sha256_str(&(hex_upper(&first) + challenge));
    let key: Vec<_> = first.iter().zip(second).map(|(a, b)| a ^ b).collect();
    let mut fields = BTreeMap::new();
    collect(envelope, &mut fields, 0)?;
    let mut fields: Vec<_> = fields.into_iter().collect();
    // Gateway canonicalization orders strings by UTF-16 code units.
    fields.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
    let message = fields
        .into_iter()
        .map(|(key, value)| format!("{key}:{value}"))
        .collect::<Vec<_>>()
        .join("&");
    let signature = text(&data["mitmSig"]);
    if signature.is_empty()
        || !hex_upper(&hmac_sha256(&key, message.as_bytes())).eq_ignore_ascii_case(signature)
    {
        return Err(bad("anti-MITM 响应签名不匹配"));
    }
    advertised_pins(envelope)
}
pub(crate) fn advertised_pins(envelope: &Value) -> Result<Vec<String>, AuthFailure> {
    let data = &envelope["data"]["antiMITMAttackData"];
    if integer(&data["enable"]) != Some(1) {
        return Ok(vec![]);
    }
    let mut pins = vec![];
    for field in ["rsaCert", "sm2encCert"] {
        let certificate = text(&data[field]);
        if !certificate.is_empty() {
            if certificate.len() > 256 * 1024 {
                return Err(bad("anti-MITM 证书字段超限"));
            }
            let digest = hex_upper(&sha256_str(
                &(certificate.to_owned() + sangfor_core::crypto::CERTIFICATE_DIGEST_SALT),
            ));
            if !pins.contains(&digest) {
                pins.push(digest);
            }
        }
    }
    Ok(pins)
}
fn collect(
    value: &Value,
    fields: &mut BTreeMap<String, String>,
    depth: usize,
) -> Result<(), AuthFailure> {
    if depth > 32 || fields.len() > 16384 {
        return Err(bad("anti-MITM 签名字段超出解析预算"));
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "mitmSig" {
                    continue;
                }
                if value.is_array() || value.is_object() {
                    collect(value, fields, depth + 1)?;
                } else {
                    fields.insert(
                        key.clone(),
                        match value {
                            Value::String(value) => value.clone(),
                            _ => value.to_string(),
                        },
                    );
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                collect(value, fields, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn integer(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str()?.parse().ok())
}
fn bad(message: &str) -> AuthFailure {
    AuthFailure::new("atrustAntiMitm", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> anyhow::Result<Value> {
        Ok(serde_json::from_str(include_str!(
            "../../tests/fixtures/anti_mitm.json"
        ))?)
    }
    #[test]
    fn sdk_verified_aes_signature_and_certificate_vector_matches_rust() -> anyhow::Result<()> {
        let envelope = fixture()?;
        assert_eq!(
            verify(&envelope)?,
            ["514177A58F877B521EA2F2CB04C3E5172F518A52CE6A2EFBFE5160208A886E7E"]
        );
        Ok(())
    }
    #[test]
    fn challenge_signature_and_flattened_scalar_tampering_are_rejected() -> anyhow::Result<()> {
        for field in ["challenge", "encryptedChallenge", "mitmSig", "rsaCert"] {
            let mut envelope = fixture()?;
            envelope["data"]["antiMITMAttackData"][field] = Value::String("tampered".to_owned());
            assert!(verify(&envelope).is_err(), "{field}");
        }
        let mut envelope = fixture()?;
        envelope["data"]["second"]["duplicate"] = Value::String("changed".to_owned());
        assert!(verify(&envelope).is_err());
        let mut envelope = fixture()?;
        envelope["data"]["antiMITMAttackData"]["enable"] = Value::from(0);
        assert!(verify(&envelope)?.is_empty());
        Ok(())
    }
}
