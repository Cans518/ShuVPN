use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::model::*;
use super::predicate::{self, Budget, Sources};
pub use super::predicate::{Predicate, Term};

pub const MAX_RULES: usize = 20_000;
pub const MAX_DOCUMENT: usize = 1024 * 1024;

pub fn capabilities() -> serde_json::Value {
    predicate::capabilities()
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub source: String,
    pub term: Term,
    pub policy: String,
    pub action: Action,
}

#[derive(Clone, Debug)]
pub struct Compiled {
    pub name: String,
    pub rules: Vec<Rule>,
    pub automatic_campus: bool,
    pub campus_offset: usize,
    pub notices: Vec<String>,
    pub uses_process: bool,
}

pub fn parse_document<T: serde::de::DeserializeOwned>(document: &str) -> Result<T, RoutingFailure> {
    if document.len() > MAX_DOCUMENT {
        return Err(RoutingFailure::new("size", "规则配置超过大小限制"));
    }
    serde_yaml_ng::from_str(document)
        .map_err(|_| RoutingFailure::new("syntax", "规则配置格式无效或包含未知字段"))
}

pub fn compile(
    profile: &Profile,
    overrides: &Overrides,
    payloads: &BTreeMap<String, Vec<String>>,
) -> Result<Compiled, RoutingFailure> {
    if profile.rules.len() + overrides.prepend.len() + overrides.append.len() > MAX_RULES
        || profile.providers.len() > 128
        || profile.groups.len() > 128
        || profile.name.trim().is_empty()
    {
        return Err(RoutingFailure::new("size", "规则数量或配置名称无效"));
    }
    let groups = policy_groups(profile, overrides)?;
    let mut providers = Sources::default();
    let mut budget = Budget::default();
    let mut provider_cost = 0;
    let mut geo_entries = 0;
    let mut geo_bytes = 0;
    let mut dataset_notices = vec![];
    for (name, provider) in &profile.providers {
        if !super::geodata::is_geo(provider) && provider.data.is_some() {
            return Err(RoutingFailure::new(
                "provider",
                "普通规则集不能包含 GeoData 描述符",
            ));
        }
        if name.trim().is_empty()
            || !matches!(
                provider.behavior.as_str(),
                "domain" | "ipcidr" | "classical" | "geoip" | "geosite"
            )
        {
            return Err(RoutingFailure::new("provider", "规则集名称或行为无效"));
        }
        let payload = payloads
            .get(name)
            .ok_or_else(|| RoutingFailure::new("provider", "缺少必需规则集"))?;
        if super::geodata::is_geo(provider) {
            let cached = super::geodata::cached(payload, provider)?;
            geo_entries += cached.lines.len();
            geo_bytes += cached.lines.iter().map(String::len).sum::<usize>();
            if geo_entries > super::geodata::MAX_ENTRIES
                || geo_bytes > super::geodata::MAX_PAYLOAD_BYTES
            {
                return Err(RoutingFailure::new("size", "GeoData 全局编译预算超限"));
            }
            let predicate = if provider.behavior == "geoip" {
                Predicate::GeoIp(Arc::new(super::geodata::GeoIpIndex::build(
                    cached.lines,
                    cached.inverse,
                )?))
            } else {
                Predicate::GeoSite(Arc::new(super::geodata::GeoSiteIndex::build(
                    cached.lines,
                    |value| budget.regex(value),
                )?))
            };
            let data = provider
                .data
                .as_ref()
                .ok_or_else(|| RoutingFailure::new("geodata", "GeoData 信息缺失"))?;
            let code = super::geodata::canonical_selector(&data.code)?;
            let categories = if provider.behavior == "geoip" {
                &mut providers.geoip
            } else {
                &mut providers.geosite
            };
            if categories.insert(code.clone(), predicate.clone()).is_some() {
                return Err(RoutingFailure::new("geodata", "GeoData 类别重复配置"));
            }
            providers.sets.insert(
                name.clone(),
                Arc::new(vec![Term {
                    predicate,
                    no_resolve: false,
                }]),
            );
            dataset_notices.push(format!(
                "{} {} · 版本 {} · SHA-256 {} · {} 条",
                provider.behavior,
                code,
                data.version,
                data.sha256.to_ascii_uppercase(),
                cached.lines.len()
            ));
            continue;
        }
        if payload.len() > MAX_RULES {
            return Err(RoutingFailure::new("size", "规则集条数过多"));
        }
        if payload.iter().any(|line| line.len() > 4096) {
            return Err(RoutingFailure::new("size", "单条规则集条目过大"));
        }
        let terms = payload
            .iter()
            .map(|line| provider_term(&provider.behavior, line, &mut budget))
            .collect::<Result<Vec<_>, _>>()?;
        provider_cost += terms.iter().map(predicate::cost).sum::<usize>();
        if provider_cost > 100_000 {
            return Err(RoutingFailure::new("size", "规则集编译超过匹配预算"));
        }
        providers.sets.insert(name.clone(), Arc::new(terms));
    }
    let mut rules = Vec::new();
    let mut final_rule = None;
    let mut notices = dataset_notices;
    let mut campus_offset = 0;
    let mut cost = 0;
    for (source, input) in [
        ("prepend", &overrides.prepend),
        ("profile", &profile.rules),
        ("append", &overrides.append),
    ] {
        for (index, entry) in input.iter().enumerate() {
            let rule = parse_rule(
                entry.expression(),
                source,
                index,
                &providers,
                &groups,
                overrides,
                &mut budget,
            )?;
            cost += predicate::cost(&rule.term);
            if cost > 100_000 {
                return Err(RoutingFailure::new("size", "规则展开超过匹配预算"));
            }
            if matches!(rule.term.predicate, Predicate::Match) {
                if source != "profile" || final_rule.is_some() || !entry.enabled() {
                    return Err(RoutingFailure::new(
                        "match",
                        "MATCH 必须在配置档中启用且只能出现一次",
                    ));
                }
                if index + 1 != input.len() {
                    notices.push("配置档 MATCH 已归一到最终兜底位置".to_owned());
                }
                final_rule = Some(rule);
            } else if entry.enabled() {
                rules.push(rule);
            }
        }
        if source == "profile" {
            campus_offset = rules.len();
        }
    }
    let final_rule =
        final_rule.ok_or_else(|| RoutingFailure::new("match", "配置缺少最终 MATCH"))?;
    rules.push(final_rule);
    let shadows = shadowed_rules(profile, overrides)?;
    for (rule, earlier) in shadows.iter().take(32) {
        notices.push(format!(
            "规则 {rule} 与更早规则 {earlier} 条件文本相同，被首条匹配遮蔽"
        ));
    }
    if shadows.len() > 32 {
        notices.push(format!(
            "另有 {} 条重复条件，完整位置见规则卡片",
            shadows.len() - 32
        ));
    }
    let uses_process = rules.iter().any(|rule| predicate::uses_process(&rule.term));
    if uses_process {
        notices.push("进程规则按当前可见套接字归属快照匹配，不能排除隐藏的共享持有者，也不是进程认证。必需身份未知时拒绝决策；已有流保留原决策。".to_owned());
    }
    Ok(Compiled {
        name: profile.name.clone(),
        rules,
        automatic_campus: profile.automatic_campus,
        campus_offset,
        notices,
        uses_process,
    })
}

/// Compare exact enabled condition text, without inferring regex or logic inclusion.
pub(crate) fn shadowed_rules(
    profile: &Profile,
    overrides: &Overrides,
) -> Result<BTreeMap<String, String>, RoutingFailure> {
    if profile.rules.len() + overrides.prepend.len() + overrides.append.len() > MAX_RULES {
        return Err(RoutingFailure::new("size", "规则数量超过限制"));
    }
    let mut seen = BTreeMap::<_, String>::new();
    let mut shadows = BTreeMap::new();
    for (source, entries) in [
        ("prepend", &overrides.prepend),
        ("profile", &profile.rules),
        ("append", &overrides.append),
    ] {
        for (index, entry) in entries.iter().enumerate() {
            if !entry.enabled() {
                continue;
            }
            let parts = rule_fields(entry.expression())?;
            if parts.kind == "MATCH" {
                continue;
            }
            let id = format!("{source}:{index}");
            let key = (parts.kind, parts.value, parts.no_resolve);
            if let Some(earlier) = seen.get(&key) {
                shadows.insert(id, earlier.clone());
            } else {
                seen.insert(key, id);
            }
        }
    }
    Ok(shadows)
}

fn builtin(policy: &str) -> Option<Action> {
    match policy {
        "DIRECT" => Some(Action::Direct),
        "REJECT" => Some(Action::Reject),
        "SHU-VPN" => Some(Action::ShuVpn),
        _ => None,
    }
}

pub(crate) fn policy_groups<'a>(
    profile: &'a Profile,
    overrides: &Overrides,
) -> Result<BTreeMap<String, &'a PolicyGroup>, RoutingFailure> {
    if profile.groups.len() > 128 {
        return Err(RoutingFailure::new("size", "策略组数量超过限制"));
    }
    let mut groups = BTreeMap::new();
    for group in &profile.groups {
        if group.kind != "select"
            || builtin(&group.name).is_some()
            || group.name.trim().is_empty()
            || group.proxies.is_empty()
            || group.proxies.len() > 128
            || groups.insert(group.name.clone(), group).is_some()
        {
            return Err(RoutingFailure::new(
                "group",
                "策略组重复、类型不支持或选项无效",
            ));
        }
    }
    for (name, selection) in &overrides.selections {
        let group = groups
            .get(name)
            .ok_or_else(|| RoutingFailure::new("group", "选择了不存在的策略组"))?;
        if !group.proxies.contains(selection) {
            return Err(RoutingFailure::new("group", "策略组选择不在候选列表中"));
        }
    }
    for group in &profile.groups {
        for policy in &group.proxies {
            resolve_policy(policy, &groups, overrides, &mut BTreeSet::new())?;
        }
    }
    Ok(groups)
}

fn resolve_policy(
    policy: &str,
    groups: &BTreeMap<String, &PolicyGroup>,
    overrides: &Overrides,
    visiting: &mut BTreeSet<String>,
) -> Result<Action, RoutingFailure> {
    if let Some(action) = builtin(policy) {
        return Ok(action);
    }
    if visiting.len() >= 16 || !visiting.insert(policy.to_owned()) {
        return Err(RoutingFailure::new("group", "策略组存在循环或嵌套过深"));
    }
    let group = groups
        .get(policy)
        .ok_or_else(|| RoutingFailure::new("policy", "未知策略或未支持的代理出站"))?;
    let selected = overrides
        .selections
        .get(policy)
        .unwrap_or(&group.proxies[0]);
    let action = resolve_policy(selected, groups, overrides, visiting)?;
    visiting.remove(policy);
    Ok(action)
}

fn parse_rule(
    line: &str,
    source: &str,
    index: usize,
    providers: &Sources,
    groups: &BTreeMap<String, &PolicyGroup>,
    overrides: &Overrides,
    budget: &mut Budget,
) -> Result<Rule, RoutingFailure> {
    let ParsedRule {
        kind,
        value,
        policy,
        no_resolve,
    } = rule_fields(line)?;
    let term = budget.term(kind, value, no_resolve, Some(providers))?;
    Ok(Rule {
        id: format!("{source}:{index}"),
        source: source.to_owned(),
        term,
        policy: policy.to_owned(),
        action: resolve_policy(policy, groups, overrides, &mut BTreeSet::new())?,
    })
}

pub(crate) struct ParsedRule<'a> {
    pub kind: &'a str,
    pub value: &'a str,
    pub policy: &'a str,
    pub no_resolve: bool,
}

pub(crate) fn rule_fields(line: &str) -> Result<ParsedRule<'_>, RoutingFailure> {
    let parts = predicate::fields(line)?;
    let (kind, value, policy, no_resolve) = match parts.as_slice() {
        ["MATCH", policy] => ("MATCH", "", *policy, false),
        [kind, value, policy] if *kind != "MATCH" => (*kind, *value, *policy, false),
        [kind, value, policy, "no-resolve"] => (*kind, *value, *policy, true),
        _ => return Err(RoutingFailure::new("rule", "规则字段数量无效")),
    };
    Ok(ParsedRule {
        kind,
        value,
        policy,
        no_resolve,
    })
}

fn provider_term(behavior: &str, line: &str, budget: &mut Budget) -> Result<Term, RoutingFailure> {
    let line = line.trim();
    match behavior {
        "domain" => {
            let predicate = if let Some(value) = line.strip_prefix("+.") {
                Predicate::Suffix(normalize_domain(value)?)
            } else if let Some(value) = line.strip_prefix('.') {
                Predicate::Wildcard(format!("**.{}", normalize_domain(value)?))
            } else if line.contains('*') {
                if line
                    .split('.')
                    .any(|label| label.contains('*') && label != "*")
                {
                    return Err(RoutingFailure::new(
                        "provider",
                        "域名通配符只能占据完整标签",
                    ));
                }
                let pattern = line
                    .trim_end_matches('.')
                    .split('.')
                    .map(|label| {
                        if label == "*" {
                            Ok("*".to_owned())
                        } else {
                            normalize_domain(label)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(".");
                Predicate::Wildcard(pattern)
            } else {
                Predicate::Domain(normalize_domain(line)?)
            };
            Ok(Term {
                predicate,
                no_resolve: false,
            })
        }
        "ipcidr" => budget.term(
            if line.contains(':') {
                "IP-CIDR6"
            } else {
                "IP-CIDR"
            },
            line,
            false,
            None,
        ),
        "classical" => {
            let parts = predicate::fields(line)?;
            match parts.as_slice() {
                [kind, value] if !matches!(*kind, "RULE-SET" | "MATCH") => {
                    budget.term(kind, value, false, None)
                }
                [kind, value, "no-resolve"] => budget.term(kind, value, true, None),
                _ => Err(RoutingFailure::new(
                    "provider",
                    "classical 载荷不能包含策略、MATCH 或嵌套规则集",
                )),
            }
        }
        _ => Err(RoutingFailure::new("provider", "规则集行为不支持")),
    }
}
