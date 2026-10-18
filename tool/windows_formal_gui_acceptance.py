"""Exercise unchanged production main/AOT with a private EXE-resource storage name."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

from windows_desktop_probe import RetainedProcess, WindowsDesktop, file_hash
from windows_private_profile import cleanup_profile, folder_roots, validate_names, version_names
from windows_tray_acceptance import ROOT, TrayAcceptance, wait_for
from package_desktop import source_identity


INVALID_RULE_DOCUMENT = '{"name":"invalid-gui","rules":["UNSUPPORTED,example,DIRECT"]}'


def refuse_security_prompt(api, result: dict) -> None:
    foreground = api.user.GetForegroundWindow()
    if (foreground and api.window_class(foreground) == "Shell_SystemDialog" and
            api.window_title(foreground) in ("Windows 安全中心", "Windows Security")):
        result["blocked_system_prompt"] = {
            "hwnd": foreground, "class": "Shell_SystemDialog",
            "title": api.window_title(foreground)}
        raise RuntimeError("Foreground Windows Security requires a user decision before GUI acceptance")


def validate_masked_password(value: str, length: int) -> dict:
    if len(value) != length or not value or any(character not in "•●*·" for character in value):
        raise RuntimeError("The controlled test password was not masked in accessibility")
    return {"masked": True, "length": len(value)}


def validate_offline_settings(snapshot: dict, capture_mode: str, routing_version: int,
                              overrides: str) -> None:
    if (snapshot.get("theme") != "dark" or snapshot.get("welcome") is not True or
            snapshot.get("http_port") != 18443 or snapshot.get("capture_mode") != capture_mode or
            type(snapshot.get("routing_version")) is not int or
            snapshot["routing_version"] != routing_version or
            snapshot.get("routing_name") != "仅校园" or snapshot.get("routing_test_action") != "REJECT"):
        raise RuntimeError("Independent native offline settings/rules did not match GUI commits")
    if (snapshot.get("routing_profile") != "name: 仅校园\nrules:\n  - MATCH,REJECT\n" or
            snapshot.get("routing_overrides") != overrides or
            snapshot.get("routing_override_actions") != {
                "before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"}):
        raise RuntimeError("Independent native rule documents or override decisions were not preserved")
    if any(snapshot.get(key) is not False for key in (
            "http_enabled", "socks_enabled", "vpn_enabled", "system_proxy_enabled")):
        raise RuntimeError("Offline settings acceptance enabled or failed to observe a network switch")


def validate_rule_rejection(before: bytes | None, after: bytes | None, rows: list[dict],
                            *, blocked_apply: dict | None = None) -> None:
    if not before or before != after:
        raise RuntimeError("Invalid rule validation changed the committed native record")
    if not any("规则类型尚未支持" in row.get("name", "") and
               row.get("offscreen") is False for row in rows):
        raise RuntimeError("Invalid rule validation did not display its native error")
    apply = [row for row in rows if row.get("name", "").split("\n")[0] == "应用预览"
             and row.get("type") == 50000]
    if len(apply) != 1:
        raise RuntimeError("Invalid rule validation left no unique disabled Apply button")
    if apply[0].get("enabled") is False:
        return
    if apply[0].get("enabled") is not True or blocked_apply is None:
        raise RuntimeError("Invalid rule validation left no unique disabled Apply button")
    target = blocked_apply.get("target") or {}
    observed = blocked_apply.get("observed_seconds")
    digest = hashlib.sha256(before).hexdigest()
    if (target.get("name", "").split("\n")[0] != "应用预览" or
            target.get("control_type") != 50000 or type(target.get("pid")) is not int or
            target["pid"] <= 0 or type(observed) not in (int, float) or
            not math.isfinite(observed) or observed < 0.5 or
            blocked_apply.get("before_sha256") != digest or
            blocked_apply.get("after_sha256") != digest):
        raise RuntimeError("Enabled UIA fallback lacks an owned physical click and unchanged native record")


def validate_profile_marker(marker: dict, products: dict) -> None:
    if (marker.get("schema") != 1 or marker.get("entrypoint") != "lib/main.dart" or
            marker.get("kind") != "production-aot-private-resource-copy" or
            marker.get("products") != products or
            marker.get("production_aot_unchanged") is not True or
            marker.get("exe_code_unchanged") is not True):
        raise RuntimeError("Not the prepared production-AOT private resource copy")
    validate_names(marker["company"], marker["product"])
    original = marker["archive_manifest"]["files"]
    if set(products) != set(original):
        raise RuntimeError("Private bundle file set differs from the original package")
    for name, actual in products.items():
        if name != "shuvpn.exe" and original[name]["sha256"] != actual:
            raise RuntimeError(f"Private bundle changed an original payload: {name}")
    if (marker["overlay"]["original_sha256"] != original["shuvpn.exe"]["sha256"] or
            marker["overlay"]["patched_sha256"] != products["shuvpn.exe"] or
            marker["overlay"].get("unchanged_outside_ranges") is not True):
        raise RuntimeError("Private executable overlay provenance is incomplete")


def validate_launches(launches: list[dict], expected: int) -> None:
    identities = [row.get("process", {}).get("created_ticks") for row in launches]
    if (len(launches) != expected or any(not isinstance(value, str) or not value for value in identities) or
            len(set(identities)) != expected or any(row.get("exit") != 0 for row in launches)):
        raise RuntimeError("Formal GUI lacks the expected distinct, normally exited original processes")


class FormalAcceptance(TrayAcceptance):
    entrypoint = "lib/main.dart"
    expected_launches = 3

    def __init__(self, args):
        self.bundle = args.bundle.resolve(strict=True)
        self.report, self.dart = args.report.resolve(), args.dart.resolve(strict=True)
        boundary = (ROOT / "build/desktop").resolve()
        if not self.report.is_relative_to(boundary) or self.report == boundary:
            raise ValueError("Formal GUI report must be new and inside build/desktop")
        self.marker = json.loads((self.bundle / "private-profile.json").read_text(encoding="utf-8"))
        self.products = {name: file_hash(self.bundle / name) for name in self.marker["products"]}
        validate_profile_marker(self.marker, self.products)
        self.profile = Path(self.marker["profile"])
        roots = [folder / self.marker["company"] for folder in folder_roots()]
        if (self.marker["profile_roots"] != [str(root) for root in roots] or
                self.profile != roots[0] / self.marker["product"] or self.profile.exists()):
            raise RuntimeError("First formal launch requires the freshly reserved private profile")
        for root in roots:
            if json.loads((root / ".shuvpn-acceptance-owner.json").read_text()) != self.marker["ownership"]:
                raise RuntimeError("Prepared private directory is not owned by this run")
        if version_names(self.bundle / "shuvpn.exe") != {
                "CompanyName": self.marker["company"], "ProductName": self.marker["product"]}:
            raise RuntimeError("Actual EXE version names differ from the private profile")
        self.report.mkdir()
        self.api = WindowsDesktop()
        self.api.prepare_input()
        self.input_lock = self.api.acquire_input()
        self.application = self.probe = self.fixture = self.explorer = self.log = self.window = None
        self.steps, self.launches = [], []
        self.result = {"passed": False, "entrypoint": self.entrypoint,
                       "verifier_source": source_identity(),
                       "production_main_aot_tested": True, "exe_resource_names_modified": True,
                       "login_success_tested": False, "connected_data_or_tun_tested": False,
                       "marker": self.marker, "profile": str(self.profile)}
        self.token = self.marker["ownership"]["token"]

    def activate_formal(self) -> None:
        refuse_security_prompt(self.api, self.result)
        if self.api.user.GetForegroundWindow() == self.window and self.visible():
            return
        owners = set()
        @self.api.callback
        def visit(window, _):
            if self.api.window_title(window) == "ShuVPN":
                owners.add(self.api.window_pid(window))
            return True
        self.api.require(self.api.user.EnumWindows(visit, 0))
        if owners != {self.probe.pid}:
            raise RuntimeError("The production taskbar title is not unique to this GUI")
        previous, since = None, time.monotonic()
        def pointed_target():
            nonlocal previous, since
            candidate = self.api.application_button(self.taskbar, self.explorer.pid, "ShuVPN")
            if candidate is None:
                return None
            identifier = candidate["automation_id"]
            if not identifier.startswith("Appid: ") or Path(identifier.removeprefix("Appid: ")) != self.bundle / "shuvpn.exe":
                raise RuntimeError("Explorer taskbar Appid does not name the owned executable")
            if candidate != previous:
                previous, since = candidate, time.monotonic()
            x, y = candidate["clickable"]
            self.api.move([x, y, x + 1, y + 1])
            observed = self.api.application_button(self.taskbar, self.explorer.pid, "ShuVPN")
            self.write("taskbar-before-activation.json", {"target": candidate, "observed": observed})
            return candidate if observed == candidate and time.monotonic() - since >= 0.3 else None
        target = wait_for(pointed_target, "owned application's taskbar layout settles after pointer movement", 20)
        x, y = target["clickable"]
        self.click([x, y, x + 1, y + 1], "left", self.explorer.pid)
        wait_for(lambda: self.api.user.GetForegroundWindow() == self.window,
                 "real taskbar click activated the exact owned main HWND")
        self.steps.append({"physical_taskbar_activation": target, "main_window": self.window})
    def snapshot(self, name: str) -> list[dict]:
        foreground = self.api.user.GetForegroundWindow()
        if foreground != self.window:
            observation = {"stage": name, "hwnd": foreground}
            if foreground:
                try:
                    kind = self.api.window_class(foreground)
                    observation.update({
                        "pid": self.api.window_pid(foreground), "class": kind,
                        "security_prompt": kind == "Shell_SystemDialog" and
                        self.api.window_title(foreground) in ("Windows 安全中心", "Windows Security"),
                    })
                except OSError as error:
                    observation["observation_error"] = repr(error)
            self.result.setdefault("outside_foreground", observation)
            raise RuntimeError("Only capture the foreground owned application window")
        import comtypes
        root = self.api._uia().ElementFromHandle(self.window)
        elements = root.FindAll(self.api.uia.TreeScope_Descendants,
                                self.api.automation.CreateTrueCondition())
        rows = []
        for index in range(elements.Length):
            element = elements.GetElement(index)
            if element.CurrentProcessId != self.probe.pid:
                continue
            region = element.CurrentBoundingRectangle
            legacy_state = legacy_role = None
            legacy_error = None
            try:
                pointer = element.GetCurrentPattern(self.api.uia.UIA_LegacyIAccessiblePatternId)
                if pointer:
                    legacy = pointer.QueryInterface(self.api.uia.IUIAutomationLegacyIAccessiblePattern)
                    legacy_state, legacy_role = legacy.CurrentState, legacy.CurrentRole
            except comtypes.COMError as error:
                legacy_error = repr(error)
            rows.append({"name": element.CurrentName or "", "type": element.CurrentControlType,
                         "enabled": bool(element.CurrentIsEnabled),
                         "offscreen": bool(element.CurrentIsOffscreen),
                         "password": element.GetCurrentPropertyValue(self.api.uia.UIA_IsPasswordPropertyId) is True,
                         "selected": element.GetCurrentPropertyValue(
                             self.api.uia.UIA_SelectionItemIsSelectedPropertyId) is True,
                         "legacy_state": legacy_state, "legacy_role": legacy_role,
                         "legacy_pattern_error": legacy_error,
                         "rect": [region.left, region.top, region.right, region.bottom]})
        self.write(name + "-accessibility.json", rows)
        self.capture(name, self.api.window_rect(self.window))
        return rows

    def dark_surface(self) -> None:
        from PIL import ImageGrab
        bounds = self.api.window_rect(self.window)
        x, y = bounds[0] + (bounds[2] - bounds[0]) * 4 // 5, bounds[1] + (bounds[3] - bounds[1]) // 3
        def dark():
            if self.api.user.GetForegroundWindow() != self.window:
                raise RuntimeError("Formal app lost foreground before rendered theme observation")
            color = list(ImageGrab.grab(bbox=(x, y, x + 1, y + 1), all_screens=True).getpixel((0, 0)))[:3]
            return color if max(color) < 100 else None
        color = wait_for(dark, "formal appearance actually renders a dark surface")
        self.steps.append({"rendered_dark_surface_rgb": color, "point": [x, y]})

    def edit_value(self, label: str) -> str:
        root = self.api._uia().ElementFromHandle(self.window)
        elements = root.FindAll(self.api.uia.TreeScope_Descendants,
                                self.api.automation.CreatePropertyCondition(
                                    self.api.uia.UIA_NamePropertyId, label))
        rows = [elements.GetElement(index) for index in range(elements.Length)
                if elements.GetElement(index).CurrentProcessId == self.probe.pid and
                elements.GetElement(index).CurrentControlType == 50004]
        if len(rows) != 1:
            raise RuntimeError(f"Expected one owned edit field: {label}")
        pointer = rows[0].GetCurrentPattern(self.api.uia.UIA_LegacyIAccessiblePatternId)
        if not pointer:
            raise RuntimeError("The owned edit field exposes no native value observation")
        legacy = pointer.QueryInterface(self.api.uia.IUIAutomationLegacyIAccessiblePattern)
        return legacy.CurrentValue or ""

    def controlled_password_value(self) -> str:
        return self.edit_value("密码")

    def masked_password(self, length: int):
        value = self.controlled_password_value()
        return validate_masked_password(value, length) if value else None

    def text_present(self, text: str) -> bool:
        root = self.api._uia().ElementFromHandle(self.window)
        elements = root.FindAll(self.api.uia.TreeScope_Descendants,
                                self.api.automation.CreateTrueCondition())
        return any(elements.GetElement(index).CurrentProcessId == self.probe.pid and
                   text in (elements.GetElement(index).CurrentName or "")
                   for index in range(elements.Length))

    def find_control(self, label: str):
        types = {"密码": 50004, "端口": 50004, "配置档 YAML / JSON": 50004,
                 "导入与高级编辑": 50020}
        return self.api.control(self.window, label,
                                control_type=types.get(label, 50000))

    def control(self, label: str) -> dict:
        return wait_for(lambda: self.find_control(label), f"formal GUI control {label}", 20)

    def validate_click_target(self, label: str, target: dict) -> None:
        if (not self.visible() or self.api.user.GetForegroundWindow() != self.window or
                target.get("pid") != self.probe.pid):
            raise RuntimeError("Formal click target is not in the original foreground GUI")

    def control_click(self, label: str) -> None:
        previous, since = None, time.monotonic()
        def pointed_control():
            nonlocal previous, since
            candidate = self.find_control(label)
            if candidate is None:
                return None
            identity = {key: candidate[key] for key in ("name", "pid", "rect", "clickable")}
            if identity != previous:
                previous, since = identity, time.monotonic()
            x, y = candidate["clickable"]
            self.api.move([x, y, x + 1, y + 1])
            current = self.find_control(label)
            self.write(f"control-before-click-{len(self.steps)}.json",
                       {"target": candidate, "observed": current})
            if current is None or any(current[key] != candidate[key] for key in identity):
                return None
            return current if time.monotonic() - since >= 0.3 else None
        target = wait_for(pointed_control, f"formal control settles under the pointer: {label}", 20)
        self.validate_click_target(label, target)
        x, y = target["clickable"]
        bounds = [x, y, x + 1, y + 1]
        self.click(bounds, "left", self.probe.pid)
        self.steps.append({"physical_control_click": target})

    def scroll(self, notches: int) -> None:
        from ctypes import wintypes
        bounds = self.api.window_rect(self.window)
        x, y = bounds[0] + (bounds[2] - bounds[0]) * 4 // 5, bounds[1] + (bounds[3] - bounds[1]) * 3 // 5
        self.api.move([x, y, x + 1, y + 1])
        if (self.api.user.GetForegroundWindow() != self.window or
                self.api.window_pid(self.api.user.WindowFromPoint(wintypes.POINT(x, y))) != self.probe.pid):
            raise RuntimeError("Scroll target is not the foreground owned GUI")
        self.api.wheel(notches)
        self.steps.append({"physical_wheel": notches, "point": [x, y]})
        time.sleep(0.3)

    def reveal_control(self, label: str) -> dict:
        candidate = self.find_control(label)
        bounds = self.api.window_rect(self.window)
        if candidate is not None:
            x, y = candidate["clickable"]
            if bounds[0] < x < bounds[2] and bounds[1] + 80 < y < bounds[3] - 24:
                return candidate
        # A prior section may have left the list below the requested row.
        self.scroll(10)
        self.scroll(10)
        for _ in range(12):
            candidate = self.find_control(label)
            bounds = self.api.window_rect(self.window)
            if candidate is not None:
                x, y = candidate["clickable"]
                if bounds[0] < x < bounds[2] and bounds[1] + 80 < y < bounds[3] - 24:
                    return candidate
            self.scroll(-3)
        raise RuntimeError(f"Owned control did not become visible after bounded scroll: {label}")

    def displayed_value(self, label: str, expected: str) -> None:
        self.reveal_control(label)
        def matches():
            row = self.find_control(label)
            return row if row and row["name"].split("\n")[-1] == expected else None
        row = wait_for(matches, f"actual GUI displays {label}={expected}")
        self.steps.append({"displayed": label, "value": expected, "control": row})

    def port_text(self, value: str) -> None:
        self.control_click("端口")
        self.api.keys([0x11, 0x41])
        self.api.type_text(value)
        wait_for(lambda: self.edit_value("端口") == value, "actual port edit contains the typed test value")
        self.steps.append({"physical_port_input": value})

    def routing_record(self):
        path = self.profile / "rules/routing-state.json"
        return path.read_bytes() if path.exists() else None

    def configure_network(self) -> None:
        self.control_click("返回")
        self.control_click("网络连接")
        self.snapshot("09-network-initial")
        self.reveal_control("VPN 捕获模式")
        self.control_click("VPN 捕获模式")
        self.control_click("完整捕获（实验性）")
        self.value("settings.vpnCaptureMode", "full")
        self.snapshot("10-full-committed")
        self.reveal_control("HTTP 代理端口")
        self.control_click("HTTP 代理端口")
        self.port_text("70000")
        before = self.state()
        self.control_click("保存")
        wait_for(lambda: self.text_present("设置值无效"), "native invalid-port message")
        after = self.state()
        if before != after:
            raise RuntimeError("Rejected port changed the native settings record")
        self.steps.append({"invalid_port_rejected": "70000", "before": before, "after": after,
                           "error": "设置值无效"})
        self.snapshot("11-invalid-port")
        self.port_text("18443")
        self.control_click("保存")
        self.value("settings.httpPort", 18443)
        self.displayed_value("HTTP 代理端口", "18443")
        self.snapshot("12-port-committed")
        self.control_click("返回")

    def configure_rules(self, seed: dict) -> int:
        overrides = seed["routing_overrides"]
        baseline = self.routing_record()
        previous_version = json.loads(baseline)["version"] if baseline else 0
        if (baseline is None or previous_version != seed["routing_version"] or
                json.loads(baseline)["current"]["overrides"] != overrides):
            raise RuntimeError("The rule preservation scenario requires the seeded nonempty overrides")
        self.control_click("规则分流")
        self.snapshot("13-rules-initial")
        if self.routing_record() != baseline:
            raise RuntimeError("Opening the rules page changed the seeded native record")
        self.control_click("仅校园模板")
        self.reveal_control("校验并预览")
        self.control_click("校验并预览")
        wait_for(lambda: self.text_present(f"预览 仅校园 · 版本 {previous_version + 1}"),
                 "successful native rule preview")
        self.control("应用预览")
        if self.routing_record() != baseline:
            raise RuntimeError("Rule template/preview committed before Apply")
        self.steps.append({"rule_preview_uncommitted": True, "prior_version": previous_version,
                           "before": json.loads(baseline) if baseline else None})
        self.snapshot("14-rules-preview")
        self.control_click("应用预览")
        def applied():
            record = self.routing_record()
            if record is None:
                return None
            state = json.loads(record)
            return state if state["version"] == previous_version + 1 else None
        state = wait_for(applied, "explicit native rules commit")
        if (state["current"]["profile"] != "name: 仅校园\nrules:\n  - MATCH,REJECT\n" or
                state["current"]["providers"] != {} or
                state["current"]["overrides"] != overrides):
            raise RuntimeError("The applied template failed to preserve the seeded overrides")
        self.steps.append({"native_routing_commit": state})
        self.snapshot("15-rules-applied")
        return state["version"]

    def reject_invalid_rule(self, routing_version: int) -> None:
        before = self.routing_record()
        if before is None or json.loads(before)["version"] != routing_version:
            raise RuntimeError("Rule rejection requires the just-committed configuration")
        self.reveal_control("导入与高级编辑")
        self.control_click("导入与高级编辑")
        self.reveal_control("配置档 YAML / JSON")
        self.control_click("配置档 YAML / JSON")
        self.api.keys([0x11, 0x41])
        self.api.type_text(INVALID_RULE_DOCUMENT)
        wait_for(lambda: self.edit_value("配置档 YAML / JSON") == INVALID_RULE_DOCUMENT,
                 "owned raw editor contains the unsupported test rule")
        self.steps.append({"physical_rule_input": INVALID_RULE_DOCUMENT})
        self.reveal_control("校验并预览")
        self.control_click("校验并预览")
        wait_for(lambda: self.text_present("规则类型尚未支持"), "native unsupported-rule message")
        rows = self.snapshot("18-invalid-rule-rejected")
        receipt = None
        apply = [row for row in rows if row["name"].split("\n")[0] == "应用预览"
                 and row["type"] == 50000]
        if len(apply) == 1 and apply[0]["enabled"] is True:
            # This engine reports enabled even for the visibly disabled button.
            # Preserve that observation and test the real input separately.
            self.control_click("应用预览")
            target = self.steps[-1]["physical_control_click"]
            started = time.monotonic()
            time.sleep(0.5)
            rows = self.snapshot("18b-invalid-rule-apply-click")
            clicked_record = self.routing_record()
            receipt = {"target": target, "observed_seconds": time.monotonic() - started,
                       "before_sha256": hashlib.sha256(before).hexdigest(),
                       "after_sha256": hashlib.sha256(clicked_record).hexdigest()
                       if clicked_record is not None else None}
        after = self.routing_record()
        validate_rule_rejection(before, after, rows, blocked_apply=receipt)
        self.steps.append({"invalid_rule_rejected": INVALID_RULE_DOCUMENT,
                           "before": json.loads(before), "after": json.loads(after),
                           "before_sha256": hashlib.sha256(before).hexdigest(),
                           "after_sha256": hashlib.sha256(after).hexdigest(),
                           "native_error": "规则类型尚未支持",
                           "uia_apply_enabled": apply[0]["enabled"] if len(apply) == 1 else None,
                           "blocked_apply_click": receipt,
                           "committed_rule_record_unchanged": True})

    def restored_network_and_rules(self, routing_version: int) -> None:
        baseline = self.routing_record()
        self.control_click("返回")
        self.control_click("网络连接")
        self.displayed_value("HTTP 代理端口", "18443")
        self.displayed_value("VPN 捕获模式", "完整捕获（实验性）")
        self.snapshot("16-network-restored")
        self.control_click("VPN 捕获模式")
        self.control_click("校园分流")
        self.value("settings.vpnCaptureMode", "campus")
        self.control_click("返回")
        self.control_click("规则分流")
        wait_for(lambda: self.text_present("仅校园") and
                 self.text_present(f"正在使用版本 {routing_version}") and
                 self.text_present("最终兜底 · REJECT"), "rules restored in the actual GUI")
        state = json.loads(self.routing_record())
        if state["version"] != routing_version or self.routing_record() != baseline:
            raise RuntimeError("Opening the restored rule page changed its native record")
        self.steps.append({"restored_rule_version": routing_version})
        self.snapshot("17-rules-restored")
        self.reveal_control("导入与高级编辑")
        self.control_click("导入与高级编辑")
        self.reveal_control("配置档 YAML / JSON")
        expected = state["current"]["profile"]
        wait_for(lambda: self.edit_value("配置档 YAML / JSON") == expected,
                 "reopened raw editor restores the committed profile after invalid draft rejection")
        self.steps.append({"restored_rule_editor": expected})
        self.snapshot("19-valid-rule-editor-restored")

    def state(self):
        path = self.profile / "settings/settings-state.json"
        if not path.exists():
            return None
        return json.loads(path.read_text(encoding="utf-8"))

    def value(self, key: str, expected):
        def committed():
            state = self.state()
            return state if state and state["values"].get(key) == expected else None
        state = wait_for(committed, f"formal GUI committed {key}={expected}")
        self.steps.append({"native_commit": key, "value": expected, "revision": state["revision"]})
        return state

    def start_launch(self, name: str) -> None:
        environment = {key: value for key, value in os.environ.items()
                       if not key.upper().startswith(("SHUVPN_", "FRB_DART_LOAD_EXTERNAL_LIBRARY"))}
        self.log = (self.report / f"{name}.log").open("wb")
        self.application = subprocess.Popen([str(self.bundle / "shuvpn.exe")], cwd=ROOT,
                                            env=environment, stdin=subprocess.DEVNULL,
                                            stdout=self.log, stderr=subprocess.STDOUT)
        self.probe = RetainedProcess(self.api, self.application.pid)
        if (self.probe.identity["created_ticks"] != self.api.created_ticks(int(self.application._handle)) or
                self.probe.identity["elevated"] or
                Path(self.probe.identity["image"]) != self.bundle / "shuvpn.exe" or
                self.probe.identity["sha256"] != self.products["shuvpn.exe"]):
            raise RuntimeError("Formal GUI process identity differs")
        self.window = wait_for(self.main_window, "owned formal main HWND")
        self.taskbar = self.api.user.FindWindowW("Shell_TrayWnd", None)
        if self.explorer is None:
            self.explorer = RetainedProcess(self.api, self.api.window_pid(self.taskbar))
        if (Path(self.explorer.identity["image"]) != Path(os.environ["SystemRoot"]) / "explorer.exe" or
                self.explorer.identity["session"] != self.probe.identity["session"]):
            raise RuntimeError("Formal GUI Explorer is not its interactive session")
        wait_for(self.state, "production bootstrap created private native settings", 30)
        wait_for(lambda: self.api.icon_rect(self.window), "production bootstrap created owned tray", 20)
        self.activate_formal()
        wait_for(lambda: self.api.enable_accessibility(self.window), "formal Flutter semantics")
        modules = self.probe.modules()
        if Path(modules.get("shuvpn_core.dll", "")) != self.bundle / "shuvpn_core.dll":
            raise RuntimeError("Formal bootstrap did not load its default paired core")
        listeners = self.api.listeners()
        owned = [row for row in listeners["matches"] if row["pid"] == self.probe.pid]
        if owned:
            raise RuntimeError("Offline formal GUI unexpectedly opened proxy listeners")
        self.launches.append({"name": name, "process": self.probe.identity,
                              "window": self.window, "paired_core": modules["shuvpn_core.dll"],
                              "listeners_owned": owned})
        self.activate_formal()
        self.snapshot(name)

    def stop_launch(self) -> None:
        if self.api.user.GetForegroundWindow() != self.window:
            raise RuntimeError("Owned formal window lost foreground before Ctrl+Q")
        self.api.keys([0x11, 0x51])
        wait_for(lambda: self.probe.exit_code() is not None, "original formal GUI exits normally", 20)
        self.probe.require_successful_exit()
        if self.application.wait(timeout=3) != 0:
            raise RuntimeError("Formal GUI Popen exit was nonzero")
        wait_for(lambda: self.api.icon_rect(self.window) is None, "formal tray icon removed")
        self.launches[-1]["exit"] = 0
        self.probe.close()
        self.probe = self.application = self.window = None
        self.log.close()
        self.log = None

    def native_probe(self, mode: str, *, rule_hosts=()) -> dict:
        if self.application is not None:
            raise RuntimeError("Independent storage probe requires the GUI exited")
        if rule_hosts and mode != "read":
            raise RuntimeError("Synthetic rule targets are only allowed for independent readback")
        command = [str(self.dart), "run", str(ROOT / "tool/gui_settings_probe.dart"),
                   str(self.bundle / "shuvpn_core.dll"), str(self.profile / "settings"), mode]
        if rule_hosts:
            command.append(json.dumps(list(rule_hosts)))
        result = subprocess.run(command,
                                cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=40)
        (self.report / f"native-{mode}-{len(self.launches)}.log").write_text(
            result.stdout + result.stderr, encoding="utf-8")
        if result.returncode:
            raise RuntimeError("Formal GUI native persistence probe failed")
        lines = [line.removeprefix("GUI_NATIVE_SETTINGS ") for line in result.stdout.splitlines()
                 if line.startswith("GUI_NATIVE_SETTINGS ")]
        if len(lines) != 1:
            raise RuntimeError("Native probe returned no unique settings result")
        return json.loads(lines[0])

    def additional_settings(self, baseline: dict) -> dict:
        return baseline

    def finish_formal(self, primary_error) -> None:
        prior_success = self.result.get("passed") is True
        self.result["passed"] = False
        self.result["launches"] = self.launches
        cleanup_failure = None
        try:
            if self.application is not None and self.application.poll() is None and self.window:
                self.api.user.SetForegroundWindow(self.window)
                if self.api.user.GetForegroundWindow() == self.window:
                    self.api.keys([0x11, 0x51])
                    try:
                        self.application.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        pass
        except BaseException as error:
            cleanup_failure = error
        try:
            self.finish()
        except BaseException as error:
            cleanup_failure = cleanup_failure or error
        if cleanup_failure is not None:
            self.result["cleanup_error"] = repr(cleanup_failure)
        try:
            cleanup_profile(self.marker)
            self.result["private_knownfolder_directories_removed"] = True
        except BaseException as error:
            cleanup_failure = cleanup_failure or error
            self.result["private_profile_cleanup_error"] = repr(error)
        self.result["passed"] = (prior_success and primary_error is None and cleanup_failure is None and
                                 self.result.get("cleanup_error") is None)
        try:
            self.write("verification.json", self.result)
        except BaseException as error:
            self.result["passed"] = False
            self.result["report_write_error"] = repr(error)
            if primary_error is None and cleanup_failure is None:
                raise
            print(f"Formal GUI report write failed: {error}", file=sys.stderr)
        if cleanup_failure is not None and primary_error is None:
            raise cleanup_failure

    def run(self) -> None:
        try:
            self.start_launch("01-welcome")
            assert self.state()["values"].get("settings.welcomeCompleted", False) is False
            self.control_click("继续")
            wait_for(lambda: self.text_present("权限管理"), "formal permissions page")
            self.snapshot("02-permissions")
            self.control_click("继续")
            wait_for(lambda: self.text_present("登录会话与账户展示状态会保存在本机"), "formal account notice")
            self.snapshot("03-account-notice")
            self.control_click("去登录")
            wait_for(lambda: self.text_present("学号"), "formal sign-in form")
            rows = self.snapshot("04-login-input-only")
            assert {row["name"] for row in rows if row["type"] == 50004} == {"用户名/学号", "密码"}
            self.control_click("密码")
            self.api.type_text("gui-fixture-only")
            try:
                masked = wait_for(lambda: self.masked_password(len("gui-fixture-only")),
                                  "controlled local password masks its accessibility value")
                self.steps.append({"controlled_password_accessibility": masked,
                                   "submitted": False})
                self.snapshot("04b-local-test-password-only")
            finally:
                self.api.keys([0x11, 0x41])
                self.api.keys([0x08])
            wait_for(lambda: self.controlled_password_value() == "",
                     "controlled local password cleared before exit")
            self.steps.append({"controlled_password_cleared": True})
            self.stop_launch()
            seed = self.native_probe("prepare-offline-settings")
            assert seed["welcome"]
            overrides = seed["routing_overrides"]
            if (not overrides or overrides == "{}" or seed["routing_version"] != 1 or
                    seed["routing_override_actions"] != {
                        "before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"}):
                raise RuntimeError("Independent native preparation did not seed working rule overrides")
            self.steps.append({"independent_native_offline_seed": seed})
            self.start_launch("05-offline-home")
            self.control_click("设置")
            self.control_click("外观")
            self.control_click("深色")
            self.value("settings.themeMode", "dark")
            self.dark_surface()
            self.snapshot("06-dark-committed")
            self.configure_network()
            routing_version = self.configure_rules(seed)
            self.reject_invalid_rule(routing_version)
            self.stop_launch()
            native = self.native_probe("read")
            validate_offline_settings(native, "full", routing_version, overrides)
            self.steps.append({"independent_native_before_reopen": native})
            self.start_launch("07-reopened-dark")
            self.control_click("设置")
            self.control_click("外观")
            self.control("深色")
            self.dark_surface()
            rows = self.snapshot("08-dark-restored")
            self.steps.append({"dark_selection_observation": [
                row for row in rows if row["name"].split("\n")[0] == "深色"]})
            self.restored_network_and_rules(routing_version)
            self.stop_launch()
            final = self.native_probe("read")
            validate_offline_settings(final, "campus", routing_version, overrides)
            validate_launches(self.launches, 3)
            final = self.additional_settings(final)
            validate_launches(self.launches, self.expected_launches)
            assert all(file_hash(self.bundle / name) == expected for name, expected in self.products.items())
            assert source_identity() == self.result["verifier_source"], "Verifier source changed during GUI acceptance"
            shutil.copytree(self.profile, self.report / "profile")
            self.result.update(passed=True, native=final, offline_welcome_seed=True,
                               production_bootstrap_tested=True, products=self.products,
                               explorer=self.explorer.identity, routing_version=final["routing_version"],
                               offline_network_and_rule_settings_tested=True,
                               invalid_rule_validation_preserved_active_configuration=True)
        except BaseException as error:
            self.result["passed"] = False
            self.result["error"] = repr(error)
            self.result["last_pointer"] = getattr(self.api, "last_pointer", None)
            if self.window and self.probe:
                try:
                    self.snapshot("failure")
                except Exception as observation_error:
                    self.result["failure_observation_error"] = repr(observation_error)
            raise
        finally:
            FormalAcceptance.finish_formal(self, sys.exc_info()[1])
        if not self.result["passed"]:
            raise RuntimeError("Formal GUI checks or final cleanup were incomplete")
        print("WINDOWS_PRODUCTION_MAIN_PORT_CAPTURE_RULES_AND_REOPEN_PASSED")


def main() -> None:
    if not __debug__ or sys.platform != "win32":
        raise RuntimeError("Use ordinary Windows Python with assertions enabled")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--dart", type=Path, required=True)
    FormalAcceptance(parser.parse_args()).run()


if __name__ == "__main__":
    main()
