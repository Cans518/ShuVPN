from pathlib import Path
import struct
import sys
import unittest
import tempfile
import json
import hashlib
from types import SimpleNamespace
from unittest.mock import patch, Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
from windows_private_profile import resource_span, validate_resource_overlay, validate_names
import windows_private_profile as profiles
from windows_formal_gui_acceptance import (
    validate_profile_marker, validate_masked_password, validate_offline_settings,
    validate_rule_rejection, FormalAcceptance,
)


def executable():
    content = bytearray(1024)
    content[:2] = b"MZ"
    struct.pack_into("<I", content, 60, 128)
    content[128:132] = b"PE\0\0"
    struct.pack_into("<HH", content, 132, 0x8664, 1)
    struct.pack_into("<H", content, 148, 240)
    struct.pack_into("<H", content, 152, 0x20B)
    section = 152 + 240
    content[section:section + 8] = b".rsrc\0\0\0"
    struct.pack_into("<II", content, section + 16, 512, 512)
    struct.pack_into("<I", content, section + 36, 0x40000040)
    return bytes(content)


class PrivateProfileTests(unittest.TestCase):
    overrides = '{"prepend":["DOMAIN,before.gui.invalid,DIRECT"],"append":["DOMAIN,after.gui.invalid,DIRECT"]}'

    def offline_snapshot(self, capture_mode):
        return {"theme": "dark", "welcome": True, "http_port": 18443,
                "capture_mode": capture_mode, "routing_version": 2, "routing_name": "仅校园",
                "routing_test_action": "REJECT", "http_enabled": False,
                "socks_enabled": False, "vpn_enabled": False, "system_proxy_enabled": False,
                "routing_profile": "name: 仅校园\nrules:\n  - MATCH,REJECT\n",
                "routing_overrides": self.overrides,
                "routing_override_actions": {"before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"}}

    def test_offline_matrix_requires_exact_persisted_settings_and_rule_version(self):
        snapshot = self.offline_snapshot("full")
        validate_offline_settings(snapshot, "full", 2, self.overrides)
        for key, value in (("capture_mode", "campus"), ("http_port", 3322),
                           ("routing_version", 0), ("routing_test_action", "SHU-VPN"),
                           ("routing_name", "已有严格校园代理")):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                validate_offline_settings({**snapshot, key: value}, "full", 2, self.overrides)

    def test_offline_matrix_rejects_any_enabled_or_unobserved_network_switch(self):
        snapshot = self.offline_snapshot("campus")
        validate_offline_settings(snapshot, "campus", 2, self.overrides)
        for key in ("http_enabled", "socks_enabled", "vpn_enabled", "system_proxy_enabled"):
            for value in (True, None):
                with self.subTest(key=key, value=value), self.assertRaises(RuntimeError):
                    validate_offline_settings({**snapshot, key: value}, "campus", 2, self.overrides)

    def test_offline_matrix_rejects_lost_or_changed_override_documents_and_decisions(self):
        snapshot = self.offline_snapshot("full")
        for key, value in (("routing_overrides", None), ("routing_overrides", "{}"),
                           ("routing_overrides", '{"prepend":["DOMAIN,before.gui.invalid,DIRECT"]}'),
                           ("routing_profile", "name: 仅校园\nrules:\n  - MATCH,DIRECT\n"),
                           ("routing_override_actions", None),
                           ("routing_override_actions", {"before.gui.invalid": "DIRECT"}),
                           ("routing_override_actions", {"before.gui.invalid": "REJECT",
                                                         "after.gui.invalid": "DIRECT"})):
            with self.subTest(key=key, value=value), self.assertRaises(RuntimeError):
                validate_offline_settings({**snapshot, key: value}, "full", 2, self.overrides)

    def test_opening_rule_page_must_preserve_seed_version_and_exact_record(self):
        seed = {"routing_overrides": self.overrides, "routing_version": 1}
        baseline = {"version": 1, "current": {"overrides": self.overrides}}
        stale = json.dumps({**baseline, "version": 2}).encode()
        adapter = SimpleNamespace(routing_record=Mock(return_value=stale),
                                  control_click=Mock(), snapshot=Mock())
        with self.assertRaisesRegex(RuntimeError, "seeded"):
            FormalAcceptance.configure_rules(adapter, seed)
        adapter.control_click.assert_not_called()
        adapter.routing_record = Mock(side_effect=[json.dumps(baseline).encode(), stale])
        with self.assertRaisesRegex(RuntimeError, "Opening"):
            FormalAcceptance.configure_rules(adapter, seed)
        adapter.control_click.assert_called_once_with("规则分流")

    def test_rule_rejection_requires_the_same_nonempty_native_record(self):
        record = b'{"version":2,"current":{"profile":"valid","overrides":"preserved"}}'
        rows = [{"name": "规则类型尚未支持", "type": 50020, "offscreen": False},
                {"name": "应用预览", "type": 50000, "offscreen": False, "enabled": False}]
        validate_rule_rejection(record, record, rows)
        for before, after in ((None, None), (b"", b""), (record, None),
                              (record, record.replace(b'"version":2', b'"version":3')),
                              (record, record.replace(b"preserved", b"lost"))):
            with self.subTest(before=before, after=after), self.assertRaises(RuntimeError):
                validate_rule_rejection(before, after, rows)

    def test_rule_rejection_cannot_pass_without_visible_native_error_and_disabled_apply(self):
        error = {"name": "规则类型尚未支持", "type": 50020, "offscreen": False}
        apply = {"name": "应用预览", "type": 50000, "offscreen": False, "enabled": False}
        for rows in ([apply], [{**error, "name": "设置值无效"}, apply],
                     [{**error, "offscreen": True}, apply],
                     [error], [error, {**apply, "enabled": True}],
                     [error, {**apply, "enabled": None}], [error, apply, apply],
                     [error, {**apply, "type": 50020}]):
            with self.subTest(rows=rows), self.assertRaises(RuntimeError):
                validate_rule_rejection(b"valid record", b"valid record", rows)

    def test_rule_rejection_alternative_requires_actual_click_receipt_and_matching_record_hashes(self):
        record = b"committed native record"
        digest = hashlib.sha256(record).hexdigest()
        rows = [{"name": "规则类型尚未支持", "type": 50020, "offscreen": False},
                {"name": "应用预览", "type": 50000, "offscreen": False, "enabled": True}]
        receipt = {"target": {"name": "应用预览", "pid": 123, "control_type": 50000},
                   "before_sha256": digest, "after_sha256": digest, "observed_seconds": 0.5}
        validate_rule_rejection(record, record, rows, blocked_apply=receipt)
        for changed in ({}, {**receipt, "after_sha256": "changed"},
                        {**receipt, "before_sha256": "changed"},
                        {**receipt, "observed_seconds": 0},
                        {**receipt, "observed_seconds": float("nan")},
                        {**receipt, "observed_seconds": float("inf")},
                        {**receipt, "target": {"name": "保存", "pid": 123, "control_type": 50000}},
                        {**receipt, "target": {"name": "应用预览", "pid": 0, "control_type": 50000}}):
            with self.subTest(receipt=changed), self.assertRaises(RuntimeError):
                validate_rule_rejection(record, record, rows, blocked_apply=changed)
        with self.assertRaises(RuntimeError):
            validate_rule_rejection(record, record + b"changed", rows, blocked_apply=receipt)

    def test_run_exception_clears_a_previously_set_passed_state(self):
        adapter = SimpleNamespace(
            result={"passed": True}, start_launch=Mock(side_effect=RuntimeError("test failure")),
            api=SimpleNamespace(), window=None, probe=None, application=None,
            launches=[], marker={}, finish=Mock(), write=Mock())
        with patch("windows_formal_gui_acceptance.cleanup_profile"):
            with self.assertRaisesRegex(RuntimeError, "test failure"):
                FormalAcceptance.run(adapter)
        self.assertFalse(adapter.result["passed"])

    def test_password_acceptance_requires_masked_native_value_not_a_missing_flag(self):
        self.assertEqual(validate_masked_password("•" * 16, 16), {"masked": True, "length": 16})
        for value in ("", "gui-fixture-only", "•••"):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                validate_masked_password(value, 16)

    def test_formal_marker_rejects_developer_aot_or_changed_dll(self):
        products = {"shuvpn.exe": "patched", "data/app.so": "aot", "core.dll": "core"}
        marker = {"schema": 1, "entrypoint": "lib/main.dart",
                  "kind": "production-aot-private-resource-copy", "products": products,
                  "production_aot_unchanged": True, "exe_code_unchanged": True,
                  "company": "abcdef", "product": "123456",
                  "archive_manifest": {"files": {
                      name: {"sha256": "original" if name == "shuvpn.exe" else value}
                      for name, value in products.items()}},
                  "overlay": {"original_sha256": "original", "patched_sha256": "patched",
                              "unchanged_outside_ranges": True}}
        validate_profile_marker(marker, products)
        for name in ("data/app.so", "core.dll"):
            changed = {**products, name: "changed"}
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                validate_profile_marker({**marker, "products": changed}, changed)
        with self.assertRaises(RuntimeError):
            validate_profile_marker({**marker, "entrypoint": "tool/connected_desktop_app.dart"}, products)

    def test_partial_claim_failure_rolls_back_only_its_new_directories(self):
        with tempfile.TemporaryDirectory() as temporary:
            first, second = Path(temporary) / "first", Path(temporary) / "already-exists"
            second.mkdir()
            (second / "keep").write_text("other owner")
            with self.assertRaises(FileExistsError):
                profiles.claim_profile([first, second], {"token": "test"})
            self.assertFalse(first.exists())
            self.assertEqual((second / "keep").read_text(), "other owner")

    def test_partial_cleanup_can_retry_after_first_root_was_removed(self):
        with tempfile.TemporaryDirectory() as temporary:
            folders = [Path(temporary) / name for name in ("roaming", "local")]
            for folder in folders:
                folder.mkdir()
            roots = [folder / "abcdef" for folder in folders]
            owner = {"token": "test", "company": "abcdef", "product": "123456"}
            marker = {**owner, "ownership": owner, "profile_roots": [str(root) for root in roots]}
            profiles.claim_profile(roots, owner)
            (roots[1] / "child").mkdir()
            with patch.object(profiles, "folder_roots", return_value=folders), \
                    patch.object(profiles.shutil, "rmtree", side_effect=PermissionError("busy")):
                with self.assertRaises(PermissionError):
                    profiles.cleanup_profile(marker)
            self.assertFalse(roots[0].exists())
            self.assertTrue((roots[1] / ".shuvpn-acceptance-owner.json").exists())
            with patch.object(profiles, "folder_roots", return_value=folders):
                profiles.cleanup_profile(marker)
            self.assertFalse(roots[1].exists())

    def test_profile_components_require_two_equal_length_safe_hex_names(self):
        validate_names("abcdef", "123456")
        for company, product in (("ShuVPN", "123456"), ("..", "123456"),
                                 ("abcdef", "../abc"), ("abcd", "123456")):
            with self.subTest(company=company, product=product), self.assertRaises(ValueError):
                validate_names(company, product)

    def test_resource_reader_rejects_executable_resource_section(self):
        content = bytearray(executable())
        self.assertEqual(resource_span(content), (512, 1024))
        struct.pack_into("<I", content, 152 + 240 + 36, 0x60000040)
        with self.assertRaises(ValueError):
            resource_span(content)

    def test_resource_reader_rejects_a_signed_executable(self):
        content = bytearray(executable())
        struct.pack_into("<II", content, 152 + 112 + 8 * 4, 900, 124)
        with self.assertRaises(ValueError):
            resource_span(content)

    def test_overlay_allows_only_declared_resource_strings(self):
        original = executable()
        changed = bytearray(original)
        changed[600:606] = b"abcdef"
        validate_resource_overlay(original, changed, [(600, 606)])
        changed[610] = 1
        with self.assertRaises(ValueError):
            validate_resource_overlay(original, changed, [(600, 606)])

    def test_overlay_rejects_code_changes_truncation_or_ranges_outside_resources(self):
        original = executable()
        for changed, ranges in ((original[:-1], [(600, 606)]),
                                (original, [(200, 206)])):
            with self.subTest(ranges=ranges), self.assertRaises(ValueError):
                validate_resource_overlay(original, changed, ranges)
        code_changed = bytearray(original)
        code_changed[200] = 1
        with self.assertRaises(ValueError):
            validate_resource_overlay(original, code_changed, [(600, 606)])


if __name__ == "__main__":
    unittest.main()
