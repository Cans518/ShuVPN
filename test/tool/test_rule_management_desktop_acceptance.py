from contextlib import redirect_stderr
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import desktop_rule_management_acceptance as management
import desktop_gui_acceptance as gui


class RuleManagementAcceptanceTest(unittest.TestCase):
    def native(self, version=3, order="REJECT", rule_id="profile:0", group="DIRECT"):
        return {
            "routing_version": version, "routing_name": management.PROFILE_NAME,
            "routing_test_action": "REJECT",
            "routing_override_actions": {"before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"},
            **{key: False for key in management.NETWORK_SWITCHES},
            "routing_custom_decisions": {
                management.ORDER_HOST: {"version": version, "source": "profile", "rule_id": rule_id,
                                        "policy": order, "action": order},
                management.GROUP_HOST: {"version": version, "source": "profile", "rule_id": "profile:2",
                                        "policy": "Office", "action": group}}}

    def test_each_management_phase_requires_the_exact_native_decision_and_safe_baseline(self):
        for version, action, rule_id, group in (
            (3, "REJECT", "profile:0", "DIRECT"), (4, "DIRECT", "profile:0", "DIRECT"),
            (5, "REJECT", "profile:1", "DIRECT"), (6, "REJECT", "profile:1", "REJECT"),
            (7, "REJECT", "profile:1", "DIRECT"),
        ):
            value = self.native(version, action, rule_id, group)
            management.validate_management_readback(value, version, action, rule_id, group)
        valid = self.native()
        for key, value in (("routing_version", 2), ("routing_name", "other"),
                           ("routing_test_action", "DIRECT"), ("routing_override_actions", {}),
                           ("http_enabled", None), ("system_proxy_enabled", True)):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                management.validate_management_readback({**valid, key: value}, 3, "REJECT", "profile:0", "DIRECT")
        for host, change in (
            (management.ORDER_HOST, {"rule_id": "profile:1"}),
            (management.ORDER_HOST, {"version": 2}),
            (management.GROUP_HOST, {"policy": "DIRECT"}),
            (management.GROUP_HOST, {"source": "append"}),
            (management.GROUP_HOST, {"action": "REJECT"}),
        ):
            invalid = copy.deepcopy(valid)
            invalid["routing_custom_decisions"][host].update(change)
            with self.subTest(host=host, change=change), self.assertRaises(RuntimeError):
                management.validate_management_readback(invalid, 3, "REJECT", "profile:0", "DIRECT")

    def test_menu_selection_rejects_duplicates_and_noninteractive_text(self):
        def node(name, role="push button", actions=1):
            return SimpleNamespace(get_name=lambda: name, get_role_name=lambda: role,
                                   get_n_actions=lambda: actions)
        expected = node("REJECT", "menu item")
        self.assertIs(management.unique_choice([node("REJECT", "label"), expected], "REJECT"), expected)
        self.assertIsNone(management.unique_choice([node("REJECT", actions=0)], "REJECT"))
        with self.assertRaises(RuntimeError):
            management.unique_choice([expected, node("REJECT")], "REJECT")

    def test_a_stale_preview_cannot_satisfy_the_next_gui_test(self):
        stale = SimpleNamespace(get_name=lambda: "规则 profile:0 · REJECT · 版本 3")
        adapter = SimpleNamespace(
            application=SimpleNamespace(pid=123), choose=Mock(), named_node=Mock(),
            outer_scroll=Mock(), steps=[], text_field=Mock(), press=Mock())

        def require(predicate, description):
            if not predicate():
                raise RuntimeError("old preview stayed visible")
            return True

        with patch.object(gui, "accessible_nodes", return_value=[stale]), \
                patch.object(gui, "wait_for", side_effect=require):
            with self.assertRaisesRegex(RuntimeError, "old preview stayed"):
                management.ManagementSession.live_test(adapter, management.ORDER_HOST, 3, "REJECT", "profile:0")
        adapter.text_field.assert_not_called()
        adapter.press.assert_not_called()

    def test_unnamed_selectable_text_is_checked_for_stale_preview_too(self):
        stale_text = "规则 profile:0 · REJECT · 版本 3"
        text = SimpleNamespace(get_character_count=lambda: len(stale_text),
                               get_text=lambda _start, _end: stale_text)
        stale = SimpleNamespace(get_name=lambda: "", get_role_name=lambda: "text",
                                get_text_iface=lambda: text)
        self.assertEqual(management.accessible_text(stale), stale_text)
        adapter = SimpleNamespace(
            application=SimpleNamespace(pid=123), choose=Mock(), named_node=Mock(),
            outer_scroll=Mock(), steps=[], text_field=Mock(), press=Mock())

        def require(predicate, description):
            if not predicate():
                raise RuntimeError("old selectable preview stayed visible")
            return True

        with patch.object(gui, "accessible_nodes", return_value=[stale]), \
                patch.object(gui, "wait_for", side_effect=require):
            with self.assertRaisesRegex(RuntimeError, "old selectable preview stayed"):
                management.ManagementSession.live_test(adapter, management.ORDER_HOST, 3, "REJECT", "profile:0")
        adapter.text_field.assert_not_called()
        adapter.press.assert_not_called()

    def test_typed_text_is_read_from_the_current_owned_accessible_node(self):
        value = management.ORDER_HOST
        typed = [""]
        text = SimpleNamespace(get_character_count=lambda: len(typed[0]),
                               get_text=lambda _start, _end: typed[0])
        current = SimpleNamespace(
            get_name=lambda: "目标域名或 IP", get_role_name=lambda: "text",
            get_process_id=lambda: 123, get_text_iface=lambda: text, clear_cache=Mock())
        stale = SimpleNamespace(get_text_iface=Mock(side_effect=RuntimeError("stale node")))
        adapter = SimpleNamespace(
            application=SimpleNamespace(pid=123), named_node=Mock(return_value=stale),
            activate_editor=Mock(), key=Mock(), steps=[])
        with patch.object(gui, "accessible_nodes", return_value=[current]), \
                patch.object(gui, "command", side_effect=lambda *args: typed.__setitem__(0, args[-1])), \
                patch.object(management, "clipboard_text", return_value=value), \
                patch.object(gui, "wait_for", side_effect=lambda predicate, _description: predicate()):
            management.ManagementSession.text_field(adapter, "目标域名或 IP", value)
        stale.get_text_iface.assert_not_called()
        current.clear_cache.assert_called_once()
        self.assertTrue(any(step.get("field_readback") == "目标域名或 IP" and
                            step["text"] == value for step in adapter.steps))

    def test_wrong_copied_value_rejects_a_correct_atspi_value(self):
        value = management.ORDER_HOST
        text = SimpleNamespace(get_character_count=lambda: len(value),
                               get_text=lambda _start, _end: value)
        current = SimpleNamespace(
            get_name=lambda: "目标域名或 IP", get_role_name=lambda: "text",
            get_process_id=lambda: 123, get_text_iface=lambda: text, clear_cache=Mock())
        adapter = SimpleNamespace(
            application=SimpleNamespace(pid=123), named_node=Mock(return_value=current),
            activate_editor=Mock(), key=Mock(), snapshot_text_fields=Mock(), steps=[])
        with patch.object(gui, "accessible_nodes", return_value=[current]), \
                patch.object(gui, "command"), \
                patch.object(management, "clipboard_text", return_value="different.gui.invalid"), \
                patch.object(gui, "wait_for", side_effect=lambda predicate, _description: predicate()):
            with self.assertRaisesRegex(RuntimeError, "copied a different value"):
                management.ManagementSession.text_field(adapter, "目标域名或 IP", value)
        adapter.snapshot_text_fields.assert_called_once()

    def test_group_restore_must_observe_the_requested_selected_value(self):
        label = "策略组 · Office"
        selected = SimpleNamespace(
            get_name=lambda: label + "\nREJECT", get_role_name=lambda: "push button",
            get_child_count=lambda: 0, get_parent=lambda: None, get_n_actions=lambda: 1)
        adapter = SimpleNamespace(application=SimpleNamespace(pid=123),
                                  node_observation=Mock(return_value={"showing": True}),
                                  outer_scroll=Mock(), steps=[])
        with patch.object(gui, "accessible_nodes", return_value=[selected]):
            with self.assertRaisesRegex(RuntimeError, "Office"):
                management.ManagementSession.group_control(adapter, "DIRECT")

    def test_artifact_subdirectory_never_changes_the_owned_native_storage_location(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            adapter = SimpleNamespace(storage_data=root / "profile/data", report=root / "baseline")
            settings = adapter.storage_data / "app/settings/settings-state.json"
            routing = adapter.storage_data / "app/rules/routing-state.json"
            settings.parent.mkdir(parents=True)
            routing.parent.mkdir()
            settings.write_text("{}")
            routing.write_text("{}")
            self.assertEqual(management.ManagementSession.state_file(adapter), settings)
            self.assertEqual(management.ManagementSession.routing_file(adapter), routing)
            duplicate = adapter.storage_data / "other/routing-state.json"
            duplicate.parent.mkdir()
            duplicate.write_text("{}")
            with self.assertRaises(RuntimeError):
                management.ManagementSession.routing_file(adapter)

    def session(self, report):
        session = management.ManagementSession.__new__(management.ManagementSession)
        session.report = report
        session.application = None
        session.launches = []
        session.steps = []
        session.product = {}
        session.close = Mock()
        return session

    def test_baseline_failure_cannot_leave_an_overall_success(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            (report / "verification.json").write_text('{"passed":true}')
            session = self.session(report)
            with patch.object(gui.Session, "run", side_effect=RuntimeError("baseline failed")):
                with self.assertRaisesRegex(RuntimeError, "baseline failed"):
                    session.run()
            self.assertFalse(json.loads((report / "verification.json").read_text())["passed"])
            session.close.assert_called_once()

    def completed_baseline(self, session):
        session.launches = [{"pid": index + 1, "exit": 0} for index in range(3)]
        (session.report / "verification.json").write_text(json.dumps({
            "launches": session.launches, "native": {
                "routing_version": 2, "routing_override_actions": {
                    "before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"},
                **{key: False for key in management.NETWORK_SWITCHES}}}))

    def test_advanced_failure_retains_the_baseline_and_reports_overall_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            session = self.session(report)
            session.management = Mock(side_effect=RuntimeError("management failed"))
            with patch.object(gui.Session, "run", side_effect=lambda _session: self.completed_baseline(session)):
                with self.assertRaisesRegex(RuntimeError, "management failed"):
                    session.run()
            self.assertTrue((report / "baseline/verification.json").exists())
            self.assertFalse(json.loads((report / "verification.json").read_text())["passed"])

    def test_final_cleanup_failure_is_not_published_as_success(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            session = self.session(report)

            def completed():
                session.launches = [{"pid": index + 1, "exit": 0} for index in range(9)]
                return {"routing_version": 7}

            session.management = Mock(side_effect=completed)
            session.close = Mock(side_effect=RuntimeError("final cleanup failed"))
            with patch.object(gui.Session, "run", side_effect=lambda _session: self.completed_baseline(session)), \
                    redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(RuntimeError, "final cleanup failed"):
                    session.run()
            self.assertFalse(json.loads((report / "verification.json").read_text())["passed"])
            self.assertEqual(session.close.call_count, 2)

    def test_window_manager_cleanup_failure_invalidates_the_owned_success_report(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            args = SimpleNamespace(inside=True, report=report, token="synthetic")

            def succeeded():
                (report / "verification.json").write_text('{"passed":true,"steps":[]}')

            session_type = Mock(return_value=SimpleNamespace(run=succeeded))
            with patch.object(gui, "validate_session"), \
                    patch.object(gui.os, "geteuid", return_value=1000, create=True), \
                    patch.object(gui, "command", return_value="none"), \
                    patch.object(gui, "wait_for"), \
                    patch.object(gui.subprocess, "Popen"), \
                    patch.object(gui, "stop_process", side_effect=RuntimeError("wm cleanup failed")):
                with self.assertRaisesRegex(RuntimeError, "wm cleanup failed"):
                    gui.main(args, session_type=session_type)
            self.assertFalse(json.loads((report / "verification.json").read_text())["passed"])

    def test_existing_report_refusal_must_not_rewrite_a_previous_success(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            original = '{"passed":true,"previous_run":true}'
            (report / "verification.json").write_text(original)
            with self.assertRaises(FileExistsError):
                gui.main(SimpleNamespace(inside=False, report=report))
            self.assertEqual((report / "verification.json").read_text(), original)


if __name__ == "__main__":
    unittest.main()
