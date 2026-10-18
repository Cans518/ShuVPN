from contextlib import redirect_stderr
import copy
import io
import json
from pathlib import Path
import sys
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import desktop_rule_drag_acceptance as drag
import desktop_gui_acceptance as gui


class RuleDragAcceptanceTest(unittest.TestCase):
    def test_drag_fixture_matches_complete_native_editor_serialization(self):
        # Captured from the real editor after the first physical prepend drag.
        profile = {
            "name": "pointer-drag-fixture",
            "rules": [
                "DOMAIN,drag.profile.gui.invalid,REJECT",
                "DOMAIN,drag.profile.gui.invalid,DIRECT",
                "MATCH,REJECT",
            ],
            "automatic_campus": False,
            "proxy-groups": [],
            "rule-providers": {},
        }
        overrides = {
            "prepend": [
                "DOMAIN,drag.prepend.gui.invalid,DIRECT",
                "DOMAIN,drag.prepend.gui.invalid,REJECT",
                "DOMAIN,before.gui.invalid,DIRECT",
            ],
            "append": [
                "DOMAIN,drag.append.gui.invalid,REJECT",
                "DOMAIN,drag.append.gui.invalid,DIRECT",
                "DOMAIN,after.gui.invalid,DIRECT",
            ],
            "selections": {},
        }
        self.assertEqual(drag.documents({"prepend"}), (profile, overrides))

    def handles(self):
        labels = [drag.drag_label("profile", position, expression)
                  for position, expression in ((2, drag.expressions("profile")[1]),
                                               (1, drag.expressions("profile")[0]))]
        return [
            {"pid": 123, "name": label, "role": "image", "showing": True, "visible": True,
             "actions": 0, "rect": [600, y, 48, 48]}
            for label, y in zip(labels, (600, 400))
        ], labels

    def test_physical_handles_require_exact_owner_names_and_complete_visible_bounds(self):
        handles, labels = self.handles()
        self.assertEqual(
            drag.validate_drag_handles(handles, 123, labels, [120, 50, 1120, 960]),
            [(624, 624), (624, 424)])
        for change in (
            {"pid": 456}, {"pid": True}, {"name": "some card"}, {"role": "push button"},
            {"showing": False}, {"actions": 1}, {"rect": [600, 400, 400, 250]},
            {"rect": [600, 50, 48, 48]}, {"rect": [600, 990, 48, 48]},
            {"rect": [100, 400, 48, 48]}, {"rect": [1220, 400, 48, 48]},
            {"rect": [600, 400, 0, 48]}, {"rect": [600, 400, True, 48]},
            {"rect": [600, 600, 48, 24]}, {"rect": [600, 600, 24, 48]},
        ):
            invalid = copy.deepcopy(handles)
            invalid[0].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                drag.validate_drag_handles(invalid, 123, labels, [120, 50, 1120, 960])

    def test_drag_rejects_reversed_or_coincident_targets_and_bad_window(self):
        handles, labels = self.handles()
        for invalid in ([], handles[:1], list(reversed(handles)), [handles[0], handles[0]]):
            with self.subTest(invalid=invalid), self.assertRaises(RuntimeError):
                drag.validate_drag_handles(invalid, 123, labels, [120, 50, 1120, 960])
        for window in ([120, 50, 0, 960], [120, 50, True, 960], [120, 50, 1120]):
            with self.assertRaises(RuntimeError):
                drag.validate_drag_handles(handles, 123, labels, window)

    def test_showing_without_visible_never_qualifies_a_physical_handle(self):
        handles, labels = self.handles()
        for value in (False, None, 1):
            invalid = copy.deepcopy(handles)
            invalid[0]["visible"] = value
            with self.subTest(visible=value), self.assertRaisesRegex(RuntimeError, "clipped or outside"):
                drag.validate_drag_handles(invalid, 123, labels, [120, 50, 1120, 960])
        invalid = copy.deepcopy(handles)
        del invalid[0]["visible"]
        with self.assertRaisesRegex(RuntimeError, "clipped or outside"):
            drag.validate_drag_handles(invalid, 123, labels, [120, 50, 1120, 960])

    def test_hidden_handle_causes_scroll_and_fresh_geometry_before_return(self):
        handles, _ = self.handles()
        hidden = copy.deepcopy(handles)
        hidden[0]["visible"] = False
        adapter = SimpleNamespace(
            application=SimpleNamespace(pid=123), outer_scroll=Mock(), steps=[],
            window_rectangle=Mock(return_value=[120, 50, 1120, 960]))
        results = [SimpleNamespace(returncode=0, stdout=json.dumps(value), stderr="")
                   for value in (hidden, handles)]
        with patch.object(drag.subprocess, "run", side_effect=results):
            actual, _, points = drag.DragSession.drag_handles(adapter, "profile")
        self.assertEqual(actual, handles)
        self.assertEqual(points, [(624, 624), (624, 424)])
        adapter.outer_scroll.assert_called_once()
        self.assertEqual(abs(adapter.outer_scroll.call_args.args[0]), 2)

    def test_search_reaches_handles_more_than_sixteen_scroll_steps_above_preview(self):
        handles, _ = self.handles()
        hidden = copy.deepcopy(handles)
        hidden[0]["visible"] = False
        position = 40
        def scroll(count):
            nonlocal position
            position = max(0, min(40, position + count))
        adapter = SimpleNamespace(
            application=SimpleNamespace(pid=123), outer_scroll=Mock(side_effect=scroll), steps=[],
            window_rectangle=Mock(return_value=[120, 50, 1120, 960]))
        def geometry(*_args, **_kwargs):
            return SimpleNamespace(returncode=0, stdout=json.dumps(
                handles if position == 0 else hidden), stderr="")
        with patch.object(drag.subprocess, "run", side_effect=geometry):
            actual, _, _ = drag.DragSession.drag_handles(adapter, "profile")
        self.assertEqual(actual, handles)
        self.assertEqual(position, 0)
        self.assertLessEqual(adapter.outer_scroll.call_count, 64)

    def test_drag_uses_press_motion_release_and_no_accessibility_tap(self):
        command = Mock()
        pause = Mock()
        drag.pointer_drag(command, (600, 600), (600, 400), pause)
        calls = [call.args for call in command.call_args_list]
        self.assertEqual(calls[0], ("mousemove", "--sync", 600, 600))
        self.assertEqual(calls[1], ("mousedown", "1"))
        self.assertEqual(calls[-2], ("mousemove", "--sync", 600, 400))
        self.assertEqual(calls[-1], ("mouseup", "1"))
        self.assertEqual(len(calls), 15)
        self.assertEqual(pause.call_count, 12)

    def test_motion_or_press_failure_still_releases_the_button(self):
        for failing in ("mousedown", "mousemove"):
            calls = []
            failure = RuntimeError("physical command failed")

            def command(*args):
                calls.append(args)
                if args[0] == failing and len(calls) > 1:
                    raise failure

            with self.subTest(failing=failing), self.assertRaises(RuntimeError) as caught:
                drag.pointer_drag(command, (600, 600), (600, 400), lambda _: None)
            self.assertIs(caught.exception, failure)
            self.assertEqual(calls[-1], ("mouseup", "1"))

    def test_cleanup_failure_preserves_the_primary_motion_failure(self):
        failure = RuntimeError("motion failed")

        def command(*args):
            if args[0] == "mousemove" and args[-1] != 600:
                raise failure
            if args[0] == "mouseup":
                raise RuntimeError("release failed")

        with redirect_stderr(io.StringIO()), self.assertRaises(RuntimeError) as caught:
            drag.pointer_drag(command, (600, 600), (600, 400), lambda _: None)
        self.assertIs(caught.exception, failure)

    def test_release_failure_alone_prevents_success(self):
        def command(*args):
            if args[0] == "mouseup":
                raise RuntimeError("release failed")
        with redirect_stderr(io.StringIO()), self.assertRaisesRegex(RuntimeError, "release failed"):
            drag.pointer_drag(command, (600, 600), (600, 400), lambda _: None)

    def native(self, moved=(), version=3):
        profile, overrides = map(json.dumps, drag.documents(moved))
        return {
            "routing_version": version, "routing_name": drag.PROFILE_NAME,
            "routing_test_action": "REJECT",
            "routing_profile": profile, "routing_overrides": overrides,
            "routing_override_actions": {"before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"},
            **{key: False for key in drag.NETWORK_SWITCHES},
            "routing_custom_decisions": {
                host: {"version": version, "source": source, "rule_id": f"{source}:0",
                       "action": "DIRECT" if source in moved else "REJECT",
                       "policy": "DIRECT" if source in moved else "REJECT"}
                for source, host in drag.HOSTS.items()},
        }

    def test_native_readback_requires_full_documents_and_all_three_exact_decisions(self):
        for moved, version in (((), 3), (("prepend",), 4),
                               (("prepend", "profile"), 5), (drag.SOURCES, 6)):
            native = self.native(moved, version)
            committed = {"current": {"profile": native["routing_profile"],
                                     "overrides": native["routing_overrides"]}}
            drag.validate_drag_readback(native, version, moved, committed)
            for key, value in (("routing_version", 1), ("routing_name", "other"),
                               ("http_enabled", True), ("system_proxy_enabled", None)):
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    drag.validate_drag_readback({**native, key: value}, version, moved, committed)
            for source, host in drag.HOSTS.items():
                for key, value in (("source", "wrong"), ("rule_id", f"{source}:1"),
                                   ("version", version - 1), ("policy", "SHU-VPN")):
                    invalid = copy.deepcopy(native)
                    invalid["routing_custom_decisions"][host][key] = value
                    with self.subTest(source=source, key=key), self.assertRaises(RuntimeError):
                        drag.validate_drag_readback(invalid, version, moved, committed)

    def test_native_readback_rejects_wrong_order_even_when_decisions_are_spoofed(self):
        native = self.native(drag.SOURCES, 6)
        profile = json.loads(native["routing_profile"])
        profile["rules"][:2] = reversed(profile["rules"][:2])
        native["routing_profile"] = json.dumps(profile)
        committed = {"current": {"profile": native["routing_profile"],
                                 "overrides": native["routing_overrides"]}}
        with self.assertRaisesRegex(RuntimeError, "complete rule order"):
            drag.validate_drag_readback(native, 6, drag.SOURCES, committed)

    def test_native_readback_rejects_changes_to_non_rule_document_fields(self):
        for document, field, value in (
            ("routing_profile", "proxy-groups", [{"name": "unexpected"}]),
            ("routing_profile", "rule-providers", {"unexpected": {}}),
            ("routing_overrides", "selections", {"unexpected": "DIRECT"}),
        ):
            native = self.native(drag.SOURCES, 6)
            changed = json.loads(native[document])
            changed[field] = value
            native[document] = json.dumps(changed)
            committed = {"current": {"profile": native["routing_profile"],
                                     "overrides": native["routing_overrides"]}}
            with self.subTest(field=field), self.assertRaisesRegex(
                    RuntimeError, "complete rule order"):
                drag.validate_drag_readback(native, 6, drag.SOURCES, committed)

    def test_drag_requires_an_actionable_preview_before_testing_its_invalidation(self):
        for enabled, actions, stored in ((True, ["Tap", "Focus"], b"original"),
                                         (False, [], b"original"),
                                         (True, ["Focus"], b"original"),
                                         (True, ["Tap", "Tap"], b"original"),
                                         (True, ["Tap"], b"changed")):
            node = SimpleNamespace(get_n_actions=lambda: len(actions),
                                   get_action_name=lambda index: actions[index])
            adapter = SimpleNamespace(
                application=SimpleNamespace(pid=123), steps=[], press=Mock(),
                named_node=Mock(return_value=node),
                node_observation=Mock(return_value={
                    "name": "应用预览", "role": "push button", "pid": 123,
                    "showing": True, "enabled": enabled}),
                routing_file=Mock(return_value=SimpleNamespace(read_bytes=lambda: stored)))
            def require(predicate, _description):
                result = predicate()
                if not result:
                    raise RuntimeError("preview unavailable")
                return result
            with self.subTest(enabled=enabled, actions=actions, stored=stored), \
                    patch.object(gui, "wait_for", side_effect=require):
                if (enabled, actions, stored) == (True, ["Tap", "Focus"], b"original"):
                    drag.DragSession.prepare_drag(adapter, b"original")
                    self.assertIn("actionable_preview_before_drag", adapter.steps[-1])
                else:
                    with self.assertRaises(RuntimeError):
                        drag.DragSession.prepare_drag(adapter, b"original")


if __name__ == "__main__":
    unittest.main()
