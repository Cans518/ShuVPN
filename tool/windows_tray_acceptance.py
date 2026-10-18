"""Verify actual Explorer pointer/menu input against the private window probe."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid
import socket

from build_windows_desktop_probe import PROBE_ENTRYPOINT, PROBE_PRODUCTS
from connected_desktop_acceptance import echo
from windows_desktop_probe import RetainedProcess, WindowsDesktop, file_hash

ROOT = Path(__file__).resolve().parent.parent


def validate_probe_marker(marker: dict, products: dict, entrypoint: str = PROBE_ENTRYPOINT) -> None:
    if (marker.get("schema") != 1 or marker.get("entrypoint") != entrypoint or
            marker.get("products") != products):
        raise RuntimeError("Bundle is not the classified developer window probe")


def validate_record(record: dict, token: str, pid: int, stage: str) -> None:
    if record.get("token") != token or record.get("pid") != pid or record.get("stage") != stage:
        raise RuntimeError("Window observation belongs to another process/token/stage")


def validate_ready(record: dict, token: str, pid: int) -> None:
    validate_record(record, token, pid, "ready")
    if (type(record.get("proxy_port")) is not int or not 0 < record["proxy_port"] < 65536
            or type(record.get("fixture_pid")) is not int or record["fixture_pid"] <= 0
            or record["fixture_pid"] == pid or record.get("tray_ready") is not True
            or record.get("status") != f"已连接 · {token}"):
        raise RuntimeError("Native window readiness is incomplete")


def validate_cleanup(record: dict, token: str, pid: int, ready: dict) -> None:
    validate_record(record, token, pid, "cleanup")
    if (record.get("fixture_pid") != ready["fixture_pid"] or record.get("fixture_exit") != 0
            or record.get("proxy_port") != ready["proxy_port"] or
            record.get("session_state") != "disconnected" or record.get("proxy_closed") is not True
            or record.get("upload", 0) < 131072 or record.get("download", 0) < 131086):
        raise RuntimeError("Window probe did not prove complete native traffic and cleanup")


def validate_commands(events: list[dict], token: str, pid: int) -> None:
    for sequence, event in enumerate(events):
        validate_record(event, token, pid, f"event-{sequence}")
    commands = [event["name"] for event in events if event["name"].startswith("native-command:")]
    if commands != ["native-command:show", "native-command:show", "native-command:quit"]:
        raise RuntimeError(f"Native input command sequence did not match the physical scenario: {commands}")


def validate_listener_cleanup(snapshot: dict, connection: str) -> None:
    tables = snapshot.get("tables", [])
    if (snapshot.get("api") != "GetExtendedTcpTable:TCP_TABLE_OWNER_PID_LISTENER" or
            len(tables) != 2 or {table.get("family") for table in tables} != {2, 23} or
            any(type(table.get("count")) is not int or table["count"] < 0 for table in tables) or
            snapshot.get("matches") != [] or
            connection not in ("refused", "timeout")):
        raise RuntimeError("Original listener cleanup lacks independent OS table evidence")


def validate_menu(menu: dict, status: str) -> None:
    rows = [row for row in menu["rows"] if not row["separator"]]
    if [row["label"] for row in rows] != [status, "显示窗口", "连接", "断开", "协议", "退出"]:
        raise RuntimeError("Unexpected native popup rows")
    if rows[0]["enabled"] or rows[2]["enabled"] or not rows[3]["enabled"]:
        raise RuntimeError("Native connected capabilities are incorrect")
    protocols = rows[4]["children"]
    if [row["label"] for row in protocols] != ["aTrust", "EasyConnect", "OpenVPN"]:
        raise RuntimeError("Unexpected native protocol submenu")
    if any(row["enabled"] for row in protocols) or [row["checked"] for row in protocols] != [True, False, False]:
        raise RuntimeError("Native protocol enabled/check states are incorrect")


def wait_for(predicate, description: str, seconds: float = 10):
    deadline, last_error = time.monotonic() + seconds, None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except OSError as error:
            last_error = error
        time.sleep(0.05)
    raise RuntimeError(f"Timed out: {description}; last OS error: {last_error}")


class TrayAcceptance:
    entrypoint = PROBE_ENTRYPOINT

    def __init__(self, args):
        self.bundle, self.fixture_path = args.bundle.resolve(strict=True), args.fixture.resolve(strict=True)
        self.report = args.report.resolve()
        boundary = (ROOT / "build/desktop").resolve()
        if not self.report.is_relative_to(boundary) or self.report == boundary:
            raise RuntimeError("Window report must be a new directory inside build/desktop")
        self.products = {name: file_hash(self.bundle / name) for name in PROBE_PRODUCTS}
        marker = self.bundle / "window-probe.json"
        if not marker.is_file():
            raise RuntimeError("Build the classified developer probe before starting GUI acceptance")
        self.marker = json.loads(marker.read_text(encoding="utf-8"))
        validate_probe_marker(self.marker, self.products, self.entrypoint)
        self.report.mkdir()
        if self.report.resolve() != self.report:
            raise RuntimeError("Window report must not traverse a reparse point")
        self.token = uuid.uuid4().hex
        self.api = WindowsDesktop()
        self.api.prepare_input()
        self.application = self.probe = self.fixture = self.explorer = None
        self.log = None
        self.window = None
        self.steps = []
        self.result = {"passed": False, "token": self.token,
                       "entrypoint": self.entrypoint, "probe_marker": self.marker,
                       "production_main_or_account_or_controller_tested": False,
                       "tun_or_system_proxy_tested": False}
        self.input_lock = self.api.acquire_input()

    def write(self, name: str, value) -> None:
        (self.report / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n",
                                       encoding="utf-8")

    def capture(self, name: str, bounds: list[int]) -> None:
        from PIL import ImageGrab
        ImageGrab.grab(bbox=tuple(bounds), all_screens=True).save(self.report / f"{name}.png")

    def record(self, stage: str):
        failure = self.report / "failure.json"
        if failure.exists():
            raise RuntimeError(f"Probe reported a failure: {failure}")
        path = self.report / f"{stage}.json"
        if not path.exists():
            if self.application.poll() is not None:
                raise RuntimeError(f"Probe exited before {stage}: {self.application.returncode}")
            return None
        result = json.loads(path.read_text(encoding="utf-8"))
        validate_record(result, self.token, self.application.pid, stage)
        return result

    def main_window(self):
        windows = self.api.windows(self.application.pid, "FLUTTER_RUNNER_WIN32_WINDOW")
        if len(windows) > 1:
            raise RuntimeError("Multiple probe main windows")
        return windows[0] if windows else None

    def launch_environment(self) -> dict:
        return dict(os.environ, SHUVPN_WINDOW_ACCEPTANCE_TOKEN=self.token,
                    SHUVPN_WINDOW_ACCEPTANCE_REPORT=str(self.report),
                    SHUVPN_WINDOW_ACCEPTANCE_FIXTURE=str(self.fixture_path))

    def readiness(self) -> dict:
        ready = wait_for(lambda: self.record("ready"), "native probe ready", 30)
        validate_ready(ready, self.token, self.probe.pid)
        return ready

    def foreground_before_capture(self, ready: dict) -> None:
        wait_for(self.visible, "owned main window becomes visible")
        self.api.user.SetForegroundWindow(self.window)
        wait_for(lambda: self.api.user.GetForegroundWindow() == self.window,
                 "owned main window foreground before its screenshot")

    def start(self) -> dict:
        environment = self.launch_environment()
        self.log = (self.report / "application.log").open("wb")
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        self.application = subprocess.Popen([str(self.bundle / "shuvpn.exe")], cwd=ROOT,
                                            env=environment, stdin=subprocess.DEVNULL,
                                            stdout=self.log, stderr=subprocess.STDOUT,
                                            startupinfo=startup)
        self.probe = RetainedProcess(self.api, self.application.pid)
        if self.probe.identity["created_ticks"] != self.api.created_ticks(int(self.application._handle)):
            raise RuntimeError("Retained probe differs from the original CreateProcess handle")
        if Path(self.probe.identity["image"]) != self.bundle / "shuvpn.exe" or self.probe.identity["elevated"]:
            raise RuntimeError("Unexpected or elevated probe executable")
        ready = self.readiness()
        if self.fixture is None:
            self.fixture = RetainedProcess(self.api, ready["fixture_pid"])
        elif self.fixture.pid != ready["fixture_pid"]:
            raise RuntimeError("Fixture changed between startup and readiness")
        if (Path(self.fixture.identity["image"]) != self.fixture_path or self.fixture.identity["elevated"]
                or self.api.parent_pid(self.fixture.pid) != self.probe.pid
                or self.fixture.identity["session"] != self.probe.identity["session"]):
            raise RuntimeError("Fixture identity/parent/session is incorrect")
        self.window = wait_for(self.main_window, "retained main HWND")
        if self.api.window_title(self.window) != "ShuVPN":
            raise RuntimeError("Owned probe has an unexpected title")
        mapped = self.probe.modules().get("shuvpn_core.dll")
        if mapped is None or Path(mapped) != self.bundle / "shuvpn_core.dll":
            raise RuntimeError("Probe did not load its paired default core")
        taskbar = self.api.user.FindWindowW("Shell_TrayWnd", None)
        if not taskbar:
            raise RuntimeError("No actual Explorer taskbar in the interactive session")
        self.explorer = RetainedProcess(self.api, self.api.window_pid(taskbar))
        if (Path(self.explorer.identity["image"]) != Path(os.environ["SystemRoot"]) / "explorer.exe"
                or self.explorer.identity["session"] != self.probe.identity["session"]):
            raise RuntimeError("Taskbar is not the actual Explorer in the probe session")
        self.taskbar = taskbar
        self.result.update(probe=self.probe.identity, fixture=self.fixture.identity,
                           explorer=self.explorer.identity, main_window=self.window,
                           paired_core=mapped, products=self.products, ready=ready)
        self.foreground_before_capture(ready)
        self.capture("01-probe-visible", self.api.window_rect(self.window))
        return ready

    def visible(self) -> bool:
        if self.probe.exit_code() is not None or self.api.window_pid(self.window) != self.probe.pid:
            raise RuntimeError("Original probe/main window no longer exists")
        return bool(self.api.user.IsWindowVisible(self.window))

    def hide(self) -> None:
        self.api.user.SetForegroundWindow(self.window)
        wait_for(lambda: self.api.user.GetForegroundWindow() == self.window, "probe foreground before close")
        self.api.keys([0x12, 0x73])
        wait_for(lambda: not self.visible(), "physical Alt+F4 hides the same window")
        self.steps.append({"physical_keys": "Alt+F4", "hidden": True, "window": self.window})

    def click(self, bounds: list[int], button: str, pid: int) -> None:
        from ctypes import wintypes
        point = wintypes.POINT((bounds[0] + bounds[2]) // 2, (bounds[1] + bounds[3]) // 2)
        window = self.api.user.WindowFromPoint(point)
        if self.api.window_pid(window) != pid:
            raise RuntimeError("Physical click target is no longer owned")
        self.api.click(bounds, button)
        self.steps.append({"physical_pointer": self.api.last_pointer, "button": button, "target_pid": pid})

    def icon(self, ready: dict):
        expected = f"ShuVPN · {ready['status']}"
        def owned():
            bounds = self.api.icon_rect(self.window)
            if bounds and bounds[2] > bounds[0] and bounds[3] > bounds[1]:
                result = self.api.find_icon(self.taskbar, self.explorer.pid, expected)
                self.write("latest-icon-observation.json", {"shell_rect": bounds, "target": result})
                if result is not None:
                    result["shell_rect"] = bounds
                return result
            self.write("latest-icon-observation.json", {"rect": bounds, "window": self.window})
            return None
        result = owned()
        if result is None:
            overflow = self.api.overflow_button(self.taskbar)
            if overflow is None:
                raise RuntimeError("Owned icon is not visible and no verified Explorer overflow control exists")
            self.click(overflow["rect"], "left", self.explorer.pid)
            self.steps.append({"physical_click": "Explorer overflow", **overflow})
            result = wait_for(owned, "own Shell_NotifyIconGetRect target in Explorer overflow")
        self.steps.append({"icon_identifier": {"window": self.window, "id": 0}, **result})
        return result

    def click_icon(self, ready: dict, button: str) -> None:
        self.icon(ready)
        previous, stable_since = None, time.monotonic()
        def stable_target():
            nonlocal previous, stable_since
            target = self.api.find_icon(self.taskbar, self.explorer.pid, f"ShuVPN · {ready['status']}")
            if target is None:
                return None
            key = (tuple(target["uia_rect"]), tuple(target["clickable"]))
            if key != previous:
                previous, stable_since = key, time.monotonic()
            return target if time.monotonic() - stable_since >= 0.2 else None
        target = wait_for(stable_target, "own Explorer icon layout settles")
        self.capture(f"icon-{len(self.steps)}", target["uia_rect"])
        x, y = target["clickable"]
        bounds = [x, y, x + 1, y + 1]
        self.api.move(bounds)
        observed = self.api.own_icon_at(bounds, f"ShuVPN · {ready['status']}", self.explorer.pid)
        self.write(f"before-{button}.json", self.api.last_icon_observation)
        if observed is None:
            raise RuntimeError("Own icon changed after moving the physical pointer")
        self.steps.append({"before_physical_icon": observed, "pointer": self.api.last_pointer,
                           "tick": self.api.kernel.GetTickCount64(), "button": button})
        self.api.press(button)

    def popup(self):
        def owned():
            menus = [window for window in self.api.windows(self.probe.pid, "#32768")
                     if self.api.user.IsWindowVisible(window)]
            roots = []
            for window in menus:
                menu = self.api.menu(window, self.window)
                if any(row["label"] == "显示窗口" for row in menu["rows"]):
                    roots.append(menu)
            if len(roots) > 1:
                raise RuntimeError("Multiple owned root popup menus")
            return roots[0] if roots else None
        return wait_for(owned, "actual native popup with main-HWND menu ownership")

    def open_popup(self, ready: dict) -> dict:
        self.click_icon(ready, "right")
        menu = self.popup()
        validate_menu(menu, ready["status"])
        self.steps.append({"physical_click": "own tray right", "native_menu": menu})
        self.capture(f"popup-{len(self.steps)}", menu["rect"])
        return menu

    def submenu(self, menu: dict):
        protocol = next(row for row in menu["rows"] if row["label"] == "协议")
        bounds = self.api.menu_item_rect(self.window, menu["handle"], protocol["position"])
        self.api.move(bounds)
        def visible():
            for window in self.api.windows(self.probe.pid, "#32768"):
                if self.api.user.IsWindowVisible(window):
                    candidate = self.api.menu(window, self.window)
                    if candidate["handle"] == protocol["submenu"]:
                        return candidate
            return None
        opened = wait_for(visible, "physical hover opens the owned protocol submenu")
        self.capture("protocol-submenu", opened["rect"])
        self.steps.append({"physical_hover": "协议", "native_submenu": opened})

    def run(self) -> None:
        try:
            ready = self.start()
            before = self.api.listeners(ready["proxy_port"])
            if before["matches"] != [{"pid": self.probe.pid, "address": "127.0.0.1",
                                      "port": ready["proxy_port"], "family": 2, "state": 2}]:
                raise RuntimeError("Real HTTP listening socket is not uniquely owned by the probe")
            first = echo(ready["proxy_port"], "http", "echo.internal.invalid")
            self.hide()
            second = echo(ready["proxy_port"], "http", "echo.internal.invalid")
            self.steps.append({"native_flows": [first, second], "hidden_flow_completed": True})
            self.click_icon(ready, "left")
            wait_for(self.visible, "physical tray left click restores the same HWND")
            self.steps.append({"physical_click": "own tray left", "visible": True})
            self.hide()
            menu = self.open_popup(ready)
            self.submenu(menu)
            show = next(row for row in menu["rows"] if row["label"] == "显示窗口")
            bounds = self.api.menu_item_rect(self.window, menu["handle"], show["position"])
            self.click(bounds, "left", self.probe.pid)
            wait_for(self.visible, "physical popup click restores the same HWND")
            self.steps.append({"physical_click": "显示窗口", "visible": True})
            self.hide()
            self.open_popup(ready)
            if self.api.window_pid(self.api.user.GetForegroundWindow()) != self.probe.pid:
                raise RuntimeError("Probe popup lost foreground before keyboard selection")
            self.api.keys([0x26])
            def selected_exit():
                current = self.popup()
                self.write("keyboard-menu.json", current)
                return current if any(row["label"] == "退出" and row["highlighted"]
                                      for row in current["rows"]) else None
            selected = wait_for(selected_exit, "physical Up highlights the actual Exit row")
            self.steps.append({"keyboard_highlight": "退出", "native_menu": selected})
            self.api.keys([0x0D])
            self.steps.append({"physical_keys": "Up, Return", "selected": "退出"})
            wait_for(lambda: self.probe.exit_code() is not None, "original GUI handle exits")
            self.probe.require_successful_exit()
            if self.application.wait(timeout=3) != 0:
                raise RuntimeError("Original GUI did not exit normally")
            final = self.record("cleanup")
            if final is None:
                raise RuntimeError("Probe exit lacked the native cleanup barrier")
            validate_cleanup(final, self.token, self.probe.pid, ready)
            event_files = sorted(self.report.glob("event-*.json"),
                                 key=lambda path: int(path.stem.split("-")[1]))
            events = [json.loads(path.read_text(encoding="utf-8")) for path in event_files]
            validate_commands(events, self.token, self.probe.pid)
            wait_for(lambda: self.fixture.exit_code() is not None, "original fixture handle exits")
            self.fixture.require_successful_exit()
            after = wait_for(
                lambda: (snapshot if not (snapshot := self.api.listeners(ready["proxy_port"]))["matches"]
                         else None), "OS TCP listener table confirms the port is gone")
            connection = "accepted"
            try:
                with socket.create_connection(("127.0.0.1", ready["proxy_port"]), timeout=1):
                    pass
            except ConnectionRefusedError:
                connection = "refused"
            except TimeoutError:
                connection = "timeout"
            validate_listener_cleanup(after, connection)
            self.result["listener_cleanup"] = {"before": before, "after": after,
                                               "connection_attempt": connection,
                                               "timeout_is_not_standalone_proof": True}
            wait_for(lambda: self.api.icon_rect(self.window) is None, "owned tray icon removed")
            if (self.report / "failure.json").exists():
                raise RuntimeError("Probe contained a failure report")
            if any(file_hash(self.bundle / name) != digest for name, digest in self.products.items()):
                raise RuntimeError("Probe bundle changed during the run")
            self.result.update(passed=True, final=final, probe_exit=0, fixture_exit=0,
                               shell_icon_removed=True, physical_protocol_submenu_tested=True,
                               native_events=events)
            print("WINDOWS_REAL_EXPLORER_TRAY_POINTER_POPUP_SUBMENU_AND_QUIT_PASSED")
        except BaseException as error:
            self.result["error"] = repr(error)
            raise
        finally:
            self.finish()

    def finish(self) -> None:
        cleanup_error = None
        try:
            if self.application is not None and self.application.poll() is None:
                if (self.probe is not None and
                        self.api.window_pid(self.api.user.GetForegroundWindow()) == self.probe.pid and
                        any(self.api.user.IsWindowVisible(window)
                            for window in self.api.windows(self.probe.pid, "#32768"))):
                    self.api.keys([0x1B])
                self.write("abort.json", {"token": self.token})
                try:
                    self.application.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    self.application.terminate()
                    self.application.wait(timeout=5)
                    cleanup_error = "Original probe required forced termination"
            if self.fixture is not None:
                wait_for(lambda: self.fixture.exit_code() is not None, "failure cleanup original fixture", 5)
            self.api.user.SetCursorPos(self.api.cursor.x, self.api.cursor.y)
        except (OSError, RuntimeError, subprocess.SubprocessError) as error:
            cleanup_error = repr(error)
        finally:
            if self.log is not None:
                self.log.close()
            self.result["steps"] = self.steps
            self.result["cleanup_error"] = cleanup_error
            if cleanup_error is not None:
                self.result["passed"] = False
            self.write("verification.json", self.result)
            for process in (self.probe, self.fixture, self.explorer):
                if process is not None:
                    process.close()
            self.api.kernel.CloseHandle(self.input_lock)
        if cleanup_error is not None:
            raise RuntimeError(cleanup_error)


def main() -> None:
    if not __debug__:
        raise RuntimeError("Run the Windows GUI verifier with assertions enabled")
    if sys.platform != "win32":
        raise RuntimeError("The Explorer verifier requires Windows")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    TrayAcceptance(parser.parse_args()).run()


if __name__ == "__main__":
    main()
