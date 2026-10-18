"""Exercise rule dialogs through named controls in the production Linux desktop."""
import hashlib
import json
import sys
import time

import desktop_gui_acceptance as gui
from rule_gui_evidence import SECTIONS, RAW_LABELS, card_label, fixture_rules, validate_rule_readback


def unique_named_node(nodes, label, role):
    matches = [node for node in nodes if node.get_name().split("\n")[0] == label and
               node.get_role_name() == role]
    if len(matches) > 1:
        raise RuntimeError(f"Ambiguous accessible rule control: {label}")
    return matches[0] if matches else None


def validate_action_target(target, pid, label, role):
    if (type(pid) is not int or pid <= 0 or target.get("pid") != pid or
            type(target.get("pid")) is not int or target.get("name", "").split("\n")[0] != label or
            target.get("role") != role or target.get("enabled") is not True or
            target.get("showing") is not True):
        raise RuntimeError("Rule action is not the expected enabled, showing, owned named control")


class RuleSession(gui.Session):
    def outer_scroll(self, count):
        gui.command("xdotool", "mousemove", "--window", self.window, 8, 500,
                    "click", "--repeat", abs(count), "--delay", 50,
                    "5" if count > 0 else "4")
        time.sleep(0.2)

    def named_node(self, label, role):
        for direction in (1, -1):
            for _ in range(12):
                node = unique_named_node(gui.accessible_nodes(self.application.pid), label, role)
                if node is not None and self.node_observation(node)["showing"]:
                    return node
                self.outer_scroll(3 * direction)
        raise RuntimeError(f"Named rule control not exposed after bounded scrolling: {label}")

    def activate(self, node, label, role="push button"):
        observation = self.node_observation(node)
        validate_action_target(observation, self.application.pid, label, role)
        if node.get_n_actions() < 1 or not node.do_action(0):
            raise RuntimeError(f"Named rule control has no successful platform action: {label}")
        self.steps.append({"named_rule_action": observation})
        time.sleep(0.35)

    def control(self, label):
        if label in ("校验并预览", "应用预览", "导入与高级编辑"):
            role = "panel" if label == "导入与高级编辑" else "push button"
            return self.activate(self.named_node(label, role), label, role)
        return super().control(label)

    def raw(self):
        result = []
        for label in RAW_LABELS:
            node = self.named_node(label, "text")
            text = node.get_text_iface()
            result.append(text.get_text(0, text.get_character_count()))
        return tuple(result)

    def add(self, source):
        label = f"新增{SECTIONS[source]}"
        self.activate(self.named_node(label, "push button"), label)

    def card_label(self, source, position, action, expression):
        return card_label(source, position, action, expression)

    def card_action(self, source, position, action, expression):
        label = self.card_label(source, position, action, expression)
        self.activate(self.named_node(label, "push button"), label)

    def card_semantics(self, source, position, expression):
        observations = []
        for action in ("编辑", "上移", "下移", "删除"):
            label = self.card_label(source, position, action, expression)
            node = self.named_node(label, "push button")
            value = self.node_observation(node)
            expected_enabled = action != "下移" and (action != "上移" or position > 1)
            if (value["pid"] != self.application.pid or value["enabled"] is not expected_enabled):
                raise RuntimeError("Named icon capability differs from the current rule position")
            observations.append(value)
        label = self.card_label(source, position, "启用或停用", expression)
        node = self.named_node(label, "toggle button")
        toggle = self.node_observation(node)
        validate_action_target(toggle, self.application.pid, label, "toggle button")
        self.steps.append({"rule_card_semantics": observations, "toggle": toggle})

    def enter(self, initial, value):
        self.activate_editor(self.editor_node(initial))
        self.key("ctrl+a")
        gui.command("xdotool", "type", "--clearmodifiers", "--delay", "1", value)
        self.editor_node(value)
        self.steps.append({"typed_and_text_readback": value})

    def applied(self, expected):
        def ready():
            record = json.loads(self.routing_file().read_bytes())
            return record if record["version"] == expected else None
        return gui.wait_for(ready, f"routing commit version {expected}")

    def source_rules(self, source, raw):
        return fixture_rules(source, raw)

    def readback(self, version, created_host, edited_host, action, source):
        native = self.probe("read", rule_hosts=(created_host, edited_host))
        validate_rule_readback(native, version, created_host, edited_host, action, source)
        self.steps.append({"independent_rule_readback": native})
        return native

    def dialogs(self, source, version):
        created_host, edited_host = (f"{kind}.{source}.gui.invalid" for kind in ("created", "edited"))
        created, edited = (f"DOMAIN,{host},DIRECT" for host in (created_host, edited_host))
        self.start(f"{source}-edit-home")
        self.control("设置")
        self.control("规则分流")
        self.control("导入与高级编辑")
        original = self.raw()
        before = self.routing_file().read_bytes()
        if json.loads(before)["version"] != version:
            raise RuntimeError("Rule dialogs did not start from the expected active version")
        self.add(source)
        self.enter("DOMAIN-SUFFIX,shu.edu.cn,SHU-VPN", created)
        self.snapshot(f"{source}-add-cancel")
        self.control("取消")
        assert self.raw() == original and self.routing_file().read_bytes() == before
        self.add(source)
        self.enter("DOMAIN-SUFFIX,shu.edu.cn,SHU-VPN", created)
        self.control("保存")
        gui.wait_for(lambda: self.source_rules(source, self.raw())[-1] == created,
                     "created rule in its intended source")
        added = self.raw()
        position = len(self.source_rules(source, added))
        self.card_semantics(source, position, created)
        assert self.routing_file().read_bytes() == before
        self.card_action(source, position, "编辑", created)
        self.enter(created, edited)
        self.control("取消")
        assert self.raw() == added and self.routing_file().read_bytes() == before
        self.card_action(source, position, "编辑", created)
        self.enter(created, edited)
        self.control("保存")
        gui.wait_for(lambda: self.source_rules(source, self.raw())[-1] == edited,
                     "edited rule in its intended source")
        assert self.routing_file().read_bytes() == before
        self.snapshot(f"{source}-edited-draft-no-commit")
        self.control("校验并预览")
        assert self.routing_file().read_bytes() == before
        self.control("应用预览")
        committed = self.applied(version + 1)
        self.snapshot(f"{source}-edit-applied")
        self.stop("ctrl+q")
        self.readback(version + 1, created_host, edited_host, "DIRECT", source)
        self.start(f"{source}-delete-reopened")
        self.control("设置")
        self.control("规则分流")
        self.control("导入与高级编辑")
        raw = self.raw()
        assert raw == (committed["current"]["profile"], committed["current"]["overrides"])
        self.snapshot(f"{source}-edited-rule-restored")
        committed_bytes = self.routing_file().read_bytes()
        self.card_action(source, position, "删除", edited)
        self.control_node("删除这条规则？", roles=("panel",))
        self.snapshot(f"{source}-delete-cancel")
        self.control("取消")
        assert self.raw() == raw and self.routing_file().read_bytes() == committed_bytes
        self.card_action(source, position, "删除", edited)
        self.control("删除")
        gui.wait_for(lambda: edited not in self.source_rules(source, self.raw()),
                     "deleted rule absent from its source draft")
        assert self.routing_file().read_bytes() == committed_bytes
        self.control("校验并预览")
        assert self.routing_file().read_bytes() == committed_bytes
        self.control("应用预览")
        self.applied(version + 2)
        self.snapshot(f"{source}-delete-applied")
        self.stop()
        return self.readback(version + 2, created_host, edited_host, "REJECT", source)

    def run(self):
        closed = False
        try:
            super().run()
            (self.report / "verification.json").rename(self.report / "offline-baseline.json")
            version, final = 2, None
            for source in SECTIONS:
                final = self.dialogs(source, version)
                version += 2
            assert len(self.launches) == 9 and all(row["exit"] == 0 for row in self.launches)
            for relative, expected in self.product.items():
                assert hashlib.sha256((self.bundle / relative).read_bytes()).hexdigest() == expected
            self.close()
            closed = True
            (self.report / "verification.json").write_text(json.dumps({
                "passed": True, "launches": self.launches, "steps": self.steps,
                "final": final, "product": self.product, "sources": list(SECTIONS),
                "scope": "production Linux window; named AT-SPI controls; physical keyboard/scroll; no pointer hit-test claim",
            }, ensure_ascii=False, indent=2))
            print("REAL_NAMED_RULE_DIALOGS_ALL_SOURCES_PASSED")
        except BaseException as error:
            if self.application is not None and self.application.poll() is None:
                try:
                    self.snapshot("rule-failure")
                except Exception as observation_error:
                    print(f"Rule failure snapshot failed: {observation_error}", file=sys.stderr)
            (self.report / "verification.json").write_text(json.dumps({
                "passed": False, "error": str(error), "launches": self.launches, "steps": self.steps,
            }, ensure_ascii=False, indent=2))
            raise
        finally:
            if not closed:
                gui.finish((self.close,))


if __name__ == "__main__":
    gui.main(gui.arguments(__doc__), session_type=RuleSession, entrypoint=__file__)
