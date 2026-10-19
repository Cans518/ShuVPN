"""Exercise the real Tauri/WebKit window on a private X11/D-Bus session."""

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

os.environ["GDK_BACKEND"] = "x11"

import gi

gi.require_version("Atspi", "2.0")
gi.require_version("Gdk", "3.0")
gi.require_version("GdkX11", "3.0")
gi.require_version("Gio", "2.0")
from gi.repository import Atspi, Gdk, GdkX11, Gio, GLib  # noqa: E402


def wait_for(check, process, description, seconds=20):
    deadline = time.monotonic() + seconds
    last_error = None
    while time.monotonic() < deadline:
        context = GLib.MainContext.default()
        for _ in range(512):
            if not context.pending():
                break
            context.iteration(False)
        if process.poll() is not None:
            raise RuntimeError(f"GUI exited before {description}: {process.returncode}")
        try:
            result = check()
        except GLib.Error as error:
            if "No such interface" not in str(error) and "No such object" not in str(error):
                raise
            last_error = error
            result = None
        if result:
            return result
        time.sleep(0.1)
    raise TimeoutError(description) from last_error


def descendants(root):
    queue = [(root, 0)]
    examined = 0
    while queue and examined < 800:
        node, depth = queue.pop(0)
        if node is None:
            continue
        examined += 1
        yield node
        if depth < 24:
            for index in range(node.get_child_count()):
                queue.append((node.get_child_at_index(index), depth + 1))


def application(pid):
    desktop = Atspi.get_desktop(0)
    return next(
        (child for child in descendants(desktop)
         if child.get_role() == Atspi.Role.APPLICATION and child.get_process_id() == pid),
        None,
    )


def has_text(root, name):
    for node in descendants(root):
        if node.get_name() == name:
            return True
        if "Text" in node.get_interfaces():
            content = node.get_text_iface()
            if content.get_text(0, content.get_character_count()).strip() == name:
                return True
    return False


def activate(root, name):
    candidates = [
        node for node in descendants(root)
        if node.get_role() in (Atspi.Role.PUSH_BUTTON, Atspi.Role.CHECK_BOX, Atspi.Role.TOGGLE_BUTTON)
        and "Action" in node.get_interfaces()
        and node.get_name() == name
    ]
    if not candidates:
        return False
    if len(candidates) != 1:
        raise RuntimeError(f"Expected one interactive control named {name!r}, found {len(candidates)}")
    if not candidates[0].get_state_set().contains(Atspi.StateType.SENSITIVE):
        return False
    action = candidates[0].get_action_iface()
    if candidates[0].get_role() in (Atspi.Role.CHECK_BOX, Atspi.Role.TOGGLE_BUTTON) and action.get_n_actions() == 1:
        return action.do_action(0)
    for index in range(action.get_n_actions()):
        if action.get_action_name(index) in ("click", "press", "activate", "toggle"):
            return action.do_action(index)
    if action.get_n_actions() == 0:
        return False
    names = [action.get_action_name(index) for index in range(action.get_n_actions())]
    raise RuntimeError(f"No activation action: {name}: {names}")


def click(root, name, process):
    wait_for(lambda: activate(root, name), process, f"activate {name}")


def expand_details(root, name, process):
    def expand():
        for node in descendants(root):
            if node.get_name() == name and "Component" in node.get_interfaces():
                native_window = window_id(process.pid)
                subprocess.run(["xdotool", "windowfocus", "--sync", str(native_window)], check=True, timeout=5)
                if not node.get_component_iface().grab_focus():
                    return False
                subprocess.run(["xdotool", "key", "--window", str(native_window), "space"], check=True, timeout=5)
                return True
        return False
    wait_for(expand, process, f"expand {name}")


def control_ready(root, name):
    return any(
        node.get_name() == name
        and node.get_state_set().contains(Atspi.StateType.SENSITIVE)
        and "Action" in node.get_interfaces()
        and node.get_action_iface().get_n_actions() > 0
        for node in descendants(root)
    )


def home_bounds(root, height):
    result = {}
    for name in ("登录账户", "连接时启用 HTTP 代理", "连接时启用 SOCKS5 代理", "连接详情"):
        for node in descendants(root):
            if node.get_name() != name or "Component" not in node.get_interfaces():
                continue
            bounds = node.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
            if node.get_state_set().contains(Atspi.StateType.SHOWING) and bounds.width > 0 and 0 <= bounds.y and bounds.y + bounds.height <= height - 30:
                result[name] = {"x": bounds.x, "y": bounds.y, "width": bounds.width, "height": bounds.height}
                break
        if name not in result:
            raise RuntimeError(f"Home control is outside the first viewport: {name}")
    return result


def window_id(pid):
    result = subprocess.run(
        ["xdotool", "search", "--onlyvisible", "--pid", str(pid), "--name", "^ShuVPN$"],
        check=False, capture_output=True, text=True,
    )
    if result.returncode == 1:
        return None
    result.check_returncode()
    windows = result.stdout.split()
    if len(windows) != 1:
        raise RuntimeError(f"Expected one native window, found {windows}")
    return int(windows[0])


def capture(window):
    display = Gdk.Display.get_default()
    display.sync()
    native = GdkX11.X11Window.foreign_new_for_display(display, window)
    # GDK caches foreign-window dimensions across another process's resize.
    geometry = dict(
        line.split("=", 1)
        for line in subprocess.check_output(
            ["xdotool", "getwindowgeometry", "--shell", str(window)], text=True
        ).splitlines() if "=" in line
    )
    image = Gdk.pixbuf_get_from_window(native, 0, 0, int(geometry["WIDTH"]), int(geometry["HEIGHT"]))
    if image is None:
        raise RuntimeError("Could not capture native Tauri window")
    return image


def screenshot(window, path):
    image = capture(window)
    image.savev(str(path), "png", [], [])


def settled_screenshot(window, path, process, previous=None):
    last_digest = None
    stable = 0

    def painted():
        nonlocal last_digest, stable
        image = capture(window)
        pixels = bytes(image.get_pixels())
        channels = image.get_n_channels()
        colors = {pixels[index:index + channels]
                  for index in range(0, len(pixels) - channels, channels * 32)}
        digest = hashlib.sha256(pixels).hexdigest()
        if len(colors) < 8 or digest == previous:
            stable = 0
            return None
        stable = stable + 1 if digest == last_digest else 1
        last_digest = digest
        if stable < 3:
            return None
        image.savev(str(path), "png", [], [])
        return digest

    return wait_for(painted, process, f"painted and stable {path.name}")


def request_close(window):
    class MessageData(ctypes.Union):
        _fields_ = [("bytes", ctypes.c_char * 20), ("shorts", ctypes.c_short * 10),
                    ("longs", ctypes.c_long * 5)]

    class ClientMessage(ctypes.Structure):
        _fields_ = [
            ("type", ctypes.c_int), ("serial", ctypes.c_ulong),
            ("send_event", ctypes.c_int), ("display", ctypes.c_void_p),
            ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong),
            ("format", ctypes.c_int), ("data", MessageData),
        ]

    class Event(ctypes.Union):
        _fields_ = [("message", ClientMessage), ("padding", ctypes.c_long * 24)]

    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    x11.XInternAtom.restype = ctypes.c_ulong
    x11.XSendEvent.argtypes = [
        ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.POINTER(Event),
    ]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("X11 display could not be opened")
    try:
        event = Event()
        event.message.type = 33  # ClientMessage, ICCCM WM_DELETE_WINDOW
        event.message.send_event = 1
        event.message.display = display
        event.message.window = window
        event.message.message_type = x11.XInternAtom(display, b"WM_PROTOCOLS", 0)
        event.message.format = 32
        event.message.data.longs[0] = x11.XInternAtom(display, b"WM_DELETE_WINDOW", 0)
        if not x11.XSendEvent(display, window, 0, 0, ctypes.byref(event)):
            raise RuntimeError("WM_DELETE_WINDOW could not be sent")
        x11.XFlush(display)
    finally:
        x11.XCloseDisplay(display)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    arguments = parser.parse_args()
    if os.getuid() == 0:
        raise RuntimeError("Run the GUI as an ordinary user")
    Gdk.init([])
    executable = arguments.executable.resolve(strict=True)
    report = arguments.report.resolve()
    report.mkdir(parents=False, exist_ok=False)
    profile = report / "profile"
    environment = dict(os.environ)
    environment["XDG_CACHE_HOME"] = str(report / "cache")
    environment["XDG_CONFIG_HOME"] = str(report / "config")
    evidence = {
        "passed": False, "executable": str(executable),
        "sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
        "uid": os.getuid(), "realCampusLoginTested": False, "steps": [],
    }
    with (report / "process.log").open("wb") as log:
        process = subprocess.Popen(
            [str(executable), "--data-dir", str(profile)],
            stdout=log, stderr=subprocess.STDOUT, env=environment,
        )
        evidence["pid"] = process.pid
        try:
            app = wait_for(lambda: application(process.pid), process, "AT-SPI application")
            window = wait_for(lambda: window_id(process.pid), process, "native window")
            wait_for(lambda: has_text(app, "登录账户"), process, "Rust startup")
            expected_version = json.loads((Path(__file__).resolve().parents[1] / "desktop/package.json").read_text())["version"]
            wait_for(lambda: has_text(app, f"{expected_version} · 开源许可"), process, "release version display")
            previous = settled_screenshot(window, report / "home.png", process)
            for node in descendants(app):
                if "Text" in node.get_interfaces():
                    content = node.get_text_iface()
                    if content.get_text(0, content.get_character_count()).strip() == "尚未连接":
                        evidence["homeTextStyle"] = content.get_attribute_run(0, True)[0]
                        if evidence["homeTextStyle"].get("family-name") != "LXGW WenKai GB":
                            raise RuntimeError("The bundled WenKai GB font was not applied")
                        break
            subprocess.run(["xdotool", "windowsize", "--sync", str(window), "380", "540"], check=True, timeout=10)
            wait_for(lambda: capture(window).get_width() == 380 and capture(window).get_height() == 540, process, "minimum native window")
            settled_screenshot(window, report / "home-minimum.png", process)
            evidence["minimumHomeBounds"] = wait_for(lambda: home_bounds(app, 540), process, "minimum home controls")
            subprocess.run(["xdotool", "windowsize", "--sync", str(window), "420", "600"], check=True, timeout=10)
            wait_for(lambda: capture(window).get_width() == 420 and capture(window).get_height() == 600, process, "default native window")
            for label, expected in (
                ("校园账户", "学工号"), ("网络设置", "保存设置"),
                ("规则分流", "校验并预览"), ("连接记录", "当前会话暂无连接记录"),
                ("连接", "登录账户"),
            ):
                click(app, label, process)
                wait_for(lambda: has_text(app, expected), process, label)
                evidence["steps"].append(label)
                previous = settled_screenshot(
                    window, report / f"page-{len(evidence['steps'])}.png",
                    process, previous,
                )
                if label == "网络设置":
                    expand_details(app, "编辑或导入自定义主题", process)
                    click(app, "保存自定义主题", process)
                    wait_for(lambda: has_text(app, "配色已保存"), process, "custom theme persistence")
                    appearance_path = profile / "appearance.json"
                    appearance = json.loads(appearance_path.read_text(encoding="utf-8"))
                    if appearance["palette"] != "custom":
                        raise RuntimeError("The custom theme was not persisted")
                    evidence["customTheme"] = appearance
                    click(app, "晴海", process)
                    wait_for(
                        lambda: json.loads(appearance_path.read_text(encoding="utf-8"))["palette"] == "ocean",
                        process, "preset theme persistence",
                    )
                    startup = report / "config/autostart/ShuVPN.desktop"
                    wait_for(lambda: control_ready(app, "开机自启动"), process, "startup control available")
                    click(app, "开机自启动", process)
                    wait_for(lambda: startup.is_file(), process, "XDG startup registration")
                    wait_for(lambda: has_text(app, "已启用，下次登录系统时生效"), process, "startup control ready")
                    entry = startup.read_text(encoding="utf-8")
                    if f'Exec="{executable}" --hidden' not in entry:
                        raise RuntimeError("Startup executable or hidden argument changed")
                    desktop_entry = Gio.DesktopAppInfo.new_from_filename(str(startup))
                    if desktop_entry is None:
                        raise RuntimeError("The desktop could not parse the startup executable")
                    parsed, command = GLib.shell_parse_argv(desktop_entry.get_commandline())
                    if not parsed or command != [str(executable), "--hidden"]:
                        raise RuntimeError("The desktop parsed a different startup command")
                    evidence["startupCommand"] = command
                    evidence["startupEntry"] = entry
                    click(app, "开机自启动", process)
                    wait_for(lambda: not startup.exists(), process, "XDG startup removal")
            request_close(window)
            wait_for(lambda: window_id(process.pid) is None, process, "close hides native window")
            secondary = subprocess.run(
                [str(executable), "--data-dir", str(profile)], env=environment,
                capture_output=True, timeout=15, check=True,
            )
            evidence["secondaryExitCode"] = secondary.returncode
            wait_for(lambda: window_id(process.pid), process, "restore hidden window")
            click(app, "网络设置", process)
            wait_for(lambda: has_text(app, "退出 ShuVPN"), process, "separate exit action")
            click(app, "退出 ShuVPN", process)
            code = process.wait(timeout=20)
            if code != 0:
                raise RuntimeError(f"Tauri exited with {code}")
            if hashlib.sha256(executable.read_bytes()).hexdigest() != evidence["sha256"]:
                raise RuntimeError("Executable changed during GUI test")
            evidence["passed"] = True
        except Exception as error:
            evidence["error"] = repr(error)
            if process.poll() is None:
                try:
                    app = application(process.pid)
                    if app is not None:
                        tree = [
                            {
                                "role": node.get_role_name(),
                                "name": node.get_name(),
                                "states": [state.value_nick for state in node.get_state_set().get_states()],
                                "actions": [
                                    node.get_action_iface().get_action_name(index)
                                    for index in range(node.get_action_iface().get_n_actions())
                                ] if "Action" in node.get_interfaces() else [],
                            }
                            for node in descendants(app)
                        ]
                        (report / "failure-accessibility.json").write_text(
                            json.dumps(tree, ensure_ascii=False, indent=2), encoding="utf-8"
                        )
                    windows = subprocess.run(
                        ["xwininfo", "-root", "-tree"], capture_output=True, text=True,
                        check=False,
                    )
                    (report / "failure-windows.txt").write_text(
                        windows.stdout + windows.stderr, encoding="utf-8"
                    )
                    window = window_id(process.pid)
                    if window is not None:
                        screenshot(window, report / "failure.png")
                except Exception as diagnostic_error:
                    evidence["diagnosticError"] = repr(diagnostic_error)
            raise
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                evidence["forcedCleanup"] = True
            evidence["exitCode"] = process.returncode
            (report / "verification.json").write_text(
                json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8"
            )
    print("TAURI_LINUX_WEBKIT_NAVIGATION_AND_CLOSE_PASSED")


if __name__ == "__main__":
    main()
