import contextlib
import copy
import importlib.util
from pathlib import Path
import socket
import sys
import unittest
from unittest.mock import Mock, patch

TOOL = Path(__file__).resolve().parents[2] / "tool"
sys.path.insert(0, str(TOOL))
SPEC = importlib.util.spec_from_file_location(
    "connected_gui", TOOL / "connected_desktop_acceptance.py")
acceptance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(acceptance)


class Fragments:
    def __init__(self, chunks):
        self.chunks = list(chunks)

    def recv(self, count):
        if not self.chunks:
            return b""
        chunk = self.chunks.pop(0)
        if len(chunk) > count:
            self.chunks.insert(0, chunk[count:])
        return chunk[:count]

    def settimeout(self, _):
        pass

    def sendall(self, _):
        pass


class ConnectedVerifierTests(unittest.TestCase):
    def records(self, direct_port=41237):
        values = []
        for host, port, identifier, action, outbound, error in (
            ("10.21.0.5", 443, "profile:3", "SHU-VPN", "SHU-VPN", None),
            ("127.0.0.1", direct_port, "profile:2", "DIRECT", "DIRECT", None),
            ("127.0.0.1", 19444, "profile:0", "REJECT", None, "rejected"),
            ("127.0.0.1", 19445, "profile:1", "SHU-VPN", None, "unrouted"),
        ):
            values.append({
                "flow": {"host": host, "port": port, "network": "tcp", "inbound": "http"},
                "decision": {"version": 1, "source": "profile", "rule_id": identifier,
                             "policy": action, "action": action, "reason": "native fixture match"},
                "outbound": outbound, "error": error,
                "message": "native fixture denial" if error else None})
        return values

    def test_connection_details_require_each_real_outcome_and_its_rule_identity(self):
        values = self.records()
        cases = acceptance.detail_records(values, 41237)
        self.assertEqual(set(cases), {"campus", "direct", "reject", "unauthorized"})
        for index, change in (
            (0, {"outbound": "DIRECT"}), (2, {"outbound": "REJECT"}),
            (3, {"error": "rejected"}), (3, {"message": ""}),
        ):
            wrong = copy.deepcopy(values)
            wrong[index].update(change)
            with self.subTest(index=index, change=change), self.assertRaises(RuntimeError):
                acceptance.detail_records(wrong, 41237)
        for key, value in (("version", 2), ("rule_id", "profile:0"), ("source", "campus"),
                           ("policy", "REJECT"), ("reason", "")):
            wrong = copy.deepcopy(values)
            wrong[3]["decision"][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                acceptance.detail_records(wrong, 41237)
        for wrong in (values[:-1], [*values, values[0]]):
            with self.assertRaises(RuntimeError):
                acceptance.detail_records(wrong, 41237)

    def test_detail_fields_do_not_turn_intent_into_a_successful_outbound(self):
        cases = acceptance.detail_records(self.records(), 41237)
        campus = acceptance.connection_fields(cases["campus"])
        denied = acceptance.connection_fields(cases["unauthorized"])
        rejected = acceptance.connection_fields(cases["reject"])
        self.assertEqual(campus["期望动作"], "SHU-VPN")
        self.assertEqual(campus["实际出站"], "SHU-VPN")
        self.assertNotIn("错误代码", campus)
        self.assertEqual(denied["期望动作"], "SHU-VPN")
        self.assertEqual(denied["实际出站"], "本条未记录成功出站")
        self.assertEqual(denied["错误代码"], "unrouted")
        self.assertEqual(rejected["期望动作"], "REJECT")
        self.assertEqual(rejected["错误代码"], "rejected")

    def test_http_details_preserve_the_actual_target_port(self):
        stream = Fragments([b"HTTP/1.1 200 Connection established\r\n\r\n", b"prelude"])
        stream.sendall = Mock()
        with patch.object(acceptance.socket, "create_connection",
                          return_value=contextlib.nullcontext(stream)):
            with acceptance.proxy_stream(18443, "http", "127.0.0.1", target_port=41237):
                pass
        self.assertIn(b"CONNECT 127.0.0.1:41237 HTTP/1.1", stream.sendall.call_args.args[0])

    def test_rejection_requires_http_403_and_does_not_accept_a_timeout(self):
        stream = Fragments([b"HTTP/1.1 403 Forbidden\r\n\r\n"])
        with patch.object(acceptance.socket, "create_connection",
                          return_value=contextlib.nullcontext(stream)):
            self.assertEqual(acceptance.reject_http(18443, 19444)["http_status"], 403)
        stream = Fragments([b"HTTP/1.1 200 Connection established\r\n\r\n"])
        with patch.object(acceptance.socket, "create_connection",
                          return_value=contextlib.nullcontext(stream)):
            with self.assertRaises(RuntimeError):
                acceptance.reject_http(18443, 19444)
        with patch.object(acceptance.socket, "create_connection", side_effect=TimeoutError):
            with self.assertRaises(TimeoutError):
                acceptance.reject_http(18443, 19444)

    def test_verified_fixture_cleanup_uses_the_original_descriptor(self):
        with patch.object(acceptance.select, "select", return_value=([518], [], [])), \
                patch.object(acceptance.signal, "pidfd_send_signal", create=True) as send:
            acceptance.finish_fixture(518)
            send.assert_not_called()
        with patch.object(acceptance.select, "select", side_effect=[([], [], []), ([518], [], [])]), \
                patch.object(acceptance.signal, "pidfd_send_signal", create=True) as send:
            acceptance.finish_fixture(518)
            send.assert_called_once_with(518, acceptance.signal.SIGTERM)

    def test_reads_fragmented_protocol_bytes(self):
        self.assertEqual(acceptance.receive_exact(Fragments([b"p", b"rel", b"ude"]), 7),
                         b"prelude")

    def test_rejects_truncated_protocol_bytes(self):
        with self.assertRaisesRegex(RuntimeError, "closed before"):
            acceptance.receive_exact(Fragments([b"pre"]), 7)

    def test_rejects_http_failure_before_accepting_echo(self):
        stream = Fragments([b"HTTP/1.1 502 Bad Gateway\r\n\r\nprelude"])
        with patch.object(acceptance.socket, "create_connection",
                          return_value=contextlib.nullcontext(stream)):
            with self.assertRaisesRegex(RuntimeError, "CONNECT failed"):
                acceptance.echo(18443, "http", "10.21.0.5")

    def test_rejects_socks_authentication_failure(self):
        stream = Fragments([b"\x05\xff"])
        with patch.object(acceptance.socket, "create_connection",
                          return_value=contextlib.nullcontext(stream)):
            with self.assertRaisesRegex(RuntimeError, "authentication"):
                acceptance.echo(11080, "socks", "2001:db8:21::5")

    def test_rejects_socks_connect_failure(self):
        stream = Fragments([b"\x05\x00", b"\x05\x05\x00\x01"])
        with patch.object(acceptance.socket, "create_connection",
                          return_value=contextlib.nullcontext(stream)):
            with self.assertRaisesRegex(RuntimeError, "CONNECT failed"):
                acceptance.echo(11080, "socks", "10.21.0.5")

    def test_actual_open_listener_is_not_cleanup(self):
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            ports = {"http": listener.getsockname()[1]}
            self.assertFalse(acceptance.listeners_closed(ports))
            accepted, _ = listener.accept()
            accepted.close()

    def test_connection_refused_is_evidence_of_listener_cleanup(self):
        with patch.object(acceptance.socket, "create_connection",
                          side_effect=ConnectionRefusedError):
            self.assertTrue(acceptance.listeners_closed({"http": 18443}))

    def test_timeout_is_not_evidence_of_listener_cleanup(self):
        with patch.object(acceptance.socket, "create_connection", side_effect=TimeoutError):
            with self.assertRaises(TimeoutError):
                acceptance.listeners_closed({"http": 18443})


if __name__ == "__main__":
    unittest.main()
