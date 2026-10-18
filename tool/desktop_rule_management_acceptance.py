"""Exercise Linux rule ordering, enablement, groups, live previews and rollback."""
import hashlib
import json
import sys

import desktop_gui_acceptance as gui
from desktop_rule_acceptance import RuleSession, unique_named_node
from rule_gui_evidence import NETWORK_SWITCHES, RAW_LABELS, card_label

PROFILE_NAME = "rule-management-fixture"
ORDER_HOST = "order.gui.invalid"
GROUP_HOST = "policy.gui.invalid"
ORDER_REJECT = f"DOMAIN,{ORDER_HOST},REJECT"
ORDER_DIRECT = f"DOMAIN,{ORDER_HOST},DIRECT"
GROUP_RULE = f"DOMAIN,{GROUP_HOST},Office"
CHOICE_ROLES = ("push button", "menu item", "radio button", "list item")


def unique_choice(nodes, label):
    matches = [node for node in nodes if node.get_name().split("\n")[0] == label and
               node.get_role_name() in CHOICE_ROLES and node.get_n_actions() > 0]
    if len(matches) > 1:
        raise RuntimeError(f"Ambiguous interactive menu choice: {label}")
    return matches[0] if matches else None


def accessible_text(node):
    name = node.get_name()
    if name:
        return name
    if node.get_role_name() == "text":
        text = node.get_text_iface()
        if text is not None:
            return text.get_text(0, text.get_character_count())
    return ""


def clipboard_text():
    import gi
    gi.require_version("Gtk", "3.0")
    gi.require_version("Gdk", "3.0")
    from gi.repository import Gdk, GLib, Gtk
    Gtk.init([])
    values = []
    clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
    clipboard.request_text(lambda _clipboard, text, _data: values.append(text), None)
    def received():
        context = GLib.MainContext.default()
        for _ in range(512):
            if not context.pending():
                break
            context.iteration(False)
        return {"text": values[0]} if values else None
    return gui.wait_for(received, "text copied in the owned private X11 display", seconds=3)["text"]


def validate_management_readback(native, version, order, rule_id, group):
    if (native.get("routing_version") != version or native.get("routing_name") != PROFILE_NAME or
            native.get("routing_test_action") != "REJECT" or
            native.get("routing_override_actions") != {
                "before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"} or
            any(native.get(key) is not False for key in NETWORK_SWITCHES)):
        raise RuntimeError("Management readback lost the expected configuration or offline settings")
    decisions = native.get("routing_custom_decisions") or {}
    for host, policy, action, identifier in (
        (ORDER_HOST, order, order, rule_id), (GROUP_HOST, "Office", group, "profile:2"),
    ):
        value = decisions.get(host) or {}
        if any(value.get(key) != expected for key, expected in {
                "version": version, "source": "profile", "rule_id": identifier,
                "policy": policy, "action": action}.items()):
            raise RuntimeError(f"Management readback has the wrong version/source/decision for {host}")


class ManagementSession(RuleSession):
    def __init__(self, args):
        super().__init__(args)
        self.storage_data = self.report / "profile/data"

    def state_file(self):
        paths = list(self.storage_data.rglob("settings-state.json"))
        return paths[0] if len(paths) == 1 else None

    def routing_file(self):
        paths = list(self.storage_data.rglob("routing-state.json"))
        if len(paths) != 1:
            raise RuntimeError("Expected exactly one owned routing record")
        return paths[0]

    def raw(self):
        # Import can insert the rule cards before the keyed expansion tile.
        # Re-find its current visible fields after that page reconstruction.
        advanced = self.named_node("导入与高级编辑", "panel")
        self.outer_scroll(2)
        if unique_named_node(gui.accessible_nodes(self.application.pid), RAW_LABELS[0], "text") is None:
            self.activate(advanced, "导入与高级编辑", "panel")
        return super().raw()

    def press(self, label, role="push button"):
        node = self.named_node(label, role)
        self.steps.append({"management_control_before_action": self.node_observation(node)})
        self.activate(node, label, role)

    def text_field(self, label, value):
        node = self.named_node(label, "text")
        self.activate_editor(node)
        self.key("ctrl+a")
        self.key("BackSpace")
        gui.command("xdotool", "type", "--clearmodifiers", "--delay", "1", value)
        def current_field():
            current = unique_named_node(gui.accessible_nodes(self.application.pid), label, "text")
            return current if current and current.get_process_id() == self.application.pid else None
        try:
            current = gui.wait_for(current_field, f"current owned text field {label}")
            current.clear_cache()
            text = current.get_text_iface()
            reported = text.get_text(0, text.get_character_count())
            self.key("ctrl+a")
            self.key("ctrl+c")
            copied = clipboard_text()
            self.steps.append({"field_readback": label, "text": copied,
                               "atspi_text": reported, "clipboard_scope": "owned private X11 display"})
            if copied != value:
                raise RuntimeError(f"Owned text field {label} copied a different value")
        except RuntimeError:
            self.snapshot_text_fields()
            raise
        self.steps.append({"management_field": label, "typed": value})

    def snapshot_text_fields(self):
        rows = []
        for node in gui.accessible_nodes(self.application.pid):
            if node.get_role_name() != "text":
                continue
            text = node.get_text_iface()
            parents, parent = [], node.get_parent()
            for _ in range(3):
                if parent is None:
                    break
                parents.append({"name": parent.get_name(), "role": parent.get_role_name()})
                parent = parent.get_parent()
            rows.append({**self.node_observation(node), "parents": parents,
                         "actions": [node.get_action_name(index) for index in range(node.get_n_actions())],
                         "text": text.get_text(0, text.get_character_count()) if text else None})
        (self.report / "text-input-mismatch.json").write_text(
            json.dumps(rows, ensure_ascii=False, indent=2))

    def choose(self, label, choice):
        self.press(label)
        def menu_item():
            node = unique_choice(gui.accessible_nodes(self.application.pid), choice)
            return node if node and self.node_observation(node)["showing"] else None
        node = gui.wait_for(menu_item, f"visible unique menu choice {choice}")
        self.steps.append({"management_menu_before_action": self.node_observation(node)})
        self.activate(node, choice, node.get_role_name())

    def result_node(self, expected):
        matches, candidates = [], []
        for node in gui.accessible_nodes(self.application.pid):
            text = accessible_text(node)
            if text.startswith("规则 ") or (not node.get_name() and node.get_role_name() == "text"):
                observed = {**self.node_observation(node), "text": text}
                candidates.append(observed)
                if text == expected and observed["showing"]:
                    matches.append(node)
        if candidates and (not self.steps or self.steps[-1].get("gui_preview_candidates") != candidates):
            self.steps.append({"gui_preview_candidates": candidates})
        if len(matches) > 1:
            raise RuntimeError("Multiple current GUI preview result nodes")
        return matches[0] if matches else None

    def live_test(self, host, version, action, rule_id):
        # Each change clears _testResult in the production page, preventing an
        # old result with the same action/version from satisfying this check.
        self.choose("网络协议", "UDP")
        self.choose("网络协议", "TCP")
        self.named_node("预览 HTTP / TCP 决策", "push button")
        self.outer_scroll(2)
        def cleared():
            return not any(text.startswith("规则 ") and " · 版本 " in text
                           for text in map(accessible_text, gui.accessible_nodes(self.application.pid)))
        gui.wait_for(cleared, "prior GUI preview result disappears before the next test")
        self.steps.append({"prior_gui_preview_cleared": True, "pid": self.application.pid})
        self.text_field("目标域名或 IP", host)
        self.text_field("目标端口", "443")
        self.press("预览 HTTP / TCP 决策")
        expected = f"规则 {rule_id} · {action} · 版本 {version}"
        def result():
            node = self.result_node(expected)
            if node:
                return {**self.node_observation(node), "text": accessible_text(node)}
            self.outer_scroll(1)
            return None
        observed = gui.wait_for(result, f"current GUI preview {expected}")
        if observed["pid"] != self.application.pid:
            raise RuntimeError("Rule preview belongs to another process")
        self.steps.append({"live_gui_preview": {
            "host": host, "port": 443, "network": "tcp", "inbound": "http",
            "version": version, "action": action, "rule_id": rule_id, "result": observed}})
        self.snapshot(f"preview-{len(self.steps)}")

    def open_rules(self, name):
        self.start(name)
        self.control("设置")
        self.control("规则分流")
        self.control("导入与高级编辑")

    def uncommitted(self, before, stage):
        assert self.routing_file().read_bytes() == before
        apply = self.named_node("应用预览", "push button")
        observed = self.node_observation(apply)
        if observed["enabled"] or apply.get_n_actions() != 0:
            raise RuntimeError("A draft edit did not invalidate the existing Apply capability")
        self.steps.append({"uncommitted_management_draft": stage, "apply": observed,
                           "record_sha256": hashlib.sha256(before).hexdigest()})

    def commit(self, version, before, name):
        self.press("校验并预览")
        assert self.routing_file().read_bytes() == before
        self.press("应用预览")
        record = self.applied(version)
        self.snapshot(name)
        return record

    def finish_stage(self, version, action, rule_id, group):
        self.stop("ctrl+q")
        native = self.probe("read", rule_hosts=(ORDER_HOST, GROUP_HOST))
        validate_management_readback(native, version, action, rule_id, group)
        self.steps.append({"independent_management_readback": native})
        return native

    def group_control(self, selected):
        label = "策略组 · Office"
        for direction in (1, -1):
            for _ in range(12):
                nodes = gui.accessible_nodes(self.application.pid)
                direct = unique_named_node(nodes, label, "push button")
                if (direct and self.node_observation(direct)["showing"] and
                        direct.get_name().split("\n")[-1] == selected):
                    return direct
                headings = [node for node in nodes if node.get_name().split("\n")[0] == label]
                for heading in headings:
                    parent = heading
                    for _ in range(3):
                        branch = gui.accessible_subtree(parent)
                        groups = {node.get_name().split("\n")[0] for node in branch
                                  if node.get_name().startswith("策略组 · ")}
                        if groups == {label}:
                            node = unique_choice(branch, selected)
                            if node and self.node_observation(node)["showing"]:
                                self.steps.append({"group_control_binding": {
                                    "group": "Office", "heading": label,
                                    "ancestor_role": parent.get_role_name(),
                                    "selected": self.node_observation(node)}})
                                return node
                        parent = parent.get_parent()
                        if parent is None:
                            break
                self.outer_scroll(3 * direction)
        raise RuntimeError("No unique visible select control associated with Office")

    def import_fixture(self):
        self.open_rules("management-import")
        original, before = self.raw(), self.routing_file().read_bytes()
        assert json.loads(before)["version"] == 2
        fixture = json.dumps({
            "name": PROFILE_NAME, "automatic_campus": False,
            "rules": [ORDER_REJECT, ORDER_DIRECT, GROUP_RULE, "MATCH,REJECT"],
            "proxy-groups": [{"name": "Office", "type": "select", "proxies": ["DIRECT", "REJECT"]}],
        }, separators=(",", ":"))
        self.enter(original[0], fixture)
        self.press("载入可视化编辑器")
        gui.wait_for(lambda: json.loads(self.raw()[0])["rules"] ==
                     [ORDER_REJECT, ORDER_DIRECT, GROUP_RULE, "MATCH,REJECT"], "imported draft rules")
        self.snapshot("management-imported-draft")
        self.uncommitted(before, "import")
        self.live_test(GROUP_HOST, 2, "REJECT", "profile:0")
        record = self.commit(3, before, "management-import-applied")
        self.live_test(GROUP_HOST, 3, "DIRECT", "profile:2")
        self.finish_stage(3, "REJECT", "profile:0", "DIRECT")
        return record

    def reorder(self):
        self.open_rules("management-order")
        before = self.routing_file().read_bytes()
        self.card_action("profile", 2, "上移", ORDER_DIRECT)
        gui.wait_for(lambda: json.loads(self.raw()[0])["rules"][:2] ==
                     [ORDER_DIRECT, ORDER_REJECT], "native editor moved the intended rule")
        self.uncommitted(before, "reorder")
        self.live_test(ORDER_HOST, 3, "REJECT", "profile:0")
        record = self.commit(4, before, "management-order-applied")
        self.live_test(ORDER_HOST, 4, "DIRECT", "profile:0")
        self.finish_stage(4, "DIRECT", "profile:0", "DIRECT")
        return record

    def disable_first(self):
        self.open_rules("management-disable")
        before = self.routing_file().read_bytes()
        label = card_label("profile", 1, "启用或停用", ORDER_DIRECT)
        node = self.named_node(label, "toggle button")
        self.activate(node, label, "toggle button")
        gui.wait_for(lambda: json.loads(self.raw()[0])["rules"][0] ==
                     {"rule": ORDER_DIRECT, "enabled": False}, "disabled first rule in the actual draft")
        self.uncommitted(before, "disable")
        self.live_test(ORDER_HOST, 4, "DIRECT", "profile:0")
        record = self.commit(5, before, "management-disable-applied")
        self.live_test(ORDER_HOST, 5, "REJECT", "profile:1")
        self.finish_stage(5, "REJECT", "profile:1", "DIRECT")
        return record

    def select_group(self):
        self.open_rules("management-group")
        before = self.routing_file().read_bytes()
        node = self.group_control("DIRECT")
        self.activate(node, node.get_name().split("\n")[0], node.get_role_name())
        choice = gui.wait_for(
            lambda: unique_choice(gui.accessible_nodes(self.application.pid), "REJECT"),
            "Office REJECT menu choice")
        self.activate(choice, "REJECT", choice.get_role_name())
        gui.wait_for(lambda: json.loads(self.raw()[1])["selections"].get("Office") == "REJECT",
                     "Office selection in the native editor draft")
        self.uncommitted(before, "group")
        self.live_test(GROUP_HOST, 5, "DIRECT", "profile:2")
        record = self.commit(6, before, "management-group-applied")
        self.live_test(GROUP_HOST, 6, "REJECT", "profile:2")
        self.finish_stage(6, "REJECT", "profile:1", "REJECT")
        return record

    def rollback(self, previous):
        self.open_rules("management-rollback")
        self.press("回退上一版本")
        restored = self.applied(7)
        assert restored["current"]["profile"] == previous["current"]["profile"]
        assert restored["current"]["overrides"] == previous["current"]["overrides"]
        gui.wait_for(lambda: self.raw() == (
            restored["current"]["profile"], restored["current"]["overrides"]), "rollback editor readback")
        self.group_control("DIRECT")
        self.live_test(GROUP_HOST, 7, "DIRECT", "profile:2")
        self.snapshot("management-rollback-applied")
        self.finish_stage(7, "REJECT", "profile:1", "DIRECT")
        return restored

    def management(self):
        self.import_fixture()
        self.reorder()
        previous = self.disable_first()
        self.select_group()
        restored = self.rollback(previous)
        self.open_rules("management-restored")
        assert self.raw() == (restored["current"]["profile"], restored["current"]["overrides"])
        self.group_control("DIRECT")
        self.live_test(ORDER_HOST, 7, "REJECT", "profile:1")
        self.live_test(GROUP_HOST, 7, "DIRECT", "profile:2")
        self.snapshot("management-reopened-restored")
        return self.finish_stage(7, "REJECT", "profile:1", "DIRECT")

    def run(self):
        root_report, closed = self.report, False
        try:
            self.report = root_report / "baseline"
            self.report.mkdir()
            # CRUD has its own verified entrypoint. This scenario starts with
            # the standard three-process offline baseline, not another CRUD run.
            gui.Session.run(self)
            baseline = json.loads((self.report / "verification.json").read_text())
            if (len(self.launches) != 3 or any(row["exit"] != 0 for row in self.launches) or
                    baseline["native"]["routing_version"] != 2):
                raise RuntimeError("Management requires the completed version-2 offline baseline")
            self.report, self.steps = root_report, []
            final = self.management()
            assert len(self.launches) == len({row["pid"] for row in self.launches}) == 9
            assert all(row["exit"] == 0 for row in self.launches)
            assert all(hashlib.sha256((self.bundle / name).read_bytes()).hexdigest() == digest
                       for name, digest in self.product.items())
            self.close()
            closed = True
            (self.report / "verification.json").write_text(json.dumps({
                "passed": True, "baseline": "baseline/verification.json",
                "launches": self.launches, "steps": self.steps, "final": final,
                "product": self.product, "scope": "synthetic rules in the production Linux GUI; no network dialing",
            }, ensure_ascii=False, indent=2))
            print("LINUX_RULE_ORDER_ENABLE_GROUP_PREVIEW_AND_ROLLBACK_PASSED")
        except BaseException as error:
            self.report = root_report
            if self.application is not None and self.application.poll() is None:
                try:
                    self.snapshot("management-failure")
                except Exception as snapshot_error:
                    print(f"Management failure snapshot failed: {snapshot_error}", file=sys.stderr)
            (self.report / "verification.json").write_text(json.dumps({
                "passed": False, "error": str(error), "launches": self.launches, "steps": self.steps,
                "baseline": "baseline/verification.json",
            }, ensure_ascii=False, indent=2))
            raise
        finally:
            self.report = root_report
            if not closed:
                gui.finish((self.close,))


if __name__ == "__main__":
    gui.main(gui.arguments(__doc__), session_type=ManagementSession, entrypoint=__file__)
