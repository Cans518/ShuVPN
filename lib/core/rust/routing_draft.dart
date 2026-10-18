import 'dart:convert';

/// Presentation data decoded from the native editor; rule syntax stays in Rust.
class RoutingDraft {
  RoutingDraft.decode(String text)
    : this._(jsonDecode(text) as Map<String, Object?>);

  RoutingDraft._(Map<String, Object?> data)
    : profile = data['profile'] as String,
      overrides = data['overrides'] as String,
      name = data['name'] as String,
      automaticCampus = data['automatic_campus'] as bool,
      rules = List.unmodifiable(
        (data['rules'] as List<Object?>).map(
          (rule) => RoutingDraftRule.decode(rule as Map<String, Object?>),
        ),
      ),
      groups = List.unmodifiable(
        (data['groups'] as List<Object?>).map(
          (group) => RoutingDraftGroup.decode(group as Map<String, Object?>),
        ),
      );

  final String profile;
  final String overrides;
  final String name;
  final bool automaticCampus;
  final List<RoutingDraftRule> rules;
  final List<RoutingDraftGroup> groups;
}

class RoutingDraftRule {
  RoutingDraftRule.decode(Map<String, Object?> data)
    : source = data['source'] as String,
      index = data['index'] as int,
      expression = data['expression'] as String,
      enabled = data['enabled'] as bool,
      kind = data['kind'] as String,
      value = data['value'] as String,
      policy = data['policy'] as String,
      noResolve = data['no_resolve'] as bool,
      finalRule = data['final_rule'] as bool,
      shadowedBy = data['shadowed_by'] as String?;

  final String source;
  final int index;
  final String expression;
  final bool enabled;
  final String kind;
  final String value;
  final String policy;
  final bool noResolve;
  final bool finalRule;
  final String? shadowedBy;

  Map<String, Object?> update({String? expression, bool? enabled}) => {
    'type': 'update',
    'source': source,
    'index': index,
    'expression': expression ?? this.expression,
    'enabled': enabled ?? this.enabled,
  };
}

class RoutingDraftGroup {
  RoutingDraftGroup.decode(Map<String, Object?> data)
    : name = data['name'] as String,
      choices = List.unmodifiable(
        (data['choices'] as List<Object?>).cast<String>().toSet(),
      ),
      selected = data['selected'] as String;

  final String name;
  final List<String> choices;
  final String selected;
}
