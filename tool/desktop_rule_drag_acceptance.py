"""Verify physical pointer rule reordering in an owned private Linux X11 GUI."""
import hashlib
import json
import os
import subprocess
import sys
import time

import desktop_gui_acceptance as gui
from desktop_rule_management_acceptance import ManagementSession
from desktop_rule_acceptance import validate_action_target
from rule_gui_evidence import NETWORK_SWITCHES, SECTIONS

PROFILE_NAME = "pointer-drag-fixture"
SOURCES = tuple(SECTIONS)
HOSTS = {source: f"drag.{source}.gui.invalid" for source in SOURCES}


def expressions(source):
    return [f"DOMAIN,{HOSTS[source]},{action}" for action in ("REJECT", "DIRECT")]


def documents(moved=()):
    rules = {source: expressions(source) for source in SOURCES}
    for source in moved:
        rules[source].reverse()
    return (
        {"name": PROFILE_NAME, "automatic_campus": False,
         "rules": [*rules["profile"], "MATCH,REJECT"],
         "proxy-groups": [], "rule-providers": {}},
        {"prepend": [*rules["prepend"], "DOMAIN,before.gui.invalid,DIRECT"],
         "append": [*rules["append"], "DOMAIN,after.gui.invalid,DIRECT"],
         "selections": {}},
    )


def drag_label(source, position, expression):
    return (f"{SECTIONS[source]} 第{position}条：拖动排序 {expression}"
            "；也可使用上移和下移按钮")


def validate_drag_handles(handles, pid, labels, window):
    if type(pid) is not int or pid <= 0 or len(handles) != 2 or len(labels) != 2:
        raise RuntimeError("Drag requires exactly two owned named handles")
    if (len(window) != 4 or any(type(value) is not int for value in window) or
            window[2] < 400 or window[3] < 400):
        raise RuntimeError("Invalid owned X11 window rectangle")
    wx, wy, width, height = window
    centers = []
    for handle, label in zip(handles, labels, strict=True):
        if (handle.get("pid") != pid or type(handle.get("pid")) is not int or
                handle.get("name") != label or handle.get("role") != "image" or
                handle.get("showing") is not True or handle.get("actions") != 0):
            raise RuntimeError("Drag handle is not the expected showing owned image")
        if handle.get("visible") is not True:
            raise RuntimeError("Drag handle is clipped or outside the visible viewport")
        rect = handle.get("rect")
        if (not isinstance(rect, list) or len(rect) != 4 or
                any(type(value) is not int for value in rect)):
            raise RuntimeError("Missing actual drag handle rectangle")
        x, y, w, h = rect
        if (w < 20 or h < 20 or w > 100 or h > 100 or w != h or
                x < wx + 8 or y < wy + 140 or
                x + w > wx + width - 8 or y + h > wy + height - 8):
            raise RuntimeError("Drag handle is clipped or outside the owned content window")
        centers.append((x + w // 2, y + h // 2))
    if centers[0][1] <= centers[1][1] or centers[0][1] - centers[1][1] < 60:
        raise RuntimeError("Expected the second rule below its first-rule target")
    return centers


def pointer_drag(command, start, end, pause=time.sleep):
    """Always release button 1, preserving any primary motion failure."""
    command("mousemove", "--sync", *start)
    try:
        command("mousedown", "1")
        for step in range(1, 13):
            point = tuple(round(a + (b - a) * step / 12) for a, b in zip(start, end))
            command("mousemove", "--sync", *point)
            pause(0.06)
    finally:
        gui.finish((lambda: command("mouseup", "1"),))


def validate_drag_readback(native, version, moved, committed):
    if (native.get("routing_version") != version or native.get("routing_name") != PROFILE_NAME or
            native.get("routing_test_action") != "REJECT" or
            any(native.get(key) is not False for key in NETWORK_SWITCHES) or
            native.get("routing_override_actions") != {
                "before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"} or
            (native.get("routing_profile"), native.get("routing_overrides")) != (
                committed["current"]["profile"], committed["current"]["overrides"])):
        raise RuntimeError("Native drag readback lost its exact committed offline configuration")
    actual = tuple(json.loads(native[key]) for key in ("routing_profile", "routing_overrides"))
    if actual != documents(moved):
        raise RuntimeError("Native drag readback has the wrong complete rule order")
    for source, host in HOSTS.items():
        expected = "DIRECT" if source in moved else "REJECT"
        decision = (native.get("routing_custom_decisions") or {}).get(host) or {}
        if any(decision.get(key) != value for key, value in {
            "version": version, "source": source, "rule_id": f"{source}:0",
            "policy": expected, "action": expected,
        }.items()):
            raise RuntimeError("Native drag readback disagrees with the first matching rule")


GEOMETRY_HELPER = r'''
import json, os, sys
sys.path.insert(0, sys.argv[1])
import desktop_gui_acceptance as gui
from desktop_rule_acceptance import unique_named_node
gui.validate_session(os.environ, os.environ.get("SHUVPN_GUI_ACCEPTANCE_TOKEN"), os.geteuid())
import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
Atspi.set_timeout(1000, 1000)
pid = int(sys.argv[2])
result = []
nodes = gui.accessible_nodes(pid)
for label in json.loads(sys.argv[3]):
    node = unique_named_node(nodes, label, "image")
    if node is None:
        raise RuntimeError("No unique named drag handle")
    component = node.get_component_iface()
    if component is None:
        raise RuntimeError("Drag handle has no geometry interface")
    rect = component.get_extents(Atspi.CoordType.SCREEN)
    state = node.get_state_set()
    result.append({"name": node.get_name(), "role": node.get_role_name(),
                   "pid": node.get_process_id(), "actions": node.get_n_actions(),
                   "showing": state.contains(Atspi.StateType.SHOWING),
                   "visible": state.contains(Atspi.StateType.VISIBLE),
                   "rect": [rect.x, rect.y, rect.width, rect.height]})
print(json.dumps(result, ensure_ascii=False))
'''


class DragSession(ManagementSession):
    def x11(self, *arguments):
        return subprocess.check_output(
            ["xdotool", *map(str, arguments)], text=True, timeout=5).strip()

    def window_rectangle(self):
        if (self.application is None or self.application.poll() is not None or
                int(self.x11("getwindowpid", self.window)) != self.application.pid):
            raise RuntimeError("The drag window no longer belongs to this live application")
        geometry = dict(line.split("=", 1) for line in
                        self.x11("getwindowgeometry", "--shell", self.window).splitlines())
        return [int(geometry[key]) for key in ("X", "Y", "WIDTH", "HEIGHT")]

    def drag_handles(self, source):
        labels = [drag_label(source, position, expression)
                  for position, expression in ((2, expressions(source)[1]),
                                               (1, expressions(source)[0]))]
        for direction in (-1, 1):
            for attempt in range(32):
                result = subprocess.run(
                    [sys.executable, "-c", GEOMETRY_HELPER, str(gui.ROOT / "tool"),
                     str(self.application.pid), json.dumps(labels)],
                    text=True, capture_output=True, timeout=5)
                if result.returncode:
                    # Missing offscreen semantics are expected during bounded scrolling.
                    if "No unique named drag handle" not in result.stderr:
                        raise RuntimeError(f"Bounded drag geometry query failed: {result.stderr[-1000:]}")
                else:
                    handles = json.loads(result.stdout)
                    rectangle = self.window_rectangle()
                    self.steps.append({"drag_handle_probe": {
                        "source": source, "direction": direction, "attempt": attempt,
                        "handles": handles, "window_rectangle": rectangle}})
                    try:
                        points = validate_drag_handles(
                            handles, self.application.pid, labels, rectangle)
                    except RuntimeError as error:
                        if "clipped or outside" not in str(error):
                            raise
                    else:
                        return handles, rectangle, points
                self.outer_scroll(2 * direction)
        raise RuntimeError("Both complete drag handles were not visible together")

    def drag(self, source):
        gui.validate_session(os.environ, self.args.token, os.geteuid())
        self.x11("windowactivate", "--sync", self.window)
        handles, rectangle, (start, end) = self.drag_handles(source)
        self.snapshot(f"{source}-before-physical-drag")
        if (self.window_rectangle() != rectangle or
                int(self.x11("getactivewindow")) != int(self.window)):
            raise RuntimeError("Owned drag window moved or lost activation before input")
        pointer_drag(self.x11, start, end)
        self.steps.append({"physical_rule_drag": {
            "source": source, "pid": self.application.pid, "window": self.window,
            "window_rectangle": rectangle, "handles": handles, "start": start, "end": end,
            "motion_steps": 12, "button_released": True}})
        self.snapshot(f"{source}-after-physical-drag")
        gui.wait_for(lambda: tuple(map(json.loads, self.raw())) == documents(self.moved | {source}),
                     f"actual pointer drag changes the {source} native editor order")

    def finish_drag_stage(self, version, committed):
        self.stop("ctrl+q")
        native = self.probe("read", rule_hosts=tuple(HOSTS.values()))
        validate_drag_readback(native, version, self.moved, committed)
        self.steps.append({"independent_drag_readback": native})
        return native

    def prepare_drag(self, before):
        self.press("校验并预览")
        def ready():
            node = self.named_node("应用预览", "push button")
            observed = self.node_observation(node)
            if not observed["enabled"]:
                return None
            validate_action_target(observed, self.application.pid, "应用预览", "push button")
            actions = [node.get_action_name(index) for index in range(node.get_n_actions())]
            observed["actions"] = actions
            self.steps.append({"prepared_apply_capability": observed})
            if actions.count("Tap") != 1:
                raise RuntimeError(f"Prepared drag preview did not expose one Tap action: {actions}")
            return observed
        observed = gui.wait_for(ready, "valid actionable preview exists before pointer drag")
        if self.routing_file().read_bytes() != before:
            raise RuntimeError("Preparing the pre-drag preview changed the committed configuration")
        self.steps.append({"actionable_preview_before_drag": observed,
                           "record_sha256": hashlib.sha256(before).hexdigest()})

    def run(self):
        root_report, closed = self.report, False
        self.moved = set()
        try:
            self.report = root_report / "baseline"
            self.report.mkdir()
            gui.Session.run(self)
            baseline = json.loads((self.report / "verification.json").read_text())
            if len(self.launches) != 3 or baseline["native"]["routing_version"] != 2:
                raise RuntimeError("Pointer drag requires the completed offline baseline")
            self.report, self.steps = root_report, []
            self.open_rules("drag-import")
            original = self.raw()
            before = self.routing_file().read_bytes()
            for old, document in zip(original, documents(), strict=True):
                self.enter(old, json.dumps(document, separators=(",", ":")))
            self.press("载入可视化编辑器")
            gui.wait_for(lambda: tuple(map(json.loads, self.raw())) == documents(),
                         "all three controlled drag sources loaded")
            self.uncommitted(before, "drag-import")
            committed = self.commit(3, before, "drag-import-applied")
            self.finish_drag_stage(3, committed)
            version = 3
            for source in SOURCES:
                self.open_rules(f"drag-{source}")
                before = self.routing_file().read_bytes()
                original = self.raw()
                self.prepare_drag(before)
                self.drag(source)
                self.uncommitted(before, f"{source}-discarded-drag")
                self.live_test(HOSTS[source], version, "REJECT", f"{source}:0")
                self.press("返回")
                self.control("规则分流")
                self.control("导入与高级编辑")
                gui.wait_for(lambda: self.raw() == original, "leaving discards an unapplied drag draft")
                if self.routing_file().read_bytes() != before:
                    raise RuntimeError("Leaving an unapplied drag changed the stored configuration")
                self.steps.append({"discarded_drag_kept_active_order": source,
                                   "record_sha256": hashlib.sha256(before).hexdigest()})
                self.prepare_drag(before)
                self.drag(source)
                self.uncommitted(before, f"{source}-commit-drag")
                committed = self.commit(version + 1, before, f"drag-{source}-applied")
                self.moved.add(source)
                version += 1
                self.live_test(HOSTS[source], version, "DIRECT", f"{source}:0")
                self.finish_drag_stage(version, committed)
            self.open_rules("drag-all-reopened")
            if self.raw() != (committed["current"]["profile"], committed["current"]["overrides"]):
                raise RuntimeError("Reopened GUI lost the exact committed drag documents")
            for source in SOURCES:
                self.live_test(HOSTS[source], version, "DIRECT", f"{source}:0")
            self.snapshot("drag-all-orders-restored")
            final = self.finish_drag_stage(version, committed)
            if (len(self.launches) != 8 or len({row["pid"] for row in self.launches}) != 8 or
                    any(row["exit"] != 0 or row["uid"] != os.geteuid() or
                        int(row["cap_eff"], 16) != 0 for row in self.launches)):
                raise RuntimeError("Not all eight ordinary GUI processes exited normally")
            if any(hashlib.sha256((self.bundle / name).read_bytes()).hexdigest() != digest
                   for name, digest in self.product.items()):
                raise RuntimeError("The formal GUI product changed during pointer acceptance")
            self.close()
            closed = True
            (self.report / "verification.json").write_text(json.dumps({
                "passed": True, "baseline": "baseline/verification.json",
                "launches": self.launches, "steps": self.steps, "final": final,
                "product": self.product, "sources": list(SOURCES),
                "scope": "physical pointer drag in the formal Linux GUI; synthetic offline rules",
            }, ensure_ascii=False, indent=2))
            print("LINUX_PHYSICAL_RULE_DRAG_DISCARD_APPLY_AND_REOPEN_PASSED")
        except BaseException as error:
            self.report = root_report
            if self.application is not None and self.application.poll() is None:
                try:
                    self.snapshot("drag-failure")
                except Exception as snapshot_error:
                    print(f"Drag failure snapshot failed: {snapshot_error}", file=sys.stderr)
            (self.report / "verification.json").write_text(json.dumps({
                "passed": False, "error": str(error), "steps": self.steps, "launches": self.launches,
            }, ensure_ascii=False, indent=2))
            raise
        finally:
            self.report = root_report
            if not closed:
                gui.finish((self.close,))


if __name__ == "__main__":
    gui.main(gui.arguments(__doc__), session_type=DragSession, entrypoint=__file__)
