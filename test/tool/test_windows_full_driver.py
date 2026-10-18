import os
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tool.package_desktop import ROOT


@unittest.skipUnless(os.name == "nt", "Windows PowerShell supervisor")
class WindowsFullDriverTest(unittest.TestCase):
    def rejected_observation(self, case):
        shell = shutil.which("pwsh") or shutil.which("powershell")
        directory = ROOT / "build" / "desktop"
        directory.mkdir(parents=True, exist_ok=True)
        token = "0123456789abcdef0123456789abcdef"
        with tempfile.TemporaryDirectory(dir=directory, prefix="full-observer-test-") as temporary:
            folder = Path(temporary)
            configuration = folder / "peer.json"
            configuration.write_text("{}", encoding="utf-8")
            observer_identity = {"pid": 43, "created_ticks": "123"}
            state = {key: [1] for key in ("routes", "addresses", "interfaces", "adapters", "dns")}
            completion = {"token": token, "mode": "--plan", "scope": "Current",
                          "passed": True, "network_restored": True, "probe_exit_code": 0,
                          "observer_identity": observer_identity, "probe_identity": {"pid": 42},
                          "before": state, "after": state}
            if case == "ForeignToken":
                completion["token"] = "f" * 32
            if case == "EmptyTable":
                state["routes"] = None
            observer_log = folder / "observer.log"
            observer_log.write_text("WINDOWS_FULL_OBSERVER_COMPLETE " + json.dumps(completion) + "\n")
            lines = ["@echo off"]
            if case != "MissingTranscript":
                for sequence, stage in enumerate(("begin", "cleaned", "finish")):
                    event = {"request": {"token": token, "sequence": sequence, "pid": 42, "stage": stage},
                             "reply": {"token": token, "sequence": sequence, "stage": stage,
                                       "ok": True, "observer_identity": observer_identity}}
                    lines.append("echo WINDOWS_FULL_EXTERNAL_EVENT " + json.dumps(event))
            lines.extend(["echo WINDOWS_FULL_FORMAL_PREVIEW_AND_NO_TUN_PASSED", "exit /b 0"])
            probe = folder / "probe.cmd"
            probe.write_text("\n".join(lines) + "\n", encoding="ascii")
            report = folder / "report"
            result = subprocess.run(
                [shell, "-NoProfile", "-NonInteractive", "-File",
                 str(ROOT / "tool/full_capture_windows_smoke.ps1"),
                 "-ProbePath", str(probe), "-InterfaceAlias", "fixture",
                 "-PeerConfig", str(configuration), "-ReportDirectory", str(report),
                 "-Mode", "Plan", "-ObservationToken", token, "-ObserverLog", str(observer_log)],
                capture_output=True, timeout=30)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            verification = json.loads((report / "verification.json").read_text(encoding="utf-8-sig"))
            self.assertFalse(verification["passed"])
            expected = {"ForeignToken": "did not prove", "MissingTranscript": "Incomplete native",
                        "EmptyTable": "empty network table"}[case]
            self.assertIn(expected, verification["observer_error"])

    def rejected_snapshot(self, case):
        shell = shutil.which("pwsh") or shutil.which("powershell")
        self.assertIsNotNone(shell)
        directory = ROOT / "build" / "desktop"
        directory.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=directory, prefix="full-snapshot-test-") as temporary:
            folder = Path(temporary)
            configuration = folder / "peer.json"
            configuration.write_text("{}", encoding="utf-8")
            harness = folder / "snapshot-fixture.ps1"
            harness.write_text(r'''
param($Driver,$Probe,$Configuration,$Report,$Case)
$ErrorActionPreference = 'Stop'
function Get-NetRoute {
    [CmdletBinding()] param([switch]$IncludeAllCompartments)
    if ($Case -eq 'Error') {
        Write-Error 'DENIED_FROM_FIXTURE' -ErrorAction Continue
        [PSCustomObject]@{InterfaceIndex=1;DestinationPrefix='0.0.0.0/0'}
    }
}
function Get-NetIPAddress {
    [CmdletBinding()] param([switch]$IncludeAllCompartments)
    [PSCustomObject]@{InterfaceIndex=1;IPAddress='192.0.2.1'}
}
function Get-NetIPInterface {
    [CmdletBinding()] param([switch]$IncludeAllCompartments)
    [PSCustomObject]@{InterfaceIndex=1;AddressFamily=2}
}
function Get-NetAdapter {
    [CmdletBinding()] param([switch]$IncludeHidden)
    [PSCustomObject]@{ifIndex=1;InterfaceGuid='fixture'}
}
function Get-DnsClientServerAddress {
    [CmdletBinding()] param()
    [PSCustomObject]@{InterfaceIndex=1;ServerAddresses=@('192.0.2.53')}
}
function Get-ItemProperty {
    [CmdletBinding()] param($LiteralPath)
    [PSCustomObject]@{ProxyEnable=0}
}
try {
    & $Driver -ProbePath $Probe -InterfaceAlias 'fixture' -PeerConfig $Configuration -ReportDirectory $Report -Mode Plan
    exit 0
} catch {
    Write-Output $_
    exit 23
}
''', encoding="utf-8")
            report = folder / "report"
            result = subprocess.run(
                [shell, "-NoProfile", "-NonInteractive", "-File", str(harness),
                 str(ROOT / "tool/full_capture_windows_smoke.ps1"), sys.executable,
                 str(configuration), str(report), case],
                capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 23, result.stdout + result.stderr)
            self.assertIn(b"restoration is unproven", result.stdout + result.stderr)
            self.assertFalse((report / "probe.log").exists())
            self.assertFalse((report / "verification.json").exists())

    def test_empty_route_table_cannot_be_a_restoration_baseline(self):
        self.rejected_snapshot("Empty")

    def test_query_error_cannot_be_a_restoration_baseline(self):
        self.rejected_snapshot("Error")

    def test_final_observer_claim_without_native_transcript_is_rejected(self):
        self.rejected_observation("MissingTranscript")

    def test_completion_from_another_observer_token_is_rejected(self):
        self.rejected_observation("ForeignToken")

    def test_observer_null_tables_cannot_prove_restoration(self):
        self.rejected_observation("EmptyTable")
