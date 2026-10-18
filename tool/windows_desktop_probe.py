"""Win32 observations and physical input for the owned desktop acceptance process."""
import ctypes as c
from ctypes import wintypes as w
import hashlib
from pathlib import Path
import sys
import time
import socket
import struct


class Guid(c.Structure):
    _fields_ = [("a", w.DWORD), ("b", w.WORD), ("d", w.WORD), ("e", c.c_ubyte * 8)]


class IconIdentifier(c.Structure):
    _fields_ = [("size", w.DWORD), ("window", w.HWND), ("id", w.UINT), ("guid", Guid)]


class GuiThread(c.Structure):
    _fields_ = [("size", w.DWORD), ("flags", w.DWORD),
                *[(name, w.HWND) for name in ("active", "focus", "capture", "menu_owner", "move", "caret")],
                ("caret_rect", w.RECT)]


class MenuBar(c.Structure):
    _fields_ = [("size", w.DWORD), ("rect", w.RECT), ("menu", w.HMENU),
                ("menu_window", w.HWND), ("flags", w.DWORD)]


class MenuItem(c.Structure):
    _fields_ = [("size", w.UINT), ("mask", w.UINT), ("type", w.UINT), ("state", w.UINT),
                ("id", w.UINT), ("submenu", w.HMENU), ("checked", w.HBITMAP),
                ("unchecked", w.HBITMAP), ("data", c.c_size_t), ("text", w.LPWSTR),
                ("length", w.UINT), ("bitmap", w.HBITMAP)]


class Module(c.Structure):
    _fields_ = [(name, w.DWORD) for name in ("size", "id", "pid", "global_refs", "refs")] + [
        ("base", c.c_void_p), ("bytes", w.DWORD), ("handle", w.HMODULE),
        ("name", w.WCHAR * 256), ("path", w.WCHAR * 260)]


class ProcessEntry(c.Structure):
    _fields_ = [("size", w.DWORD), ("usage", w.DWORD), ("pid", w.DWORD),
                ("heap", c.c_size_t), ("module", w.DWORD), ("threads", w.DWORD),
                ("parent", w.DWORD), ("priority", w.LONG), ("flags", w.DWORD),
                ("name", w.WCHAR * 260)]


class Tcp4(c.Structure):
    _fields_ = [(name, w.DWORD) for name in (
        "state", "address", "port", "remote_address", "remote_port", "pid")]


class Tcp6(c.Structure):
    _fields_ = [("address", c.c_ubyte * 16), ("scope", w.DWORD), ("port", w.DWORD),
                ("remote_address", c.c_ubyte * 16), ("remote_scope", w.DWORD),
                ("remote_port", w.DWORD), ("state", w.DWORD), ("pid", w.DWORD)]


class Mouse(c.Structure):
    _fields_ = [("x", w.LONG), ("y", w.LONG), ("data", w.DWORD),
                ("flags", w.DWORD), ("time", w.DWORD), ("extra", c.c_size_t)]


class Keyboard(c.Structure):
    _fields_ = [("key", w.WORD), ("scan", w.WORD), ("flags", w.DWORD),
                ("time", w.DWORD), ("extra", c.c_size_t)]


class Hardware(c.Structure):
    _fields_ = [("message", w.DWORD), ("low", w.WORD), ("high", w.WORD)]


class InputBody(c.Union):
    _fields_ = [("mouse", Mouse), ("keyboard", Keyboard), ("hardware", Hardware)]


class Input(c.Structure):
    _fields_ = [("type", w.DWORD), ("body", InputBody)]


def file_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rectangle(value: w.RECT) -> list[int]:
    return [value.left, value.top, value.right, value.bottom]


def icon_name_matches(name: str, tooltip: str) -> bool:
    return name in (tooltip, f"ShuVPN {tooltip}")


class WindowsDesktop:
    def __init__(self):
        if sys.platform != "win32":
            raise RuntimeError("The desktop adapter requires Windows")
        self.user = c.WinDLL("user32", use_last_error=True)
        self.kernel = c.WinDLL("kernel32", use_last_error=True)
        self.shell = c.WinDLL("shell32", use_last_error=True)
        self.security = c.WinDLL("advapi32", use_last_error=True)
        self.ip = c.WinDLL("iphlpapi", use_last_error=True)
        self.callback = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
        self._bind()
        self.automation = None

    def prepare_input(self) -> None:
        if not self.user.SetProcessDpiAwarenessContext(c.c_void_p(-4)):
            context = self.user.GetThreadDpiAwarenessContext()
            if self.user.GetAwarenessFromDpiAwarenessContext(context) != 2:
                raise c.WinError(c.get_last_error())
        self.cursor = w.POINT()
        self.require(self.user.GetCursorPos(c.byref(self.cursor)))

    def _bind(self):
        signatures = {
            self.kernel: {
                "OpenProcess": (w.HANDLE, [w.DWORD, w.BOOL, w.DWORD]),
                "CreateMutexW": (w.HANDLE, [c.c_void_p, w.BOOL, w.LPCWSTR]),
                "CloseHandle": (w.BOOL, [w.HANDLE]),
                "WaitForSingleObject": (w.DWORD, [w.HANDLE, w.DWORD]),
                "GetExitCodeProcess": (w.BOOL, [w.HANDLE, c.POINTER(w.DWORD)]),
                "GetProcessTimes": (w.BOOL, [w.HANDLE, *[c.POINTER(w.FILETIME)] * 4]),
                "QueryFullProcessImageNameW": (w.BOOL, [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)]),
                "ProcessIdToSessionId": (w.BOOL, [w.DWORD, c.POINTER(w.DWORD)]),
                "CreateToolhelp32Snapshot": (w.HANDLE, [w.DWORD, w.DWORD]),
                "Module32FirstW": (w.BOOL, [w.HANDLE, c.POINTER(Module)]),
                "Module32NextW": (w.BOOL, [w.HANDLE, c.POINTER(Module)]),
                "Process32FirstW": (w.BOOL, [w.HANDLE, c.POINTER(ProcessEntry)]),
                "Process32NextW": (w.BOOL, [w.HANDLE, c.POINTER(ProcessEntry)]),
                "GetTickCount64": (c.c_ulonglong, []),
            },
            self.security: {
                "OpenProcessToken": (w.BOOL, [w.HANDLE, w.DWORD, c.POINTER(w.HANDLE)]),
                "GetTokenInformation": (w.BOOL, [w.HANDLE, c.c_int, c.c_void_p, w.DWORD, c.POINTER(w.DWORD)]),
            },
            self.user: {
                "EnumWindows": (w.BOOL, [self.callback, w.LPARAM]),
                "EnumChildWindows": (w.BOOL, [w.HWND, self.callback, w.LPARAM]),
                "GetWindowThreadProcessId": (w.DWORD, [w.HWND, c.POINTER(w.DWORD)]),
                "GetClassNameW": (c.c_int, [w.HWND, w.LPWSTR, c.c_int]),
                "GetWindowTextW": (c.c_int, [w.HWND, w.LPWSTR, c.c_int]),
                "IsWindowVisible": (w.BOOL, [w.HWND]),
                "IsWindow": (w.BOOL, [w.HWND]),
                "GetWindowRect": (w.BOOL, [w.HWND, c.POINTER(w.RECT)]),
                "SetForegroundWindow": (w.BOOL, [w.HWND]),
                "GetForegroundWindow": (w.HWND, []),
                "FindWindowW": (w.HWND, [w.LPCWSTR, w.LPCWSTR]),
                "WindowFromPoint": (w.HWND, [w.POINT]),
                "GetGUIThreadInfo": (w.BOOL, [w.DWORD, c.POINTER(GuiThread)]),
                "GetMenuBarInfo": (w.BOOL, [w.HWND, w.LONG, w.LONG, c.POINTER(MenuBar)]),
                "GetMenuItemCount": (c.c_int, [w.HMENU]),
                "GetMenuItemInfoW": (w.BOOL, [w.HMENU, w.UINT, w.BOOL, c.POINTER(MenuItem)]),
                "GetMenuItemRect": (w.BOOL, [w.HWND, w.HMENU, w.UINT, c.POINTER(w.RECT)]),
                "GetSystemMetrics": (c.c_int, [c.c_int]),
                "GetCursorPos": (w.BOOL, [c.POINTER(w.POINT)]),
                "GetPhysicalCursorPos": (w.BOOL, [c.POINTER(w.POINT)]),
                "GetClipCursor": (w.BOOL, [c.POINTER(w.RECT)]),
                "MonitorFromPoint": (w.HANDLE, [w.POINT, w.DWORD]),
                "SetCursorPos": (w.BOOL, [c.c_int, c.c_int]),
                "SendInput": (w.UINT, [w.UINT, c.POINTER(Input), c.c_int]),
                "MapVirtualKeyW": (w.UINT, [w.UINT, w.UINT]),
                "SetProcessDpiAwarenessContext": (w.BOOL, [c.c_void_p]),
                "GetThreadDpiAwarenessContext": (c.c_void_p, []),
                "GetAwarenessFromDpiAwarenessContext": (c.c_int, [c.c_void_p]),
            },
            self.shell: {
                "Shell_NotifyIconGetRect": (c.c_long, [c.POINTER(IconIdentifier), c.POINTER(w.RECT)]),
            },
            self.ip: {
                "GetExtendedTcpTable": (w.DWORD, [c.c_void_p, c.POINTER(w.DWORD),
                                                  w.BOOL, w.ULONG, c.c_int, w.ULONG]),
            },
        }
        for library, functions in signatures.items():
            for name, (result, arguments) in functions.items():
                function = getattr(library, name)
                function.restype, function.argtypes = result, arguments

    @staticmethod
    def require(result):
        if not result:
            raise c.WinError(c.get_last_error())
        return result

    def acquire_input(self):
        handle = self.require(self.kernel.CreateMutexW(
            None, False, "Local\\ShuVPN.Desktop.WindowAcceptance"))
        if c.get_last_error() == 183:
            self.kernel.CloseHandle(handle)
            raise RuntimeError("Another owned Windows desktop acceptance is active")
        return handle

    def window_pid(self, window: int) -> int:
        pid = w.DWORD()
        self.user.GetWindowThreadProcessId(window, c.byref(pid))
        return pid.value

    def created_ticks(self, handle) -> str:
        created, exited, kernel, user = (w.FILETIME() for _ in range(4))
        self.require(self.kernel.GetProcessTimes(handle, c.byref(created), c.byref(exited),
                                                c.byref(kernel), c.byref(user)))
        return str(created.dwLowDateTime | created.dwHighDateTime << 32)

    def window_class(self, window: int) -> str:
        value = c.create_unicode_buffer(256)
        self.require(self.user.GetClassNameW(window, value, len(value)))
        return value.value

    def window_title(self, window: int) -> str:
        value = c.create_unicode_buffer(512)
        self.user.GetWindowTextW(window, value, len(value))
        return value.value

    def windows(self, pid: int, class_name: str) -> list[int]:
        result = []
        @self.callback
        def visit(window, _):
            if self.window_pid(window) == pid and self.window_class(window) == class_name:
                result.append(window)
            return True
        self.require(self.user.EnumWindows(visit, 0))
        return result

    def window_rect(self, window: int) -> list[int]:
        value = w.RECT()
        self.require(self.user.GetWindowRect(window, c.byref(value)))
        return rectangle(value)

    def icon_rect(self, window: int):
        identifier = IconIdentifier(size=c.sizeof(IconIdentifier), window=window, id=0)
        value = w.RECT()
        status = self.shell.Shell_NotifyIconGetRect(c.byref(identifier), c.byref(value))
        return rectangle(value) if status >= 0 else None

    def menu(self, window: int, main_window: int) -> dict:
        thread = self.user.GetWindowThreadProcessId(window, None)
        state = GuiThread(size=c.sizeof(GuiThread))
        self.require(self.user.GetGUIThreadInfo(thread, c.byref(state)))
        pid = self.window_pid(main_window)
        owners = [main_window, *self.windows(pid, "FLUTTER_RUNNER_WIN32_WINDOW_TRAY")]
        if state.menu_owner not in owners or self.window_pid(state.menu_owner) != pid:
            raise RuntimeError("Popup menu is not owned by a retained probe window")
        bar = MenuBar(size=c.sizeof(MenuBar))
        self.require(self.user.GetMenuBarInfo(window, -4, 0, c.byref(bar)))
        if not bar.menu:
            raise RuntimeError("Owned popup has no readable native menu")
        return {"window": window, "pid": self.window_pid(window), "thread": thread,
                "owner": state.menu_owner, "handle": bar.menu, "rect": self.window_rect(window),
                "owner_class": self.window_class(state.menu_owner),
                "active_window": state.active, "focus_window": state.focus,
                "gui_flags": state.flags, "foreground": self.user.GetForegroundWindow(),
                "rows": self.menu_rows(bar.menu)}

    def menu_rows(self, menu: int) -> list[dict]:
        count = self.user.GetMenuItemCount(menu)
        if count < 0:
            raise c.WinError(c.get_last_error())
        result = []
        for position in range(count):
            label = c.create_unicode_buffer(512)
            item = MenuItem(size=c.sizeof(MenuItem), mask=0x147,
                            text=c.cast(label, w.LPWSTR), length=len(label))
            self.require(self.user.GetMenuItemInfoW(menu, position, True, c.byref(item)))
            result.append({"position": position, "id": item.id, "label": label.value,
                           "enabled": item.state & 3 == 0, "checked": bool(item.state & 8),
                           "highlighted": bool(item.state & 0x80), "raw_state": item.state,
                           "separator": bool(item.type & 0x800), "submenu": item.submenu,
                           "children": self.menu_rows(item.submenu) if item.submenu else []})
        return result

    def menu_item_rect(self, main_window: int, menu: int, position: int) -> list[int]:
        result = w.RECT()
        self.require(self.user.GetMenuItemRect(main_window, menu, position, c.byref(result)))
        return rectangle(result)

    def _send(self, inputs: list[Input]):
        array = (Input * len(inputs))(*inputs)
        if self.user.SendInput(len(inputs), array, c.sizeof(Input)) != len(inputs):
            raise RuntimeError("Windows did not accept all physical input events")

    def move(self, bounds: list[int]):
        x, y = (bounds[0] + bounds[2]) // 2, (bounds[1] + bounds[3]) // 2
        left, top = self.user.GetSystemMetrics(76), self.user.GetSystemMetrics(77)
        width, height = self.user.GetSystemMetrics(78), self.user.GetSystemMetrics(79)
        clip, observed, physical = w.RECT(), w.POINT(), w.POINT()
        self.require(self.user.GetClipCursor(c.byref(clip)))
        self.require(self.user.GetCursorPos(c.byref(observed)))
        self.require(self.user.GetPhysicalCursorPos(c.byref(physical)))
        self.last_pointer = {"requested": [x, y], "observed": [observed.x, observed.y],
                             "physical": [physical.x, physical.y],
                             "before": [observed.x, observed.y],
                             "desktop": [left, top, width, height],
                             "clip": rectangle(clip), "reached": False}
        if (width <= 1 or height <= 1 or
                not left <= x < left + width or not top <= y < top + height or
                not clip.left <= x < clip.right or not clip.top <= y < clip.bottom):
            raise RuntimeError(f"Pointer target is outside the desktop or clip: {self.last_pointer}")
        if not self.user.MonitorFromPoint(w.POINT(x, y), 0):
            raise RuntimeError(f"Pointer target is outside any monitor: {self.last_pointer}")
        normalized = [round((x - left) * 65535 / (width - 1)),
                      round((y - top) * 65535 / (height - 1))]
        self.last_pointer["normalized"] = normalized
        self._send([Input(type=0, body=InputBody(mouse=Mouse(
            x=normalized[0], y=normalized[1], flags=0xC001)))])
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            self.require(self.user.GetCursorPos(c.byref(observed)))
            self.require(self.user.GetPhysicalCursorPos(c.byref(physical)))
            self.last_pointer.update(observed=[observed.x, observed.y],
                                     physical=[physical.x, physical.y])
            if (abs(observed.x - x) <= 2 and abs(observed.y - y) <= 2 and
                    abs(physical.x - x) <= 2 and abs(physical.y - y) <= 2):
                self.last_pointer["reached"] = True
                break
            time.sleep(0.01)
        else:
            raise RuntimeError(f"Physical pointer did not reach its target: {self.last_pointer}")
        return x, y

    def click(self, bounds: list[int], button: str):
        self.move(bounds)
        self.press(button)

    def press(self, button: str):
        down, up = {"left": (2, 4), "right": (8, 16)}[button]
        self._send([Input(type=0, body=InputBody(mouse=Mouse(flags=down)))])
        time.sleep(0.05)
        self._send([Input(type=0, body=InputBody(mouse=Mouse(flags=up)))])

    def wheel(self, notches: int) -> None:
        if type(notches) is not int or not 0 < abs(notches) <= 10:
            raise ValueError("A desktop wheel step must be 1–10 signed notches")
        self._send([Input(type=0, body=InputBody(mouse=Mouse(
            data=(notches * 120) & 0xFFFFFFFF, flags=0x0800)))])

    def keys(self, keys: list[int]):
        scans = {key: self.user.MapVirtualKeyW(key, 4) for key in keys}
        if not all(scans.values()):
            raise RuntimeError("A physical key has no keyboard-layout scan code")
        navigation = {0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E}
        extended = {key: key in navigation or bool(scans[key] & 0xFF00) for key in keys}
        inputs = [Input(type=1, body=InputBody(keyboard=Keyboard(
            scan=scans[key] & 0xFF, flags=9 if extended[key] else 8))) for key in keys]
        inputs += [Input(type=1, body=InputBody(keyboard=Keyboard(
            scan=scans[key] & 0xFF, flags=11 if extended[key] else 10)))
                   for key in reversed(keys)]
        self._send(inputs)

    def type_text(self, text: str) -> None:
        if not text or any(not 32 <= ord(character) < 127 for character in text):
            raise ValueError("Desktop acceptance types only explicit printable ASCII test data")
        events = []
        for character in text:
            for flags in (4, 6):  # Unicode key down/up, independent of the active IME.
                events.append(Input(type=1, body=InputBody(keyboard=Keyboard(
                    scan=ord(character), flags=flags))))
        self._send(events)

    def _uia(self):
        if self.automation is None:
            import comtypes.client
            self.uia = comtypes.client.GetModule("UIAutomationCore.dll")
            self.automation = comtypes.client.CreateObject(
                self.uia.CUIAutomation, interface=self.uia.IUIAutomation)
        return self.automation

    def own_icon_at(self, bounds: list[int], expected: str, explorer_pid: int):
        automation = self._uia()
        point = self.uia.tagPOINT((bounds[0] + bounds[2]) // 2, (bounds[1] + bounds[3]) // 2)
        element = automation.ElementFromPoint(point)
        self.last_icon_observation = {"rect": bounds, "ancestors": []}
        for _ in range(6):
            if not element:
                break
            name = element.CurrentName or ""
            help_text = element.CurrentHelpText or ""
            self.last_icon_observation["ancestors"].append({
                "name": name if "ShuVPN" in name else "<other>",
                "help": help_text if "ShuVPN" in help_text else "<other>",
                "class": element.CurrentClassName, "pid": element.CurrentProcessId,
                "automation_id": element.CurrentAutomationId,
                "native_window": element.CurrentNativeWindowHandle,
                "enabled": bool(element.CurrentIsEnabled),
                "offscreen": bool(element.CurrentIsOffscreen),
            })
            if icon_name_matches(name, expected) and element.CurrentProcessId == explorer_pid:
                point, available = element.GetClickablePoint()
                region = element.CurrentBoundingRectangle
                return {"name": name, "pid": explorer_pid, "rect": bounds,
                        "uia_rect": [region.left, region.top, region.right, region.bottom],
                        "clickable": [point.x, point.y] if available else None}
            element = automation.ControlViewWalker.GetParentElement(element)
        return None

    def find_icon(self, taskbar: int, explorer_pid: int, tooltip: str):
        automation = self._uia()
        names = automation.CreateOrCondition(
            automation.CreatePropertyCondition(self.uia.UIA_NamePropertyId, tooltip),
            automation.CreatePropertyCondition(self.uia.UIA_NamePropertyId, f"ShuVPN {tooltip}"))
        condition = automation.CreateAndCondition(names, automation.CreatePropertyCondition(
            self.uia.UIA_ControlTypePropertyId, self.uia.UIA_ButtonControlTypeId))
        roots = [taskbar]
        for class_name in ("TopLevelWindowForOverflowXamlIsland", "NotifyIconOverflowWindow"):
            roots += [window for window in self.windows(explorer_pid, class_name)
                      if self.user.IsWindowVisible(window)]
        matches = {}
        for window in roots:
            elements = automation.ElementFromHandle(window).FindAll(
                self.uia.TreeScope_Descendants, condition)
            for index in range(elements.Length):
                element = elements.GetElement(index)
                if element.CurrentProcessId != explorer_pid or element.CurrentIsOffscreen or not element.CurrentIsEnabled:
                    continue
                point, available = element.GetClickablePoint()
                if not available:
                    continue
                region = element.CurrentBoundingRectangle
                matches[tuple(element.GetRuntimeId())] = {
                    "name": element.CurrentName, "pid": explorer_pid, "host_window": window,
                    "uia_rect": [region.left, region.top, region.right, region.bottom],
                    "clickable": [point.x, point.y],
                }
        if len(matches) > 1:
            raise RuntimeError("Multiple visible Explorer icons contain this exact nonce")
        return next(iter(matches.values())) if matches else None

    def overflow_button(self, taskbar: int):
        automation = self._uia()
        root = automation.ElementFromHandle(taskbar)
        for identifier in ("SystemTrayIcon", "NotificationChevron"):
            condition = automation.CreatePropertyCondition(self.uia.UIA_AutomationIdPropertyId, identifier)
            element = root.FindFirst(self.uia.TreeScope_Descendants, condition)
            if element and ("hidden" in element.CurrentName.lower() or "隐藏" in element.CurrentName):
                value = element.CurrentBoundingRectangle
                return {"name": element.CurrentName,
                        "rect": [value.left, value.top, value.right, value.bottom]}
        return None

    def application_button(self, taskbar: int, explorer_pid: int, title: str):
        automation = self._uia()
        elements = automation.ElementFromHandle(taskbar).FindAll(
            self.uia.TreeScope_Descendants,
            automation.CreatePropertyCondition(self.uia.UIA_ControlTypePropertyId,
                                               self.uia.UIA_ButtonControlTypeId))
        matches = []
        for index in range(elements.Length):
            element = elements.GetElement(index)
            name = element.CurrentName or ""
            if (name != title and not name.startswith(title + " - ") or
                    element.CurrentProcessId != explorer_pid or element.CurrentIsOffscreen or
                    not element.CurrentIsEnabled):
                continue
            point, available = element.GetClickablePoint()
            if available:
                region = element.CurrentBoundingRectangle
                matches.append({"name": name, "pid": explorer_pid,
                                "automation_id": element.CurrentAutomationId,
                                "rect": [region.left, region.top, region.right, region.bottom],
                                "clickable": [point.x, point.y]})
        if len(matches) > 1:
            raise RuntimeError("Multiple Explorer taskbar buttons share the owned app title")
        return matches[0] if matches else None

    def control_at(self, point: list[int], pid: int, label: str, *, control_type: int):
        automation = self._uia()
        element = automation.ElementFromPoint(self.uia.tagPOINT(*point))
        self.last_control_hit = {"point": list(point), "owned": True, "ancestors": []}
        for _ in range(8):
            if not element:
                return None
            if element.CurrentProcessId != pid:
                self.last_control_hit["owned"] = False
                return None
            observed = {
                "name": element.CurrentName or "", "pid": element.CurrentProcessId,
                "control_type": element.CurrentControlType,
                "enabled": bool(element.CurrentIsEnabled),
                "offscreen": bool(element.CurrentIsOffscreen),
            }
            self.last_control_hit["ancestors"].append(observed)
            if (observed["name"].split("\n")[0] == label and
                    observed["control_type"] == control_type and
                    observed["enabled"] and not observed["offscreen"]):
                bounds = element.CurrentBoundingRectangle
                return {**observed, "clickable": list(point),
                        "rect": [bounds.left, bounds.top, bounds.right, bounds.bottom]}
            element = automation.ControlViewWalker.GetParentElement(element)
        return None

    def control(self, window: int, label: str, *, control_type: int | None = None):
        automation = self._uia()
        root = automation.ElementFromHandle(window)
        elements = root.FindAll(self.uia.TreeScope_Descendants,
                               automation.CreateTrueCondition())
        matches = []
        for index in range(elements.Length):
            element = elements.GetElement(index)
            name = element.CurrentName or ""
            if (name.split("\n")[0] != label or
                    (control_type is not None and element.CurrentControlType != control_type) or
                    element.CurrentProcessId != self.window_pid(window) or
                    element.CurrentIsOffscreen or not element.CurrentIsEnabled):
                continue
            point, available = element.GetClickablePoint()
            region = element.CurrentBoundingRectangle
            if region.right > region.left and region.bottom > region.top:
                matches.append({"name": name, "pid": element.CurrentProcessId,
                                "control_type": element.CurrentControlType,
                                "enabled": bool(element.CurrentIsEnabled),
                                "offscreen": bool(element.CurrentIsOffscreen),
                                "runtime_id": list(element.GetRuntimeId()),
                                "rect": [region.left, region.top, region.right, region.bottom],
                                "clickable": ([point.x, point.y] if available else [
                                    (region.left + region.right) // 2,
                                    (region.top + region.bottom) // 2]),
                                "point_source": "UIA ClickablePoint" if available else "UIA rectangle center"})
        if len(matches) > 1:
            raise RuntimeError("Multiple owned Flutter controls match the connection label")
        return matches[0] if matches else None

    def enable_accessibility(self, window: int):
        """Request the owned Flutter MSAA provider, which enables engine semantics."""
        import comtypes.client
        interface = comtypes.client.GetModule("oleacc.dll").IAccessible
        children = []
        @self.callback
        def visit(child, _):
            if (self.window_class(child) == "FLUTTERVIEW" and
                    self.window_pid(child) == self.window_pid(window)):
                children.append(child)
            return True
        self.user.EnumChildWindows(window, visit, 0)
        if len(children) != 1:
            raise RuntimeError("Owned main window has no unique Flutter child")
        library = c.WinDLL("oleacc", use_last_error=True)
        library.AccessibleObjectFromWindow.argtypes = [
            w.HWND, w.DWORD, c.c_void_p, c.POINTER(c.POINTER(interface))]
        library.AccessibleObjectFromWindow.restype = c.c_long
        accessible = c.POINTER(interface)()
        status = library.AccessibleObjectFromWindow(
            children[0], 0xFFFFFFFC, c.byref(interface._iid_), c.byref(accessible))
        if status == -2147467259:  # Engine semantics may not exist until the next frame.
            return None
        if status != 0 or not accessible:
            raise RuntimeError(f"Owned MSAA provider request failed: {status}")
        return {"api": "AccessibleObjectFromWindow:OBJID_CLIENT",
                "child": children[0], "pid": self.window_pid(window),
                "role": accessible.accRole[0], "children": accessible.accChildCount}

    def parent_pid(self, pid: int) -> int:
        snapshot = self.kernel.CreateToolhelp32Snapshot(2, 0)
        if snapshot == c.c_void_p(-1).value:
            raise c.WinError(c.get_last_error())
        try:
            entry = ProcessEntry(size=c.sizeof(ProcessEntry))
            found = self.kernel.Process32FirstW(snapshot, c.byref(entry))
            while found:
                if entry.pid == pid:
                    return entry.parent
                found = self.kernel.Process32NextW(snapshot, c.byref(entry))
            raise RuntimeError("Owned child process is absent from the OS snapshot")
        finally:
            self.kernel.CloseHandle(snapshot)

    def listeners(self, port: int | None = None) -> dict:
        observations, matches = [], []
        for family, entry in ((socket.AF_INET, Tcp4), (socket.AF_INET6, Tcp6)):
            size = w.DWORD()
            status = self.ip.GetExtendedTcpTable(None, c.byref(size), False, family, 3, 0)
            if status not in (0, 122) or size.value < 4:
                raise RuntimeError(f"Cannot size the real TCP listener table: {status}")
            for _ in range(3):
                buffer = c.create_string_buffer(size.value)
                status = self.ip.GetExtendedTcpTable(buffer, c.byref(size), False, family, 3, 0)
                if status != 122:
                    break
            if status != 0:
                raise RuntimeError(f"Cannot read the real TCP listener table: {status}")
            count = w.DWORD.from_buffer_copy(buffer.raw[:4]).value
            if 4 + count * c.sizeof(entry) > len(buffer):
                raise RuntimeError("TCP listener count exceeded the returned OS buffer")
            observations.append({"family": int(family), "count": count})
            for index in range(count):
                row = entry.from_buffer_copy(buffer.raw, 4 + index * c.sizeof(entry))
                if row.state != 2:
                    raise RuntimeError("Listener-only OS table contained a non-listening row")
                actual_port = socket.ntohs(row.port & 0xFFFF)
                if port is None or actual_port == port:
                    address = (struct.pack("=I", row.address) if family == socket.AF_INET
                               else bytes(row.address))
                    matches.append({"pid": row.pid, "address": socket.inet_ntop(family, address),
                                    "port": actual_port, "family": int(family), "state": row.state})
        return {"api": "GetExtendedTcpTable:TCP_TABLE_OWNER_PID_LISTENER",
                "tables": observations, "matches": matches}


class RetainedProcess:
    def __init__(self, desktop: WindowsDesktop, pid: int):
        self.desktop, self.pid = desktop, pid
        self.handle = desktop.require(desktop.kernel.OpenProcess(0x101000, False, pid))
        try:
            self.identity = self._identity()
        except BaseException:
            self.close()
            raise

    def _identity(self) -> dict:
        api = self.desktop
        if self.exit_code() is not None:
            raise RuntimeError("Cannot identify a process that already exited")
        created = api.created_ticks(self.handle)
        buffer, length = c.create_unicode_buffer(32768), w.DWORD(32768)
        api.require(api.kernel.QueryFullProcessImageNameW(self.handle, 0, buffer, c.byref(length)))
        image = Path(buffer.value).resolve()
        token = w.HANDLE()
        api.require(api.security.OpenProcessToken(self.handle, 8, c.byref(token)))
        try:
            elevated, size = w.DWORD(), w.DWORD()
            api.require(api.security.GetTokenInformation(token, 20, c.byref(elevated),
                                                         c.sizeof(elevated), c.byref(size)))
        finally:
            api.kernel.CloseHandle(token)
        session = w.DWORD()
        api.require(api.kernel.ProcessIdToSessionId(self.pid, c.byref(session)))
        result = {"pid": self.pid, "created_ticks": created,
                  "image": str(image), "sha256": file_hash(image), "elevated": bool(elevated.value),
                  "session": session.value}
        if self.exit_code() is not None:
            raise RuntimeError("Process exited during identity observation")
        return result

    def exit_code(self):
        kernel = self.desktop.kernel
        wait = kernel.WaitForSingleObject(self.handle, 0)
        if wait == 258:
            return None
        if wait != 0:
            raise c.WinError(c.get_last_error())
        code = w.DWORD()
        self.desktop.require(kernel.GetExitCodeProcess(self.handle, c.byref(code)))
        return code.value

    def modules(self) -> dict[str, str]:
        api = self.desktop
        snapshot = api.kernel.CreateToolhelp32Snapshot(0x18, self.pid)
        if snapshot == c.c_void_p(-1).value:
            raise c.WinError(c.get_last_error())
        try:
            entry = Module(size=c.sizeof(Module))
            api.require(api.kernel.Module32FirstW(snapshot, c.byref(entry)))
            result = {}
            while True:
                result[entry.name] = str(Path(entry.path).resolve())
                if not api.kernel.Module32NextW(snapshot, c.byref(entry)):
                    break
            return result
        finally:
            api.kernel.CloseHandle(snapshot)

    def require_successful_exit(self) -> None:
        code = self.exit_code()
        if code != 0:
            raise RuntimeError(f"Original process {self.pid} lacks successful exit: {code}")

    def close(self):
        if self.handle:
            self.desktop.kernel.CloseHandle(self.handle)
            self.handle = None
