use std::collections::BTreeMap;
use std::sync::Arc;

use super::model::{normalize_domain, Inbound, Network, RoutingFailure};

const MAX_DEPTH: usize = 16;
const MAX_NODES: usize = 100_000;
const MAX_REGEXES: usize = 128;
const MAX_REGEX_BYTES: usize = 1024;
const REGEX_PROGRAM_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug)]
pub enum Predicate {
    Domain(String),
    Suffix(String),
    Keyword(String),
    Wildcard(String),
    Regex(Arc<regex::Regex>),
    GeoIp(Arc<super::geodata::GeoIpIndex>),
    GeoSite(Arc<super::geodata::GeoSiteIndex>),
    Ip(ipnet::IpNet),
    Ports(Vec<(u16, u16)>),
    Network(Network),
    Inbound(Vec<Inbound>),
    ProcessName(String),
    ProcessPath(String),
    Set(Arc<Vec<Term>>),
    And(Vec<Term>),
    Or(Vec<Term>),
    Not(Box<Term>),
    Match,
}

#[derive(Clone, Debug)]
pub struct Term {
    pub predicate: Predicate,
    pub no_resolve: bool,
}

#[derive(Default)]
pub(super) struct Budget {
    nodes: usize,
    regexes: usize,
}

#[derive(Default)]
pub(super) struct Sources {
    pub sets: BTreeMap<String, Arc<Vec<Term>>>,
    pub geoip: BTreeMap<String, Predicate>,
    pub geosite: BTreeMap<String, Predicate>,
}

/// Commas inside a logical expression, regex group, class, quantifier, or
/// escaped literal are payload bytes rather than rule field separators.
pub(super) fn fields(line: &str) -> Result<Vec<&str>, RoutingFailure> {
    if line.len() > 4096 || line.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(error("rule", "单条规则过大或包含控制字符"));
    }
    let mut result = vec![];
    let mut start = 0;
    // Character-class state: 0 at the opening bracket, 1 after negation,
    // 2 after a member. A leading ']' is a literal rather than a closer.
    let mut stack: Vec<(char, u8)> = vec![];
    let mut escaped = false;
    let mut quoted = false;
    let mut regex_depth = None;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            if let Some(('[', state)) = stack.last_mut() {
                *state = 2;
            }
            escaped = true;
            continue;
        }
        if quoted {
            if character == '"' {
                quoted = false;
            }
            continue;
        }
        if let Some(('[', state)) = stack.last_mut() {
            match character {
                '^' if *state == 0 => {
                    *state = 1;
                    continue;
                }
                ']' if *state < 2 => {
                    *state = 2;
                    continue;
                }
                '[' | ']' => {}
                _ => {
                    *state = 2;
                    continue;
                }
            }
            if character == '[' {
                *state = 2;
            }
        }
        if character == '"'
            && regex_depth.is_none()
            && line[..index]
                .trim_end()
                .strip_suffix(',')
                .is_some_and(|prefix| {
                    follows_kind(prefix, "PROCESS-NAME") || follows_kind(prefix, "PROCESS-PATH")
                })
        {
            quoted = true;
            continue;
        }
        if character == ',' {
            if regex_depth == Some(0) && stack.is_empty() {
                regex_depth = None;
            } else if regex_depth.is_none() && follows_kind(&line[..index], "DOMAIN-REGEX") {
                regex_depth = Some(stack.len());
            }
        }
        match character {
            '(' | '[' | '{' => {
                if stack.len() >= 64 {
                    return Err(error("size", "规则分隔嵌套过深"));
                }
                stack.push((character, 0));
            }
            ')' | ']' | '}' => {
                let expected = match character {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if stack.pop().map(|(kind, _)| kind) != Some(expected) {
                    return Err(error("rule", "规则括号不匹配"));
                }
                if regex_depth.is_some_and(|depth| stack.len() < depth) {
                    regex_depth = None;
                }
            }
            ',' if stack.is_empty() => {
                result.push(line[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if !stack.is_empty() || escaped || quoted {
        return Err(error("rule", "规则括号或转义未结束"));
    }
    result.push(line[start..].trim());
    Ok(result)
}

fn follows_kind(prefix: &str, kind: &str) -> bool {
    prefix.trim_end().strip_suffix(kind).is_some_and(|before| {
        let before = before.trim_end();
        before.is_empty() || before.ends_with(['(', ','])
    })
}

impl Budget {
    pub(super) fn term(
        &mut self,
        kind: &str,
        value: &str,
        no_resolve: bool,
        providers: Option<&Sources>,
    ) -> Result<Term, RoutingFailure> {
        self.parse(kind, value, no_resolve, providers, 0)
    }

    fn parse(
        &mut self,
        kind: &str,
        value: &str,
        no_resolve: bool,
        providers: Option<&Sources>,
        depth: usize,
    ) -> Result<Term, RoutingFailure> {
        self.nodes += 1;
        if depth > MAX_DEPTH || self.nodes > MAX_NODES {
            return Err(error("size", "规则表达式超出节点或深度预算"));
        }
        if no_resolve && !matches!(kind, "IP-CIDR" | "IP-CIDR6" | "GEOIP" | "RULE-SET") {
            return Err(error("rule", "no-resolve 仅适用于目标 IP 规则或规则集"));
        }
        let predicate = match kind {
            "AND" | "OR" | "NOT" => {
                let expressions = fields(unwrap_group(value)?)?;
                if expressions.is_empty()
                    || expressions.iter().any(|term| term.is_empty())
                    || (kind == "NOT" && expressions.len() != 1)
                {
                    return Err(error("rule", "逻辑规则缺少条件或 NOT 条件数量无效"));
                }
                let mut terms = Vec::with_capacity(expressions.len());
                for expression in expressions {
                    let parts = fields(unwrap_group(expression)?)?;
                    let (child_kind, child_value, child_no_resolve) = match parts.as_slice() {
                        [kind, value] if *kind != "MATCH" => (*kind, *value, false),
                        [kind, value, "no-resolve"] if *kind != "MATCH" => (*kind, *value, true),
                        _ => return Err(error("rule", "逻辑条件不能携带策略或 MATCH")),
                    };
                    terms.push(self.parse(
                        child_kind,
                        child_value,
                        child_no_resolve,
                        providers,
                        depth + 1,
                    )?);
                }
                match kind {
                    "AND" => Predicate::And(terms),
                    "OR" => Predicate::Or(terms),
                    _ => Predicate::Not(Box::new(
                        terms.pop().ok_or_else(|| error("rule", "NOT 条件缺失"))?,
                    )),
                }
            }
            "RULE-SET" => Predicate::Set(
                providers
                    .ok_or_else(|| error("provider", "classical 载荷不能嵌套规则集"))?
                    .sets
                    .get(value)
                    .ok_or_else(|| error("provider", "规则引用了缺失的规则集"))?
                    .clone(),
            ),
            "GEOIP" | "GEOSITE" => {
                let providers =
                    providers.ok_or_else(|| error("provider", "classical 载荷不能引用 GeoData"))?;
                let sources = if kind == "GEOIP" {
                    &providers.geoip
                } else {
                    &providers.geosite
                };
                sources
                    .get(&super::geodata::canonical_selector(value)?)
                    .ok_or_else(|| error("geodata", "缺少匹配类别的 GeoData 数据集"))?
                    .clone()
            }
            "DOMAIN" => Predicate::Domain(normalize_domain(value)?),
            "DOMAIN-SUFFIX" => Predicate::Suffix(normalize_domain(value)?),
            "DOMAIN-KEYWORD" => Predicate::Keyword(normalize_domain(value)?),
            "DOMAIN-REGEX" => Predicate::Regex(self.regex(value)?),
            "PROCESS-NAME" | "PROCESS-PATH" => {
                let literal: String = if value.starts_with('"') {
                    serde_json::from_str(value)
                        .map_err(|_| error("process", "进程字面量需要有效的 JSON 字符串"))?
                } else {
                    value.to_owned()
                };
                let value = literal.as_str();
                if value.is_empty() || value.len() > 4096 {
                    return Err(error("process", "进程名称或路径为空或过长"));
                }
                if value.chars().any(char::is_control) {
                    return Err(error("process", "进程字面量包含控制字符"));
                }
                if kind == "PROCESS-NAME" {
                    if value.contains('/') || (cfg!(target_os = "windows") && value.contains('\\'))
                    {
                        return Err(error("process", "PROCESS-NAME 只接受可执行文件名"));
                    }
                    Predicate::ProcessName(if cfg!(target_os = "windows") {
                        value.to_lowercase()
                    } else {
                        value.to_owned()
                    })
                } else {
                    if !std::path::Path::new(value).is_absolute() {
                        return Err(error("process", "PROCESS-PATH 需要绝对可执行文件路径"));
                    }
                    Predicate::ProcessPath(if cfg!(target_os = "windows") {
                        value.replace('/', "\\").to_lowercase()
                    } else {
                        value.to_owned()
                    })
                }
            }
            "IP-CIDR" | "IP-CIDR6" => Predicate::Ip(
                value
                    .parse::<ipnet::IpNet>()
                    .map_err(|_| error("cidr", "CIDR 无效"))?,
            ),
            "DST-PORT" => {
                let mut ports = vec![];
                for range in value.split('/') {
                    if ports.len() >= 128 {
                        return Err(error("size", "端口分段过多"));
                    }
                    let (low, high) = range.split_once('-').unwrap_or((range, range));
                    let low = low.parse::<u16>().map_err(|_| error("port", "端口无效"))?;
                    let high = high.parse::<u16>().map_err(|_| error("port", "端口无效"))?;
                    if low == 0 || low > high {
                        return Err(error("port", "端口范围无效"));
                    }
                    ports.push((low, high));
                }
                Predicate::Ports(ports)
            }
            "NETWORK" => Predicate::Network(match value.to_ascii_lowercase().as_str() {
                "tcp" => Network::Tcp,
                "udp" => Network::Udp,
                _ => return Err(error("network", "网络协议不支持")),
            }),
            "IN-TYPE" => Predicate::Inbound(
                value
                    .split('/')
                    .map(|kind| match kind.to_ascii_lowercase().as_str() {
                        "http" => Ok(Inbound::Http),
                        "socks" | "socks5" => Ok(Inbound::Socks),
                        "tun" => Ok(Inbound::Tun),
                        _ => Err(error("inbound", "入站类型不支持")),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            "MATCH" => Predicate::Match,
            _ => return Err(error("unsupported", "规则类型尚未支持")),
        };
        Ok(Term {
            predicate,
            no_resolve,
        })
    }

    pub(super) fn regex(&mut self, value: &str) -> Result<Arc<regex::Regex>, RoutingFailure> {
        self.regexes += 1;
        if value.is_empty() || value.len() > MAX_REGEX_BYTES || self.regexes > MAX_REGEXES {
            return Err(error("size", "正则数量或表达式长度超过预算"));
        }
        regex::RegexBuilder::new(value)
            .size_limit(REGEX_PROGRAM_BYTES)
            .dfa_size_limit(REGEX_PROGRAM_BYTES)
            .nest_limit(32)
            .build()
            .map(Arc::new)
            .map_err(|_| error("regex", "正则语法不支持或编译超出预算"))
    }
}

fn unwrap_group(value: &str) -> Result<&str, RoutingFailure> {
    value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| error("rule", "逻辑条件必须使用括号包围"))
}

pub(super) fn cost(term: &Term) -> usize {
    match &term.predicate {
        Predicate::Ports(ports) => ports.len().max(1),
        Predicate::Inbound(values) => values.len().max(1),
        Predicate::Set(terms) => terms.iter().map(cost).sum::<usize>().max(1),
        Predicate::And(terms) | Predicate::Or(terms) => 1 + terms.iter().map(cost).sum::<usize>(),
        Predicate::Not(term) => 1 + cost(term),
        Predicate::Regex(regex) => 1 + regex.as_str().len(),
        _ => 1,
    }
}

pub(super) fn uses_process(term: &Term) -> bool {
    match &term.predicate {
        Predicate::ProcessName(_) | Predicate::ProcessPath(_) => true,
        Predicate::Set(terms) => terms.iter().any(uses_process),
        Predicate::And(terms) | Predicate::Or(terms) => terms.iter().any(uses_process),
        Predicate::Not(term) => uses_process(term),
        _ => false,
    }
}

fn error(code: &str, message: &str) -> RoutingFailure {
    RoutingFailure::new(code, message)
}

pub(super) fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "schema": 1,
        "supported": ["DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-REGEX",
            "IP-CIDR", "IP-CIDR6", "DST-PORT", "NETWORK", "IN-TYPE", "RULE-SET",
            "AND", "OR", "NOT", "MATCH", "GEOIP", "GEOSITE", "PROCESS-NAME", "PROCESS-PATH"],
        "unsupported": ["SCRIPT"],
        "geoDataFormat": "v2fly protobuf .dat with pinned version and SHA-256",
        "geoDataMaxBytes": super::geodata::MAX_BYTES,
        "geoDataMaxEntries": super::geodata::MAX_ENTRIES,
        "logicMaxDepth": MAX_DEPTH,
        "expressionMaxNodes": MAX_NODES,
        "regexMaxCount": MAX_REGEXES,
        "regexMaxBytes": MAX_REGEX_BYTES,
        "regexProgramBytes": REGEX_PROGRAM_BYTES,
        "regexDialect": "Rust regex: no backreferences or look-around",
        "domainInput": "lowercase IDNA ASCII",
        "processIdentity": cfg!(any(target_os = "windows", target_os = "linux")),
        "processIdentityScope": "current visible local socket ownership snapshot; hidden shared holders cannot be excluded; not historical sender attestation",
        "processIdentityMatrix": {
            "Windows": {"HTTP": "本机 TCP 客户端", "SOCKS": "本机 TCP 客户端", "TUN": "本机 TCP；UDP 需有确定地址族的端点及唯一可见归属"},
            "Linux": {"HTTP": "本机 TCP 客户端", "SOCKS": "本机 TCP 客户端", "TUN": "当前网络空间的 TCP/UDP，受可见权限限制；跨族通配绑定不可确定时拒绝归属"}
        },
        "processIdentityUnavailable": "remote LAN clients, no visible owner, visible ambiguity, lookup budget or permissions preventing attribution; a required unknown identity fails instead of falling through",
        "processMatchCase": if cfg!(target_os = "windows") { "case-insensitive" } else { "case-sensitive" },
        "processMatchStyle": "literal executable name or absolute image path; no glob expansion",
        "processLiteralSyntax": "quote names/paths containing expression delimiters as a JSON string value",
        "processLookupBudgetMs": 500,
        "processLookupConcurrency": 2,
        "processDnsCapture": "dedicated captured campus DNS requests use the DNS transport, not ordinary outbound process rules",
        "tunCoverage": "only traffic entering the configured capture set"
    })
}
