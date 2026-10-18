"""Exercise TUN routing against an isolated physical peer, never host routes."""
import json
import os
import ctypes
from pathlib import Path
import select
import subprocess
import shutil
import sys
import tempfile
import time


PEER = r"""
import json, selectors, socket, ssl, struct, subprocess, sys, threading
print("PEER_READY", flush=True)
if sys.stdin.readline().strip() != "configure":
    raise RuntimeError("missing setup signal")
def ip(*args):
    subprocess.run(["/usr/sbin/ip", *args], check=True, capture_output=True)
ip("link", "set", "lo", "up")
ip("link", "set", "shupeer", "up")
ip("address", "add", "192.0.2.2/24", "dev", "shupeer")
ip("-6", "address", "add", "2001:db8:ffff::2/64", "dev", "shupeer", "nodad")
ip("address", "add", "10.22.0.5/32", "dev", "lo")
ip("-6", "address", "add", "2001:db8:22::5/128", "dev", "lo", "nodad")
ip("address", "add", "10.99.0.53/32", "dev", "lo")
v4 = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
v6 = socket.socket(socket.AF_INET6, socket.SOCK_DGRAM)
v6.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
v4.bind(("10.22.0.5", 0))
v6.bind(("2001:db8:22::5", 0))
tcp4 = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
tcp6 = socket.socket(socket.AF_INET6, socket.SOCK_STREAM)
tcp6.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
tcp4.bind(("10.22.0.5", 0))
tcp6.bind(("2001:db8:22::5", 0))
tcp4.listen(16)
tcp6.listen(16)
dns_udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
dns_udp.bind(("10.99.0.53", 53))
dns_tcp = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
dns_tcp.bind(("10.99.0.53", 53))
dns_tcp.listen(16)
https = None
tls = None
if len(sys.argv) == 3:
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(sys.argv[1], sys.argv[2])
    https = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    https.bind(("10.22.0.5", 0))
    https.listen(16)
print(json.dumps([v4.getsockname()[1], v6.getsockname()[1],
                  tcp4.getsockname()[1], tcp6.getsockname()[1],
                  https.getsockname()[1] if https else 0]), flush=True)
events = selectors.DefaultSelector()
events.register(v4, selectors.EVENT_READ, "udp")
events.register(v6, selectors.EVENT_READ, "udp")
events.register(tcp4, selectors.EVENT_READ, "tcp")
events.register(tcp6, selectors.EVENT_READ, "tcp")
events.register(dns_udp, selectors.EVENT_READ, "dns-udp")
events.register(dns_tcp, selectors.EVENT_READ, "dns-tcp")
events.register(sys.stdin, selectors.EVENT_READ, "control")
sources = []
http_release = threading.Event()
if https:
    events.register(https, selectors.EVENT_READ, "https")
def dns_reply(query, tcp=False):
    cursor = 12
    labels = []
    while query[cursor]:
        size = query[cursor]
        labels.append(query[cursor + 1:cursor + 1 + size].decode("ascii"))
        cursor += size + 1
    cursor += 1
    kind = struct.unpack("!H", query[cursor:cursor + 2])[0]
    question = query[12:cursor + 4]
    name = ".".join(labels)
    identifier = struct.unpack("!H", query[:2])[0]
    if name == "badid.fixture.test":
        identifier = (identifier + 1) % 65536
    if name == "truncated.fixture.test" and not tcp:
        return struct.pack("!HHHHHH", identifier, 0x8380, 1, 0, 0, 0) + question
    address = socket.inet_pton(socket.AF_INET if kind == 1 else socket.AF_INET6,
                              "10.22.0.5" if kind == 1 else "2001:db8:22::5")
    answer = b"\xc0\x0c" + struct.pack("!HHIH", kind, 1, 60, len(address)) + address
    return struct.pack("!HHHHHH", identifier, 0x8180, 1, 1, 0, 0) + question + answer
def exact(stream, length):
    result = b""
    while len(result) < length:
        chunk = stream.recv(length - len(result))
        if not chunk:
            raise RuntimeError("peer stream ended early")
        result += chunk
    return result
def dns_stream(stream):
    with stream:
        length = struct.unpack("!H", exact(stream, 2))[0]
        answer = dns_reply(exact(stream, length), tcp=True)
        stream.sendall(struct.pack("!H", len(answer)) + answer)
def https_stream(stream):
    try:
        with tls.wrap_socket(stream, server_side=True) as secure:
            request = b""
            while b"\r\n\r\n" not in request:
                chunk = secure.recv(4096)
                if not chunk:
                    return
                request += chunk
                if len(request) > 16384:
                    raise RuntimeError("oversized probe request")
            body = b"payload:\n - '.physical.fixture.test'\n"
            if request.split(b"\r\n", 1)[0] == b"GET /held HTTP/1.1":
                secure.sendall(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 8\r\n\r\nhead")
                if not http_release.wait(15):
                    raise RuntimeError("held HTTPS response was not released")
                secure.sendall(b"tail")
                return
            secure.sendall(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: "
                           + str(len(body)).encode("ascii") + b"\r\n\r\n" + body)
    except ssl.SSLError:
        stream.close()  # The untrusted-certificate case is intentionally rejected.
def echo(stream):
    with stream:
        while True:
            payload = stream.recv(16384)
            if not payload:
                stream.shutdown(socket.SHUT_WR)
                return
            stream.sendall(payload)
while True:
    for event, _ in events.select():
        if event.data == "control":
            command = sys.stdin.readline().strip()
            if command == "http-release":
                http_release.set()
                print("HTTP_RELEASED", flush=True)
                continue
            if command != "source-audit":
                raise RuntimeError("unexpected probe control command")
            print(json.dumps(sources), flush=True)
            continue
        if event.data in ("tcp", "dns-tcp", "https"):
            stream, peer = event.fileobj.accept()
            sources.append([event.data, peer[0]])
            handler = {"tcp": echo, "dns-tcp": dns_stream, "https": https_stream}[event.data]
            threading.Thread(target=handler, args=(stream,), daemon=True).start()
        elif event.data == "dns-udp":
            packet, peer = event.fileobj.recvfrom(65536)
            sources.append([event.data, peer[0]])
            event.fileobj.sendto(dns_reply(packet), peer)
        else:
            packet, peer = event.fileobj.recvfrom(65536)
            sources.append([event.data, peer[0]])
            event.fileobj.sendto(packet, peer)
"""


def line(process):
    if not select.select([process.stdout], [], [], 5)[0]:
        raise RuntimeError("physical peer startup timed out")
    value = process.stdout.readline()
    if not value:
        raise RuntimeError("physical peer exited: " + process.stderr.read())
    return value.strip()

def helper_processes(helper):
    matches = set()
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            command = (entry / "cmdline").read_bytes().split(b"\0")[0]
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
        if command == os.fsencode(helper):
            matches.add(int(entry.name))
    return matches


def desktop_probe(binary, helper, arguments, directory, release_http, *, crash=False):
    staged_helper = Path(directory) / "helper"
    staged_probe = Path(directory) / "probe"
    os.chown(directory, 0, 65534)
    os.chmod(directory, 0o750)
    for source, target in ((helper, staged_helper), (binary, staged_probe)):
        shutil.copyfile(source, target)
        os.chown(target, 0, 65534)
        os.chmod(target, 0o750)
    subprocess.run(["setcap", "cap_net_admin,cap_net_raw+ep", str(staged_helper)], check=True)
    if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "cannot become probe child subreaper")
    environment = dict(
        os.environ,
        SHUVPN_PROBE_HOST_NETNS=os.readlink("/proc/1/ns/net"),
        SHUVPN_LINUX_TUN_HELPER=str(staged_helper),
        SHUVPN_PROBE_HELPER_CRASH="1" if crash else "0",
    )
    child = subprocess.Popen([
        "setpriv", "--reuid=65534", "--regid=65534", "--clear-groups",
        "--inh-caps=-all", "--ambient-caps=-all",
        str(staged_probe), "--desktop-helper", *arguments,
    ], cwd=directory, env=environment, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    observed = set()
    replacements = set()
    killed = None
    confirmed = False
    try:
        deadline = time.monotonic() + 30
        pending = b""
        while time.monotonic() < deadline:
            if not select.select([child.stdout], [], [], 0.1)[0]:
                continue
            chunk = os.read(child.stdout.fileno(), 4096)
            if not chunk:
                break
            pending += chunk
            while b"\n" in pending:
                message, pending = pending.split(b"\n", 1)
                print(message.decode("utf-8", "replace"), flush=True)
                if message in (b"DESKTOP_LINUX_HELPER_READY", b"DESKTOP_LINUX_RESTART_READY"):
                    live = helper_processes(staged_helper)
                    if len(live) != 1:
                        raise RuntimeError(f"expected one owned helper while parent is alive: {live}")
                    observed |= live
                    reply = (b"helper-observed\n" if message == b"DESKTOP_LINUX_HELPER_READY"
                             else b"restart-observed\n")
                    child.stdin.write(reply)
                    child.stdin.flush()
                if message == b"DESKTOP_LINUX_HELPER_KILL_READY":
                    live = helper_processes(staged_helper)
                    if not crash or len(live) != 1 or killed is not None:
                        raise RuntimeError(f"unexpected helper crash stage: {live}")
                    killed = next(iter(live))
                    observed.add(killed)
                    os.kill(killed, 9)
                    child.stdin.write(b"helper-killed\n")
                    child.stdin.flush()
                if message.startswith(b"LINUX_TUN_HELPER_REPLACED "):
                    replacement = int(message.split()[-1])
                    if not crash or killed is None or replacement in observed:
                        raise RuntimeError(f"unexpected replacement helper PID: {replacement}")
                    replacements.add(replacement)
                    observed.add(replacement)
                if message == b"DESKTOP_LINUX_RETAINED_HTTP_RELEASE_READY":
                    release_http()
                    child.stdin.write(b"http-released\n")
                    child.stdin.flush()
                if message == b"DESKTOP_LINUX_CLEANUP_READY":
                    live = helper_processes(staged_helper)
                    remaining = {pid for pid in observed if Path(f"/proc/{pid}").exists()}
                    if live or remaining or child.poll() is not None:
                        raise RuntimeError(f"helpers survived formal stop: {live | remaining}")
                    if crash and (killed is None or not replacements):
                        raise RuntimeError("helper crash did not exercise broker replacement")
                    confirmed = True
                    child.stdin.write(b"cleanup-confirmed\n")
                    child.stdin.flush()
        child.wait(timeout=max(0.1, deadline - time.monotonic()))
        if child.returncode or not confirmed:
            raise RuntimeError(f"desktop helper probe failed with exit {child.returncode}")
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=3)
        for pid in helper_processes(staged_helper):
            try:
                os.kill(pid, 9)
            except ProcessLookupError:
                pass
            observed.add(pid)
        for pid in observed:
            deadline = time.monotonic() + 3
            while True:
                try:
                    reaped, _ = os.waitpid(pid, os.WNOHANG)
                except ChildProcessError:
                    break
                if reaped == pid:
                    break
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"owned helper {pid} could not be reaped")
                time.sleep(0.01)


def main():
    if os.geteuid() != 0:
        raise RuntimeError("run the probe as root inside unshare --net")
    if os.readlink("/proc/self/ns/net") == os.readlink("/proc/1/ns/net"):
        raise RuntimeError("refusing to modify the host network namespace")
    root = Path(__file__).resolve().parent.parent
    egress = sys.argv[1:] == ["--egress"]
    full = sys.argv[1:] == ["--full"]
    desktop = len(sys.argv) == 4 and sys.argv[1] in (
        "--desktop-helper", "--desktop-helper-crash",
    )
    crash = desktop and sys.argv[1] == "--desktop-helper-crash"
    if sys.argv[1:] and not (egress or desktop or full):
        raise RuntimeError(
            "expected no arguments, --egress, --full, or "
            "--desktop-helper/--desktop-helper-crash <profile> <helper>"
        )
    binary = (
        Path(sys.argv[2]).resolve() / "examples/packet_namespace_smoke" if desktop
        else Path(os.environ.get("SHUVPN_PROBE_PROFILE", str(root / "rust/target/release"))) /
             ("examples/egress_namespace_smoke" if egress else "examples/packet_namespace_smoke")
    )
    if not binary.is_file():
        raise RuntimeError("build packet_namespace_smoke first")
    certificate_dir = tempfile.TemporaryDirectory(prefix="shuvpn-egress-cert-")
    extra = []
    public_der = Path(certificate_dir.name) / "certificate.der"
    if egress or desktop:
        cert = Path(certificate_dir.name) / "certificate.pem"
        key = Path(certificate_dir.name) / "private-key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                        "-days", "1", "-subj", "/CN=provider.fixture.test",
                        "-addext", "basicConstraints=critical,CA:FALSE",
                        "-addext", "extendedKeyUsage=serverAuth",
                        "-addext", "subjectAltName=DNS:provider.fixture.test,IP:10.22.0.5",
                        "-keyout", str(key), "-out", str(cert)], check=True, capture_output=True)
        subprocess.run(["openssl", "x509", "-in", str(cert), "-outform", "DER",
                        "-out", str(public_der)], check=True, capture_output=True)
        if desktop:
            os.chown(public_der, 0, 65534)
            os.chmod(public_der, 0o640)
        extra = [str(cert), str(key)]
    peer = subprocess.Popen(
        ["unshare", "--net", sys.executable, "-u", "-c", PEER, *extra],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    try:
        if line(peer) != "PEER_READY":
            raise RuntimeError("unexpected peer startup")
        def ip(*args):
            subprocess.run(["/usr/sbin/ip", *args], check=True, capture_output=True)
        ip("link", "set", "lo", "up")
        ip("link", "add", "shuphys", "type", "veth", "peer", "name", "shupeer")
        ip("link", "set", "shupeer", "netns", str(peer.pid))
        ip("link", "set", "shuphys", "up")
        ip("address", "add", "192.0.2.1/24", "dev", "shuphys")
        ip("-6", "address", "add", "2001:db8:ffff::1/64", "dev", "shuphys", "nodad")
        peer.stdin.write("configure\n")
        peer.stdin.flush()
        ports = json.loads(line(peer))
        ip("route", "add", "default", "via", "192.0.2.2", "dev", "shuphys")
        ip("-6", "route", "add", "default", "via", "2001:db8:ffff::2", "dev", "shuphys")
        probe_args = [str(ports[4]), str(public_der)] if egress else list(map(str, ports[:4]))
        if desktop:
            def release_http():
                peer.stdin.write("http-release\n")
                peer.stdin.flush()
                if line(peer) != "HTTP_RELEASED":
                    raise RuntimeError("physical HTTPS peer did not acknowledge release")
            desktop_probe(binary, Path(sys.argv[3]).resolve(),
                          [*probe_args, str(ports[4]), str(public_der)], certificate_dir.name,
                          release_http,
                          crash=crash)
            peer.stdin.write("source-audit\n")
            peer.stdin.flush()
            observed = json.loads(line(peer))
            physical = {"192.0.2.1", "2001:db8:ffff::1"}
            if any(address not in physical for _, address in observed):
                raise RuntimeError(f"physical peer saw a leaked virtual source: {observed}")
            required = {
                ("udp", "192.0.2.1"), ("udp", "2001:db8:ffff::1"),
                ("tcp", "192.0.2.1"), ("tcp", "2001:db8:ffff::1"),
                ("dns-udp", "192.0.2.1"), ("dns-tcp", "192.0.2.1"),
            }
            if not required.issubset(map(tuple, observed)) or not any(
                    transport == "https" for transport, _ in observed):
                raise RuntimeError(f"physical source coverage incomplete: {observed}")
            print("DESKTOP_LINUX_PHYSICAL_SOURCE_AUDIT_PASSED", json.dumps(observed))
        else:
            environment = os.environ.copy()
            if full:
                environment["SHUVPN_PROBE_FULL_CAPTURE"] = "1"
                case = environment.get("SHUVPN_PROBE_POLICY_CASE", "")
                if case == "foreign-local":
                    ip("route", "add", "local", "198.51.100.7/32", "dev", "lo", "table", "local")
                    environment["SHUVPN_PROBE_EXPECT_PLAN_FAILURE"] = "1"
                elif case == "foreign-local-unicast":
                    ip("route", "add", "198.51.100.7/32", "dev", "shuphys", "table", "local")
                    environment["SHUVPN_PROBE_EXPECT_PLAN_FAILURE"] = "1"
                elif case == "foreign-local-blackhole":
                    ip("route", "add", "blackhole", "198.51.100.0/24", "table", "local")
                    environment["SHUVPN_PROBE_EXPECT_PLAN_FAILURE"] = "1"
                elif case == "missing-local":
                    # Keep loopback routing until the in-process gateway is connected.
                    environment["SHUVPN_PROBE_REMOVE_LOCAL_RULE"] = "1"
                    environment["SHUVPN_PROBE_EXPECT_PLAN_FAILURE"] = "1"
                elif case in {"missing-main", "missing-main-v6"}:
                    family = "-6" if case == "missing-main-v6" else "-4"
                    ip(family, "rule", "del", "priority", "32766")
                    environment["SHUVPN_PROBE_EXPECT_PLAN_FAILURE"] = "1"
                elif case:
                    raise RuntimeError("unknown isolated full capture policy case")
            subprocess.run([str(binary), *probe_args], cwd=root, check=True, timeout=45, env=environment)
            if full and not environment.get("SHUVPN_PROBE_EXPECT_PLAN_FAILURE"):
                peer.stdin.write("source-audit\n")
                peer.stdin.flush()
                observed = json.loads(line(peer))
                physical = {"192.0.2.1", "2001:db8:ffff::1"}
                if any(address not in physical for _, address in observed):
                    raise RuntimeError(f"full capture leaked a virtual physical source: {observed}")
                required = {("udp", "192.0.2.1"), ("udp", "2001:db8:ffff::1"),
                            ("tcp", "192.0.2.1"), ("tcp", "2001:db8:ffff::1"),
                            ("dns-udp", "192.0.2.1"), ("dns-tcp", "192.0.2.1")}
                if not required.issubset(map(tuple, observed)):
                    raise RuntimeError(f"full capture physical coverage incomplete: {observed}")
                print("FULL_CAPTURE_PHYSICAL_SOURCE_AND_DNS_AUDIT_PASSED", json.dumps(observed))
    finally:
        peer.kill()
        peer.wait(timeout=5)
        certificate_dir.cleanup()


if __name__ == "__main__":
    main()
