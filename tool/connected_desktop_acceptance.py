"""Exercise connected production widgets with a real native local protocol fixture."""
from contextlib import contextmanager
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import socket
import sys
import time

import desktop_gui_acceptance as gui
from desktop_tray_acceptance import TraySession
from desktop_rule_acceptance import unique_named_node, validate_action_target


def receive_exact(stream, count):
    result = bytearray()
    while len(result) < count:
        part = stream.recv(count - len(result))
        if not part:
            raise RuntimeError("Proxy closed before the expected protocol bytes")
        result.extend(part)
    return bytes(result)


@contextmanager
def proxy_stream(port, protocol, address, *, target_port=443):
    """Open a loopback proxy and require the remote protocol fixture prelude."""
    with socket.create_connection(("127.0.0.1", port), timeout=5) as stream:
        stream.settimeout(5)
        if protocol == "http":
            target = f"[{address}]:{target_port}" if ":" in address else f"{address}:{target_port}"
            stream.sendall(f"CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n".encode())
            header = bytearray()
            while not header.endswith(b"\r\n\r\n"):
                header.extend(receive_exact(stream, 1))
                if len(header) > 8192:
                    raise RuntimeError("Oversized HTTP CONNECT response")
            if not header.startswith(b"HTTP/1.1 200 "):
                raise RuntimeError(f"CONNECT failed: {header!r}")
        elif protocol == "socks":
            stream.sendall(b"\x05\x01\x00")
            if receive_exact(stream, 2) != b"\x05\x00":
                raise RuntimeError("SOCKS authentication negotiation failed")
            family = socket.AF_INET6 if ":" in address else socket.AF_INET
            kind = 4 if family == socket.AF_INET6 else 1
            stream.sendall(b"\x05\x01\x00" + bytes([kind]) +
                           socket.inet_pton(family, address) + target_port.to_bytes(2, "big"))
            version, status, reserved, kind = receive_exact(stream, 4)
            if (version, status, reserved) != (5, 0, 0) or kind not in (1, 4):
                raise RuntimeError("SOCKS CONNECT failed")
            receive_exact(stream, (4 if kind == 1 else 16) + 2)
        else:
            raise ValueError("Unknown proxy protocol")
        if receive_exact(stream, 7) != b"prelude":
            raise RuntimeError("Native gateway prelude did not traverse the proxy")
        yield stream


def echo(port, protocol, address, *, target_port=443):
    """Independent client: establish a real proxy flow, check prelude/data/FIN."""
    with proxy_stream(port, protocol, address, target_port=target_port) as stream:
        payload = bytes(range(256)) * 256
        stream.sendall(payload)
        stream.shutdown(socket.SHUT_WR)
        if receive_exact(stream, len(payload)) != payload or stream.recv(1) != b"":
            raise RuntimeError("Native proxy echo or half-close was incorrect")
    return {"protocol": protocol, "address": address, "port": port,
            "target_port": target_port, "payload_bytes": len(payload),
            "prelude": True, "half_close_eof": True}


def reject_http(proxy_port, target_port):
    with socket.create_connection(("127.0.0.1", proxy_port), timeout=5) as stream:
        stream.settimeout(5)
        stream.sendall(f"CONNECT 127.0.0.1:{target_port} HTTP/1.1\r\n\r\n".encode())
        response = bytearray()
        while not response.endswith(b"\r\n\r\n"):
            response.extend(receive_exact(stream, 1))
            if len(response) > 8192:
                raise RuntimeError("Oversized rejection response")
        if response.split(b"\r\n", 1)[0] != b"HTTP/1.1 403 Forbidden":
            raise RuntimeError("Rule/authorization rejection did not return HTTP 403")
    return {"target": "127.0.0.1", "port": target_port, "http_status": 403}


def detail_records(records, direct_port):
    selected = {}
    for name, host, port, rule_id, action, outbound, error in (
        ("campus", "10.21.0.5", 443, "profile:3", "SHU-VPN", "SHU-VPN", None),
        ("direct", "127.0.0.1", direct_port, "profile:2", "DIRECT", "DIRECT", None),
        ("reject", "127.0.0.1", 19444, "profile:0", "REJECT", None, "rejected"),
        ("unauthorized", "127.0.0.1", 19445, "profile:1", "SHU-VPN", None, "unrouted"),
    ):
        matches = [row for row in records if all(row["flow"].get(key) == value for key, value in {
            "host": host, "port": port, "network": "tcp", "inbound": "http"}.items())]
        if len(matches) != 1:
            raise RuntimeError(f"Expected one real native {name} observation")
        row = matches[0]
        decision = row.get("decision") or {}
        if (any(decision.get(key) != value for key, value in {
                "version": 1, "source": "profile", "rule_id": rule_id,
                "policy": action, "action": action}.items()) or
                row.get("outbound") != outbound or row.get("error") != error or
                not isinstance(decision.get("reason"), str) or not decision["reason"] or
                (error is not None and (not isinstance(row.get("message"), str) or not row["message"]))):
            raise RuntimeError(f"Native {name} observation lost its actual decision/outcome")
        selected[name] = row
    return selected


def connection_fields(record):
    decision = record["decision"]
    fields = {
        "规则版本": str(decision["version"]), "命中规则": decision["rule_id"],
        "规则来源": "配置档规则", "策略": decision["policy"], "期望动作": decision["action"],
        "实际出站": record["outbound"] or "本条未记录成功出站", "命中原因": decision["reason"],
    }
    if record.get("error") is not None:
        fields.update({"错误代码": record["error"], "失败或拒绝原因": record["message"]})
    return fields


@contextmanager
def loopback_listener(port=0):
    with socket.socket() as listener:
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind(("127.0.0.1", port))
        listener.listen()
        listener.settimeout(5)
        yield listener


@contextmanager
def detail_traffic(proxy_port):
    with loopback_listener() as direct, loopback_listener(19444) as rejected, \
            loopback_listener(19445) as unauthorized:
        direct_port = direct.getsockname()[1]
        def serve():
            stream, peer = direct.accept()
            with stream:
                stream.settimeout(5)
                stream.sendall(b"prelude")
                payload = bytearray()
                while chunk := stream.recv(16384):
                    payload.extend(chunk)
                    stream.sendall(chunk)
            return {"peer": peer, "received": len(payload),
                    "sha256": hashlib.sha256(payload).hexdigest()}
        with ThreadPoolExecutor(max_workers=1, thread_name_prefix="gui-direct-fixture") as executor:
            receiver = executor.submit(serve)
            flow = echo(proxy_port, "http", "127.0.0.1", target_port=direct_port)
            receipt = receiver.result(timeout=6)
        payload = bytes(range(256)) * 256
        if receipt["received"] != len(payload) or receipt["sha256"] != hashlib.sha256(payload).hexdigest():
            raise RuntimeError("The independent DIRECT target did not receive the actual payload")
        responses = [reject_http(proxy_port, port) for port in (19444, 19445)]
        evidence = {"direct_port": direct_port, "flow": flow, "receiver": receipt,
                    "rejections": responses}
        yield evidence
        if select.select([rejected, unauthorized], [], [], 0.2)[0]:
            raise RuntimeError("A rejected or unauthorized target received a physical connection")
        evidence["forbidden_listeners_untouched"] = True


def finish_fixture(descriptor):
    if select.select([descriptor], [], [], 3)[0]:
        return
    signal.pidfd_send_signal(descriptor, signal.SIGTERM)
    if not select.select([descriptor], [], [], 3)[0]:
        signal.pidfd_send_signal(descriptor, signal.SIGKILL)
        if not select.select([descriptor], [], [], 3)[0]:
            raise RuntimeError("The verified fixture survived targeted cleanup")
    print("Verified connected fixture required forced cleanup", file=sys.stderr)


def listeners_closed(ports):
    for port in ports.values():
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return False
        except ConnectionRefusedError:
            pass
    return True


def traffic_evidence(native, state):
    """Check four real flows and wait for complete native traffic decisions."""
    result = [echo(state["ports"][protocol], protocol, address)
              for protocol in ("http", "socks")
              for address in ("10.21.0.5", "2001:db8:21::5")]
    uploaded = sum(flow["payload_bytes"] for flow in result)
    downloaded = uploaded + 7 * len(result)

    def observed():
        current = native()
        if current is None or current["round"] != state["round"]:
            return None
        session = current["session"]
        if session is None or session["upload"] < uploaded or session["download"] < downloaded:
            return None
        decisions = session["decisions"]
        for inbound in ("http", "socks"):
            for address in ("10.21.0.5", "2001:db8:21::5"):
                if not any(row["flow"]["inbound"] == inbound and
                           row["flow"]["host"] == address and
                           row["flow"]["port"] == 443 and
                           row["flow"]["network"] == "tcp" and
                           row.get("outbound") == "SHU-VPN" and
                           row["decision"]["action"] == "SHU-VPN" for row in decisions):
                    return None
        return current
    evidence = gui.wait_for(observed, "native traffic totals and real proxy decisions")
    if evidence["session"]["tun"].get("state") == "running":
        raise RuntimeError("Proxy GUI fixture unexpectedly started TUN")
    return {"flows": result, "native_traffic": evidence}


class ConnectedSession(TraySession):
    def named(self, label, role="push button"):
        return gui.wait_for(
            lambda: unique_named_node(gui.accessible_nodes(self.application.pid), label, role),
            f"owned connected control {label}")

    def activate_record_control(self, node, label, role):
        observed = self.node_observation(node)
        validate_action_target(observed, self.application.pid, label, role)
        if node.get_n_actions() < 1 or not node.do_action(0):
            raise RuntimeError(f"Connected control action failed: {label}")
        self.steps.append({"connected_gui_action": observed})
        time.sleep(0.35)

    def record_control(self, label):
        node = self.named(label)
        self.activate_record_control(node, label, "push button")

    def record_header(self, record):
        flow = record["flow"]
        outcome = ("无成功出站记录" if record["outbound"] is None
                   else f"已打开 {record['outbound']} 出站")
        expected = (f"{flow['host']} · {flow['port']}\n"
                    f"{flow['network'].upper()} · {flow['inbound'].upper()} · {outcome}")
        def find():
            matches = [node for node in gui.accessible_nodes(self.application.pid)
                       if node.get_name() == expected and node.get_n_actions() > 0]
            if len(matches) > 1:
                raise RuntimeError("Multiple UI records have the same native flow identity")
            return matches[0] if matches else None
        return gui.wait_for(find, f"record header {expected}")

    def open_records(self):
        self.record_control("连接记录")
        gui.wait_for(lambda: any(row["name"].startswith("当前会话最近最多128条出站结果")
                                for row in gui.accessibility(self.application.pid)),
                     "history scope notice in the production records page")
        gui.wait_for(lambda: self.node_observation(self.named("刷新连接记录"))["enabled"],
                     "records initial read completed")
        self.record_control("刷新连接记录")
        gui.wait_for(lambda: self.node_observation(self.named("刷新连接记录"))["enabled"],
                     "explicit records refresh completed")

    def inspect_record(self, kind, record, *, round_number, run_id):
        node = self.record_header(record)
        self.activate_record_control(node, node.get_name().split("\n")[0], node.get_role_name())
        fields = {}
        for label, value in connection_fields(record).items():
            expected = f"{label}：{value}"
            def read():
                matches = [node for node in gui.accessible_nodes(self.application.pid)
                           if node.get_name() == expected]
                if len(matches) > 1:
                    raise RuntimeError(f"Duplicate connection detail: {label}")
                if not matches:
                    return None
                observed = self.node_observation(matches[0])
                if observed["pid"] != self.application.pid or not observed["showing"]:
                    raise RuntimeError("Connection detail belongs to another or hidden GUI")
                return observed
            fields[label] = gui.wait_for(read, f"native-backed detail {expected}")
        current = self.native()
        if current["round"] != round_number or current["owner"]["run_id"] != run_id:
            raise RuntimeError("The native run changed while reading GUI connection details")
        self.steps.append({"connection_detail": {
            "kind": kind, "native": record, "fields": fields,
            "round": round_number, "run_id": run_id}})
        gui.command("xdotool", "mousemove", "--window", self.window, 8, 500,
                    "click", "--repeat", 10, "--delay", 50, "5")
        self.snapshot(f"detail-{kind}")
        node = self.record_header(record)
        self.activate_record_control(node, node.get_name().split("\n")[0], node.get_role_name())

    def details(self, state):
        run_id = state["owner"]["run_id"]
        with detail_traffic(state["ports"]["http"]) as network:
            def captured():
                current = self.native()
                if (current["round"] != state["round"] or
                        current["owner"]["run_id"] != run_id or current["owner"]["phase"] != "connected"):
                    raise RuntimeError("Connection changed during details acceptance")
                records = current["session"]["decisions"]
                return records if len(records) >= 7 else None
            records = gui.wait_for(captured, "four kinds of real outbound observations")
            cases = detail_records(records, network["direct_port"])
            self.open_records()
            for kind, record in cases.items():
                self.inspect_record(kind, record, round_number=state["round"], run_id=run_id)
            self.record_control("返回")
        self.steps.append({"detail_network_evidence": network})

    def disconnected_records(self):
        message = "当前没有可读取的原生会话。连接校园网后，可在这里刷新记录。"
        gui.wait_for(lambda: any(row["name"] == message
                                for row in gui.accessibility(self.application.pid)),
                     "records page clears the disconnected native generation")
        node = self.named("刷新连接记录")
        observed = self.node_observation(node)
        if observed["enabled"] or node.get_n_actions() != 0:
            raise RuntimeError("Disconnected history refresh still exposes an action")
        names = [row["name"] for row in gui.accessibility(self.application.pid)]
        if any(name.startswith(("规则版本：", "命中规则：", "实际出站：")) for name in names):
            raise RuntimeError("Disconnected records retained old detail values")
        if any(name.startswith(("10.21.0.5 · ", "2001:db8:21::5 · ", "127.0.0.1 · "))
               for name in names):
            raise RuntimeError("Disconnected records retained old collapsed cards")
        self.steps.append({"disconnected_records": {"message": message, "refresh": observed}})
        self.snapshot("records-disconnected")
        self.record_control("返回")

    def native(self):
        if list(self.report.glob("fatal-*.json")):
            raise RuntimeError("Native observation failed; inspect fatal report")
        path = self.report / "native-state.json"
        if not path.exists():
            return None
        state = json.loads(path.read_text())
        if state["token"] != self.args.token or state["pid"] != self.application.pid:
            raise RuntimeError("Native observation does not belong to this GUI")
        if state["uid"] != os.geteuid():
            raise RuntimeError("Native observation changed user")
        if state["network"] != os.readlink("/proc/self/ns/net"):
            raise RuntimeError("Native observation escaped the network namespace")
        return state

    def state(self, round_number, connected):
        def ready():
            state = self.native()
            if state is None or state["round"] != round_number:
                return None
            owner = state["owner"]
            if owner["cleanup_pending"] or owner["closed"]:
                raise RuntimeError("Ordinary disconnect terminally closed the native owner")
            if connected:
                return state if (owner["run_id"] is not None and
                                 owner["phase"] == "connected" and
                                 state["session"] is not None and
                                 state["session"]["state"] == "connected") else None
            return state if owner["run_id"] is None else None
        return gui.wait_for(ready, f"native round {round_number} connected={connected}")

    def orb(self, connected):
        label = "连接状态：已连接" if connected else "连接状态：未连接"
        gui.wait_for(lambda: any(row["name"].split("\n")[0] == label
                                for row in gui.accessibility(self.application.pid)),
                     f"actual GUI {label}")
        return label

    def menu_state(self, tray, connected):
        def ready():
            layout = tray.layout()
            (self.report / "latest-menu.json").write_text(
                json.dumps(layout, ensure_ascii=False, indent=2) + "\n")
            return (tray.item("连接").get("enabled", True) is not connected and
                    tray.item("断开").get("enabled", True) is connected and
                    tray.item("aTrust").get("enabled", True) is not connected)
        gui.wait_for(ready, f"native menu connected={connected}")
        layout = tray.layout()
        if connected and not any(row.get("label") == "已连接 · aTrust"
                                 for row in layout["rows"]):
            raise RuntimeError("Native tray did not display the connected status")
        self.steps.append({"native_menu_connected": connected, "layout": layout})

    def traffic(self, state):
        self.steps.append(traffic_evidence(self.native, state))

    def run(self):
        fixture_fd = None
        fixture_identity = None
        try:
            self.start_panel()
            os.environ["SHUVPN_CONNECTED_GUI_REPORT"] = str(self.report)
            os.environ["SHUVPN_CONNECTED_GUI_TOKEN"] = self.args.token
            self.start("01-fixture-disconnected")
            state = self.state(0, False)
            fixture_pid = state["fixture_pid"]
            fixture_fd = os.pidfd_open(fixture_pid)
            fixture_exe = Path(f"/proc/{fixture_pid}/exe").resolve()
            if fixture_exe != Path(os.environ["SHUVPN_CONNECTED_GUI_FIXTURE"]).resolve():
                raise RuntimeError("Unexpected native fixture executable")
            fixture_status = dict(row.split(":", 1) for row in
                                  Path(f"/proc/{fixture_pid}/status").read_text().splitlines())
            if (int(fixture_status["PPid"]) != self.application.pid or
                    list(map(int, fixture_status["Uid"].split())) != [os.geteuid()] * 4 or
                    int(fixture_status["CapEff"], 16) != 0 or
                    os.readlink(f"/proc/{fixture_pid}/ns/net") != os.readlink("/proc/self/ns/net") or
                    select.select([fixture_fd], [], [], 0)[0]):
                raise RuntimeError("Native fixture identity did not match this ordinary GUI")
            fixture_identity = {"pid": fixture_pid, "parent_pid": self.application.pid,
                                "exe": str(fixture_exe), "uid": os.geteuid(),
                                "cap_eff": fixture_status["CapEff"].strip(),
                                "sha256": hashlib.sha256(fixture_exe.read_bytes()).hexdigest()}
            tray = self.menu()
            self.menu_state(tray, False)
            self.control(self.orb(False))
            state = self.state(1, True)
            self.orb(True)
            self.menu_state(tray, True)
            self.traffic(state)
            self.details(state)
            self.snapshot("02-orb-connected")
            self.control(self.orb(True))
            self.state(1, False)
            gui.wait_for(lambda: listeners_closed(state["ports"]), "orb disconnect closes listeners")
            self.orb(False)
            self.menu_state(tray, False)
            tray.click("连接")
            state = self.state(2, True)
            self.orb(True)
            self.menu_state(tray, True)
            self.traffic(state)
            self.hide_window()
            self.show_window(tray)
            self.snapshot("03-tray-reconnected")
            self.open_records()
            tray.click("断开")
            self.state(2, False)
            gui.wait_for(lambda: listeners_closed(state["ports"]), "tray disconnect closes listeners")
            self.disconnected_records()
            self.orb(False)
            self.menu_state(tray, False)
            tray.click("连接")
            state = self.state(3, True)
            self.orb(True)
            self.menu_state(tray, True)
            self.traffic(state)
            self.snapshot("04-reconnected-after-tray-disconnect")
            app_pid = self.application.pid
            self.hide_window()
            self.panel_menu_click("退出")
            self.wait_exit()
            final = json.loads((self.report / f"native-{app_pid}-shutdown-3.json").read_text())
            if (final["token"] != self.args.token or final["pid"] != app_pid or
                    final["uid"] != os.geteuid() or
                    final["network"] != os.readlink("/proc/self/ns/net") or
                    final["fixture_pid"] != fixture_pid or
                    final["owner"] != {"phase": "disconnected", "closed": True, "run_id": None,
                                       "cleanup_pending": False} or final["fixture_exit"] != 0):
                raise RuntimeError("Quit did not complete the native cleanup barrier")
            if (final["session"]["tun"] != {"state": "stopped", "cleanup_pending": False} or
                    final["session"]["egress"]["active"] or
                    final["session"]["egress"]["pending_cleanup"] or
                    final["session"]["egress"]["sockets"]):
                raise RuntimeError("Native session retained TUN cleanup or egress resources")
            if not select.select([fixture_fd], [], [], 3)[0]:
                raise RuntimeError("Original native fixture process survived application quit")
            if not listeners_closed(state["ports"]):
                raise RuntimeError("Native listeners survived quit")
            if list(self.report.glob("fatal-*.json")):
                raise RuntimeError("Native observation contained a failure")
            for relative, expected in self.product.items():
                if hashlib.sha256((self.bundle / relative).read_bytes()).hexdigest() != expected:
                    raise RuntimeError("GUI bundle changed during acceptance")
            self.close()
            self.stop_panel()
            (self.report / "verification.json").write_text(json.dumps({
                "passed": True,
                "launches": self.launches, "steps": self.steps, "hosts": self.hosts,
                "product": self.product, "final": final, "fixture_pidfd_exit": True,
                "fixture_identity": fixture_identity,
                "entrypoint": "tool/connected_desktop_app.dart",
                "production_widgets_controller_and_default_native_library": True,
                "production_bootstrap_and_connect_account_tested": False,
                "login_success_tested": False, "tun_tested": False,
            }, ensure_ascii=False, indent=2) + "\n")
            print("REAL_CONNECTED_GUI_ORB_TRAY_RECONNECT_AND_QUIT_PASSED")
        except BaseException as error:
            if self.application is not None and self.application.poll() is None:
                try:
                    self.snapshot("failure")
                except Exception as snapshot_error:
                    print(f"Connected failure snapshot failed: {snapshot_error}", file=sys.stderr)
            (self.report / "verification.json").write_text(json.dumps({
                "passed": False, "error": str(error), "steps": self.steps,
                "launches": self.launches, "fixture_identity": fixture_identity,
            }, ensure_ascii=False, indent=2))
            raise
        finally:
            actions = [self.close, self.stop_panel]
            if fixture_identity is not None:
                actions.append(lambda: finish_fixture(fixture_fd))
            if fixture_fd is not None:
                actions.append(lambda: os.close(fixture_fd))
            gui.finish(actions)


if __name__ == "__main__":
    gui.main(gui.arguments(__doc__), session_type=ConnectedSession, entrypoint=__file__)
