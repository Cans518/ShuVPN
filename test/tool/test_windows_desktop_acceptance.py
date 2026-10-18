import copy
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
import os
import socket
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import windows_tray_acceptance as tray
from windows_desktop_probe import RetainedProcess, WindowsDesktop, icon_name_matches


class WindowsObservationTests(unittest.TestCase):
    def setUp(self):
        self.token, self.pid = "a" * 32, 421
        self.ready = {"token": self.token, "pid": self.pid, "stage": "ready",
                      "proxy_port": 18443, "fixture_pid": 422, "tray_ready": True,
                      "status": f"已连接 · {self.token}"}
        self.cleanup = {**self.ready, "stage": "cleanup", "session_state": "disconnected",
                        "fixture_exit": 0, "proxy_closed": True,
                        "upload": 131072, "download": 131086}

    def test_valid_native_observations(self):
        tray.validate_ready(self.ready, self.token, self.pid)
        tray.validate_cleanup(self.cleanup, self.token, self.pid, self.ready)

    def test_wrong_entrypoint_or_changed_aot_is_rejected_before_launch(self):
        products = {"data/app.so": "a" * 64}
        marker = {"schema": 1, "entrypoint": tray.PROBE_ENTRYPOINT, "products": products}
        tray.validate_probe_marker(marker, products)
        with self.assertRaises(RuntimeError):
            tray.validate_probe_marker({**marker, "entrypoint": "lib/main.dart"}, products)
        with self.assertRaises(RuntimeError):
            tray.validate_probe_marker(marker, {"data/app.so": "b" * 64})

    def test_foreign_token_pid_or_stage_is_rejected(self):
        for key, value in (("token", "f" * 32), ("pid", 900), ("stage", "ready")):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                tray.validate_cleanup({**self.cleanup, key: value}, self.token, self.pid, self.ready)

    def test_incomplete_readiness_is_rejected(self):
        for key, value in (("proxy_port", 0), ("proxy_port", 65536), ("proxy_port", True),
                           ("fixture_pid", self.pid), ("tray_ready", False), ("status", "已连接")):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                tray.validate_ready({**self.ready, key: value}, self.token, self.pid)

    def test_icon_requires_exact_nonce_tooltip_even_when_explorer_prepends_title(self):
        tooltip = f"ShuVPN · 已连接 · {self.token}"
        self.assertTrue(icon_name_matches(tooltip, tooltip))
        self.assertTrue(icon_name_matches(f"ShuVPN {tooltip}", tooltip))
        for name in ("ShuVPN", tooltip + "other", "foreign " + tooltip,
                     tooltip.replace(self.token, "b" * 32)):
            with self.subTest(name=name):
                self.assertFalse(icon_name_matches(name, tooltip))

    def test_navigation_scan_keeps_e0_when_layout_mapping_omits_prefix(self):
        sent = []
        adapter = SimpleNamespace(
            user=SimpleNamespace(MapVirtualKeyW=lambda key, mode: 0x4F),
            _send=sent.append)
        WindowsDesktop.keys(adapter, [0x23])
        self.assertEqual([(event.body.keyboard.scan, event.body.keyboard.flags)
                          for event in sent[0]], [(0x4F, 9), (0x4F, 11)])

    def pointer_adapter(self, observed):
        def cursor(pointer):
            pointer._obj.x, pointer._obj.y = observed
            return True

        def clip(pointer):
            pointer._obj.left, pointer._obj.top = 0, 0
            pointer._obj.right, pointer._obj.bottom = 3072, 1920
            return True

        sent = []
        return SimpleNamespace(
            user=SimpleNamespace(
                GetSystemMetrics=lambda index: {76: 0, 77: 0, 78: 3072, 79: 1920}[index],
                GetCursorPos=cursor, GetPhysicalCursorPos=cursor, GetClipCursor=clip,
                MonitorFromPoint=lambda point, flags: 1),
            require=WindowsDesktop.require, _send=sent.append), sent

    def test_pointer_rejects_outside_desktop_or_clip_before_sending_input(self):
        adapter, sent = self.pointer_adapter((500, 500))
        for bounds in ([3072, 100, 3073, 101], [100, -1, 101, 0]):
            with self.subTest(bounds=bounds), self.assertRaisesRegex(RuntimeError, "outside"):
                WindowsDesktop.move(adapter, bounds)
        adapter.user.MonitorFromPoint = lambda point, flags: None
        with self.assertRaisesRegex(RuntimeError, "monitor"):
            WindowsDesktop.move(adapter, [100, 100, 101, 101])
        self.assertEqual(sent, [])

    def test_failed_pointer_retains_geometry_and_both_cursor_observations(self):
        adapter, sent = self.pointer_adapter((957, 1219))
        with patch("windows_desktop_probe.time.monotonic", side_effect=[0, 2]):
            with self.assertRaisesRegex(RuntimeError, "Physical pointer"):
                WindowsDesktop.move(adapter, [1713, 1872, 1714, 1873])
        self.assertEqual(len(sent), 1)
        self.assertEqual(adapter.last_pointer["requested"], [1713, 1872])
        self.assertEqual(adapter.last_pointer["observed"], [957, 1219])
        self.assertEqual(adapter.last_pointer["physical"], [957, 1219])
        self.assertEqual(adapter.last_pointer["clip"], [0, 0, 3072, 1920])
        self.assertFalse(adapter.last_pointer["reached"])

    def test_control_type_excludes_hover_tooltip_but_rejects_duplicate_buttons(self):
        def element(kind):
            return SimpleNamespace(
                CurrentName="返回", CurrentProcessId=self.pid, CurrentIsOffscreen=False,
                CurrentIsEnabled=True, CurrentControlType=kind,
                CurrentBoundingRectangle=SimpleNamespace(left=10, top=10, right=30, bottom=30),
                GetClickablePoint=lambda: (SimpleNamespace(x=20, y=20), True),
                GetRuntimeId=lambda: [])
        rows = [element(50000), element(50020)]
        collection = SimpleNamespace(Length=2, GetElement=lambda index: rows[index])
        root = SimpleNamespace(FindAll=lambda scope, condition: collection)
        automation = SimpleNamespace(ElementFromHandle=lambda handle: root,
                                     CreateTrueCondition=lambda: None)
        adapter = SimpleNamespace(_uia=lambda: automation,
                                  uia=SimpleNamespace(TreeScope_Descendants=4),
                                  window_pid=lambda window: self.pid)
        selected = WindowsDesktop.control(adapter, 123, "返回", control_type=50000)
        self.assertEqual(selected["control_type"], 50000)
        with self.assertRaisesRegex(RuntimeError, "Multiple"):
            WindowsDesktop.control(adapter, 123, "返回")
        rows[1] = element(50000)
        with self.assertRaisesRegex(RuntimeError, "Multiple"):
            WindowsDesktop.control(adapter, 123, "返回", control_type=50000)

    def test_control_hit_test_requires_the_owned_named_enabled_ancestor_at_the_actual_point(self):
        label = "新增前置规则"
        def element(**changes):
            return SimpleNamespace(
                **{"CurrentName": label, "CurrentProcessId": self.pid,
                   "CurrentControlType": 50000, "CurrentIsEnabled": True,
                   "CurrentIsOffscreen": False,
                   "CurrentBoundingRectangle": SimpleNamespace(left=10, top=10, right=90, bottom=90),
                   "parent": None, **changes})
        selected = element()
        leaf = element(CurrentName="glyph", CurrentControlType=50020, parent=selected)
        current = leaf
        automation = SimpleNamespace(
            ElementFromPoint=lambda point: current,
            ControlViewWalker=SimpleNamespace(GetParentElement=lambda node: node.parent))
        adapter = SimpleNamespace(
            _uia=lambda: automation, uia=SimpleNamespace(tagPOINT=lambda x, y: (x, y)))
        actual = WindowsDesktop.control_at(adapter, [50, 50], self.pid, label, control_type=50000)
        self.assertEqual(actual["name"], label)
        self.assertEqual(actual["clickable"], [50, 50])
        for change in (
            {"CurrentName": "other"}, {"CurrentControlType": 50022},
            {"CurrentIsEnabled": False}, {"CurrentIsOffscreen": True},
            {"CurrentProcessId": self.pid + 1},
        ):
            current = element(**change)
            with self.subTest(change=change):
                self.assertIsNone(WindowsDesktop.control_at(
                    adapter, [50, 50], self.pid, label, control_type=50000))
        self.assertFalse(adapter.last_control_hit["owned"])

    def test_native_command_sequence_rejects_extra_or_wrong_commands(self):
        events = [{"token": self.token, "pid": self.pid, "stage": f"event-{index}",
                   "name": "native-command:" + command}
                  for index, command in enumerate(("show", "show", "quit"))]
        tray.validate_commands(events, self.token, self.pid)
        changed = copy.deepcopy(events)
        changed[1]["name"] = "native-command:disconnect"
        with self.assertRaises(RuntimeError):
            tray.validate_commands(changed, self.token, self.pid)

    def test_timeout_needs_real_listener_tables_and_cannot_hide_an_open_port(self):
        snapshot = {"api": "GetExtendedTcpTable:TCP_TABLE_OWNER_PID_LISTENER",
                    "tables": [{"family": 2, "count": 1}, {"family": 23, "count": 0}],
                    "matches": []}
        tray.validate_listener_cleanup(snapshot, "timeout")
        for bad in ({}, {**snapshot, "tables": []}, {**snapshot, "matches": [{"pid": 999}]}):
            with self.subTest(snapshot=bad), self.assertRaises(RuntimeError):
                tray.validate_listener_cleanup(bad, "timeout")
        with self.assertRaises(RuntimeError):
            tray.validate_listener_cleanup(snapshot, "accepted")

    def test_cleanup_requires_complete_traffic_original_fixture_and_closed_proxy(self):
        for key, value in (("fixture_pid", 999), ("fixture_exit", 7), ("proxy_closed", False),
                           ("upload", 65536), ("download", 131072),
                           ("session_state", "connected")):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                tray.validate_cleanup({**self.cleanup, key: value}, self.token, self.pid, self.ready)

    def test_optimizer_is_rejected_before_creating_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / "report"
            result = subprocess.run(
                [sys.executable, "-O", tray.__file__, "--report", str(report)],
                capture_output=True, text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("assertions enabled", result.stderr)
            self.assertFalse(report.exists())

    def test_checked_protocol_cannot_be_enabled_in_connected_menu(self):
        labels = [self.ready["status"], "显示窗口", "连接", "断开", "协议", "退出"]
        menu = {"rows": [
            {"label": label, "separator": False, "enabled": label not in labels[:1] + ["连接"],
             "children": []} for label in labels]}
        menu["rows"][4]["children"] = [
            {"label": label, "enabled": False, "checked": label == "aTrust"}
            for label in ("aTrust", "EasyConnect", "OpenVPN")]
        tray.validate_menu(menu, self.ready["status"])
        changed = copy.deepcopy(menu)
        changed["rows"][4]["children"][0]["enabled"] = True
        with self.assertRaisesRegex(RuntimeError, "enabled/check"):
            tray.validate_menu(changed, self.ready["status"])

    @unittest.skipUnless(sys.platform == "win32", "Windows original process handles")
    def test_retained_real_process_exit_zero_and_seven(self):
        api = WindowsDesktop()
        for code in (0, 7):
            with self.subTest(code=code):
                child = subprocess.Popen(
                    [sys.executable, "-c", f"import sys;sys.stdin.buffer.read(1);sys.exit({code})"],
                    stdin=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
                process = None
                try:
                    process = RetainedProcess(api, child.pid)
                    self.assertEqual(process.identity["created_ticks"],
                                     api.created_ticks(int(child._handle)))
                    with self.assertRaises(RuntimeError):
                        process.require_successful_exit()
                    child.stdin.write(b"x")
                    child.stdin.close()
                    self.assertEqual(child.wait(timeout=8), code)
                    self.assertEqual(process.exit_code(), code)
                    if code == 0:
                        process.require_successful_exit()
                    else:
                        with self.assertRaisesRegex(RuntimeError, "successful exit: 7"):
                            process.require_successful_exit()
                finally:
                    if child.poll() is None:
                        child.kill()
                        child.wait(timeout=3)
                    if process is not None:
                        process.close()

    @unittest.skipUnless(sys.platform == "win32", "Real Windows owner-PID listener tables")
    def test_real_ipv4_and_ipv6_listener_rows_have_the_current_owner_and_disappear(self):
        api = WindowsDesktop()
        for family, address in ((socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")):
            with self.subTest(family=family):
                with socket.socket(family, socket.SOCK_STREAM) as listener:
                    if family == socket.AF_INET6:
                        listener.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
                    listener.bind((address, 0))
                    listener.listen()
                    port = listener.getsockname()[1]
                    self.assertIn({"pid": os.getpid(), "address": address, "port": port,
                                   "family": int(family), "state": 2},
                                  api.listeners(port)["matches"])
                tray.wait_for(
                    lambda: not any(row["pid"] == os.getpid() for row in api.listeners(port)["matches"]),
                    "original real listener disappears from both OS tables", 2)


if __name__ == "__main__":
    unittest.main()
