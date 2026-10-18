"""Exercise production Windows widgets/controller and persistent native owner."""
import argparse
import json
import os
from pathlib import Path
import socket
import sys

from build_windows_desktop_probe import CONNECTED_ENTRYPOINT
from connected_desktop_acceptance import traffic_evidence
from windows_desktop_probe import RetainedProcess, file_hash
from windows_tray_acceptance import TrayAcceptance, validate_listener_cleanup, wait_for


def validate_native(record: dict, token: str, pid: int, profile: Path) -> None:
    if (record.get("token") != token or record.get("pid") != pid or
            record.get("platform") != "windows" or
            Path(record.get("profile", "")) != profile):
        raise RuntimeError("Connected native state belongs to another process/profile")


def validate_menu(menu: dict, status: str, connected: bool) -> None:
    rows = [row for row in menu["rows"] if not row["separator"]]
    if [row["label"] for row in rows] != [status, "显示窗口", "连接", "断开", "协议", "退出"]:
        raise RuntimeError("Connected GUI native menu labels differ")
    if (rows[0]["enabled"] or rows[2]["enabled"] is connected or
            rows[3]["enabled"] is not connected or not rows[5]["enabled"]):
        raise RuntimeError("Connected GUI native menu capabilities differ")
    protocols = rows[4]["children"]
    if ([row["label"] for row in protocols] != ["aTrust", "EasyConnect", "OpenVPN"] or
            [row["checked"] for row in protocols] != [True, False, False] or
            [row["enabled"] for row in protocols] != [not connected, False, False]):
        raise RuntimeError("Connected GUI native protocol states differ")


def validate_shutdown(final: dict, fixture_pid: int) -> None:
    if (final.get("round") != 3 or final.get("stage") != "shutdown" or
            final.get("fixture_pid") != fixture_pid or final.get("fixture_exit") != 0 or
            final.get("owner") != {"phase": "disconnected", "closed": True,
                                   "run_id": None, "cleanup_pending": False}):
        raise RuntimeError("Terminal native owner/fixture barrier is incomplete")
    session = final.get("session") or {}
    egress = session.get("egress") or {}
    if (session.get("state") != "disconnected" or
            session.get("upload", 0) < 262144 or session.get("download", 0) < 262172 or
            session.get("tun") != {"state": "stopped", "cleanup_pending": False} or
            egress.get("active") is not False or egress.get("pending_cleanup") is not False or
            egress.get("sockets") != []):
        raise RuntimeError("Terminal session traffic/resources are incomplete")


def validate_disconnected(record: dict) -> None:
    if (record.get("stage") != "disconnected" or record.get("owner") != {
            "phase": "disconnected", "closed": False, "run_id": None, "cleanup_pending": False}):
        raise RuntimeError("Ordinary disconnect did not preserve a reusable owner")
    session = record.get("session") or {}
    egress = session.get("egress") or {}
    if (session.get("state") != "disconnected" or
            session.get("tun") != {"state": "stopped", "cleanup_pending": False} or
            egress.get("active") is not False or egress.get("pending_cleanup") is not False or
            egress.get("sockets") != []):
        raise RuntimeError("Ordinary disconnect retained native session resources")


class ConnectedAcceptance(TrayAcceptance):
    entrypoint = CONNECTED_ENTRYPOINT

    def launch_environment(self) -> dict:
        return dict(os.environ, SHUVPN_GUI_ACCEPTANCE_TOKEN=self.token,
                    SHUVPN_CONNECTED_GUI_TOKEN=self.token,
                    SHUVPN_CONNECTED_WINDOWS_TOKEN=self.token,
                    SHUVPN_CONNECTED_GUI_REPORT=str(self.report),
                    SHUVPN_CONNECTED_GUI_FIXTURE=str(self.fixture_path))

    def native(self):
        if list(self.report.glob("fatal-*.json")):
            raise RuntimeError("Connected native observation failed")
        path = self.report / "native-state.json"
        if not path.exists():
            if self.application.poll() is not None:
                raise RuntimeError("Connected application exited without native state")
            return None
        record = json.loads(path.read_text(encoding="utf-8"))
        validate_native(record, self.token, self.probe.pid, self.report / "profile")
        return record

    def state(self, number: int, connected: bool) -> dict:
        def ready():
            record = self.native()
            if record is None or record["round"] != number:
                return None
            owner = record["owner"]
            if owner["closed"]:
                raise RuntimeError("Ordinary operation terminally closed the owner")
            if owner["cleanup_pending"]:
                return None
            if connected:
                session = record["session"]
                return record if (owner["phase"] == "connected" and owner["run_id"] is not None and
                                  session is not None and session["state"] == "connected") else None
            return record if owner["phase"] == "disconnected" and owner["run_id"] is None else None
        return wait_for(ready, f"native owner round={number} connected={connected}", 20)

    def status(self, connected: bool) -> dict:
        label = "已连接" if connected else "未连接"
        return {"status": f"{label} · aTrust · {self.token}"}

    def readiness(self) -> dict:
        started = wait_for(lambda: self.record("fixture-started"), "original fixture startup", 30)
        self.fixture = RetainedProcess(self.api, started["fixture_pid"])
        record = self.state(0, False)
        return {**record, **self.status(False)}

    def foreground_before_capture(self, ready: dict) -> None:
        # Activate through the real Explorer icon rather than relying on focus permission.
        self.click_icon(ready, "left")
        wait_for(self.visible, "real Explorer shows the initial connected app window")
        wait_for(lambda: self.api.user.GetForegroundWindow() == self.window,
                 "real Explorer activation gives the owned app foreground")
        self.result["accessibility_request"] = wait_for(
            lambda: self.api.enable_accessibility(self.window),
            "owned Flutter MSAA provider enables semantics")

    def disconnected(self, number: int) -> dict:
        self.state(number, False)
        path = self.report / f"native-{self.probe.pid}-disconnected-{number}.json"
        wait_for(path.exists, "stable native ordinary-disconnect barrier")
        record = json.loads(path.read_text(encoding="utf-8"))
        validate_native(record, self.token, self.probe.pid, self.report / "profile")
        if record["round"] != number or record["fixture_pid"] != self.fixture.pid:
            raise RuntimeError("Disconnect barrier belongs to another round/fixture")
        validate_disconnected(record)
        return record

    def orb(self, connected: bool) -> dict:
        label = "连接状态：已连接" if connected else "连接状态：未连接"
        return wait_for(lambda: self.api.control(self.window, label), f"owned UIA orb {label}", 20)

    def click_orb(self, connected: bool) -> None:
        self.api.user.SetForegroundWindow(self.window)
        wait_for(lambda: self.api.user.GetForegroundWindow() == self.window, "owned app foreground")
        target = self.orb(connected)
        x, y = target["clickable"]
        bounds = [x, y, x + 1, y + 1]
        self.api.move(bounds)
        current = self.orb(connected)
        if (current["name"] != target["name"] or current["pid"] != target["pid"] or
                current["rect"] != target["rect"] or current["clickable"] != target["clickable"] or
                current["control_type"] != 50000):
            raise RuntimeError("Flutter connection control moved before physical input")
        self.capture(f"orb-{len(self.steps)}", target["rect"])
        self.click(bounds, "left", self.probe.pid)
        self.steps.append({"physical_orb_click": target})

    def open_popup(self, ready: dict) -> dict:
        self.click_icon(ready, "right")
        menu = self.popup()
        validate_menu(menu, ready["status"], ready["status"].startswith("已连接"))
        self.steps.append({"physical_click": "own tray right", "native_menu": menu})
        self.capture(f"popup-{len(self.steps)}", menu["rect"])
        return menu

    def menu_state(self, connected: bool) -> None:
        self.open_popup(self.status(connected))
        self.api.keys([0x1B])
        wait_for(lambda: not any(self.api.user.IsWindowVisible(window)
                                for window in self.api.windows(self.probe.pid, "#32768")),
                 "own popup dismissed")

    def menu_click(self, label: str, connected: bool) -> None:
        menu = self.open_popup(self.status(connected))
        row = next(row for row in menu["rows"] if row["label"] == label)
        if not row["enabled"]:
            raise RuntimeError("Requested native menu command is disabled")
        self.click(self.api.menu_item_rect(self.window, menu["handle"], row["position"]),
                   "left", self.probe.pid)
        self.steps.append({"physical_menu_click": label})

    def listeners_owned(self, state: dict) -> None:
        observations = {}
        for kind, port in state["ports"].items():
            table = self.api.listeners(port)
            if table["matches"] != [{"pid": self.probe.pid, "address": "127.0.0.1",
                                      "port": port, "family": 2, "state": 2}]:
                raise RuntimeError("Connected listener is not uniquely owned by this GUI")
            observations[kind] = table
        self.steps.append({"round": state["round"], "listeners_owned": observations})

    def listeners_closed(self, state: dict) -> None:
        observations = {}
        for kind, port in state["ports"].items():
            table = wait_for(lambda: (snapshot if not (snapshot := self.api.listeners(port))["matches"]
                                     else None), "OS table shows the connected proxy listener closed")
            connection = "accepted"
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=0.3):
                    pass
            except ConnectionRefusedError:
                connection = "refused"
            except TimeoutError:
                connection = "timeout"
            validate_listener_cleanup(table, connection)
            observations[kind] = {"table": table, "connection": connection,
                                  "timeout_is_not_standalone_proof": True}
        self.steps.append({"round": state["round"], "listeners_closed": observations})

    def traffic(self, state: dict) -> None:
        self.listeners_owned(state)
        evidence = traffic_evidence(self.native, state)
        self.steps.append(evidence)

    def quit_menu(self) -> None:
        self.open_popup(self.status(True))
        if self.api.window_pid(self.api.user.GetForegroundWindow()) != self.probe.pid:
            raise RuntimeError("Connected popup lost foreground before physical exit")
        self.api.keys([0x26])
        def highlighted():
            menu = self.popup()
            return menu if any(row["label"] == "退出" and row["highlighted"]
                               for row in menu["rows"]) else None
        selected = wait_for(highlighted, "physical Up highlights owned Exit")
        self.steps.append({"keyboard_highlight": "退出", "native_menu": selected})
        self.api.keys([0x0D])
        self.steps.append({"physical_keys": "Up, Return", "selected": "退出"})

    def run(self) -> None:
        try:
            # The window probe's combined scope flag does not describe this entry.
            self.result.pop("production_main_or_account_or_controller_tested", None)
            self.result.update(production_widgets_controller_and_default_native_library=True,
                               production_controller_tested=True, production_main_tested=False,
                               native_private_account_settings_rules_tested=True,
                               production_bootstrap_and_connect_account_tested=False,
                               login_success_tested=False, windows_network_namespace=False)
            self.start()
            self.orb(False)
            self.menu_state(False)
            self.click_orb(False)
            first = self.state(1, True)
            self.orb(True)
            self.menu_state(True)
            self.traffic(first)
            self.capture("02-orb-connected", self.api.window_rect(self.window))
            self.click_orb(True)
            disconnected = self.disconnected(1)
            self.steps.append({"ordinary_disconnect": disconnected})
            self.listeners_closed(first)
            self.orb(False)
            self.menu_click("连接", False)
            second = self.state(2, True)
            self.orb(True)
            self.hide()
            self.traffic(second)
            self.click_icon(self.status(True), "left")
            wait_for(self.visible, "real Explorer left restores the connected application")
            self.capture("03-tray-reconnected", self.api.window_rect(self.window))
            self.menu_click("断开", True)
            disconnected = self.disconnected(2)
            self.steps.append({"ordinary_disconnect": disconnected})
            self.listeners_closed(second)
            self.orb(False)
            self.menu_click("连接", False)
            third = self.state(3, True)
            self.orb(True)
            self.traffic(third)
            if len({s["owner"]["run_id"] for s in (first, second, third)}) != 3:
                raise RuntimeError("Three connection rounds did not create distinct native runs")
            self.capture("04-tray-connected-again", self.api.window_rect(self.window))
            self.hide()
            self.quit_menu()
            wait_for(lambda: self.probe.exit_code() is not None, "original connected GUI exits", 20)
            self.probe.require_successful_exit()
            if self.application.wait(timeout=3) != 0:
                raise RuntimeError("Connected GUI did not exit normally")
            final = json.loads((self.report / f"native-{self.probe.pid}-shutdown-3.json").read_text(encoding="utf-8"))
            validate_native(final, self.token, self.probe.pid, self.report / "profile")
            validate_shutdown(final, self.fixture.pid)
            wait_for(lambda: self.fixture.exit_code() is not None, "original connected fixture exits")
            self.fixture.require_successful_exit()
            self.listeners_closed(third)
            wait_for(lambda: self.api.icon_rect(self.window) is None, "connected tray icon removed")
            paths = sorted(self.report.glob("command-*.json"), key=lambda p: int(p.stem.split("-")[1]))
            commands = [json.loads(path.read_text(encoding="utf-8")) for path in paths]
            for index, command in enumerate(commands):
                if (command.get("token") != self.token or command.get("pid") != self.probe.pid or
                        command.get("sequence") != index):
                    raise RuntimeError("Connected command belongs to another process/sequence")
            if [c["command"] for c in commands] != ["show", "connect", "show", "disconnect", "connect", "quit"]:
                raise RuntimeError("Physical connected command sequence differs")
            if list(self.report.glob("fatal-*.json")) or (self.report / "abort.json").exists():
                raise RuntimeError("Connected acceptance contains a failure/abort")
            if any(file_hash(self.bundle / name) != value for name, value in self.products.items()):
                raise RuntimeError("Connected bundle changed during the run")
            self.result.update(passed=True, final=final, commands=commands, probe_exit=0,
                               fixture_exit=0, shell_icon_removed=True,
                               distinct_run_ids=[s["owner"]["run_id"] for s in (first, second, third)])
            print("WINDOWS_PRODUCTION_WIDGETS_OWNER_THREE_ROUND_PROXY_GUI_PASSED")
        except BaseException as error:
            self.result["error"] = repr(error)
            raise
        finally:
            self.finish()


def main() -> None:
    if not __debug__ or sys.platform != "win32":
        raise RuntimeError("Use ordinary Windows Python with assertions enabled")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    ConnectedAcceptance(parser.parse_args()).run()


if __name__ == "__main__":
    main()
