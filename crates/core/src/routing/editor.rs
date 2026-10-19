//! Pure draft edits. Provider loading, compilation and publication remain in the store.
use super::{
    compiler::{self, rule_fields},
    model::{Overrides, Profile, RoutingFailure, RuleEntry},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Source {
    Prepend,
    Profile,
    Append,
}

impl Source {
    fn name(self) -> &'static str {
        match self {
            Self::Prepend => "prepend",
            Self::Profile => "profile",
            Self::Append => "append",
        }
    }
}

#[derive(Serialize)]
struct EditableRule {
    source: Source,
    index: usize,
    expression: String,
    enabled: bool,
    kind: String,
    value: String,
    policy: String,
    no_resolve: bool,
    final_rule: bool,
    shadowed_by: Option<String>,
}

#[derive(Serialize)]
struct EditableGroup {
    name: String,
    choices: Vec<String>,
    selected: String,
}

#[derive(Serialize)]
struct Document {
    profile: String,
    overrides: String,
    name: String,
    automatic_campus: bool,
    rules: Vec<EditableRule>,
    groups: Vec<EditableGroup>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Edit {
    Add {
        source: Source,
        expression: String,
    },
    Update {
        source: Source,
        index: usize,
        expression: String,
        enabled: bool,
    },
    Remove {
        source: Source,
        index: usize,
    },
    Move {
        source: Source,
        index: usize,
        to: usize,
    },
    Select {
        group: String,
        policy: String,
    },
    AutomaticCampus {
        enabled: bool,
    },
    Rename {
        name: String,
    },
}

pub fn inspect(profile: &str, overrides: &str) -> Result<String, RoutingFailure> {
    let (parsed, edits) = parse(profile, overrides)?;
    encode(&parsed, &edits, profile.to_owned(), overrides.to_owned())
}

pub fn edit(profile: &str, overrides: &str, operation: &str) -> Result<String, RoutingFailure> {
    let (mut parsed, mut edits) = parse(profile, overrides)?;
    // Reject malformed drafts before indexing or normalizing their rules.
    describe(&parsed, &edits)?;
    if operation.len() > 16 * 1024 {
        return Err(RoutingFailure::new("size", "规则编辑操作过大"));
    }
    let operation: Edit = serde_json::from_str(operation)
        .map_err(|_| RoutingFailure::new("edit", "规则编辑操作格式无效"))?;
    match operation {
        Edit::Add { source, expression } => {
            if rule_fields(&expression)?.kind == "MATCH" {
                return Err(RoutingFailure::new("match", "请编辑已有的最终兜底规则"));
            }
            entries(&mut parsed, &mut edits, source).push(RuleEntry::from(expression));
        }
        Edit::Update {
            source,
            index,
            expression,
            enabled,
        } => {
            let rules = entries(&mut parsed, &mut edits, source);
            let original = rules.get(index).ok_or_else(index_error)?;
            let was_final = rule_fields(original.expression())?.kind == "MATCH";
            let is_final = rule_fields(&expression)?.kind == "MATCH";
            if was_final != is_final || (is_final && !enabled) {
                return Err(RoutingFailure::new("match", "最终兜底必须保留并启用"));
            }
            rules[index] = RuleEntry::new(expression, enabled);
        }
        Edit::Remove { source, index } => {
            let rules = entries(&mut parsed, &mut edits, source);
            let original = rules.get(index).ok_or_else(index_error)?;
            if rule_fields(original.expression())?.kind == "MATCH" {
                return Err(RoutingFailure::new("match", "最终兜底不能删除"));
            }
            rules.remove(index);
        }
        Edit::Move { source, index, to } => {
            let rules = entries(&mut parsed, &mut edits, source);
            let original = rules.get(index).ok_or_else(index_error)?;
            let target = rules.get(to).ok_or_else(index_error)?;
            if rule_fields(original.expression())?.kind == "MATCH"
                || rule_fields(target.expression())?.kind == "MATCH"
            {
                return Err(RoutingFailure::new("match", "最终兜底的位置固定"));
            }
            let moved = rules.remove(index);
            rules.insert(to, moved);
        }
        Edit::Select { group, policy } => {
            let selected = parsed
                .groups
                .iter()
                .find(|candidate| candidate.name == group)
                .ok_or_else(|| RoutingFailure::new("group", "策略组不存在"))?;
            if !selected.proxies.contains(&policy) {
                return Err(RoutingFailure::new("group", "策略不在候选列表中"));
            }
            edits.selections.insert(group, policy);
        }
        Edit::AutomaticCampus { enabled } => parsed.automatic_campus = enabled,
        Edit::Rename { name } => {
            if name.trim().is_empty() || name.len() > 1024 {
                return Err(RoutingFailure::new("profile", "配置档名称为空或过长"));
            }
            parsed.name = name;
        }
    }
    // Normalize the fixed fallback after ordinary rule insertion/reordering.
    let mut final_index = None;
    for (index, rule) in parsed.rules.iter().enumerate() {
        if rule_fields(rule.expression())?.kind == "MATCH" {
            final_index = Some(index);
            break;
        }
    }
    let final_index =
        final_index.ok_or_else(|| RoutingFailure::new("match", "配置缺少最终 MATCH"))?;
    let fallback = parsed.rules.remove(final_index);
    parsed.rules.push(fallback);
    let profile = serde_json::to_string_pretty(&parsed).map_err(encoding_error)?;
    let overrides = serde_json::to_string_pretty(&edits).map_err(encoding_error)?;
    if profile.len() > compiler::MAX_DOCUMENT || overrides.len() > compiler::MAX_DOCUMENT {
        return Err(RoutingFailure::new("size", "规则配置超过大小限制"));
    }
    encode(&parsed, &edits, profile, overrides)
}

fn parse(profile: &str, overrides: &str) -> Result<(Profile, Overrides), RoutingFailure> {
    Ok((
        compiler::parse_document(profile)?,
        if overrides.trim().is_empty() {
            Overrides::default()
        } else {
            compiler::parse_document(overrides)?
        },
    ))
}

fn entries<'a>(
    profile: &'a mut Profile,
    overrides: &'a mut Overrides,
    source: Source,
) -> &'a mut Vec<RuleEntry> {
    match source {
        Source::Prepend => &mut overrides.prepend,
        Source::Profile => &mut profile.rules,
        Source::Append => &mut overrides.append,
    }
}

fn describe(profile: &Profile, overrides: &Overrides) -> Result<Vec<EditableRule>, RoutingFailure> {
    if profile.rules.len() + overrides.prepend.len() + overrides.append.len() > compiler::MAX_RULES
    {
        return Err(RoutingFailure::new("size", "规则数量超过限制"));
    }
    let mut output = vec![];
    let mut final_rule = None;
    for (source, rules) in [
        (Source::Prepend, &overrides.prepend),
        (Source::Profile, &profile.rules),
        (Source::Append, &overrides.append),
    ] {
        for (index, entry) in rules.iter().enumerate() {
            let parts = rule_fields(entry.expression())?;
            let rule = EditableRule {
                source,
                index,
                expression: entry.expression().to_owned(),
                enabled: entry.enabled(),
                kind: parts.kind.to_owned(),
                value: parts.value.to_owned(),
                policy: parts.policy.to_owned(),
                no_resolve: parts.no_resolve,
                final_rule: parts.kind == "MATCH",
                shadowed_by: None,
            };
            if rule.final_rule {
                if source != Source::Profile || !rule.enabled || final_rule.is_some() {
                    return Err(RoutingFailure::new(
                        "match",
                        "配置档必须有唯一且启用的最终 MATCH",
                    ));
                }
                final_rule = Some(rule);
            } else {
                output.push(rule);
            }
        }
    }
    output.push(final_rule.ok_or_else(|| RoutingFailure::new("match", "配置缺少最终 MATCH"))?);
    let shadows = compiler::shadowed_rules(profile, overrides)?;
    for rule in &mut output {
        rule.shadowed_by = shadows
            .get(&format!("{}:{}", rule.source.name(), rule.index))
            .cloned();
    }
    Ok(output)
}

fn encode(
    profile: &Profile,
    overrides: &Overrides,
    profile_document: String,
    overrides_document: String,
) -> Result<String, RoutingFailure> {
    let rules = describe(profile, overrides)?;
    compiler::policy_groups(profile, overrides)?;
    let groups = profile
        .groups
        .iter()
        .map(|group| {
            let selected = overrides
                .selections
                .get(&group.name)
                .or_else(|| group.proxies.first())
                .ok_or_else(|| RoutingFailure::new("group", "策略组没有候选项"))?;
            if !group.proxies.contains(selected) {
                return Err(RoutingFailure::new("group", "策略不在候选列表中"));
            }
            Ok(EditableGroup {
                name: group.name.clone(),
                choices: group.proxies.clone(),
                selected: selected.clone(),
            })
        })
        .collect::<Result<_, RoutingFailure>>()?;
    serde_json::to_string(&Document {
        profile: profile_document,
        overrides: overrides_document,
        name: profile.name.clone(),
        automatic_campus: profile.automatic_campus,
        rules,
        groups,
    })
    .map_err(encoding_error)
}

fn index_error() -> RoutingFailure {
    RoutingFailure::new("edit", "规则位置已改变，请重新加载编辑内容")
}

fn encoding_error(_: serde_json::Error) -> RoutingFailure {
    RoutingFailure::new("encoding", "规则编辑内容编码失败")
}
