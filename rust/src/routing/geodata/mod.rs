use std::collections::BTreeSet;
use std::net::IpAddr;

use super::model::{GeoDataSpec, Provider, RoutingFailure};
use sangfor_core::crypto::{hex_upper, sha256};

mod index;
mod wire;
pub(super) use index::{GeoIpIndex, GeoSiteIndex};
use wire::{Message, Value};

pub(super) const MAX_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_ENTRIES: usize = 500_000;
pub(super) const MAX_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
const HEADER: &str = "@shuvpn-geodata:";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    behavior: String,
    code: String,
    version: String,
    sha256: String,
    payload_sha256: String,
    inverse: bool,
}

pub(super) struct CachedData<'a> {
    pub lines: &'a [String],
    pub inverse: bool,
}

pub(super) fn is_geo(provider: &Provider) -> bool {
    matches!(provider.behavior.as_str(), "geoip" | "geosite")
}

pub(super) fn selector(code: &str) -> Result<(String, BTreeSet<String>), RoutingFailure> {
    let mut parts = code.split('@');
    let name = parts.next().unwrap_or("").trim();
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(failure("GeoData 类别标识无效"));
    }
    let mut attributes = BTreeSet::new();
    for part in parts.map(str::trim).filter(|part| !part.is_empty()) {
        if part.len() > 64
            || attributes.len() >= 16
            || part
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        {
            return Err(failure("GeoSite 属性标识无效"));
        }
        attributes.insert(part.to_ascii_lowercase());
    }
    Ok((name.to_ascii_lowercase(), attributes))
}

pub(super) fn canonical_selector(code: &str) -> Result<String, RoutingFailure> {
    let (name, attributes) = selector(code)?;
    Ok(std::iter::once(name)
        .chain(attributes)
        .collect::<Vec<_>>()
        .join("@"))
}

pub(super) fn validate(provider: &Provider) -> Result<&GeoDataSpec, RoutingFailure> {
    let data = provider
        .data
        .as_ref()
        .ok_or_else(|| failure("GeoData 缺少类别、版本或 SHA-256"))?;
    let (_, attributes) = selector(&data.code)?;
    if provider.format != "dat"
        || provider.kind == "inline"
        || !provider.payload.is_empty()
        || data.version.trim().is_empty()
        || data.version.len() > 128
        || data.version.bytes().any(|byte| byte.is_ascii_control())
        || data.sha256.len() != 64
        || !data.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || (provider.behavior == "geoip" && !attributes.is_empty())
    {
        return Err(failure("GeoData 格式、来源、版本、摘要或属性无效"));
    }
    Ok(data)
}

pub(super) fn decode(bytes: &[u8], provider: &Provider) -> Result<Vec<String>, RoutingFailure> {
    let spec = validate(provider)?;
    if bytes.len() > MAX_BYTES {
        return Err(failure("GeoData 文件超过大小预算"));
    }
    if !hex_upper(&sha256(bytes)).eq_ignore_ascii_case(&spec.sha256) {
        return Err(RoutingFailure::new("checksum", "GeoData SHA-256 校验失败"));
    }
    let (code, attributes) = selector(&spec.code)?;
    let mut list = Message::new(bytes);
    let mut selected = None;
    let mut groups = 0;
    while let Some((field, value)) = list.next()? {
        if field != 1 {
            continue;
        }
        let Value::Bytes(group) = value else {
            return Err(failure("GeoData entry 类型无效"));
        };
        groups += 1;
        if groups > 100_000 {
            return Err(failure("GeoData 类别数量超过预算"));
        }
        let mut message = Message::new(group);
        let mut name = "";
        while let Some((field, value)) = message.next()? {
            if field == 1 {
                let Value::Bytes(value) = value else {
                    return Err(failure("GeoData 类别类型无效"));
                };
                name = wire::text(value)?;
            }
        }
        if name.eq_ignore_ascii_case(&code) {
            if selected.is_some() {
                return Err(failure("GeoData 类别重复"));
            }
            selected = Some(group);
        }
    }
    let group = selected.ok_or_else(|| failure("GeoData 不包含指定类别"))?;
    let mut lines = vec![];
    let mut inverse = false;
    let mut message = Message::new(group);
    let mut count = 0;
    let mut size = 0;
    while let Some((field, value)) = message.next()? {
        if field == 3 && provider.behavior == "geoip" {
            let Value::Number(value) = value else {
                return Err(failure("GeoIP inverse_match 类型无效"));
            };
            inverse = value != 0;
        }
        if field != 2 {
            continue;
        }
        let Value::Bytes(entry) = value else {
            return Err(failure("GeoData 条目类型无效"));
        };
        count += 1;
        if count > MAX_ENTRIES {
            return Err(failure("GeoData 类别条目超过预算"));
        }
        let line = if provider.behavior == "geoip" {
            Some(cidr(entry)?)
        } else {
            domain(entry, &attributes)?
        };
        if let Some(line) = line {
            size += line.len();
            if size > MAX_PAYLOAD_BYTES {
                return Err(failure("GeoData 选定内容超过预算"));
            }
            lines.push(line);
        }
    }
    let manifest = Manifest {
        behavior: provider.behavior.clone(),
        code: canonical_selector(&spec.code)?,
        version: spec.version.clone(),
        sha256: spec.sha256.to_ascii_uppercase(),
        payload_sha256: payload_digest(&lines, inverse)?,
        inverse,
    };
    lines.insert(
        0,
        format!(
            "{HEADER}{}",
            serde_json::to_string(&manifest).map_err(|_| failure("GeoData 版本编码失败"))?
        ),
    );
    Ok(lines)
}

pub(super) fn cached<'a>(
    payload: &'a [String],
    provider: &Provider,
) -> Result<CachedData<'a>, RoutingFailure> {
    let spec = validate(provider)?;
    let manifest: Manifest = serde_json::from_str(
        payload
            .first()
            .and_then(|line| line.strip_prefix(HEADER))
            .ok_or_else(|| failure("GeoData 缓存缺少来源信息"))?,
    )
    .map_err(|_| failure("GeoData 来源信息损坏"))?;
    let lines = &payload[1..];
    if lines.len() > MAX_ENTRIES
        || lines.iter().map(String::len).sum::<usize>() > MAX_PAYLOAD_BYTES
        || manifest.behavior != provider.behavior
        || manifest.code != canonical_selector(&spec.code)?
        || manifest.version != spec.version
        || !manifest.sha256.eq_ignore_ascii_case(&spec.sha256)
        || manifest.payload_sha256 != payload_digest(lines, manifest.inverse)?
        || (provider.behavior == "geosite" && manifest.inverse)
    {
        return Err(failure("GeoData 缓存内容或来源信息不匹配"));
    }
    Ok(CachedData {
        lines,
        inverse: manifest.inverse,
    })
}

fn payload_digest(lines: &[String], inverse: bool) -> Result<String, RoutingFailure> {
    let bytes =
        serde_json::to_vec(&(inverse, lines)).map_err(|_| failure("GeoData 内容编码失败"))?;
    Ok(hex_upper(&sha256(&bytes)))
}

fn cidr(bytes: &[u8]) -> Result<String, RoutingFailure> {
    let mut message = Message::new(bytes);
    let mut ip = None;
    let mut prefix = 0;
    let mut textual = None;
    while let Some((field, value)) = message.next()? {
        match (field, value) {
            (1, Value::Bytes(value)) => ip = Some(value),
            (2, Value::Number(value)) => {
                prefix = u8::try_from(value).map_err(|_| failure("GeoIP 掩码无效"))?
            }
            (68000, Value::Bytes(value)) => textual = Some(wire::text(value)?),
            (1 | 2 | 68000, _) => return Err(failure("GeoIP CIDR 字段类型无效")),
            _ => {}
        }
    }
    let address = match ip.filter(|value| !value.is_empty()) {
        Some(value) if value.len() == 4 => IpAddr::V4(std::net::Ipv4Addr::new(
            value[0], value[1], value[2], value[3],
        )),
        Some(value) if value.len() == 16 => {
            let octets: [u8; 16] = value
                .try_into()
                .map_err(|_| failure("GeoIP 地址长度无效"))?;
            IpAddr::V6(std::net::Ipv6Addr::from(octets))
        }
        None => textual
            .ok_or_else(|| failure("GeoIP 地址缺失"))?
            .parse()
            .map_err(|_| failure("GeoIP 文本地址无效"))?,
        _ => return Err(failure("GeoIP 地址长度无效")),
    };
    ipnet::IpNet::new(address, prefix)
        .map(|network| network.trunc().to_string())
        .map_err(|_| failure("GeoIP 地址族或掩码无效"))
}

fn domain(bytes: &[u8], wanted: &BTreeSet<String>) -> Result<Option<String>, RoutingFailure> {
    let mut message = Message::new(bytes);
    let mut kind = 0;
    let mut value = "";
    let mut attributes = BTreeSet::new();
    let mut count = 0;
    while let Some((field, item)) = message.next()? {
        match (field, item) {
            (1, Value::Number(value)) => kind = value,
            (2, Value::Bytes(text)) => value = wire::text(text)?,
            (3, Value::Bytes(attribute)) => {
                count += 1;
                if count > 128 {
                    return Err(failure("GeoSite 属性数量超过预算"));
                }
                let mut attribute = Message::new(attribute);
                let mut key = None;
                while let Some((field, item)) = attribute.next()? {
                    match (field, item) {
                        (1, Value::Bytes(value)) => {
                            key = Some(wire::text(value)?);
                        }
                        (2 | 3, Value::Number(_)) => {}
                        (1..=3, _) => return Err(failure("GeoSite 属性类型无效")),
                        _ => {}
                    }
                }
                if let Some(key) = key {
                    attributes.insert(key.to_ascii_lowercase());
                }
            }
            (1..=3, _) => return Err(failure("GeoSite Domain 字段类型无效")),
            _ => {}
        }
    }
    if kind > 3
        || value.is_empty()
        || value.len() > 1024
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(failure("GeoSite 域名类型或内容无效"));
    }
    if !wanted
        .iter()
        .all(|attribute| attributes.contains(attribute))
    {
        return Ok(None);
    }
    serde_json::to_string(&(kind, value))
        .map(Some)
        .map_err(|_| failure("GeoSite 内容编码失败"))
}

fn failure(message: &str) -> RoutingFailure {
    RoutingFailure::new("geodata", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(dead_code)]
    mod fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/geodata.rs"
        ));
    }

    fn provider(behavior: &str, code: &str, bytes: &[u8]) -> Provider {
        Provider {
            kind: "file".to_owned(),
            behavior: behavior.to_owned(),
            format: "dat".to_owned(),
            payload: vec![],
            path: Some("fixture.dat".to_owned()),
            url: None,
            interval: None,
            data: Some(GeoDataSpec {
                code: code.to_owned(),
                version: "unit-v1".to_owned(),
                sha256: hex_upper(&sha256(bytes)),
            }),
        }
    }

    #[test]
    fn malformed_wire_unknown_domain_type_and_invalid_cidr_are_rejected_before_compiling(
    ) -> anyhow::Result<()> {
        let invalid_ip = fixture::bytes(
            1,
            &[
                fixture::bytes(1, b"CN"),
                fixture::bytes(
                    2,
                    &[fixture::bytes(1, &[1, 2, 3]), fixture::number(2, 8)].concat(),
                ),
            ]
            .concat(),
        );
        let invalid_prefix = fixture::bytes(
            1,
            &[
                fixture::bytes(1, b"CN"),
                fixture::bytes(
                    2,
                    &[fixture::bytes(1, &[10, 0, 0, 0]), fixture::number(2, 33)].concat(),
                ),
            ]
            .concat(),
        );
        for bytes in [
            vec![0],
            vec![10, 0x80],
            vec![10, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1],
            invalid_ip,
            invalid_prefix,
        ] {
            assert!(decode(&bytes, &provider("geoip", "CN", &bytes)).is_err());
        }
        let domain = fixture::bytes(
            1,
            &[
                fixture::bytes(1, b"TEST"),
                fixture::site_domain(9, "example.com", &[]),
            ]
            .concat(),
        );
        assert!(decode(&domain, &provider("geosite", "TEST", &domain)).is_err());
        let duplicate = [fixture::site(), fixture::site()].concat();
        assert!(decode(&duplicate, &provider("geosite", "TEST", &duplicate)).is_err());
        let valid = fixture::site();
        let descriptor = provider("geosite", "TEST", &valid);
        let mut payload = decode(&valid, &descriptor)?;
        payload[1] = "[3,\"tampered.example\"]".to_owned();
        assert!(cached(&payload, &descriptor).is_err());
        let ip = fixture::ip(false);
        let descriptor = provider("geoip", "CN", &ip);
        let mut payload = decode(&ip, &descriptor)?;
        let mut manifest: Manifest = serde_json::from_str(
            payload[0]
                .strip_prefix(HEADER)
                .ok_or_else(|| failure("fixture manifest"))?,
        )?;
        manifest.inverse = true;
        payload[0] = format!("{HEADER}{}", serde_json::to_string(&manifest)?);
        assert!(cached(&payload, &descriptor).is_err());
        Ok(())
    }

    #[test]
    fn attributes_are_presence_filters_and_extensions_are_data_not_paths() -> anyhow::Result<()> {
        let valid = fixture::site();
        let selected = decode(&valid, &provider("geosite", "test@ADS", &valid))?;
        assert_eq!(selected.len(), 4);
        let empty = decode(&valid, &provider("geosite", "TEST@absent", &valid))?;
        assert_eq!(empty.len(), 1);
        let attribute = [fixture::bytes(1, b"ads"), fixture::bytes(1, b"other")].concat();
        let domain = [
            fixture::number(1, 3),
            fixture::bytes(2, b"example.com"),
            fixture::bytes(3, &attribute),
        ]
        .concat();
        let duplicate_key = fixture::bytes(
            1,
            &[fixture::bytes(1, b"TEST"), fixture::bytes(2, &domain)].concat(),
        );
        assert_eq!(
            decode(
                &duplicate_key,
                &provider("geosite", "TEST@ads@other", &duplicate_key)
            )?
            .len(),
            1
        );
        assert_eq!(
            decode(
                &duplicate_key,
                &provider("geosite", "TEST@other", &duplicate_key)
            )?
            .len(),
            2
        );
        let cidr = [
            fixture::bytes(68000, b"2001:db8::1"),
            fixture::number(2, 32),
        ]
        .concat();
        let data = fixture::bytes(
            1,
            &[
                fixture::bytes(1, b"CN"),
                fixture::bytes(2, &cidr),
                fixture::bytes(68000, b"/must/not/be/opened"),
            ]
            .concat(),
        );
        let payload = decode(&data, &provider("geoip", "CN", &data))?;
        assert_eq!(payload[1], "2001:db8::/32");
        Ok(())
    }
}
