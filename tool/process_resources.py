"""Retain child identity and read OS resource counters; errors are never zero samples."""
import ctypes as c
from ctypes import wintypes as w
import hashlib
import os
from pathlib import Path
import select
import sys


class ProcessResources:
    def __init__(self, pid: int, executable: Path):
        self.pid = pid
        self.executable = executable.resolve(strict=True)
        self.retained = None
        self.memory_handle = None
        self.pidfd = None
        try:
            if sys.platform == "win32":
                from windows_desktop_probe import WindowsDesktop, RetainedProcess
                self.desktop = WindowsDesktop()
                self.retained = RetainedProcess(self.desktop, pid)
                self.identity = self.retained.identity
                if Path(self.identity["image"]) != self.executable or self.identity["elevated"]:
                    raise RuntimeError("Child executable or ordinary identity does not match")
                self.identity["parent_pid"] = self.desktop.parent_pid(pid)
                self.memory_handle = self.desktop.require(
                    self.desktop.kernel.OpenProcess(0x1410, False, pid))
                if self.desktop.created_ticks(self.memory_handle) != self.identity["created_ticks"]:
                    raise RuntimeError("Memory handle belongs to another process generation")
                self.psapi = c.WinDLL("psapi", use_last_error=True)
                self.psapi.GetProcessMemoryInfo.argtypes = [w.HANDLE, c.c_void_p, w.DWORD]
                self.psapi.GetProcessMemoryInfo.restype = w.BOOL
                self.desktop.kernel.GetProcessHandleCount.argtypes = [w.HANDLE, c.POINTER(w.DWORD)]
                self.desktop.kernel.GetProcessHandleCount.restype = w.BOOL
            elif sys.platform == "linux":
                if os.geteuid() == 0:
                    raise RuntimeError("Resource observer must run as an ordinary user")
                self.pidfd = os.pidfd_open(pid)
                self.identity = self._linux_identity()
                self.identity["sha256"] = hashlib.sha256(self.executable.read_bytes()).hexdigest()
            else:
                raise RuntimeError("Only Windows and Linux resource counters are supported")
        except BaseException:
            self.close()
            raise

    def _linux_status(self) -> dict:
        return dict(row.split(":", 1) for row in
                    Path(f"/proc/{self.pid}/status").read_text().splitlines())

    def _linux_identity(self) -> dict:
        status = self._linux_status()
        executable = Path(f"/proc/{self.pid}/exe").resolve(strict=True)
        if (executable != self.executable or
                list(map(int, status["Uid"].split())) != [os.geteuid()] * 4 or
                int(status["CapEff"].strip(), 16) != 0):
            raise RuntimeError("Child executable or ordinary identity does not match")
        start = Path(f"/proc/{self.pid}/stat").read_text().rsplit(")", 1)[1].split()[19]
        return {"pid": self.pid, "image": str(executable),
                "created_ticks": start, "uid": os.geteuid(), "cap_eff": status["CapEff"].strip(),
                "parent_pid": int(status["PPid"])}

    def alive(self) -> bool:
        if self.retained is not None:
            return self.retained.exit_code() is None
        if self.pidfd is not None:
            return not select.select([self.pidfd], [], [], 0)[0]
        raise RuntimeError("Process observer is closed")

    def sample(self) -> dict:
        if not self.alive():
            raise RuntimeError("Cannot sample an exited process")
        if self.retained is not None:
            class Memory(c.Structure):
                _fields_ = [("cb", w.DWORD), ("faults", w.DWORD),
                            *[(name, c.c_size_t) for name in (
                                "peak_working", "working", "peak_paged", "paged",
                                "peak_nonpaged", "nonpaged", "pagefile",
                                "peak_pagefile", "private")]]
            memory = Memory(cb=c.sizeof(Memory))
            self.desktop.require(self.psapi.GetProcessMemoryInfo(
                self.memory_handle, c.byref(memory), c.sizeof(memory)))
            handles = w.DWORD()
            self.desktop.require(self.desktop.kernel.GetProcessHandleCount(
                self.memory_handle, c.byref(handles)))
            from windows_desktop_probe import ProcessEntry
            snapshot = self.desktop.kernel.CreateToolhelp32Snapshot(2, 0)
            if snapshot == c.c_void_p(-1).value:
                raise c.WinError(c.get_last_error())
            try:
                entry = ProcessEntry(size=c.sizeof(ProcessEntry))
                found = self.desktop.kernel.Process32FirstW(snapshot, c.byref(entry))
                while found and entry.pid != self.pid:
                    found = self.desktop.kernel.Process32NextW(snapshot, c.byref(entry))
                if not found:
                    raise RuntimeError("Original process is absent from the thread-count snapshot")
                threads = entry.threads
            finally:
                self.desktop.kernel.CloseHandle(snapshot)
            result = {"rss_bytes": memory.working, "private_bytes": memory.private,
                      "handles": handles.value, "threads": threads}
        else:
            if self._linux_identity() != {
                    name: value for name, value in self.identity.items() if name != "sha256"}:
                raise RuntimeError("Linux process identity changed during observation")
            status = self._linux_status()
            result = {"fds": len(list(Path(f"/proc/{self.pid}/fd").iterdir())),
                      "threads": int(status["Threads"])}
            for name, field in (("rss_bytes", "VmRSS"), ("vms_bytes", "VmSize")):
                value, unit = status[field].split()
                if unit != "kB":
                    raise RuntimeError("Unexpected Linux memory counter unit")
                result[name] = int(value) * 1024
        if not self.alive() or any(type(value) is not int or value <= 0 for value in result.values()):
            raise RuntimeError("Invalid or late process resource observation")
        return result

    def require_library(self, library: Path) -> None:
        if not self.alive():
            raise RuntimeError("Cannot inspect an exited process")
        if self.retained is not None:
            paths = [Path(value).resolve() for value in self.retained.modules().values()]
        else:
            paths = [Path(row.split()[-1]) for row in
                     Path(f"/proc/{self.pid}/maps").read_text().splitlines()
                     if len(row.split()) >= 6 and row.split()[-1].startswith("/")]
        if library.resolve(strict=True) not in paths or not self.alive():
            raise RuntimeError("The exact requested native library is not mapped by the child")

    def close(self) -> None:
        if self.memory_handle:
            self.desktop.kernel.CloseHandle(self.memory_handle)
            self.memory_handle = None
        if self.retained is not None:
            self.retained.close()
            self.retained = None
        if self.pidfd is not None:
            os.close(self.pidfd)
            self.pidfd = None
