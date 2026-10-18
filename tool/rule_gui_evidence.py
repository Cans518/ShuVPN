"""Shared synthetic rule fixtures and native readback checks for desktop GUI probes."""
import json

SECTIONS = {"prepend": "前置规则", "profile": "配置档规则", "append": "后置规则"}
RAW_LABELS = ("配置档 YAML / JSON", "前置 / 后置规则与策略组选择")
NETWORK_SWITCHES = ("http_enabled", "socks_enabled", "vpn_enabled", "system_proxy_enabled")


def card_label(source, position, action, expression):
    return f"{SECTIONS[source]} 第{position}条：{action} {expression}"


def fixture_rules(source, raw):
    # The Rust editor parses real documents; this accepts only the controlled JSON fixture.
    profile, overrides = map(json.loads, raw)
    entries = profile["rules"] if source == "profile" else overrides[source]
    if any(not isinstance(rule, str) for rule in entries):
        raise RuntimeError("Unexpected fixture rule shape")
    return [rule for rule in entries if rule != "MATCH,REJECT"]


def validate_rule_readback(native, version, created_host, edited_host, action, source):
    if (native["routing_version"] != version or native["routing_test_action"] != "REJECT" or
            native["routing_override_actions"] != {
                "before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"} or
            any(native[key] is not False for key in NETWORK_SWITCHES)):
        raise RuntimeError("Independent native readback lost the active fixture configuration")
    decisions = native["routing_custom_decisions"]
    if (decisions[created_host]["action"] != "REJECT" or
            decisions[created_host]["version"] != version or
            decisions[created_host]["source"] != "profile" or
            decisions[edited_host]["action"] != action or
            decisions[edited_host]["version"] != version or
            decisions[edited_host]["source"] != (source if action == "DIRECT" else "profile")):
        raise RuntimeError("Independent native test does not match the committed rule operation")
