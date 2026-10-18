"""Exercise real desktop controls on an owned private X11/DBus display."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parent.parent
INVALID_RULE_DOCUMENT = '{"name":"invalid-gui","rules":["UNSUPPORTED,example,DIRECT"]}'


def validate_rule_rejection(before, after, observation):
    if not before or before != after:
        raise RuntimeError("Invalid rule input changed the committed native record")
    target = observation.get("target") or {}
    attempt = observation.get("attempt") or {}
    elapsed = observation.get("observed_seconds")
    digest = hashlib.sha256(before).hexdigest()
    if (observation.get("input") != INVALID_RULE_DOCUMENT or
            observation.get("error") != "规则类型尚未支持" or
            observation.get("error_showing") is not True or
            target.get("name", "").split("\n")[0] != "应用预览" or
            target.get("role") != "push button" or type(target.get("pid")) is not int or
            target["pid"] <= 0 or type(elapsed) not in (int, float) or
            not math.isfinite(elapsed) or elapsed < 0.5 or
            type(attempt.get("actions")) is not int or attempt["actions"] < 0 or
            "result" not in attempt or
            (attempt["actions"] == 0 and attempt.get("result") is not None) or
            (attempt["actions"] > 0 and type(attempt.get("result")) is not bool) or
            observation.get("before_sha256") != digest or
            observation.get("after_sha256") != digest):
        raise RuntimeError("Invalid rule rejection lacks visible native error and owned action evidence")

def validate_session(environment, token, uid):
    if uid == 0:
        raise RuntimeError("Run the GUI acceptance as an ordinary user")
    if not token or token != environment.get("SHUVPN_GUI_ACCEPTANCE_TOKEN"):
        raise RuntimeError("Missing private GUI session token")
    for current, parent in (("DISPLAY", "SHUVPN_GUI_PARENT_DISPLAY"),
                            ("DBUS_SESSION_BUS_ADDRESS", "SHUVPN_GUI_PARENT_DBUS")):
        if not environment.get(current) or environment[current] == environment.get(parent):
            raise RuntimeError(f"GUI session must own a private {current}")


def command(*args):
    return subprocess.check_output(list(map(str, args)), text=True).strip()


def wait_for(predicate, description, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.05)
    raise RuntimeError(f"Timed out: {description}")

def stop_process(process):
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=3)


def finish(actions):
    primary = sys.exc_info()[1]
    failures = []
    for action in actions:
        try:
            action()
        except (OSError, RuntimeError, subprocess.SubprocessError) as failure:
            failures.append(failure)
            print(f"GUI cleanup failed: {failure}", file=sys.stderr)
    if failures and primary is None:
        raise failures[0]


def capture(path):
    import gi
    gi.require_version("Gdk", "3.0")
    from gi.repository import Gdk
    Gdk.init([])
    window = Gdk.get_default_root_window()
    image = Gdk.pixbuf_get_from_window(window, 0, 0, window.get_width(), window.get_height())
    if image is None:
        raise RuntimeError("Could not capture private X11 display")
    image.savev(str(path), "png", [], [])

def pixel(x, y):
    import gi
    gi.require_version("Gdk", "3.0")
    from gi.repository import Gdk
    image = Gdk.pixbuf_get_from_window(Gdk.get_default_root_window(), x, y, 1, 1)
    if image is None:
        raise RuntimeError("Cannot inspect theme indicator")
    return list(image.get_pixels())[:3]

def accessible_subtree(root):
    rows = []
    def visit(node, depth):
        # A transition can remove a child after AT-SPI reports its count.
        if node is None or depth > 24 or len(rows) >= 500:
            return
        rows.append(node)
        for index in range(node.get_child_count()):
            visit(node.get_child_at_index(index), depth + 1)
    visit(root, 0)
    return rows


def accessible_nodes(pid):
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
    from gi.repository import GLib
    context = GLib.MainContext.default()
    for _ in range(512):
        if not context.pending():
            break
        context.iteration(False)
    desktop = Atspi.get_desktop(0)
    rows = []
    for index in range(desktop.get_child_count()):
        application = desktop.get_child_at_index(index)
        if application is not None and application.get_process_id() == pid:
            rows.extend(accessible_subtree(application))
    return rows


def accessibility(pid):
    return [{"name": node.get_name(), "role": node.get_role_name()}
            for node in accessible_nodes(pid)]


class Session:
    def __init__(self, args):
        self.args = args
        self.report = args.report.resolve()
        self.bundle = args.bundle.resolve()
        self.application = None
        self.window = None
        self.launches = []
        self.steps = []
        self.log = None
        self.product = {relative: hashlib.sha256((self.bundle / relative).read_bytes()).hexdigest()
                        for relative in ("shuvpn", "lib/libshuvpn_core.so", "lib/libapp.so")}

    def snapshot(self, name):
        time.sleep(0.35)
        path = self.report / f"{name}.png"
        capture(path)
        (self.report / f"{name}-accessibility.json").write_text(
            json.dumps(accessibility(self.application.pid), ensure_ascii=False, indent=2) + "\n")
        self.steps.append({"name": name, "screenshot": path.name})

    def start(self, name):
        self.log = (self.report / f"{name}.log").open("wb")
        self.application = subprocess.Popen([str(self.bundle / "shuvpn")],
                                            stdout=self.log, stderr=subprocess.STDOUT)
        def visible():
            if self.application.poll() is not None:
                raise RuntimeError(f"Application exited during startup: {self.application.returncode}")
            found = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid",
                                    str(self.application.pid)], capture_output=True, text=True)
            return found.stdout.splitlines()[0] if found.returncode == 0 else None
        self.window = wait_for(visible, "owned application window")
        if command("xdotool", "getwindowname", self.window) != "ShuVPN":
            raise RuntimeError("Unexpected owned window title")
        if Path(f"/proc/{self.application.pid}/exe").resolve() != self.bundle / "shuvpn":
            raise RuntimeError("Unexpected application executable")
        status = dict(row.split(":", 1) for row in
                      Path(f"/proc/{self.application.pid}/status").read_text().splitlines())
        if list(map(int, status["Uid"].split())) != [os.geteuid()] * 4:
            raise RuntimeError("GUI process changed user identity")
        if int(status["CapEff"].strip(), 16) != 0:
            raise RuntimeError("GUI process has effective capabilities")
        command("xdotool", "windowactivate", "--sync", self.window)
        command("xdotool", "windowsize", "--sync", self.window, "1120", "960")
        command("xdotool", "windowmove", "--sync", self.window, "120", "50")
        library = str(self.bundle / "lib/libshuvpn_core.so")
        mappings = wait_for(
            lambda: library if any(row.split()[-1:] == [library] for row in
                                   Path(f"/proc/{self.application.pid}/maps").read_text().splitlines()) else None,
            "paired native library")
        self.launches.append({"name": name, "pid": self.application.pid,
                              "window": self.window, "library": mappings,
                              "uid": os.geteuid(), "cap_eff": status["CapEff"].strip()})
        self.snapshot(name)

    def click(self, x, y):
        if self.application.poll() is not None:
            raise RuntimeError("Application exited before UI action")
        command("xdotool", "windowactivate", "--sync", self.window)
        command("xdotool", "mousemove", "--window", self.window, x, y, "click", "1")
        time.sleep(0.35)

    def control_node(self, label, *, roles=None):
        if roles is None:
            roles = ("panel",) if label == "导入与高级编辑" else (
                "push button", "text", "password text")
        def find():
            matches = [node for node in accessible_nodes(self.application.pid)
                       if node.get_name().split("\n")[0] == label and
                       node.get_role_name() in roles]
            return matches[0] if len(matches) == 1 else None
        return wait_for(find, f"unique accessible control {label}")

    def control(self, label):
        node = self.control_node(label)
        if node.get_n_actions() < 1:
            raise RuntimeError(f"Control has no platform action: {label}")
        command("xdotool", "windowactivate", "--sync", self.window)
        action = node.get_action_name(0)
        if not node.do_action(0):
            raise RuntimeError(f"Platform action failed: {label}")
        self.steps.append({"control": label, "role": node.get_role_name(),
                           "platform_action": action})
        time.sleep(0.35)

    def editor_node(self, expected):
        def find():
            matches = []
            for node in accessible_nodes(self.application.pid):
                if node.get_role_name() != "text":
                    continue
                text = node.get_text_iface()
                if text is not None and text.get_text(0, text.get_character_count()) == expected:
                    matches.append(node)
            return matches[0] if len(matches) == 1 else None
        return wait_for(find, "unique raw editor with the expected exact text")

    def node_observation(self, node):
        import gi
        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi
        state = node.get_state_set()
        return {"name": node.get_name(), "role": node.get_role_name(),
                "pid": node.get_process_id(),
                "enabled": state.contains(Atspi.StateType.ENABLED),
                "showing": state.contains(Atspi.StateType.SHOWING)}

    def activate_editor(self, node):
        observation = self.node_observation(node)
        if observation["pid"] != self.application.pid or node.get_n_actions() < 1:
            raise RuntimeError("Owned raw editor exposes no focus action")
        command("xdotool", "windowactivate", "--sync", self.window)
        if not node.do_action(0):
            raise RuntimeError("Raw editor platform focus action failed")
        self.steps.append({"platform_editor_focus": observation})
        time.sleep(0.35)

    def text(self, value):
        fields = [node for node in accessible_nodes(self.application.pid)
                  if node.get_role_name() == "text"]
        if len(fields) != 1 or fields[0].get_name().split("\n")[0] != "端口":
            raise RuntimeError("Expected one editable platform text control")
        self.click(560, 495)
        self.key("ctrl+a")
        command("xdotool", "type", "--clearmodifiers", "--delay", "1", value)
        time.sleep(0.2)

    def key(self, keys):
        command("xdotool", "windowactivate", "--sync", self.window)
        command("xdotool", "key", "--clearmodifiers", keys)
        time.sleep(0.2)

    def scroll(self, count):
        command("xdotool", "mousemove", "--window", self.window, 750, 500,
                "click", "--repeat", abs(count), "--delay", 70, "5" if count > 0 else "4")
        time.sleep(0.35)

    def displayed_value(self, label, value):
        wait_for(lambda: any(node["name"].split("\n")[0] == label and
                             node["name"].split("\n")[-1] == value
                             for node in accessibility(self.application.pid)),
                 f"GUI displays {label}={value}")
        self.steps.append({"displayed": label, "value": value})

    def dark_indicator(self):
        values = dict(line.split("=", 1) for line in
                      command("xdotool", "getwindowgeometry", "--shell", self.window).splitlines())
        x, y = int(values["X"]), int(values["Y"])
        assert int(values["WIDTH"]) == 1120 and int(values["HEIGHT"]) == 960
        actual = wait_for(
            lambda: pixel(x + 44, y + 265) if pixel(x + 44, y + 265) == [65, 174, 242] else None,
            "rendered dark selection indicator")
        self.steps.append({"dark_selection_indicator_rgb": actual})

    def stop(self, method="alt+F4"):
        self.key(method)
        try:
            status = self.application.wait(timeout=10)
        except subprocess.TimeoutExpired:
            raise RuntimeError("Application did not exit normally")
        if status != 0:
            raise RuntimeError(f"Application exit was {status}")
        self.launches[-1]["exit"] = status
        self.application = None
        self.log.close()

    def state_file(self):
        files = list((self.report / "profile/data").rglob("settings-state.json"))
        return files[0] if len(files) == 1 else None

    def routing_file(self):
        paths = list((self.report / "profile/data").rglob("routing-state.json"))
        if len(paths) != 1:
            raise RuntimeError("Expected one private native routing record")
        return paths[0]

    def reject_invalid_rule(self, expected_version):
        before = self.routing_file().read_bytes()
        current = json.loads(before)
        if current["version"] != expected_version:
            raise RuntimeError("Rule rejection did not start from the just-committed version")
        self.control("导入与高级编辑")
        self.snapshot("21a-raw-rule-editor")
        self.activate_editor(self.editor_node(current["current"]["profile"]))
        self.key("ctrl+a")
        command("xdotool", "type", "--clearmodifiers", "--delay", "1", INVALID_RULE_DOCUMENT)
        self.editor_node(INVALID_RULE_DOCUMENT)
        self.steps.append({"physical_rule_input": INVALID_RULE_DOCUMENT})
        self.control("校验并预览")
        error_node = self.control_node("规则类型尚未支持", roles=("panel",))
        self.snapshot("21-invalid-rule-rejected")
        apply = self.control_node("应用预览")
        target = self.node_observation(apply)
        if target["pid"] != self.application.pid:
            raise RuntimeError("Apply observation belongs to another process")
        actions = apply.get_n_actions()
        result = apply.do_action(0) if actions > 0 else None
        started = time.monotonic()
        time.sleep(0.5)
        error_node = self.control_node("规则类型尚未支持", roles=("panel",))
        error = self.node_observation(error_node)
        after = self.routing_file().read_bytes()
        observation = {"input": INVALID_RULE_DOCUMENT, "error": error["name"],
                       "error_showing": error["showing"], "error_node": error,
                       "target": target, "observed_seconds": time.monotonic() - started,
                       "attempt": {"actions": actions, "result": result},
                       "before_sha256": hashlib.sha256(before).hexdigest(),
                       "after_sha256": hashlib.sha256(after).hexdigest()}
        validate_rule_rejection(before, after, observation)
        self.steps.append({"invalid_rule_rejected": observation,
                           "before": current, "after": json.loads(after)})
        self.snapshot("22-invalid-rule-apply-click")

    def state(self):
        return json.loads(wait_for(self.state_file, "native settings record").read_text())

    def value(self, key, expected):
        state = wait_for(lambda: self.state() if self.state()["values"].get(key) == expected else None,
                         f"GUI committed {key}={expected}")
        self.steps.append({"native_commit": key, "value": expected, "revision": state["revision"]})
        return state

    def probe(self, mode, *, rule_hosts=()):
        if self.application is not None:
            raise RuntimeError("Native persistence probe requires the GUI owner closed")
        if rule_hosts and mode != "read":
            raise RuntimeError("Extra rule targets are only allowed for independent readback")
        arguments = [str(self.args.dart), "run", str(ROOT / "tool/gui_settings_probe.dart"),
                     str(self.bundle / "lib/libshuvpn_core.so"), str(self.state_file().parent), mode]
        if rule_hosts:
            arguments.append(json.dumps(list(rule_hosts)))
        result = subprocess.run(arguments,
                                cwd=ROOT, text=True, capture_output=True, timeout=30)
        (self.report / f"native-{mode}-{len(self.launches)}.log").write_text(
            result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError("Independent native settings read/prepare failed")
        match = re.search(r"GUI_NATIVE_SETTINGS (.+)", result.stdout)
        if match is None:
            raise RuntimeError("Native settings probe did not return a snapshot")
        return json.loads(match[1])

    def run(self):
        try:
            self.start("01-welcome")
            assert self.state()["values"].get("settings.welcomeCompleted", False) is False
            self.control("继续")
            self.snapshot("02-permissions")
            assert any(row["name"] == "权限管理" for row in accessibility(self.application.pid))
            self.control("继续")
            self.snapshot("03-account-notice")
            names = [row["name"] for row in accessibility(self.application.pid)]
            assert any("登录会话与账户展示状态会保存在本机" in name for name in names)
            assert not any("不保存您的校园账户信息" in name for name in names)
            self.control("去登录")
            self.snapshot("04-login-input-only")
            assert any(row["role"] == "password text" for row in accessibility(self.application.pid))
            self.stop()
            prepared = self.probe("prepare-offline-settings")
            assert prepared["welcome"]
            self.start("05-offline-home")
            self.control("设置")
            self.snapshot("06-settings")
            self.control("外观")
            self.snapshot("07-appearance")
            self.control("深色")
            self.value("settings.themeMode", "dark")
            self.dark_indicator()
            self.snapshot("08-dark-committed")
            self.click(28, 104)
            self.control("网络连接")
            self.snapshot("10-network-initial")
            self.scroll(7)
            self.snapshot("11-network-lower")
            self.control("VPN 捕获模式")
            self.snapshot("12-capture-choice")
            self.control("完整捕获（实验性）")
            self.value("settings.vpnCaptureMode", "full")
            self.snapshot("13-full-committed")
            self.control("HTTP 代理端口")
            self.text("70000")
            before = self.state()
            self.control("保存")
            assert self.state() == before, "Invalid port unexpectedly changed committed settings"
            assert any(node["name"] == "设置值无效" for node in accessibility(self.application.pid))
            self.snapshot("14-invalid-port")
            self.text("18443")
            self.snapshot("14b-valid-input")
            self.control("保存")
            self.value("settings.httpPort", 18443)
            self.snapshot("15-valid-port")
            self.click(28, 104)
            self.control("规则分流")
            self.snapshot("16-routing")
            prior = json.loads(self.routing_file().read_bytes())["version"]
            self.control("仅校园模板")
            self.control("校验并预览")
            self.snapshot("17-rule-preview")
            assert json.loads(self.routing_file().read_bytes())["version"] == prior
            self.control("应用预览")
            def applied():
                state = json.loads(self.routing_file().read_bytes())
                return state if state["version"] > prior else None
            routing = wait_for(applied, "rule preview committed natively")
            self.steps.append({"native_routing_version": routing["version"]})
            self.snapshot("18-rule-applied")
            self.reject_invalid_rule(routing["version"])
            self.stop("ctrl+q")
            native = self.probe("read")
            assert native["theme"] == "dark" and native["capture_mode"] == "full"
            assert native["http_port"] == 18443
            assert native["routing_version"] == routing["version"]
            assert native["routing_name"] == "仅校园" and native["routing_test_action"] == "REJECT"
            assert native["routing_profile"] == routing["current"]["profile"]
            assert native["routing_overrides"] == prepared["routing_overrides"]
            assert native["routing_override_actions"] == {
                "before.gui.invalid": "DIRECT", "after.gui.invalid": "DIRECT"}
            self.start("09-reopened-dark")
            self.control("设置")
            self.control("外观")
            self.dark_indicator()
            self.snapshot("09b-dark-selection-restored")
            self.click(28, 104)
            self.control("网络连接")
            self.displayed_value("HTTP 代理端口", "18443")
            self.displayed_value("VPN 捕获模式", "完整捕获（实验性）")
            self.control("VPN 捕获模式")
            self.snapshot("19-full-restored")
            self.control("校园分流")
            self.value("settings.vpnCaptureMode", "campus")
            self.click(28, 104)
            self.control("规则分流")
            self.snapshot("20-rules-restored")
            names = [node["name"] for node in accessibility(self.application.pid)]
            assert any(name.split("\n")[0] == "仅校园" and
                       f"正在使用版本 {routing['version']}" in name for name in names)
            assert any("最终兜底 · REJECT" in name for name in names)
            before_reopen = self.routing_file().read_bytes()
            self.control("导入与高级编辑")
            self.editor_node(routing["current"]["profile"])
            self.steps.append({"restored_rule_editor": routing["current"]["profile"]})
            self.snapshot("23-valid-rule-editor-restored")
            assert self.routing_file().read_bytes() == before_reopen
            self.stop()
            second = self.probe("read")
            assert second["theme"] == "dark" and second["capture_mode"] == "campus"
            assert second["http_port"] == 18443
            assert second["routing_version"] == routing["version"]
            assert second["routing_name"] == "仅校园" and second["routing_test_action"] == "REJECT"
            assert second["routing_profile"] == native["routing_profile"]
            assert second["routing_overrides"] == native["routing_overrides"]
            assert second["routing_override_actions"] == native["routing_override_actions"]
            assert second["system_proxy_enabled"] is False
            assert not second["http_enabled"] and not second["socks_enabled"] and not second["vpn_enabled"]
            assert len({launch["pid"] for launch in self.launches}) == 3
            for relative, expected in self.product.items():
                assert hashlib.sha256((self.bundle / relative).read_bytes()).hexdigest() == expected
            (self.report / "verification.json").write_text(json.dumps({
                "launches": self.launches, "steps": self.steps, "native": second,
                "offline_welcome_seed": True, "login_success_tested": False,
                "product": self.product, "routing_version": routing["version"],
                "invalid_rule_validation_preserved_active_configuration": True,
            }, indent=2) + "\n")
            print("REAL_GUI_OFFLINE_SETTINGS_REOPEN_PASSED")
        except BaseException:
            if self.application is not None and self.application.poll() is None:
                try:
                    self.snapshot("failure")
                except Exception as observation_error:
                    print(f"Failure observation failed: {observation_error}", file=sys.stderr)
            raise
        finally:
            finish((self.close,))

    def close(self):
        try:
            if self.application is not None:
                stop_process(self.application)
        finally:
            if self.log is not None:
                self.log.close()


def main(args, *, session_type=Session, entrypoint=__file__):
    args.owns_report = False
    try:
        run_session(args, session_type=session_type, entrypoint=entrypoint)
    except BaseException as error:
        if args.owns_report:
            record = args.report / "verification.json"
            try:
                if record.is_file():
                    verification = json.loads(record.read_text())
                    if verification.get("passed") is True:
                        verification.update(passed=False, entrypoint_error=str(error))
                        record.write_text(json.dumps(verification, ensure_ascii=False, indent=2) + "\n")
            except (OSError, ValueError) as report_error:
                print(f"Failed to record GUI entrypoint failure: {report_error}", file=sys.stderr)
        raise


def run_session(args, *, session_type, entrypoint):
    if not __debug__:
        raise RuntimeError("Run the GUI verifier with assertions enabled")
    if not args.inside:
        args.report = args.report.resolve()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.mkdir()
        args.owns_report = True
        token = uuid.uuid4().hex
        environment = dict(os.environ, SHUVPN_GUI_ACCEPTANCE_TOKEN=token,
                           SHUVPN_GUI_PARENT_DISPLAY=os.environ.get("DISPLAY", ""),
                           SHUVPN_GUI_PARENT_DBUS=os.environ.get("DBUS_SESSION_BUS_ADDRESS", ""))
        profile = args.report / "profile"
        for name, relative in (("XDG_CONFIG_HOME", "."), ("XDG_DATA_HOME", "data"),
                               ("XDG_CACHE_HOME", "cache")):
            directory = profile / relative
            directory.mkdir(parents=True, exist_ok=True)
            directory.chmod(0o700)
            environment[name] = str(directory)
        # DBus and its activated services must inherit the private paths too.
        with tempfile.TemporaryDirectory(prefix="shuvpn-gui-", dir="/tmp") as runtime:
            os.chmod(runtime, 0o700)
            environment["XDG_RUNTIME_DIR"] = runtime
            environment["GDK_BACKEND"] = "x11"
            subprocess.run(["xvfb-run", "-a", "-s", "-screen 0 1440x1100x24 -nolisten tcp",
                            "dbus-run-session", sys.executable, str(Path(entrypoint).resolve()),
                            "--inside", "--token", token, "--bundle", str(args.bundle.resolve()),
                            "--report", str(args.report), "--dart", str(args.dart.resolve())],
                           env=environment, check=True)
        return
    validate_session(os.environ, args.token, os.geteuid())
    args.owns_report = True
    assert not re.search(r"0x[1-9a-f]", command("xprop", "-root", "_NET_SUPPORTING_WM_CHECK"))
    with (args.report / "openbox.log").open("wb") as log:
        wm = subprocess.Popen(["openbox", "--sm-disable"], stdout=log, stderr=subprocess.STDOUT)
        try:
            wait_for(lambda: wm.poll() is None and re.search(
                r"0x[1-9a-f]", command("xprop", "-root", "_NET_SUPPORTING_WM_CHECK")),
                "owned Openbox on fresh display")
            session_type(args).run()
        finally:
            finish((lambda: stop_process(wm),))


def arguments(description=__doc__):
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--dart", type=Path, required=True)
    parser.add_argument("--inside", action="store_true")
    parser.add_argument("--token")
    return parser.parse_args()


if __name__ == "__main__":
    main(arguments())
