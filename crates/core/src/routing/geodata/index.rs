use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;

use super::failure;
use crate::routing::model::{normalize_domain, RoutingFailure};

#[derive(Debug)]
pub struct GeoIpIndex {
    ipv4: Vec<HashSet<u32>>,
    ipv6: Vec<HashSet<u128>>,
    inverse: bool,
}

impl GeoIpIndex {
    pub(crate) fn build(lines: &[String], inverse: bool) -> Result<Self, RoutingFailure> {
        let mut index = Self {
            ipv4: (0..=32).map(|_| HashSet::new()).collect(),
            ipv6: (0..=128).map(|_| HashSet::new()).collect(),
            inverse,
        };
        for line in lines {
            match line
                .parse::<ipnet::IpNet>()
                .map_err(|_| failure("GeoIP 缓存网段无效"))?
            {
                ipnet::IpNet::V4(network) => {
                    index.ipv4[usize::from(network.prefix_len())]
                        .insert(u32::from(network.network()));
                }
                ipnet::IpNet::V6(network) => {
                    index.ipv6[usize::from(network.prefix_len())]
                        .insert(u128::from(network.network()));
                }
            }
        }
        Ok(index)
    }

    pub(crate) fn matches(&self, addresses: &[IpAddr]) -> bool {
        let inside = addresses.iter().any(|address| match address {
            IpAddr::V4(address) => {
                let address = u32::from(*address);
                self.ipv4.iter().enumerate().any(|(prefix, networks)| {
                    let mask = if prefix == 0 {
                        0
                    } else {
                        u32::MAX << (32 - prefix)
                    };
                    networks.contains(&(address & mask))
                })
            }
            IpAddr::V6(address) => {
                let address = u128::from(*address);
                self.ipv6.iter().enumerate().any(|(prefix, networks)| {
                    let mask = if prefix == 0 {
                        0
                    } else {
                        u128::MAX << (128 - prefix)
                    };
                    networks.contains(&(address & mask))
                })
            }
        });
        if self.inverse {
            !inside
        } else {
            inside
        }
    }
}

#[derive(Debug)]
pub struct GeoSiteIndex {
    full: HashSet<String>,
    suffix: HashSet<String>,
    keywords: Option<aho_corasick::AhoCorasick>,
    regexes: Vec<Arc<regex::Regex>>,
}

impl GeoSiteIndex {
    pub(crate) fn build(
        lines: &[String],
        mut compile_regex: impl FnMut(&str) -> Result<Arc<regex::Regex>, RoutingFailure>,
    ) -> Result<Self, RoutingFailure> {
        let mut index = Self {
            full: HashSet::new(),
            suffix: HashSet::new(),
            keywords: None,
            regexes: vec![],
        };
        let mut keywords = HashSet::new();
        let mut keyword_bytes = 0;
        for line in lines {
            let (kind, value): (u64, String) =
                serde_json::from_str(line).map_err(|_| failure("GeoSite 缓存 Domain 无效"))?;
            if value.is_empty()
                || value.len() > 1024
                || value.bytes().any(|byte| byte.is_ascii_control())
            {
                return Err(failure("GeoSite 缓存内容无效"));
            }
            match kind {
                0 => {
                    if keywords.insert(value.clone()) {
                        keyword_bytes += value.len();
                    }
                    if keywords.len() > 50_000 || keyword_bytes > 4 * 1024 * 1024 {
                        return Err(failure("GeoSite 关键词索引超过预算"));
                    }
                }
                1 => index.regexes.push(compile_regex(&value)?),
                2 => {
                    index.suffix.insert(normalize_domain(&value)?);
                }
                3 => {
                    index.full.insert(value);
                }
                _ => return Err(failure("GeoSite 缓存域名类型无效")),
            }
        }
        if !keywords.is_empty() {
            let matcher = aho_corasick::AhoCorasickBuilder::new()
                .kind(Some(aho_corasick::AhoCorasickKind::ContiguousNFA))
                .build(keywords)
                .map_err(|_| failure("GeoSite 关键词索引编译失败"))?;
            if matcher.memory_usage() > 32 * 1024 * 1024 {
                return Err(failure("GeoSite 关键词索引超过内存预算"));
            }
            index.keywords = Some(matcher);
        }
        Ok(index)
    }

    pub(crate) fn matches(&self, host: &str) -> bool {
        if self.full.contains(host) || self.suffix.contains(host) {
            return true;
        }
        if host
            .match_indices('.')
            .any(|(offset, _)| self.suffix.contains(&host[offset + 1..]))
        {
            return true;
        }
        self.keywords
            .as_ref()
            .is_some_and(|matcher| matcher.is_match(host))
            || self.regexes.iter().any(|regex| regex.is_match(host))
    }
}
