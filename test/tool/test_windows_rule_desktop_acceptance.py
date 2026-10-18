import copy
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
import rule_gui_evidence as evidence
from windows_formal_gui_acceptance import FormalAcceptance, validate_launches
from windows_rule_acceptance import RuleAcceptance, validate_rule_control


class WindowsRuleAcceptanceTest(unittest.TestCase):
    def test_expression_read_waits_for_the_owned_modal_field_before_reading_its_value(self):
        ready = False
        value = "initial"
        def control(_label):
            nonlocal ready
            ready = True
        def edit_value(_label):
            if not ready:
                raise AssertionError("The modal field has not appeared yet")
            return value
        def type_text(text):
            nonlocal value
            value = text
        adapter = SimpleNamespace(
            control=Mock(side_effect=control), edit_value=edit_value, rule_click=Mock(),
            api=SimpleNamespace(keys=Mock(), type_text=type_text), steps=[])
        with patch("windows_rule_acceptance.wait_for", side_effect=lambda predicate, *_args: predicate()):
            RuleAcceptance.enter_expression(adapter, "initial", "changed")
        self.assertEqual(value, "changed")
        adapter.control.assert_called_once()

    def test_final_control_validation_precedes_the_physical_click(self):
        candidate = {"name": "保存", "pid": 123, "rect": [10, 100, 90, 180],
                     "clickable": [50, 140]}
        current = {**candidate, "enabled": False}
        adapter = SimpleNamespace(
            find_control=Mock(side_effect=[candidate, current]),
            api=SimpleNamespace(move=Mock()), write=Mock(),
            click=Mock(), probe=SimpleNamespace(pid=123), steps=[],
            validate_click_target=Mock(side_effect=RuntimeError("final target rejected")))
        with patch("windows_formal_gui_acceptance.time.monotonic", side_effect=[0, 0, 1]), \
                patch("windows_formal_gui_acceptance.wait_for",
                      side_effect=lambda predicate, *_args: predicate()):
            with self.assertRaisesRegex(RuntimeError, "final target rejected"):
                FormalAcceptance.control_click(adapter, "保存")
        adapter.click.assert_not_called()
        self.assertIs(adapter.validate_click_target.call_args.args[1], current)

    def test_report_write_failure_preserves_the_original_run_failure(self):
        adapter = SimpleNamespace(
            result={"passed": False}, start_launch=Mock(side_effect=RuntimeError("original failure")),
            api=SimpleNamespace(), window=None, probe=None, application=None, launches=[], marker={},
            finish=Mock(), write=Mock(side_effect=OSError("report write failed")))
        with patch("windows_formal_gui_acceptance.cleanup_profile"), \
                redirect_stderr(io.StringIO()) as messages:
            with self.assertRaisesRegex(RuntimeError, "original failure"):
                FormalAcceptance.run(adapter)
        self.assertFalse(adapter.result["passed"])
        self.assertIn("report write failed", adapter.result["report_write_error"])
        self.assertIn("report write failed", messages.getvalue())

    def test_report_write_failure_revokes_success_and_preserves_an_earlier_cleanup_failure(self):
        for cleanup in (None, RuntimeError("cleanup failed first")):
            adapter = SimpleNamespace(
                result={"passed": True}, api=SimpleNamespace(), window=None, probe=None,
                application=None, launches=[], marker={},
                finish=Mock(side_effect=cleanup),
                write=Mock(side_effect=OSError("report write failed")))
            expected = RuntimeError if cleanup is not None else OSError
            with self.subTest(cleanup=cleanup), \
                    patch("windows_formal_gui_acceptance.cleanup_profile"), \
                    redirect_stderr(io.StringIO()):
                with self.assertRaises(expected) as caught:
                    FormalAcceptance.finish_formal(adapter, None)
            self.assertFalse(adapter.result["passed"])
            if cleanup is not None:
                self.assertIs(caught.exception, cleanup)
            else:
                self.assertIn("report write failed", str(caught.exception))

    def test_system_security_prompt_blocks_before_starting_or_typing_into_an_application(self):
        adapter = SimpleNamespace(
            result={}, api=SimpleNamespace(
                user=SimpleNamespace(GetForegroundWindow=Mock(return_value=123),
                                     SetForegroundWindow=Mock()),
                window_class=Mock(return_value="Shell_SystemDialog"),
                window_title=Mock(return_value="Windows 安全中心")))
        with patch.object(FormalAcceptance, "start_launch") as start:
            with self.assertRaisesRegex(RuntimeError, "user decision"):
                RuleAcceptance.start_launch(adapter, "blocked")
            start.assert_not_called()
        self.assertEqual(adapter.result["blocked_system_prompt"]["hwnd"], 123)
        for activate in (RuleAcceptance.activate_formal, FormalAcceptance.activate_formal):
            with self.subTest(activate=activate), self.assertRaisesRegex(RuntimeError, "user decision"):
                activate(adapter)
        adapter.api.user.SetForegroundWindow.assert_not_called()

    def test_foreign_foreground_snapshot_records_only_identity_and_keeps_first_observation(self):
        adapter = SimpleNamespace(
            result={}, window=456, api=SimpleNamespace(
                user=SimpleNamespace(GetForegroundWindow=Mock(return_value=123)),
                window_pid=Mock(return_value=321),
                window_class=Mock(return_value="Chrome_WidgetWin_1"),
                window_title=Mock(side_effect=AssertionError("Do not read foreign document titles"))))
        for name in ("initial", "failure"):
            with self.assertRaisesRegex(RuntimeError, "foreground owned"):
                FormalAcceptance.snapshot(adapter, name)
        self.assertEqual(adapter.result["outside_foreground"], {
            "stage": "initial", "hwnd": 123, "pid": 321, "class": "Chrome_WidgetWin_1",
            "security_prompt": False})
        adapter.api.window_title.assert_not_called()

    def test_launch_counts_require_distinct_original_processes_and_normal_exits(self):
        launches = [{"process": {"created_ticks": str(index + 1)}, "exit": 0} for index in range(9)]
        validate_launches(launches, 9)
        validate_launches(launches[:3], 3)
        for changed in (launches[:3], launches + launches[:1],
                        [{**launches[0], "exit": 1}, *launches[1:]],
                        [launches[0], *launches[:-1]]):
            with self.subTest(launches=changed), self.assertRaises(RuntimeError):
                validate_launches(changed, 9)

    def test_named_rule_actions_reject_another_owner_role_name_or_unusable_geometry(self):
        label = "前置规则 第2条：删除 DOMAIN,edited.prepend.gui.invalid,DIRECT"
        target = {"name": label, "pid": 123, "control_type": 50000, "rect": [10, 100, 90, 180],
                  "clickable": [50, 140], "enabled": True, "offscreen": False}
        validate_rule_control(target, 123, label, [0, 0, 500, 500])
        for change in ({}, {**target, "pid": True}, {**target, "pid": 456},
                       {**target, "name": label.replace("前置", "后置")},
                       {**target, "control_type": 50020}, {**target, "enabled": False},
                       {**target, "offscreen": True}, {**target, "rect": [10, 100, 10, 180]},
                       {**target, "clickable": [50, 600]}, {**target, "clickable": [400, 140]}):
            with self.subTest(target=change), self.assertRaises(RuntimeError):
                validate_rule_control(change, 123, label, [0, 0, 500, 500])

    def test_rule_native_readback_requires_each_source_version_and_all_network_switches_off(self):
        original, edited = "created.prepend.gui.invalid", "edited.prepend.gui.invalid"
        native = {
            "routing_version": 3, "routing_test_action": "REJECT",
            "routing_override_actions": {"before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"},
            **{key: False for key in evidence.NETWORK_SWITCHES},
            "routing_custom_decisions": {
                original: {"action": "REJECT", "source": "profile", "version": 3},
                edited: {"action": "DIRECT", "source": "prepend", "version": 3}}}
        evidence.validate_rule_readback(native, 3, original, edited, "DIRECT", "prepend")
        for source, version in (("prepend", 3), ("profile", 5), ("append", 7)):
            for action in ("DIRECT", "REJECT"):
                example = copy.deepcopy(native)
                example["routing_version"] = version
                example["routing_custom_decisions"][original]["version"] = version
                example["routing_custom_decisions"][edited].update(
                    version=version, action=action, source=source if action == "DIRECT" else "profile")
                evidence.validate_rule_readback(example, version, original, edited, action, source)
        for key, value in (("routing_version", 4), ("routing_test_action", "DIRECT"),
                           ("routing_override_actions", {}), ("http_enabled", None),
                           ("socks_enabled", True), ("vpn_enabled", True),
                           ("system_proxy_enabled", True)):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                evidence.validate_rule_readback({**native, key: value}, 3, original, edited, "DIRECT", "prepend")
        for host, patch_value in ((original, {"version": 2}), (original, {"source": "append"}),
                                  (edited, {"source": "append"}), (edited, {"action": "REJECT"}),
                                  (edited, {"version": 2})):
            invalid = copy.deepcopy(native)
            invalid["routing_custom_decisions"][host].update(patch_value)
            with self.subTest(host=host, patch=patch_value), self.assertRaises(RuntimeError):
                evidence.validate_rule_readback(invalid, 3, original, edited, "DIRECT", "prepend")

    def test_fixture_rule_lists_keep_the_source_and_omit_only_the_final_match(self):
        raw = ('{"rules":["DOMAIN,a.gui.invalid,DIRECT","MATCH,REJECT"]}',
               '{"prepend":["DOMAIN,b.gui.invalid,DIRECT"],"append":["DOMAIN,c.gui.invalid,DIRECT"]}')
        self.assertEqual(evidence.fixture_rules("profile", raw), ["DOMAIN,a.gui.invalid,DIRECT"])
        self.assertEqual(evidence.fixture_rules("prepend", raw), ["DOMAIN,b.gui.invalid,DIRECT"])
        self.assertEqual(evidence.fixture_rules("append", raw), ["DOMAIN,c.gui.invalid,DIRECT"])
        with self.assertRaises(RuntimeError):
            evidence.fixture_rules("profile", ('{"rules":[{}]}', raw[1]))

    def test_windows_custom_readback_requires_the_gui_closed_and_cannot_prepare_hosts(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory)
            session = SimpleNamespace(application=None, dart=Path("dart"), bundle=report,
                                      profile=report, report=report, launches=[])
            response = SimpleNamespace(returncode=0, stdout='GUI_NATIVE_SETTINGS {"ok":true}', stderr="")
            with patch("windows_formal_gui_acceptance.subprocess.run", return_value=response) as command:
                value = FormalAcceptance.native_probe(session, "read", rule_hosts=("edited.prepend.gui.invalid",))
                self.assertTrue(value["ok"])
                self.assertEqual(json.loads(command.call_args.args[0][-1]), ["edited.prepend.gui.invalid"])
                command.reset_mock()
                session.application = object()
                with self.assertRaises(RuntimeError):
                    FormalAcceptance.native_probe(session, "read", rule_hosts=("edited.prepend.gui.invalid",))
                session.application = None
                with self.assertRaises(RuntimeError):
                    FormalAcceptance.native_probe(session, "prepare-offline-settings",
                                                  rule_hosts=("edited.prepend.gui.invalid",))
                command.assert_not_called()
                for invalid in (
                    SimpleNamespace(returncode=1, stdout="", stderr="probe failure"),
                    SimpleNamespace(returncode=0, stdout="unrelated output", stderr=""),
                    SimpleNamespace(returncode=0, stdout=response.stdout + "\n" + response.stdout, stderr=""),
                ):
                    command.return_value = invalid
                    with self.subTest(response=invalid), self.assertRaises(RuntimeError):
                        FormalAcceptance.native_probe(session, "read", rule_hosts=("edited.prepend.gui.invalid",))

    def test_dialog_extension_requires_the_exact_baseline_before_starting_six_more_windows(self):
        adapter = SimpleNamespace(
            result={}, launches=[{"exit": 0} for _ in range(3)],
            dialogs=Mock(side_effect=[{"routing_version": 4}, {"routing_version": 6}, {"routing_version": 8}]))
        final = RuleAcceptance.additional_settings(adapter, {"routing_version": 2})
        self.assertEqual(final["routing_version"], 8)
        self.assertEqual(adapter.dialogs.call_args_list, [
            unittest.mock.call("prepend", 2), unittest.mock.call("profile", 4), unittest.mock.call("append", 6)])
        adapter.dialogs.reset_mock()
        with self.assertRaises(RuntimeError):
            RuleAcceptance.additional_settings(adapter, {"routing_version": 1})
        adapter.dialogs.assert_not_called()

    def test_failed_dialog_extension_preserves_its_valid_offline_baseline(self):
        adapter = SimpleNamespace(
            result={"passed": False}, launches=[{"exit": 0} for _ in range(3)],
            dialogs=Mock(side_effect=RuntimeError("dialog failed")))
        with self.assertRaisesRegex(RuntimeError, "dialog failed"):
            RuleAcceptance.additional_settings(adapter, {"routing_version": 2})
        self.assertFalse(adapter.result["passed"])
        self.assertEqual(adapter.result["offline_baseline"]["native"]["routing_version"], 2)

    def test_run_preserves_the_original_failure_when_both_cleanup_stages_fail(self):
        adapter = SimpleNamespace(
            result={"passed": True}, start_launch=Mock(side_effect=RuntimeError("original failure")),
            api=SimpleNamespace(), window=None, probe=None, application=None, launches=[], marker={},
            finish=Mock(side_effect=RuntimeError("secondary process cleanup")), write=Mock())
        with patch("windows_formal_gui_acceptance.cleanup_profile",
                   side_effect=RuntimeError("secondary profile cleanup")):
            with self.assertRaisesRegex(RuntimeError, "original failure"):
                FormalAcceptance.run(adapter)
        self.assertFalse(adapter.result["passed"])
        self.assertIn("secondary process cleanup", adapter.result["cleanup_error"])
        self.assertIn("secondary profile cleanup", adapter.result["private_profile_cleanup_error"])
        adapter.write.assert_called_with("verification.json", adapter.result)

    def test_final_cleanup_failure_revokes_success_and_writes_the_final_report(self):
        adapter = SimpleNamespace(
            result={"passed": True}, api=SimpleNamespace(), window=None, probe=None, application=None,
            launches=[], marker={}, finish=Mock(side_effect=RuntimeError("final cleanup failed")), write=Mock())
        with patch("windows_formal_gui_acceptance.cleanup_profile"):
            with self.assertRaisesRegex(RuntimeError, "final cleanup failed"):
                FormalAcceptance.finish_formal(adapter, None)
        self.assertFalse(adapter.result["passed"])
        adapter.write.assert_called_with("verification.json", adapter.result)


if __name__ == "__main__":
    unittest.main()
