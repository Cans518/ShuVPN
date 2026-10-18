from contextlib import redirect_stderr
import io
import json
from pathlib import Path
from types import SimpleNamespace
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import desktop_gui_acceptance as gui
import desktop_rule_acceptance as rules


class NamedRuleAcceptanceTest(unittest.TestCase):
    def test_named_lookup_rejects_duplicates_and_does_not_use_unnamed_icon_order(self):
        label = "前置规则 第2条：编辑 DOMAIN,created.prepend.gui.invalid,DIRECT"

        def node(name, role="push button"):
            return SimpleNamespace(get_name=lambda: name, get_role_name=lambda: role)

        expected = node(label)
        self.assertIs(rules.unique_named_node([node(""), expected], label, "push button"), expected)
        self.assertIsNone(rules.unique_named_node([node("") for _ in range(4)], label, "push button"))
        self.assertIsNone(rules.unique_named_node([node(label, "panel")], label, "push button"))
        with self.assertRaises(RuntimeError):
            rules.unique_named_node([expected, node(label)], label, "push button")

    def test_named_action_is_bound_to_the_enabled_showing_process_role_and_full_label(self):
        label = "前置规则 第2条：删除 DOMAIN,edited.prepend.gui.invalid,DIRECT"
        valid = {"pid": 123, "name": label, "role": "push button", "enabled": True, "showing": True}
        rules.validate_action_target(valid, 123, label, "push button")
        for change in ({}, {**valid, "pid": 456}, {**valid, "pid": True},
                       {**valid, "name": label.replace("前置", "后置")},
                       {**valid, "name": ""}, {**valid, "role": "panel"},
                       {**valid, "enabled": False}, {**valid, "showing": False}):
            with self.subTest(target=change), self.assertRaises(RuntimeError):
                rules.validate_action_target(change, 123, label, "push button")

    def failed_session(self, report):
        session = rules.RuleSession.__new__(rules.RuleSession)
        session.report = report
        session.application = None
        session.launches = []
        session.steps = []
        session.close = Mock()
        return session

    def test_a_baseline_failure_cannot_leave_a_stale_successful_rule_report(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            (report / "verification.json").write_text('{"passed":true}')
            session = self.failed_session(report)
            with patch.object(gui.Session, "run", side_effect=RuntimeError("baseline failed")):
                with self.assertRaisesRegex(RuntimeError, "baseline failed"):
                    session.run()
            self.assertFalse(json.loads((report / "verification.json").read_text())["passed"])
            session.close.assert_called_once()

    def test_a_dialog_failure_retains_the_baseline_but_reports_overall_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            (report / "verification.json").write_text('{"passed":true}')
            session = self.failed_session(report)
            session.dialogs = Mock(side_effect=RuntimeError("dialog failed"))
            with patch.object(gui.Session, "run"):
                with self.assertRaisesRegex(RuntimeError, "dialog failed"):
                    session.run()
            self.assertTrue(json.loads((report / "offline-baseline.json").read_text())["passed"])
            self.assertFalse(json.loads((report / "verification.json").read_text())["passed"])
            session.close.assert_called_once()

    def test_custom_hosts_reach_only_an_independent_readback_with_the_gui_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            session = SimpleNamespace(
                application=None, args=SimpleNamespace(dart=Path("dart")),
                bundle=report, report=report, launches=[{}, {}, {}],
                state_file=lambda: report / "settings/settings-state.json",
            )
            native = {"routing_custom_decisions": {"edited.prepend.gui.invalid": {"action": "DIRECT"}}}
            response = SimpleNamespace(returncode=0, stdout="GUI_NATIVE_SETTINGS " + json.dumps(native), stderr="")
            with patch.object(gui.subprocess, "run", return_value=response) as command:
                value = gui.Session.probe(session, "read", rule_hosts=("edited.prepend.gui.invalid",))
                self.assertEqual(value, native)
                self.assertEqual(json.loads(command.call_args.args[0][-1]), ["edited.prepend.gui.invalid"])
                command.reset_mock()
                session.application = object()
                with self.assertRaises(RuntimeError):
                    gui.Session.probe(session, "read", rule_hosts=("edited.prepend.gui.invalid",))
                session.application = None
                with self.assertRaises(RuntimeError):
                    gui.Session.probe(session, "prepare-offline-settings", rule_hosts=("edited.prepend.gui.invalid",))
                command.assert_not_called()

    def test_final_cleanup_failure_is_reported_false_before_success_can_be_published(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            (report / "verification.json").write_text('{"passed":true}')
            session = self.failed_session(report)
            session.launches = [{"exit": 0} for _ in range(9)]
            session.product = {}
            session.dialogs = Mock(return_value={})
            session.close = Mock(side_effect=RuntimeError("final cleanup failed"))
            with patch.object(gui.Session, "run"), redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(RuntimeError, "final cleanup failed"):
                    session.run()
            self.assertFalse(json.loads((report / "verification.json").read_text())["passed"])
            self.assertEqual(session.close.call_count, 2)

    def test_secondary_cleanup_error_does_not_replace_the_primary_baseline_error(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            session = self.failed_session(report)
            session.close = Mock(side_effect=RuntimeError("secondary cleanup failed"))
            with patch.object(gui.Session, "run", side_effect=RuntimeError("primary baseline failed")), \
                    redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(RuntimeError, "primary baseline failed"):
                    session.run()
            data = json.loads((report / "verification.json").read_text())
            self.assertFalse(data["passed"])
            self.assertEqual(data["error"], "primary baseline failed")


if __name__ == "__main__":
    unittest.main()
