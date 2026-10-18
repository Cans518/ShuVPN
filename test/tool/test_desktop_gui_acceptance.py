from pathlib import Path
from types import SimpleNamespace
import hashlib
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import desktop_gui_acceptance as gui


class GuiSessionBoundaryTest(unittest.TestCase):
    def rejection_observation(self, record):
        digest = hashlib.sha256(record).hexdigest()
        return {"input": gui.INVALID_RULE_DOCUMENT,
                "error": "规则类型尚未支持", "error_showing": True,
                "target": {"name": "应用预览", "role": "push button", "pid": 123},
                "attempt": {"actions": 0, "result": None},
                "observed_seconds": 0.5,
                "before_sha256": digest, "after_sha256": digest}

    def test_rule_rejection_requires_the_original_native_bytes_after_the_click(self):
        record = b'{"version":2,"current":{"profile":"valid","overrides":"keep"}}'
        observation = self.rejection_observation(record)
        gui.validate_rule_rejection(record, record, observation)
        for before, after in ((None, None), (b"", b""), (record, None),
                              (record, record.replace(b'"version":2', b'"version":3')),
                              (record, record.replace(b"keep", b"lost"))):
            with self.subTest(before=before, after=after), self.assertRaises(RuntimeError):
                gui.validate_rule_rejection(before, after, observation)

    def test_rule_rejection_needs_the_input_native_error_and_owned_click_observation(self):
        record = b"native record"
        valid = self.rejection_observation(record)
        for changed in ({}, {**valid, "input": "{}"}, {**valid, "error": "设置值无效"},
                        {**valid, "error_showing": False},
                        {**valid, "target": {"name": "保存", "role": "push button", "pid": 123}},
                        {**valid, "target": {"name": "应用预览", "role": "push button", "pid": 0}},
                        {**valid, "before_sha256": "different"},
                        {**valid, "after_sha256": "different"},
                        {**valid, "observed_seconds": 0},
                        {**valid, "observed_seconds": float("nan")},
                        {**valid, "observed_seconds": float("inf")},
                        {**valid, "attempt": {}},
                        {**valid, "attempt": {"actions": 0}},
                        {**valid, "attempt": {"actions": -1}},
                        {**valid, "attempt": {"actions": 0, "result": True}},
                        {**valid, "attempt": {"actions": 1, "result": "accepted"}}):
            with self.subTest(observation=changed), self.assertRaises(RuntimeError):
                gui.validate_rule_rejection(record, record, changed)

    def test_expander_and_native_error_accept_the_observed_unique_panel_role(self):
        def node(name, role):
            return SimpleNamespace(get_name=lambda: name, get_role_name=lambda: role)
        expander = node("导入与高级编辑\nYAML / JSON", "panel")
        error = node("规则类型尚未支持", "panel")
        adapter = SimpleNamespace(application=SimpleNamespace(pid=123))
        with patch.object(gui, "accessible_nodes", return_value=[expander, error]), \
                patch.object(gui, "wait_for", side_effect=lambda predicate, description: predicate()):
            self.assertIs(gui.Session.control_node(adapter, "导入与高级编辑"), expander)
            self.assertIs(gui.Session.control_node(
                adapter, "规则类型尚未支持", roles=("panel",)), error)
        with patch.object(gui, "accessible_nodes", return_value=[error, error]), \
                patch.object(gui, "wait_for", side_effect=lambda predicate, description: predicate()):
            self.assertIsNone(gui.Session.control_node(
                adapter, "规则类型尚未支持", roles=("panel",)))

    def setUp(self):
        self.environment = {
            "DISPLAY": ":99", "SHUVPN_GUI_PARENT_DISPLAY": ":0",
            "DBUS_SESSION_BUS_ADDRESS": "unix:path=/tmp/private",
            "SHUVPN_GUI_PARENT_DBUS": "unix:path=/run/user/1000/bus",
            "SHUVPN_GUI_ACCEPTANCE_TOKEN": "owned-token",
        }

    def test_owned_session_is_accepted(self):
        gui.validate_session(self.environment, "owned-token", 1000)

    def test_root_wrong_token_and_inherited_desktop_are_rejected(self):
        for token, uid in (("wrong", 1000), ("owned-token", 0), (None, 1000)):
            with self.subTest(token=token, uid=uid), self.assertRaises(RuntimeError):
                gui.validate_session(self.environment, token, uid)
        for key, parent in (("DISPLAY", "SHUVPN_GUI_PARENT_DISPLAY"),
                            ("DBUS_SESSION_BUS_ADDRESS", "SHUVPN_GUI_PARENT_DBUS")):
            environment = dict(self.environment)
            environment[key] = environment[parent]
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                gui.validate_session(environment, "owned-token", 1000)

    def test_missing_session_endpoints_are_rejected(self):
        for key in ("DISPLAY", "DBUS_SESSION_BUS_ADDRESS"):
            environment = dict(self.environment)
            environment.pop(key)
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                gui.validate_session(environment, "owned-token", 1000)

    @unittest.skipUnless(sys.platform == "linux", "Linux private DBus session")
    def test_private_profile_and_runtime_exist_before_dbus_starts(self):
        with tempfile.TemporaryDirectory() as temporary:
            args = SimpleNamespace(
                inside=False, report=Path(temporary) / "report",
                bundle=Path(temporary), dart=Path(sys.executable),
            )
            observed_runtime = []

            def start_private_session(command, *, env, check):
                self.assertIn("dbus-run-session", command)
                self.assertTrue(check)
                runtime = Path(env["XDG_RUNTIME_DIR"])
                self.assertNotEqual(str(runtime), "/run/user/0")
                self.assertTrue(runtime.is_dir())
                self.assertEqual(runtime.stat().st_mode & 0o777, 0o700)
                observed_runtime.append(runtime)
                for name in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"):
                    directory = Path(env[name])
                    self.assertTrue(directory.is_relative_to(args.report / "profile"))
                    self.assertTrue(directory.is_dir())
                    self.assertEqual(directory.stat().st_mode & 0o777, 0o700)

            with patch.dict(gui.os.environ, {
                "XDG_RUNTIME_DIR": "/run/user/0",
                "XDG_CONFIG_HOME": "/root/.config",
                "XDG_DATA_HOME": "/root/.local/share",
                "XDG_CACHE_HOME": "/root/.cache",
            }), patch.object(gui.subprocess, "run", side_effect=start_private_session):
                gui.main(args)
            self.assertEqual(len(observed_runtime), 1)
            self.assertFalse(observed_runtime[0].exists())

    def test_optimized_run_cannot_disable_assertions_or_create_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / "must-not-create"
            result = subprocess.run([
                sys.executable, "-O", str(Path(gui.__file__)), "--bundle", temporary,
                "--dart", sys.executable, "--report", str(report),
            ], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("assertions enabled", result.stderr)
            self.assertFalse(report.exists())

    def test_cleanup_runs_every_action_and_keeps_the_original_failure(self):
        called = []
        def cleanup_error():
            called.append("application")
            raise RuntimeError("cleanup failure")
        with patch.object(gui.sys, "stderr"), self.assertRaisesRegex(ValueError, "original"):
            try:
                raise ValueError("original")
            finally:
                gui.finish((cleanup_error, lambda: called.append("panel")))
        self.assertEqual(called, ["application", "panel"])
        with patch.object(gui.sys, "stderr"), self.assertRaisesRegex(RuntimeError, "cleanup failure"):
            gui.finish((cleanup_error,))

    def test_accessibility_traversal_tolerates_a_removed_transition_child(self):
        class Node:
            def __init__(self, children):
                self.children = children
            def get_child_count(self):
                return len(self.children)
            def get_child_at_index(self, index):
                return self.children[index]
        visible = Node([])
        root = Node([None, visible])
        self.assertEqual(gui.accessible_subtree(root), [root, visible])
        self.assertEqual(gui.accessible_subtree(None), [])
