import asyncio
import json
import socket
import struct
import unittest

from tool.full_capture_peer import Peer, dns_answer


def query(name="direct.fixture.test", kind=1):
    labels = b"".join(bytes([len(part)]) + part.encode() for part in name.split("."))
    return struct.pack("!6H", 518, 0x0100, 1, 0, 0, 0) + labels + b"\0" + struct.pack("!HH", kind, 1)


class FullCapturePeerTest(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.peer = Peer("127.0.0.1", "::1")
        self.addAsyncCleanup(self.peer.close)
        self.config = await self.peer.start(dns_port=0)

    async def echo(self, host):
        reader, writer = await asyncio.open_connection(host, self.config["tcp_port"])
        try:
            payload = bytes(range(251)) * 300
            writer.write(payload)
            await writer.drain()
            writer.write_eof()
            self.assertEqual(await reader.read(), payload)
        finally:
            writer.close()
            await writer.wait_closed()
        response, _ = await self.datagram(host, self.config["udp_port"], b"full-peer-udp")
        self.assertEqual(response, b"full-peer-udp")

    async def datagram(self, host, port, payload):
        def exchange():
            family = socket.AF_INET if host == "127.0.0.1" else socket.AF_INET6
            destination = (host, port) if family == socket.AF_INET else (host, port, 0, 0)
            with socket.socket(family, socket.SOCK_DGRAM) as udp:
                udp.settimeout(2)
                udp.sendto(payload, destination)
                return udp.recvfrom(4096)

        return await asyncio.to_thread(exchange)

    async def test_dual_stack_echo_and_independent_source_audit(self):
        await self.echo("127.0.0.1")
        await self.echo("::1")
        audit = await self.audit()
        self.assertEqual(audit["tcp"], [["direct", "127.0.0.1"], ["direct", "::1"]])
        self.assertEqual(audit["udp"], [["direct", "127.0.0.1"], ["direct", "::1"]])
        self.assertEqual(audit["token"], self.config["token"])

    async def audit(self):
        reader, writer = await asyncio.open_connection("127.0.0.1", self.config["control_port"])
        try:
            request = {"token": self.config["token"], "command": "audit"}
            writer.write((json.dumps(request) + "\n").encode())
            await writer.drain()
            return json.loads(await reader.readline())
        finally:
            writer.close()
            await writer.wait_closed()

    async def test_dns_udp_truncation_retries_on_tcp_and_bad_id_is_distinct(self):
        request = query("truncated.fixture.test")
        truncated, _ = await self.datagram("127.0.0.1", self.config["dns_port"], request)
        self.assertTrue(struct.unpack("!H", truncated[2:4])[0] & 0x200)
        reader, writer = await asyncio.open_connection("127.0.0.1", self.config["dns_port"])
        try:
            writer.write(struct.pack("!H", len(request)) + request)
            await writer.drain()
            length = struct.unpack("!H", await reader.readexactly(2))[0]
            answer = await reader.readexactly(length)
            self.assertEqual(answer[-4:], socket.inet_pton(socket.AF_INET, "127.0.0.1"))
            self.assertEqual(struct.unpack("!H", answer[6:8])[0], 1)
        finally:
            writer.close()
            await writer.wait_closed()
        answer6 = dns_answer(query(kind=28), "127.0.0.1", "::1")
        self.assertEqual(answer6[-16:], socket.inet_pton(socket.AF_INET6, "::1"))
        bad, _ = await self.datagram(
            "127.0.0.1", self.config["dns_port"], query("badid.fixture.test"))
        self.assertNotEqual(struct.unpack("!H", bad[:2])[0], 518)
        records = (await self.audit())["dns"]
        self.assertEqual([(entry["name"], entry["transport"], entry["truncated"])
                          for entry in records],
                         [("truncated.fixture.test", "udp", True),
                          ("truncated.fixture.test", "tcp", False),
                          ("badid.fixture.test", "udp", False)])
        self.assertEqual(records[-1]["query_id"], 518)
        self.assertEqual(records[-1]["response_id"], struct.unpack("!H", bad[:2])[0])

    async def test_control_rejects_wrong_observer_token(self):
        reader, writer = await asyncio.open_connection("127.0.0.1", self.config["control_port"])
        try:
            writer.write(b'{"token":"wrong","command":"audit"}\n')
            await writer.drain()
            self.assertEqual(await asyncio.wait_for(reader.read(), 2), b"")
        finally:
            writer.close()
            await writer.wait_closed()
