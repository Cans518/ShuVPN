import copy
from contextlib import nullcontext
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import connection_soak_acceptance as soak
from process_resources import ProcessResources


class SoakEvidenceTest(unittest.TestCase):
    def state(self):
        return {"round": 1, "ports": {"http": 1234, "socks": 1235},
                "owner": {"phase": "connected", "run_id": "5", "closed": False,
                          "cleanup_pending": False, "error_code": None, "error_message": None},
                "session": {"state": "connected", "tun": {"state": "stopped", "cleanup_pending": False},
                            "upload": 65536, "download": 65543, "connections": 1,
                            "egress": {"active": True, "pending_cleanup": False, "sockets": []},
                            "decisions": [{"flow": {"host": "10.21.0.5", "port": 443,
                                                   "network": "tcp", "inbound": "http"},
                                           "outbound": "SHU-VPN", "decision": {"action": "SHU-VPN"}}]}}

    def test_native_traffic_requires_actual_bytes_connection_and_matching_outbound(self):
        valid = self.state()
        flows = [{"protocol": "http", "address": "10.21.0.5", "payload_bytes": 65536}]
        soak.validate_traffic(valid, flows)
        for patch in ({"upload": 0}, {"download": 65536}, {"connections": 2},
                      {"decisions": []}, {"decisions": [
                          {**valid["session"]["decisions"][0], "outbound": "DIRECT"}]}):
            with self.subTest(patch=patch), self.assertRaises(RuntimeError):
                soak.validate_traffic({**valid, "session": {**valid["session"], **patch}}, flows)

    def test_disconnect_requires_reusable_owner_and_complete_native_resource_barrier(self):
        value = self.state()
        value["command"] = "disconnect"
        value["owner"].update(phase="disconnected", run_id=None)
        value["session"].update(state="disconnected", egress={
            "active": False, "pending_cleanup": False, "sockets": []})
        soak.validate_native(value, connected=False)
        for key, patch in (("owner", {"closed": True}), ("owner", {"cleanup_pending": True}),
                           ("owner", {"run_id": "5"}), ("session", {"state": "connected"}),
                           ("session", {"egress": {"active": False, "pending_cleanup": True, "sockets": []}}),
                           ("session", {"tun": {"state": "running", "cleanup_pending": False}})):
            changed = copy.deepcopy(value)
            changed[key].update(patch)
            with self.subTest(patch=patch), self.assertRaises(RuntimeError):
                soak.validate_native(changed, connected=False)
        with self.assertRaises(RuntimeError):
            soak.validate_native({**value, "session": None}, connected=False)

    def test_growth_never_treats_missing_negative_or_unbounded_samples_as_success(self):
        baseline = {"rss_bytes": 1024, "fds": 10, "threads": 4}
        self.assertEqual(soak.growth(baseline, baseline), {name: 0 for name in baseline})
        self.assertEqual(soak.growth(baseline, {**baseline, "fds": 9})["fds"], -1)
        for current in ({}, {"rss_bytes": 1}, {**baseline, "fds": 0},
                        {**baseline, "fds": "10"}, {**baseline, "fds": True},
                        {**baseline, "rss_bytes": float("nan")},
                        {**baseline, "rss_bytes": float("inf")},
                        {**baseline, "fds": 43},
                        {**baseline, "threads": 13},
                        {**baseline, "rss_bytes": 1024 + 32 * 1024 * 1024 + 1}):
            with self.subTest(current=current), self.assertRaises(RuntimeError):
                soak.growth(baseline, current)

    def test_fixture_plan_rejects_external_nodes_and_extra_authorizations(self):
        plan = {"sid": "test-sid", "acceptAnyCertificate": False, "certificateDigests": ["pinned"],
                "heartbeatSeconds": 0.05,
                "nodes": {"group": ["127.0.0.1:12345"]}, "dnsServers": ["10.99.0.53"],
                "routes": [{"host": host, "protocol": protocol}
                           for host in ("10.21.0.0/16", "2001:db8:21::/64")
                           for protocol in ("tcp", "udp")]}
        safe = soak.fixture_plan(plan)
        self.assertEqual(safe["dnsServers"], [])
        self.assertEqual(plan["dnsServers"], ["10.99.0.53"])
        for changed in ({**plan, "nodes": {"group": ["192.0.2.1:12345"]}},
                        {**plan, "acceptAnyCertificate": True},
                        {**plan, "certificateDigests": []},
                        {**plan, "heartbeatSeconds": 5.0},
                        {**plan, "routes": plan["routes"] + [{"host": "0.0.0.0/0", "protocol": "tcp"}]},
                        {**plan, "routes": plan["routes"][:-1]}):
            with self.subTest(plan=changed), self.assertRaises(RuntimeError):
                soak.fixture_plan(changed)

    def test_heartbeat_profiles_are_explicit_and_never_mutate_the_original_fixture_plan(self):
        plan = {"sid": "test-sid", "acceptAnyCertificate": False, "certificateDigests": ["pinned"],
                "heartbeatSeconds": 0.05, "nodes": {"group": ["127.0.0.1:12345"]},
                "routes": [{"host": host, "protocol": protocol}
                           for host in ("10.21.0.0/16", "2001:db8:21::/64")
                           for protocol in ("tcp", "udp")]}
        self.assertEqual(soak.fixture_plan(plan)["heartbeatSeconds"], 0.05)
        self.assertEqual(soak.fixture_plan(plan, heartbeat_seconds=5.0)["heartbeatSeconds"], 5.0)
        self.assertEqual(plan["heartbeatSeconds"], 0.05)
        for value in (True, 0, -1, 0.1, "5", float("nan"), float("inf")):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                soak.fixture_plan(plan, heartbeat_seconds=value)

    def test_trace_readback_requires_bounded_ordered_events_and_valid_counters(self):
        trace = {"connection": 1, "peer": "127.0.0.1:12345", "elapsed_seconds": 0.2,
                 "requests": 1, "responses": 1, "pending_request": False,
                 "max_response_gap_seconds": 0.1, "max_write_seconds": 0.001,
                 "last_response_age_seconds": 0.1,
                 "last_events": [
                     {"event": "request_received", "elapsed_seconds": 0.1, "requests": 1, "responses": 0},
                     {"event": "response_flushed", "elapsed_seconds": 0.101, "requests": 1, "responses": 1}]}
        prefix = "FIXTURE_HEARTBEAT_TRACE "
        self.assertEqual(soak.heartbeat_traces("unrelated diagnostic\n" + prefix + json.dumps(trace)), [trace])
        for patch in ({"requests": True}, {"responses": 2}, {"connection": True},
                      {"max_response_gap_seconds": float("nan")},
                      {"last_response_age_seconds": -1}, {"peer": "192.0.2.1:12345"},
                      {"last_events": trace["last_events"] * 33},
                      {"last_events": list(reversed(trace["last_events"]))},
                      {"last_events": []},
                      {"last_events": trace["last_events"][:1]},
                      {"last_events": [{**trace["last_events"][0], "event": "unknown"}]}):
            with self.subTest(patch=patch), self.assertRaises(RuntimeError):
                soak.heartbeat_traces(prefix + json.dumps({**trace, **patch}))
        with self.assertRaises(RuntimeError):
            soak.heartbeat_traces(prefix + "{bad")
        with self.assertRaises(RuntimeError):
            soak.heartbeat_traces("\n".join([prefix + json.dumps(trace)] * 2))
        self.assertEqual(soak.heartbeat_traces(""), [])

    def test_post_failure_observation_errors_preserve_the_primary_error_and_report(self):
        for fault in ("invalid_stderr", "deleted_product"):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as directory:
                workspace = Path(directory)
                products = {}
                for name in ("probe", "fixture", "library"):
                    path = workspace / name
                    path.write_bytes(b"controlled product")
                    products[name] = path
                args = SimpleNamespace(**products, report=workspace / "build/desktop/failure",
                                       hold_seconds=1, cycles=1, warmup=1, trace_heartbeats=True)

                def start(command, report, name, *, env=None):
                    (report / "fixture-stderr.log").write_bytes(b"\xff" if fault == "invalid_stderr" else b"")
                    if fault == "deleted_product":
                        products["fixture"].unlink()
                    return SimpleNamespace(
                        receive=Mock(side_effect=RuntimeError("primary fixture failure")),
                        close=Mock(return_value={"pid": 123, "exit": 0, "forced": False}))

                with patch.object(soak, "ROOT", workspace), patch.object(soak, "JsonProcess", side_effect=start):
                    with self.assertRaisesRegex(RuntimeError, "primary fixture failure"):
                        soak.run(args)
                result = json.loads((args.report / "verification.json").read_text())
                self.assertFalse(result["passed"])
                self.assertEqual(result["error"], "RuntimeError: primary fixture failure")
                self.assertTrue(result["trace_capture_error"] if fault == "invalid_stderr" else
                                result["cleanup_errors"])

    def test_response_identity_sequence_and_command_are_bound_to_the_original_child(self):
        valid = {"token": "owned", "pid": 123, "id": 1, "command": "snapshot"}
        for changed in (valid, {**valid, "token": "other"}, {**valid, "pid": 456},
                        {**valid, "id": 2}, {**valid, "command": "connect"}):
            child = SimpleNamespace(process=SimpleNamespace(pid=123, stdin=io.StringIO()),
                                    receive=lambda: changed)
            probe = soak.Probe(child, "owned")
            if changed == valid:
                self.assertEqual(probe.call("snapshot"), valid)
            else:
                with self.subTest(response=changed), self.assertRaises(RuntimeError):
                    probe.call("snapshot")

    def test_exited_process_is_not_an_empty_listener_table(self):
        observer = SimpleNamespace(alive=lambda: False)
        with self.assertRaises(RuntimeError):
            soak.listener_barrier(observer, {"http": 1234, "socks": 1235})

    def test_premature_eof_preserves_the_flow_address_progress_and_primary_error(self):
        stream = SimpleNamespace(sendall=lambda value: None, recv=lambda count: b"")
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(soak, "proxy_stream", return_value=nullcontext(stream)):
            with self.assertRaisesRegex(RuntimeError, "Held http 10.21.0.5 after 0 verified batches"):
                soak.held_flow(1234, "http", "10.21.0.5", 600, threading.Event(), Path(directory))
            evidence = json.loads((Path(directory) / "held-http-v4.json").read_text())
            self.assertEqual(evidence["address"], "10.21.0.5")
            self.assertEqual(evidence["completed_batches"], 0)
            self.assertIn("Proxy closed before the expected protocol bytes", evidence["error"])
            self.assertFalse(evidence["duration_complete"])
            self.assertFalse(evidence["half_close_eof"])

    @unittest.skipUnless(sys.platform in ("win32", "linux"), "Windows/Linux process counters")
    def test_os_counters_bind_a_live_child_and_reject_samples_after_original_exit(self):
        child = subprocess.Popen(
            [sys.executable, "-c", "print('ready', flush=True); input()"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        observer = None
        try:
            self.assertEqual(child.stdout.readline().strip(), "ready")
            with self.assertRaises(RuntimeError):
                ProcessResources(child.pid, Path(__file__))
            observer = ProcessResources(child.pid, Path(sys.executable))
            sample = observer.sample()
            self.assertGreater(sample["rss_bytes"], 0)
            self.assertTrue("handles" in sample or "fds" in sample)
            self.assertGreater(sample["threads"], 0)
            self.assertEqual(set(sample), {"rss_bytes", "threads", "private_bytes", "handles"}
                             if sys.platform == "win32" else {"rss_bytes", "threads", "vms_bytes", "fds"})
            with self.assertRaises(RuntimeError):
                observer.require_library(Path(__file__))
            child.stdin.write("done\n")
            child.stdin.flush()
            self.assertEqual(child.wait(timeout=5), 0)
            self.assertFalse(observer.alive())
            with self.assertRaises(RuntimeError):
                observer.sample()
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=5)
            if observer is not None:
                observer.close()
            child.stdin.close()
            child.stdout.close()


if __name__ == "__main__":
    unittest.main()
