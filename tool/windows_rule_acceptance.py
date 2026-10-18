"""Exercise three rule sources in a production Windows private-resource copy."""
import argparse
from ctypes import wintypes
import json
from pathlib import Path
import sys
import time

from rule_gui_evidence import SECTIONS, RAW_LABELS, card_label, fixture_rules, validate_rule_readback
from windows_formal_gui_acceptance import FormalAcceptance, refuse_security_prompt
from windows_tray_acceptance import wait_for

EXPRESSION_LABEL = "规则表达式，例如 DOMAIN,oa.shu.edu.cn,SHU-VPN"

def _control_type(label):
    if label in (*RAW_LABELS, EXPRESSION_LABEL, "密码", "端口"):
        return 50004
    return 50020 if label == "导入与高级编辑" else 50000


def validate_rule_control(target, pid, label, bounds, *, control_type=50000):
    rectangle, point = target.get("rect", ()), target.get("clickable", ())
    if (type(pid) is not int or pid <= 0 or type(target.get("pid")) is not int or
            target["pid"] != pid or target.get("name", "").split("\n")[0] != label or
            target.get("control_type") != control_type or target.get("enabled") is not True or
            target.get("offscreen") is not False or len(rectangle) != 4 or len(point) != 2 or
            any(type(value) is not int for value in (*rectangle, *point, *bounds)) or
            not rectangle[0] < point[0] < rectangle[2] or
            not rectangle[1] < point[1] < rectangle[3] or
            not bounds[0] < point[0] < bounds[2] or
            not bounds[1] + 80 < point[1] < bounds[3] - 24):
        raise RuntimeError("Rule pointer target is not the expected owned, visible named control")


class RuleAcceptance(FormalAcceptance):
    expected_launches = 9

    def start_launch(self, name):
        refuse_security_prompt(self.api, self.result)
        super().start_launch(name)

    def activate_formal(self):
        refuse_security_prompt(self.api, self.result)
        if self.api.window_pid(self.window) != self.probe.pid or not self.visible():
            raise RuntimeError("Foreground request is not bound to the original visible rule window")
        self.api.user.SetForegroundWindow(self.window)
        if self.api.user.GetForegroundWindow() == self.window:
            self.steps.append({"verified_foreground_request": self.window, "pid": self.probe.pid,
                               "physical_taskbar_activation_claimed": False})
            return
        super().activate_formal()

    def find_control(self, label):
        return self.api.control(self.window, label, control_type=_control_type(label))

    def validate_click_target(self, label, target):
        super().validate_click_target(label, target)
        kind = _control_type(label)
        validate_rule_control(target, self.probe.pid, label,
                              self.api.window_rect(self.window), control_type=kind)
        hit = self.api.control_at(target["clickable"], self.probe.pid, label, control_type=kind)
        if hit is None:
            raise RuntimeError(f"The physical point does not hit the owned named rule control: {label}")
        validate_rule_control(hit, self.probe.pid, label,
                              self.api.window_rect(self.window), control_type=kind)
        target["hit_test"] = hit

    def reveal_control(self, label):
        kind = _control_type(label)
        for direction in (1, -1):
            for _ in range(12):
                candidate = self.find_control(label)
                if candidate is not None:
                    try:
                        validate_rule_control(candidate, self.probe.pid, label,
                                              self.api.window_rect(self.window), control_type=kind)
                    except RuntimeError:
                        pass
                    else:
                        hit = self.api.control_at(
                            candidate["clickable"], self.probe.pid, label, control_type=kind)
                        self.steps.append({"rule_control_hit_probe": {
                            "target": candidate, "hit": hit,
                            "observation": getattr(self.api, "last_control_hit", None)}})
                        if hit is not None:
                            validate_rule_control(hit, self.probe.pid, label,
                                                  self.api.window_rect(self.window), control_type=kind)
                            return candidate
                self.scroll(3 * direction)
        raise RuntimeError(f"Owned rule control was not visibly hit after bounded scroll: {label}")

    def scroll(self, notches):
        bounds = self.api.window_rect(self.window)
        x, y = bounds[0] + 12, bounds[1] + (bounds[3] - bounds[1]) * 3 // 5
        self.api.move([x, y, x + 1, y + 1])
        if (self.api.user.GetForegroundWindow() != self.window or
                self.api.window_pid(self.api.user.WindowFromPoint(wintypes.POINT(x, y))) != self.probe.pid):
            raise RuntimeError("Rule page scroll target is not the foreground owned GUI")
        self.api.wheel(notches)
        self.steps.append({"physical_rule_page_wheel": notches, "point": [x, y]})
        time.sleep(0.3)

    def rule_click(self, label):
        target = self.reveal_control(label)
        kind = _control_type(label)
        bounds = self.api.window_rect(self.window)
        validate_rule_control(target, self.probe.pid, label, bounds, control_type=kind)
        self.control_click(label)
        clicked = self.steps[-1]["physical_control_click"]
        validate_rule_control(clicked, self.probe.pid, label, bounds, control_type=kind)
        self.steps.append({"named_rule_pointer_action": clicked})

    def raw(self):
        return tuple(self.edit_value(label) for label in RAW_LABELS)

    def enter_expression(self, initial, value):
        self.control(EXPRESSION_LABEL)
        wait_for(lambda: self.edit_value(EXPRESSION_LABEL) == initial, "rule prompt initial native value")
        self.rule_click(EXPRESSION_LABEL)
        self.api.keys([0x11, 0x41])
        self.api.type_text(value)
        wait_for(lambda: self.edit_value(EXPRESSION_LABEL) == value, "typed rule expression readback")
        self.steps.append({"physical_rule_expression": value})

    def applied(self, version):
        def committed():
            value = json.loads(self.routing_record())
            return value if value["version"] == version else None
        return wait_for(committed, f"explicit native rule commit version {version}")

    def readback(self, version, created_host, edited_host, action, source):
        native = self.native_probe("read", rule_hosts=(created_host, edited_host))
        validate_rule_readback(native, version, created_host, edited_host, action, source)
        self.steps.append({"independent_rule_readback": native})
        return native

    def card_semantics(self, source, position, expression, before):
        self.reveal_control(card_label(source, position, "编辑", expression))
        rows = self.snapshot(f"{source}-named-rule-controls")
        observations = []
        for action in ("编辑", "上移", "下移", "删除", "启用或停用"):
            label = card_label(source, position, action, expression)
            expected_type = 50002 if action == "启用或停用" else 50000
            matches = [row for row in rows if row["name"].split("\n")[0] == label and
                       row["type"] == expected_type]
            if len(matches) != 1 or matches[0]["offscreen"]:
                raise RuntimeError("Rule card lacks its unique visible named Windows control")
            observations.append({"pid": self.probe.pid, **matches[0]})
        self.steps.append({"windows_rule_control_observations": observations,
                           "disabled_flag_claimed": False})
        for action in (("上移", "下移") if position == 1 else ("下移",)):
            label = card_label(source, position, action, expression)
            observed = next(row for row in observations if row["name"].split("\n")[0] == label)
            if observed["enabled"]:
                original = self.raw()
                self.rule_click(label)
                started = time.monotonic()
                time.sleep(0.5)
                assert self.raw() == original and self.routing_record() == before
                self.steps.append({"edge_move_no_effect": label, "control": observed,
                                   "observed_seconds": time.monotonic() - started})

    def dialogs(self, source, version):
        created_host, edited_host = (f"{kind}.{source}.gui.invalid" for kind in ("created", "edited"))
        created, edited = (f"DOMAIN,{host},DIRECT" for host in (created_host, edited_host))
        self.start_launch(f"{source}-edit-home")
        for label in ("设置", "规则分流", "导入与高级编辑"):
            self.rule_click(label)
        original, before = self.raw(), self.routing_record()
        if json.loads(before)["version"] != version:
            raise RuntimeError("Rule dialog started from the wrong active version")
        self.rule_click(f"新增{SECTIONS[source]}")
        self.enter_expression("DOMAIN-SUFFIX,shu.edu.cn,SHU-VPN", created)
        self.snapshot(f"{source}-add-cancel")
        self.rule_click("取消")
        assert self.raw() == original and self.routing_record() == before
        self.rule_click(f"新增{SECTIONS[source]}")
        self.enter_expression("DOMAIN-SUFFIX,shu.edu.cn,SHU-VPN", created)
        self.rule_click("保存")
        wait_for(lambda: fixture_rules(source, self.raw())[-1] == created, "created rule in intended source")
        added = self.raw()
        position = len(fixture_rules(source, added))
        self.card_semantics(source, position, created, before)
        assert self.routing_record() == before
        self.rule_click(card_label(source, position, "编辑", created))
        self.enter_expression(created, edited)
        self.rule_click("取消")
        assert self.raw() == added and self.routing_record() == before
        self.rule_click(card_label(source, position, "编辑", created))
        self.enter_expression(created, edited)
        self.rule_click("保存")
        wait_for(lambda: fixture_rules(source, self.raw())[-1] == edited, "edited rule in intended source")
        assert self.routing_record() == before
        self.snapshot(f"{source}-edited-draft-no-commit")
        self.rule_click("校验并预览")
        assert self.routing_record() == before
        self.rule_click("应用预览")
        committed = self.applied(version + 1)
        self.snapshot(f"{source}-edit-applied")
        self.stop_launch()
        self.readback(version + 1, created_host, edited_host, "DIRECT", source)
        self.start_launch(f"{source}-delete-reopened")
        for label in ("设置", "规则分流", "导入与高级编辑"):
            self.rule_click(label)
        raw = self.raw()
        assert raw == (committed["current"]["profile"], committed["current"]["overrides"])
        self.snapshot(f"{source}-edited-rule-restored")
        committed_bytes = self.routing_record()
        self.rule_click(card_label(source, position, "删除", edited))
        wait_for(lambda: self.text_present("删除这条规则？"), "actual delete confirmation")
        self.snapshot(f"{source}-delete-cancel")
        self.rule_click("取消")
        assert self.raw() == raw and self.routing_record() == committed_bytes
        self.rule_click(card_label(source, position, "删除", edited))
        wait_for(lambda: self.text_present("删除这条规则？"), "actual second delete confirmation")
        self.rule_click("删除")
        wait_for(lambda: edited not in fixture_rules(source, self.raw()), "deleted rule absent from draft")
        assert self.routing_record() == committed_bytes
        self.rule_click("校验并预览")
        assert self.routing_record() == committed_bytes
        self.rule_click("应用预览")
        self.applied(version + 2)
        self.snapshot(f"{source}-delete-applied")
        self.stop_launch()
        return self.readback(version + 2, created_host, edited_host, "REJECT", source)

    def additional_settings(self, baseline):
        if (baseline.get("routing_version") != 2 or len(self.launches) != 3 or
                any(row.get("exit") != 0 for row in self.launches)):
            raise RuntimeError("Three-source dialogs require the completed version-2 offline baseline")
        self.result["offline_baseline"] = {"native": baseline, "launches": list(self.launches)}
        version, final = 2, None
        for source in SECTIONS:
            final = self.dialogs(source, version)
            version += 2
        self.result["rule_dialog_sources"] = list(SECTIONS)
        return final


def main():
    if not __debug__ or sys.platform != "win32":
        raise RuntimeError("Use ordinary Windows Python with assertions enabled")
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("bundle", "report", "dart"):
        parser.add_argument("--" + name, type=Path, required=True)
    acceptance = RuleAcceptance(parser.parse_args())
    acceptance.run()
    assert acceptance.result["passed"] and len(acceptance.launches) == 9
    print("WINDOWS_THREE_SOURCE_RULE_DIALOGS_PASSED")


if __name__ == "__main__":
    main()
