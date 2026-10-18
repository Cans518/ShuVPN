use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{rand_core::OsRng, BigUint, Pkcs1v15Encrypt, RsaPublicKey};

use crate::auth::model::AuthFailure;

const MODULUS: &str = concat!(
    "e5fda0a0465f5fff838df4c7b0a159d5e7c38b394d802c18b614a739c88b1f4a",
    "98af2b17bb03a162b498c7bdadd6a4cee0bd53a29cc7a1a7a89fd9434891b68d",
    "fa99567f9230a84571b0d6697a2c5ce06b1b63d757124dd6b518f0192c832f24",
    "b3104487fe4a49568c4eee28d162a53eda8491c1304d78f3a4d47f8b450a2481"
);

pub(super) fn public_key() -> Result<RsaPublicKey, AuthFailure> {
    let modulus = BigUint::parse_bytes(MODULUS.as_bytes(), 16)
        .ok_or_else(|| AuthFailure::new("ssoKey", "认证公钥无效"))?;
    RsaPublicKey::new(modulus, BigUint::from(65537u32))
        .map_err(|_| AuthFailure::new("ssoKey", "认证公钥无效"))
}

pub(super) fn encrypt(key: &RsaPublicKey, password: &str) -> Result<String, AuthFailure> {
    key.encrypt(&mut OsRng, Pkcs1v15Encrypt, password.as_bytes())
        .map(|bytes| STANDARD.encode(bytes))
        .map_err(|_| AuthFailure::new("passwordEncryption", "密码过长或无法加密"))
}
