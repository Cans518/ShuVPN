#!/usr/bin/env python3
"""Independent dual-stack peer for a disposable Full capture test network."""

import argparse
import asyncio
import ipaddress
import json
import secrets
import struct


def dns_question(query):
    if len(query) < 12 or struct.unpack("!H", query[4:6])[0] != 1:
        raise ValueError("one DNS question required")
    offset = 12
    labels = []
    while offset < len(query) and query[offset]:
        length = query[offset]
        if length > 63 or offset + 1 + length >= len(query):
            raise ValueError("invalid DNS name")
        labels.append(query[offset + 1:offset + 1 + length].decode("ascii"))
        offset += 1 + length
    end = offset + 5
    if end != len(query):
        raise ValueError("unexpected DNS question size")
    kind, cls = struct.unpack("!HH", query[offset + 1:end])
    if cls != 1 or kind not in (1, 28):
        raise ValueError("A or AAAA query required")
    return ".".join(labels), kind, query[12:end]


def dns_answer(query, ipv4, ipv6, tcp=False):
    name, kind, question = dns_question(query)
    identifier = struct.unpack("!H", query[:2])[0]
    if name == "badid.fixture.test":
        identifier ^= 1
    truncated = name == "truncated.fixture.test" and not tcp
    header = struct.pack("!6H", identifier, 0x8380 if truncated else 0x8180,
                         1, 0 if truncated else 1, 0, 0)
    address = ipaddress.ip_address(ipv4 if kind == 1 else ipv6).packed
    answer = b"\xc0\x0c" + struct.pack("!HHIH", kind, 1, 30, len(address)) + address
    return header + question + (b"" if truncated else answer)


class Peer:
    def __init__(self, ipv4, ipv6):
        self.ipv4, self.ipv6 = ipv4, ipv6
        self.token = secrets.token_hex(16)
        self.tcp = []
        self.udp = []
        self.dns = []
        self.servers = []
        self.transports = []
        self.clients = set()

    async def client(self, reader, writer, role):
        task = asyncio.current_task()
        self.clients.add(task)
        source = writer.get_extra_info("peername")[0]
        if role != "control":
            self.tcp.append([role, source])
        try:
            if role == "control":
                while line := await reader.readline():
                    request = json.loads(line)
                    if request != {"token": self.token, "command": "audit"}:
                        return
                    writer.write((json.dumps({"token": self.token,
                                              "tcp": self.tcp, "udp": self.udp,
                                              "dns": self.dns}) + "\n").encode())
                    await writer.drain()
            elif role == "dns":
                length = struct.unpack("!H", await reader.readexactly(2))[0]
                query = await reader.readexactly(length)
                answer = self.answer(query, source, tcp=True)
                writer.write(struct.pack("!H", len(answer)) + answer)
                await writer.drain()
            else:
                while chunk := await reader.read(65536):
                    writer.write(chunk)
                    await writer.drain()
        except (asyncio.IncompleteReadError, ConnectionError, ValueError):
            pass
        finally:
            writer.close()
            try:
                await writer.wait_closed()
            except ConnectionError:
                pass
            self.clients.discard(task)

    async def tcp_server(self, host, port, role):
        server = await asyncio.start_server(
            lambda reader, writer: self.client(reader, writer, role),
            host, port, limit=65536)
        self.servers.append(server)
        return server.sockets[0].getsockname()[1]

    async def udp_server(self, host, port, role):
        peer = self

        class Datagram(asyncio.DatagramProtocol):
            def connection_made(self, transport):
                self.transport = transport

            def datagram_received(self, packet, address):
                peer.udp.append([role, address[0]])
                try:
                    response = (peer.answer(packet, address[0])
                                if role == "dns" else packet)
                    self.transport.sendto(response, address)
                except ValueError:
                    pass

        transport, _ = await asyncio.get_running_loop().create_datagram_endpoint(
            Datagram, local_addr=(host, port))
        self.transports.append(transport)
        return transport.get_extra_info("sockname")[1]

    async def start(self, dns_port=53):
        try:
            tcp_port = await self.tcp_server(self.ipv4, 0, "direct")
            await self.tcp_server(self.ipv6, tcp_port, "direct")
            blocked_port = await self.tcp_server(self.ipv4, 0, "blocked")
            await self.tcp_server(self.ipv6, blocked_port, "blocked")
            for _ in range(16):
                udp_port = await self.udp_server(self.ipv4, 0, "direct")
                if udp_port != blocked_port:
                    break
            else:
                raise RuntimeError("could not allocate UDP outside the blocked port")
            await self.udp_server(self.ipv6, udp_port, "direct")
            dns_port = await self.tcp_server(self.ipv4, dns_port, "dns")
            await self.udp_server(self.ipv4, dns_port, "dns")
            control_port = await self.tcp_server(self.ipv4, 0, "control")
        except BaseException:
            await self.close()
            raise
        return {"peer4": self.ipv4, "peer6": self.ipv6, "tcp_port": tcp_port,
                "udp_port": udp_port, "blocked_port": blocked_port,
                "control_port": control_port, "dns_port": dns_port, "token": self.token}

    def answer(self, query, source, tcp=False):
        response = dns_answer(query, self.ipv4, self.ipv6, tcp)
        name, kind, _ = dns_question(query)
        self.dns.append({"transport": "tcp" if tcp else "udp", "name": name,
                         "query_id": struct.unpack("!H", query[:2])[0],
                         "response_id": struct.unpack("!H", response[:2])[0],
                         "type": kind, "truncated": bool(response[2] & 2),
                         "source": source})
        return response

    async def close(self):
        for server in self.servers:
            server.close()
        await asyncio.gather(*(server.wait_closed() for server in self.servers))
        for transport in self.transports:
            transport.close()
        clients = list(self.clients)
        for task in clients:
            task.cancel()
        await asyncio.gather(*clients, return_exceptions=True)


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ipv4", required=True, type=ipaddress.IPv4Address)
    parser.add_argument("--ipv6", required=True, type=ipaddress.IPv6Address)
    args = parser.parse_args()
    peer = Peer(str(args.ipv4), str(args.ipv6))
    try:
        print(json.dumps(await peer.start()), flush=True)
        await asyncio.Event().wait()
    finally:
        await peer.close()


if __name__ == "__main__":
    asyncio.run(main())
