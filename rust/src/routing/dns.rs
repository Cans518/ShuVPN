use super::model::RoutingFailure;
use hickory_proto::{
    op::{Message, MessageType, ResponseCode},
    rr::{DNSClass, RData, RecordType},
};
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum Source {
    Campus { server: String },
    System,
}

pub(crate) struct Answer {
    pub addresses: Vec<IpAddr>,
    pub ttl: Option<u32>,
    pub observed: Instant,
}

impl Answer {
    pub(crate) fn expired(&self, now: Instant) -> bool {
        self.ttl.is_some_and(|ttl| {
            ttl > 0 && self.observed + Duration::from_secs(u64::from(ttl)) <= now
        })
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct Resolution {
    pub host: String,
    pub addresses: Vec<IpAddr>,
    pub source: Source,
    pub ttl_seconds: Option<u32>,
    pub version: u64,
    pub cached: bool,
    pub age_ms: u64,
    pub cache_ttl_remaining_ms: Option<u64>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing)]
    pub observed: Instant,
    #[serde(skip_serializing)]
    pub expires: Option<Instant>,
}

impl Resolution {
    pub(crate) fn new(
        host: String,
        source: Source,
        version: u64,
        answers: Vec<Answer>,
        mut warnings: Vec<String>,
    ) -> Self {
        let now = Instant::now();
        let before = answers.len();
        let answers: Vec<_> = answers
            .into_iter()
            .filter(|answer| !answer.expired(now))
            .collect();
        if answers.len() != before {
            warnings.push("解析期间已过期的地址已丢弃".to_owned());
        }
        let observed = answers
            .iter()
            .map(|answer| answer.observed)
            .min()
            .unwrap_or(now);
        let ttl_seconds = answers.iter().filter_map(|answer| answer.ttl).min();
        let expires = answers
            .iter()
            .filter_map(|answer| {
                answer
                    .ttl
                    .map(|ttl| answer.observed + Duration::from_secs(u64::from(ttl)))
            })
            .min()
            .map(|deadline| deadline.min(now + Duration::from_secs(86_400)));
        let mut addresses = Vec::new();
        for answer in answers {
            for address in answer.addresses {
                if !addresses.contains(&address) && addresses.len() < 64 {
                    addresses.push(address);
                }
            }
        }
        Self {
            host,
            addresses,
            source,
            ttl_seconds,
            version,
            cached: false,
            age_ms: millis(now.saturating_duration_since(observed)),
            cache_ttl_remaining_ms: expires
                .map(|deadline| millis(deadline.saturating_duration_since(now))),
            warnings,
            observed,
            expires,
        }
    }
}

type Key = (String, Source, u64);
#[derive(Default)]
struct Entries {
    epoch: u64,
    version: u64,
    values: BTreeMap<Key, Resolution>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Epoch(u64);
#[derive(Default)]
pub(crate) struct Cache {
    entries: Mutex<Entries>,
    pub campus_queries: AtomicU64,
    pub system_lookups: AtomicU64,
    pub failed_queries: AtomicU64,
    hits: AtomicU64,
}
impl Cache {
    pub(crate) fn epoch(&self) -> Result<Epoch, RoutingFailure> {
        Ok(Epoch(
            self.entries.lock().map_err(|_| cache_failure())?.epoch,
        ))
    }

    pub(crate) fn lookup(
        &self,
        host: &str,
        source: &Source,
        version: u64,
        epoch: Epoch,
    ) -> Result<(Option<Resolution>, Epoch), RoutingFailure> {
        let mut entries = self.entries.lock().map_err(|_| cache_failure())?;
        if entries.epoch != epoch.0 {
            return Ok((None, epoch));
        }
        if version > entries.version {
            entries.version = version;
            entries
                .values
                .retain(|(_, _, current), _| *current >= version);
        }
        let now = Instant::now();
        entries
            .values
            .retain(|_, resolution| resolution.expires.is_some_and(|deadline| deadline > now));
        let result = entries
            .values
            .get(&(host.to_owned(), source.clone(), version))
            .cloned()
            .map(|mut resolution| {
                resolution.cached = true;
                resolution.age_ms = millis(now.saturating_duration_since(resolution.observed));
                resolution.cache_ttl_remaining_ms = resolution
                    .expires
                    .map(|deadline| millis(deadline.saturating_duration_since(now)));
                self.hits.fetch_add(1, Ordering::Relaxed);
                resolution
            });
        Ok((result, Epoch(entries.epoch)))
    }

    pub(crate) fn insert(
        &self,
        resolution: &Resolution,
        epoch: Epoch,
    ) -> Result<(), RoutingFailure> {
        self.put(resolution, epoch, None).map(|_| ())
    }

    pub(crate) fn insert_family(
        &self,
        resolution: &Resolution,
        epoch: Epoch,
        family: RecordType,
    ) -> Result<Resolution, RoutingFailure> {
        self.put(resolution, epoch, Some(family))
    }

    fn put(
        &self,
        resolution: &Resolution,
        epoch: Epoch,
        family: Option<RecordType>,
    ) -> Result<Resolution, RoutingFailure> {
        let mut resolution = resolution.clone();
        let now = Instant::now();
        if resolution.addresses.is_empty()
            || !resolution.warnings.is_empty()
            || resolution.expires.is_none_or(|deadline| deadline <= now)
        {
            return Ok(resolution);
        }
        let mut entries = self.entries.lock().map_err(|_| cache_failure())?;
        if entries.epoch != epoch.0 || resolution.version < entries.version {
            return Ok(resolution);
        }
        if resolution.version > entries.version {
            entries.version = resolution.version;
            entries
                .values
                .retain(|(_, _, version), _| *version >= resolution.version);
        }
        entries
            .values
            .retain(|_, cached| cached.expires.is_some_and(|deadline| deadline > now));
        let key = (
            resolution.host.clone(),
            resolution.source.clone(),
            resolution.version,
        );
        if let Some(family) = family {
            if let Some(previous) = entries.values.get(&key) {
                for address in &previous.addresses {
                    let other = match family {
                        RecordType::A => address.is_ipv6(),
                        RecordType::AAAA => address.is_ipv4(),
                        _ => false,
                    };
                    if other
                        && !resolution.addresses.contains(address)
                        && resolution.addresses.len() < 64
                    {
                        resolution.addresses.push(*address);
                    }
                }
                resolution.expires = resolution
                    .expires
                    .zip(previous.expires)
                    .map(|(a, b)| a.min(b));
                resolution.ttl_seconds = resolution
                    .ttl_seconds
                    .zip(previous.ttl_seconds)
                    .map(|(a, b)| a.min(b));
                resolution.observed = resolution.observed.min(previous.observed);
                resolution.age_ms = millis(now.saturating_duration_since(resolution.observed));
                resolution.cache_ttl_remaining_ms = resolution
                    .expires
                    .map(|end| millis(end.saturating_duration_since(now)));
            }
        }
        if entries.values.len() >= 512 {
            if let Some(key) = entries
                .values
                .iter()
                .min_by_key(|(_, cached)| cached.observed)
                .map(|(key, _)| key.clone())
            {
                entries.values.remove(&key);
            }
        }
        entries.values.insert(
            (
                resolution.host.clone(),
                resolution.source.clone(),
                resolution.version,
            ),
            resolution.clone(),
        );
        Ok(resolution)
    }

    pub(crate) fn clear(&self) -> Result<(), RoutingFailure> {
        let mut entries = self.entries.lock().map_err(|_| cache_failure())?;
        entries.epoch = entries.epoch.wrapping_add(1);
        entries.version = 0;
        entries.values.clear();
        Ok(())
    }

    pub(crate) fn statistics(&self) -> Result<serde_json::Value, RoutingFailure> {
        let entries = self.entries.lock().map_err(|_| cache_failure())?;
        Ok(serde_json::json!({
            "campus_queries":self.campus_queries.load(Ordering::Relaxed),
            "system_lookups":self.system_lookups.load(Ordering::Relaxed),
            "failed_queries":self.failed_queries.load(Ordering::Relaxed),
            "cache_hits":self.hits.load(Ordering::Relaxed),
            "cached_entries":entries.values.len(),
        }))
    }
}

pub(crate) fn parse_response(
    query: &Message,
    response: &Message,
) -> Result<Answer, RoutingFailure> {
    let observed = Instant::now();
    if response.id() != query.id()
        || response.op_code() != query.op_code()
        || response.message_type() != MessageType::Response
        || response.queries() != query.queries()
        || response.truncated()
        || query.queries().len() != 1
    {
        return Err(RoutingFailure::new("dns", "校园 DNS 响应标识不匹配"));
    }
    if response.response_code() != ResponseCode::NoError {
        return Err(RoutingFailure::new("dns", "校园 DNS 返回查询失败"));
    }
    let question = &query.queries()[0];
    let mut owner = question.name().clone();
    let mut visited = vec![owner.clone()];
    let mut ttl = None;
    loop {
        let mut alias = None;
        for record in response
            .answers()
            .iter()
            .filter(|record| record.dns_class() == DNSClass::IN && record.name() == &owner)
        {
            if let RData::CNAME(target) = record.data() {
                if alias.as_ref().is_some_and(|existing| existing != &target.0) {
                    return Err(RoutingFailure::new("dns", "DNS 别名目标冲突"));
                }
                alias = Some(target.0.clone());
                ttl = minimum(ttl, record.ttl());
            }
        }
        let Some(alias) = alias else { break };
        if visited.len() >= 17 || visited.contains(&alias) {
            return Err(RoutingFailure::new("dns", "DNS 别名链循环或超过限制"));
        }
        if response.answers().iter().any(|record| {
            record.name() == &owner
                && record.dns_class() == DNSClass::IN
                && matches!(record.data(), RData::A(_) | RData::AAAA(_))
        }) {
            return Err(RoutingFailure::new("dns", "DNS 别名与地址记录冲突"));
        }
        owner = alias;
        visited.push(owner.clone());
    }
    let mut addresses = Vec::new();
    for record in response
        .answers()
        .iter()
        .filter(|record| record.dns_class() == DNSClass::IN && record.name() == &owner)
    {
        let address = match (question.query_type(), record.data()) {
            (RecordType::A, RData::A(address)) => IpAddr::V4(address.0),
            (RecordType::AAAA, RData::AAAA(address)) => IpAddr::V6(address.0),
            _ => continue,
        };
        if !addresses.contains(&address) && addresses.len() < 32 {
            addresses.push(address);
        }
        ttl = minimum(ttl, record.ttl());
    }
    if addresses.is_empty() {
        ttl = None;
    }
    Ok(Answer {
        addresses,
        ttl,
        observed,
    })
}

fn minimum(current: Option<u32>, value: u32) -> Option<u32> {
    Some(current.map(|current| current.min(value)).unwrap_or(value))
}
fn millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}
fn cache_failure() -> RoutingFailure {
    RoutingFailure::new("dns", "DNS 缓存状态不可用")
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::{
        op::Query,
        rr::{
            rdata::{A, CNAME},
            Name, Record,
        },
    };

    fn resolution(version: u64, ttl: Option<u32>) -> anyhow::Result<Resolution> {
        Ok(Resolution::new(
            "target.shu.edu.cn".to_owned(),
            Source::Campus {
                server: "10.99.0.53".to_owned(),
            },
            version,
            vec![Answer {
                addresses: vec!["10.99.0.7".parse()?],
                ttl,
                observed: Instant::now(),
            }],
            vec![],
        ))
    }

    #[test]
    fn separately_received_address_families_merge_without_extending_the_older_ttl(
    ) -> anyhow::Result<()> {
        let cache = Cache::default();
        let epoch = cache.epoch()?;
        let first = resolution(1, Some(20))?;
        cache.insert_family(&first, epoch, RecordType::A)?;
        let mut second = resolution(1, Some(60))?;
        second.addresses = vec!["2001:db8::7".parse()?];
        let merged = cache.insert_family(&second, epoch, RecordType::AAAA)?;
        assert!(merged.addresses.contains(&"10.99.0.7".parse()?));
        assert!(merged.addresses.contains(&"2001:db8::7".parse()?));
        assert!(merged.expires <= first.expires);
        let stored = cache
            .lookup(&first.host, &first.source, 1, epoch)?
            .0
            .ok_or_else(|| anyhow::anyhow!("cache missing"))?;
        assert_eq!(stored.addresses.len(), 2);
        Ok(())
    }

    #[test]
    fn cache_separates_sources_and_versions_and_rejects_results_from_a_previous_epoch(
    ) -> anyhow::Result<()> {
        let cache = Cache::default();
        let first = resolution(1, Some(30))?;
        let epoch = cache.epoch()?;
        cache.lookup(&first.host, &first.source, 1, epoch)?;
        cache.insert(&first, epoch)?;
        assert!(cache
            .lookup(&first.host, &Source::System, 1, epoch)?
            .0
            .is_none());
        let second = resolution(2, Some(30))?;
        let (_, epoch2) = cache.lookup(&second.host, &second.source, 2, epoch)?;
        cache.insert(&first, epoch)?;
        assert!(cache
            .lookup(&first.host, &first.source, 1, epoch)?
            .0
            .is_none());
        cache.clear()?;
        cache.insert(&second, epoch2)?;
        let current = cache.epoch()?;
        assert!(cache
            .lookup(&second.host, &second.source, 2, current)?
            .0
            .is_none());
        let mut fresh = resolution(2, Some(30))?;
        fresh.addresses = vec!["10.99.0.8".parse()?];
        cache.insert(&fresh, current)?;
        assert!(cache
            .lookup(&second.host, &second.source, 2, epoch2)?
            .0
            .is_none());
        cache.insert(&second, epoch2)?;
        assert_eq!(
            cache
                .lookup(&fresh.host, &fresh.source, 2, current)?
                .0
                .ok_or_else(|| anyhow::anyhow!("fresh cache entry missing"))?
                .addresses,
            fresh.addresses
        );
        Ok(())
    }

    #[test]
    fn zero_unknown_and_expired_ttl_never_produce_a_cache_hit() -> anyhow::Result<()> {
        let cache = Cache::default();
        for ttl in [Some(0), None, Some(1)] {
            let mut entry = resolution(1, ttl)?;
            if ttl == Some(1) {
                entry.expires = Some(Instant::now() - Duration::from_secs(1));
            }
            let epoch = cache.epoch()?;
            cache.lookup(&entry.host, &entry.source, 1, epoch)?;
            cache.insert(&entry, epoch)?;
            assert!(cache
                .lookup(&entry.host, &entry.source, 1, epoch)?
                .0
                .is_none());
        }
        Ok(())
    }

    #[test]
    fn an_answer_that_expired_while_another_family_was_pending_is_not_usable() -> anyhow::Result<()>
    {
        let result = Resolution::new(
            "target.shu.edu.cn".to_owned(),
            Source::Campus {
                server: "10.99.0.53".to_owned(),
            },
            1,
            vec![Answer {
                addresses: vec!["10.99.0.7".parse()?],
                ttl: Some(1),
                observed: Instant::now() - Duration::from_secs(2),
            }],
            vec![],
        );
        assert!(result.addresses.is_empty());
        let zero = resolution(1, Some(0))?;
        assert!(!zero.addresses.is_empty());
        Ok(())
    }

    #[test]
    fn cache_retains_at_most_512_domain_results() -> anyhow::Result<()> {
        let cache = Cache::default();
        for index in 0..513 {
            let mut entry = resolution(1, Some(60))?;
            entry.host = format!("host{index}.shu.edu.cn");
            let epoch = cache.epoch()?;
            cache.lookup(&entry.host, &entry.source, 1, epoch)?;
            cache.insert(&entry, epoch)?;
        }
        assert_eq!(cache.statistics()?["cached_entries"], 512);
        Ok(())
    }

    #[test]
    fn cname_ttl_limits_addresses_and_cycles_and_conflicting_aliases_are_rejected(
    ) -> anyhow::Result<()> {
        let name = Name::from_ascii("target.shu.edu.cn.")?;
        let alias = Name::from_ascii("alias.shu.edu.cn.")?;
        let mut query = Message::new();
        query
            .set_id(1)
            .add_query(Query::query(name.clone(), RecordType::A));
        let mut response = query.clone();
        response
            .set_message_type(MessageType::Response)
            .add_answer(Record::from_rdata(
                alias.clone(),
                60,
                RData::A(A("10.99.0.7".parse()?)),
            ))
            .add_answer(Record::from_rdata(
                name.clone(),
                2,
                RData::CNAME(CNAME(alias.clone())),
            ));
        assert_eq!(parse_response(&query, &response)?.ttl, Some(2));
        response.add_answer(Record::from_rdata(
            alias,
            60,
            RData::CNAME(CNAME(name.clone())),
        ));
        assert!(parse_response(&query, &response).is_err());
        let mut conflict = query.clone();
        conflict
            .set_message_type(MessageType::Response)
            .add_answer(Record::from_rdata(
                name.clone(),
                60,
                RData::CNAME(CNAME(Name::from_ascii("a.example.")?)),
            ))
            .add_answer(Record::from_rdata(
                name,
                60,
                RData::CNAME(CNAME(Name::from_ascii("b.example.")?)),
            ));
        assert!(parse_response(&query, &conflict).is_err());
        Ok(())
    }
}
