import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import windows_connected_acceptance as connected
import connected_desktop_acceptance as traffic
from build_windows_desktop_probe import CONNECTED_ENTRYPOINT
from windows_tray_acceptance import validate_probe_marker


class ConnectedWindowsTests(unittest.TestCase):
    def setUp(self):
        self.profile = Path("private/profile")
        self.record = {"token": "a" * 32, "pid": 1234, "platform": "windows",
                       "profile": str(self.profile)}
        self.final = {"round": 3, "stage": "shutdown", "fixture_pid": 555,
                      "fixture_exit": 0,
                      "owner": {"phase": "disconnected", "closed": True,
                                "run_id": None, "cleanup_pending": False},
                      "session": {"state": "disconnected", "upload": 262144,
                                  "download": 262172,
                                  "tun": {"state": "stopped", "cleanup_pending": False},
                                  "egress": {"active": False, "pending_cleanup": False,
                                             "sockets": []}}}

    def test_connected_marker_cannot_accept_window_or_formal_entrypoint(self):
        marker = {"schema": 1, "entrypoint": CONNECTED_ENTRYPOINT, "products": {"aot": "a"}}
        validate_probe_marker(marker, marker["products"], CONNECTED_ENTRYPOINT)
        for wrong in ("tool/desktop_window_smoke.dart", "lib/main.dart"):
            with self.subTest(wrong=wrong), self.assertRaises(RuntimeError):
                validate_probe_marker({**marker, "entrypoint": wrong},
                                      marker["products"], CONNECTED_ENTRYPOINT)

    def test_foreign_native_identity_or_profile_is_rejected(self):
        connected.validate_native(self.record, "a" * 32, 1234, self.profile)
        for key, value in (("token", "b" * 32), ("pid", 1235),
                           ("platform", "linux"), ("profile", "host/auth")):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                connected.validate_native({**self.record, key: value}, "a" * 32,
                                          1234, self.profile)

    def test_terminal_barrier_rejects_reusable_owner_or_surviving_run(self):
        connected.validate_shutdown(self.final, 555)
        for key, value in (("closed", False), ("run_id", "1"), ("cleanup_pending", True)):
            bad = copy.deepcopy(self.final)
            bad["owner"][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                connected.validate_shutdown(bad, 555)
        with self.assertRaises(RuntimeError):
            connected.validate_shutdown({**self.final, "fixture_exit": 7}, 555)

    def test_terminal_barrier_rejects_partial_traffic_and_egress_leak(self):
        for key, value in (("upload", 65536), ("download", 262144),
                           ("state", "connected")):
            bad = copy.deepcopy(self.final)
            bad["session"][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                connected.validate_shutdown(bad, 555)
        for key, value in (("active", True), ("pending_cleanup", True), ("sockets", [1])):
            bad = copy.deepcopy(self.final)
            bad["session"]["egress"][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                connected.validate_shutdown(bad, 555)

    def test_ordinary_disconnect_requires_session_cleanup_and_reusable_owner(self):
        record = copy.deepcopy(self.final)
        record["stage"] = "disconnected"
        record["owner"]["closed"] = False
        connected.validate_disconnected(record)
        for key, value in (("active", True), ("pending_cleanup", True), ("sockets", [1])):
            bad = copy.deepcopy(record)
            bad["session"]["egress"][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                connected.validate_disconnected(bad)
        with self.assertRaises(RuntimeError):
            connected.validate_disconnected({**record, "session": None})
        record["owner"]["closed"] = True
        with self.assertRaises(RuntimeError):
            connected.validate_disconnected(record)

    def test_traffic_waits_for_current_round_complete_totals_and_decisions(self):
        decisions = [{"flow": {"inbound": kind, "host": address, "port": 443, "network": "tcp"},
                      "outbound": "SHU-VPN", "decision": {"action": "SHU-VPN"}}
                     for kind in ("http", "socks") for address in ("10.21.0.5", "2001:db8:21::5")]
        good = {"round": 3, "session": {"upload": 262144, "download": 262172,
                                      "decisions": decisions, "tun": {"state": "stopped"}}}
        stale = {**good, "round": 2}
        partial = copy.deepcopy(good)
        partial["session"]["download"] = 7
        wrong_decisions = copy.deepcopy(good)
        wrong_decisions["session"]["decisions"][-1]["outbound"] = "DIRECT"
        observations = iter([stale, partial, wrong_decisions, good])

        def wait(predicate, _):
            self.assertIsNone(predicate())
            self.assertIsNone(predicate())
            self.assertIsNone(predicate())
            return predicate()

        with patch.object(traffic, "echo", return_value={"payload_bytes": 65536}), \
                patch.object(traffic.gui, "wait_for", side_effect=wait):
            result = traffic.traffic_evidence(lambda: next(observations),
                                              {"round": 3, "ports": {"http": 1, "socks": 2}})
        self.assertEqual(result["native_traffic"], good)
        self.assertEqual(len(result["flows"]), 4)


if __name__ == "__main__":
    unittest.main()
