"""Exercise the release application's native menu on a private XFCE tray host."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

import desktop_gui_acceptance as gui


def menu_rows(layout):
    identifier, properties, children = layout
    return [{"id": identifier, **properties},
            *(row for child in children for row in menu_rows(child))]


def owned_item(items, pid, owner_pid):
    matches = []
    for item in items:
        service, separator, path = item.partition("/")
        if not service or not separator:
            raise RuntimeError("Unexpected StatusNotifier item address")
        if owner_pid(service) == pid:
            matches.append((service, "/" + path))
    if not matches:
        raise LookupError(f"No tray item owned by PID {pid} yet")
    if len(matches) != 1:
        raise RuntimeError(f"Expected one tray item owned by PID {pid}: {matches}")
    return matches[0]

def descendant_chain(pid, ancestor, parent_pid):
    chain = []
    while pid and pid not in chain:
        chain.append(pid)
        if pid == ancestor:
            return chain
        pid = parent_pid(pid)
    raise RuntimeError("Tray watcher does not belong to the owned panel")


class TrayMenu:
    def __init__(self, pid):
        import gi
        gi.require_version("Gio", "2.0")
        from gi.repository import Gio, GLib
        self.GLib = GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.service, self.path = owned_item(self.items(), pid, self.owner_pid)
        self.menu = self.call(
            self.service, self.path, "org.freedesktop.DBus.Properties", "Get",
            "(ss)", ("org.kde.StatusNotifierItem", "Menu"))[0]
        self.pid = pid

    def call(self, service, path, interface, method, signature=None, values=None):
        from gi.repository import Gio
        parameters = self.GLib.Variant(signature, values) if signature else None
        return self.bus.call_sync(
            service, path, interface, method, parameters, None,
            Gio.DBusCallFlags.NONE, 3000, None).unpack()

    def owner_pid(self, service):
        return self.call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                         "org.freedesktop.DBus", "GetConnectionUnixProcessID",
                         "(s)", (service,))[0]

    def items(self):
        return self.call("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher",
                         "org.freedesktop.DBus.Properties", "Get", "(ss)",
                         ("org.kde.StatusNotifierWatcher", "RegisteredStatusNotifierItems"))[0]

    def layout(self):
        revision, root = self.call(self.service, self.menu, "com.canonical.dbusmenu",
                                   "GetLayout", "(iias)", (0, -1, []))
        return {"revision": revision, "rows": menu_rows(root)}

    def item(self, label):
        rows = [row for row in self.layout()["rows"] if row.get("label") == label]
        if not rows:
            raise LookupError(f"Native menu item is not published yet: {label}")
        if len(rows) != 1:
            raise RuntimeError(f"Expected one native menu item {label}: {rows}")
        return rows[0]

    def click(self, label):
        if self.owner_pid(self.service) != self.pid:
            raise RuntimeError("Tray service no longer belongs to the application")
        item = self.item(label)
        if not item.get("enabled", True) or not item.get("visible", True):
            raise RuntimeError(f"Native menu item is disabled or hidden: {label}")
        self.call(self.service, self.menu, "com.canonical.dbusmenu", "Event",
                  "(isvu)", (item["id"], "clicked", self.GLib.Variant("i", 0), 0))


class TraySession(gui.Session):
    def __init__(self, args):
        super().__init__(args)
        self.panel = None
        self.panel_log = None
        self.hosts = []
        relative = "lib/libsystem_tray_plugin.so"
        self.product[relative] = hashlib.sha256((self.bundle / relative).read_bytes()).hexdigest()

    def watcher_ready(self):
        return "(true," in gui.command(
            "gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
            "--object-path", "/org/freedesktop/DBus", "--method",
            "org.freedesktop.DBus.NameHasOwner", "org.kde.StatusNotifierWatcher")

    def start_panel(self):
        if self.watcher_ready():
            raise RuntimeError("Private session already has a tray watcher")
        config = Path(os.environ["XDG_CONFIG_HOME"]) / "xfce4/xfconf/xfce-perchannel-xml"
        config.mkdir(parents=True, exist_ok=True)
        target = config / "xfce4-panel.xml"
        if not target.exists():
            shutil.copyfile("/etc/xdg/xfce4/panel/default.xml", target)
        self.panel_log = (self.report / f"panel-{len(self.hosts) + 1}.log").open("wb")
        self.panel = subprocess.Popen(["xfce4-panel", "--disable-wm-check"],
                                      stdout=self.panel_log, stderr=subprocess.STDOUT)
        gui.wait_for(lambda: self.panel.poll() is None and self.watcher_ready(),
                     "owned XFCE StatusNotifier watcher")
        self.hosts.append({"panel_pid": self.panel.pid})

    def stop_panel(self):
        if self.panel is None:
            return
        gui.stop_process(self.panel)
        self.panel = None
        self.panel_log.close()
        self.panel_log = None
        gui.wait_for(lambda: not self.watcher_ready(), "tray host removed")

    def menu(self):
        def complete_menu():
            try:
                tray = TrayMenu(self.application.pid)
                for label in ("显示窗口", "连接", "断开", "aTrust", "EasyConnect", "OpenVPN", "退出"):
                    tray.item(label)
                return tray
            except LookupError:
                return None
        tray = gui.wait_for(complete_menu, "complete native tray menu")
        host_pid = tray.owner_pid("org.kde.StatusNotifierWatcher")
        gui.wait_for(lambda: tray.call(
            "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher",
            "org.freedesktop.DBus.Properties", "Get", "(ss)",
            ("org.kde.StatusNotifierWatcher", "IsStatusNotifierHostRegistered"))[0] is True,
            "real StatusNotifier host registered")
        def parent(pid):
            status = Path(f"/proc/{pid}/status").read_text().splitlines()
            return int(next(row.split(":", 1)[1] for row in status if row.startswith("PPid:")))
        chain = descendant_chain(host_pid, self.panel.pid, parent)
        executable = Path(f"/proc/{host_pid}/exe").resolve()
        if executable.name not in ("xfce4-panel", "wrapper-2.0"):
            raise RuntimeError("Unexpected tray watcher executable")
        self.hosts[-1]["watcher_pid"] = host_pid
        self.hosts[-1]["watcher_exe"] = str(executable)
        self.hosts[-1]["watcher_sha256"] = hashlib.sha256(executable.read_bytes()).hexdigest()
        self.hosts[-1]["watcher_ancestry"] = chain
        self.hosts[-1]["host_registered"] = True
        self.launches[-1].setdefault("tray_registrations", []).append({
            "service": tray.service, "item": tray.path, "menu": tray.menu,
            "watcher_pid": host_pid, "panel_pid": self.panel.pid,
        })
        (self.report / f"host-{host_pid}-accessibility.json").write_text(
            json.dumps(gui.accessibility(host_pid), ensure_ascii=False, indent=2) + "\n")
        (self.report / f"panel-{self.panel.pid}-accessibility.json").write_text(
            json.dumps(gui.accessibility(self.panel.pid), ensure_ascii=False, indent=2) + "\n")
        return tray

    def visible(self):
        if self.application.poll() is not None:
            raise RuntimeError("Application exited while testing tray visibility")
        state = gui.command("xwininfo", "-id", self.window)
        return "Map State: IsViewable" in state

    def hide_window(self):
        self.key("alt+F4")
        gui.wait_for(lambda: not self.visible(), "close hides to tray")
        self.steps.append({"window_hidden": True, "pid": self.application.pid})

    def show_window(self, tray):
        tray.click("显示窗口")
        gui.wait_for(self.visible, "native menu restores same window")
        assert gui.command("xdotool", "getwindowname", self.window) == "ShuVPN"
        self.steps.append({"native_menu_event": "显示窗口", "pid": self.application.pid})

    def panel_menu_click(self, label):
        gui.capture(self.report / f"panel-before-popup-{len(self.launches)}.png")
        host_pid = self.hosts[-1]["watcher_pid"]
        windows = gui.command("xdotool", "search", "--pid", host_pid).splitlines()
        candidates = []
        for window in windows:
            info = gui.command("xwininfo", "-id", window)
            if "Map State: IsViewable" not in info:
                continue
            embedded = gui.command("xprop", "-id", window, "_XEMBED_INFO")
            if " = " not in embedded:
                continue
            def integer(field):
                return int(re.search(rf"{field}:\s+(-?\d+)", info)[1])
            rectangle = [integer("Absolute upper-left X"), integer("Absolute upper-left Y"),
                         integer("Width"), integer("Height")]
            if rectangle[2] > 5 and rectangle[3] > 5:
                candidates.append((window, rectangle))
        (self.report / "host-windows.json").write_text(json.dumps(candidates, indent=2) + "\n")
        if len(candidates) != 1:
            raise RuntimeError("Expected one visible owned tray host window")
        window, (x, y, width, height) = candidates[0]
        gui.command("xdotool", "mousemove", x + width // 2, y + height // 2, "click", "3")
        self.steps.append({"host_mouse": "open tray menu", "window": window,
                           "pid": host_pid, "rectangle": [x, y, width, height]})
        def popup_ready():
            popups = {}
            for pid in (self.panel.pid, host_pid, self.application.pid):
                found = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(pid)],
                                       capture_output=True, text=True)
                if found.returncode not in (0, 1):
                    raise RuntimeError("Could not inspect owned popup windows")
                for identifier in found.stdout.splitlines():
                    kind = gui.command("xprop", "-id", identifier, "_NET_WM_WINDOW_TYPE")
                    if "_NET_WM_WINDOW_TYPE_POPUP_MENU" in kind:
                        popups[identifier] = {"window": identifier, "pid": pid, "type": kind}
            if len(popups) > 1:
                raise RuntimeError("Multiple owned popup menus are visible")
            return next(iter(popups.values())) if popups else None
        popup = gui.wait_for(popup_ready, "owned native popup menu")
        gui.capture(self.report / f"panel-popup-{len(self.launches)}.png")
        rows = [row for row in TrayMenu(self.application.pid).layout()["rows"]
                if row.get("label") and row.get("enabled", True)]
        if label == "显示窗口" and rows[0]["label"] == label:
            key = "Home"
        elif label == "退出" and rows[-1]["label"] == label:
            key = "End"
        else:
            raise RuntimeError("Unexpected native menu keyboard order")
        gui.command("xdotool", "key", "--clearmodifiers", key, "Return")
        self.steps.append({"panel_keyboard": label, "key": key, "popup": popup})

    def wait_exit(self):
        status = self.application.wait(timeout=10)
        if status != 0:
            raise RuntimeError(f"Tray application exit was {status}")
        self.launches[-1]["exit"] = status
        self.application = None
        self.log.close()

    def reselect_protocol(self, tray):
        tray.click("aTrust")
        def restored():
            state = tray.item("aTrust")
            (self.report / "protocol-reselected.json").write_text(
                json.dumps(state, ensure_ascii=False, indent=2) + "\n")
            return state.get("toggle-state") == 1
        gui.wait_for(restored, "current protocol remains selected after native activation")
        self.steps.append({"native_menu_event": "aTrust", "selected_after_activation": True})

    def run(self):
        try:
            self.start_panel()
            self.start("01-tray-welcome")
            tray = self.menu()
            gui.wait_for(lambda: tray.item("aTrust").get("toggle-state") == 1 and
                         tray.item("连接").get("enabled", True) is False and
                         tray.item("断开").get("enabled", True) is False,
                         "initial native menu state committed")
            (self.report / "welcome-native-menu.json").write_text(
                json.dumps(tray.layout(), ensure_ascii=False, indent=2) + "\n")
            assert tray.item("连接").get("enabled") is False
            assert tray.item("断开").get("enabled") is False
            assert tray.item("EasyConnect").get("enabled") is False
            assert tray.item("OpenVPN").get("enabled") is False
            self.steps.append({"welcome_menu": tray.layout()})
            self.reselect_protocol(tray)
            self.hide_window()
            self.panel_menu_click("显示窗口")
            gui.wait_for(self.visible, "mouse menu restores same window")
            self.snapshot("02-restored-by-native-menu")
            self.hide_window()
            tray.click("退出")
            self.wait_exit()
            assert self.watcher_ready()
            gui.wait_for(lambda: not any(item.startswith(tray.service + "/")
                                        for item in tray.items()), "tray item removed after quit")
            assert self.probe("prepare-offline-settings")["welcome"]
            self.start("03-tray-guest")
            tray = self.menu()
            gui.wait_for(lambda: tray.item("连接").get("enabled", True),
                         "guest tray connect enabled")
            self.reselect_protocol(tray)
            tray.click("连接")
            gui.wait_for(lambda: any(row["name"] == "上海大学校园账户"
                                    for row in gui.accessibility(self.application.pid)),
                         "tray connect opens the account page")
            self.control("未登录")
            gui.wait_for(lambda: any(row["role"] == "password text"
                                    for row in gui.accessibility(self.application.pid)),
                         "tray connect opens the real login form")
            self.snapshot("04-tray-connect-login")
            self.hide_window()
            self.show_window(tray)
            self.stop_panel()
            self.stop()
            self.start_panel()
            self.start("05-tray-host-restarted")
            tray = self.menu()
            running_pid = self.application.pid
            self.stop_panel()
            self.start_panel()
            tray = self.menu()
            assert self.application.pid == running_pid
            self.reselect_protocol(tray)
            self.snapshot("06-same-app-host-recovered")
            self.hide_window()
            self.panel_menu_click("退出")
            self.wait_exit()
            gui.wait_for(lambda: not any(item.startswith(tray.service + "/")
                                        for item in tray.items()), "restarted tray item removed")
            native = self.probe("read")
            assert not native["http_enabled"] and not native["socks_enabled"] and not native["vpn_enabled"]
            assert len({launch["pid"] for launch in self.launches}) == 3
            for relative, expected in self.product.items():
                assert hashlib.sha256((self.bundle / relative).read_bytes()).hexdigest() == expected
            (self.report / "verification.json").write_text(json.dumps({
                "launches": self.launches, "steps": self.steps, "hosts": self.hosts,
                "product": self.product, "native": native,
                "menu_activation": ["native com.canonical.dbusmenu.Event",
                                    "owned XFCE panel right click and keyboard"],
                "panel_mouse_open_tested": True, "panel_keyboard_selection_tested": True,
                "login_success_tested": False,
            }, ensure_ascii=False, indent=2) + "\n")
            print("REAL_TRAY_HIDE_MENU_RESTORE_QUIT_HOST_LOSS_AND_RESTART_PASSED")
        finally:
            gui.finish((self.close, self.stop_panel))


if __name__ == "__main__":
    gui.main(gui.arguments(__doc__), session_type=TraySession, entrypoint=__file__)
