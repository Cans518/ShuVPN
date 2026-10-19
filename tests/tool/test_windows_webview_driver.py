import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from tool.package_desktop import ROOT


@unittest.skipUnless(os.name == "nt", "Windows WebView2 driver")
class WindowsWebViewDriverTest(unittest.TestCase):
    def test_early_child_failure_reports_exit_and_preserves_stderr(self):
        node = shutil.which("node")
        self.assertIsNotNone(node)
        temporary_root = ROOT / "build"
        temporary_root.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=temporary_root, prefix="webview-driver-") as temporary:
            report = Path(temporary) / "report"
            result = subprocess.run([
                node, str(ROOT / "tool/tauri_windows_smoke.mjs"), sys.executable, str(report),
            ], cwd=ROOT, capture_output=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            evidence = json.loads((report / "verification.json").read_text(encoding="utf-8"))
            self.assertFalse(evidence["passed"])
            run = evidence["runs"][0]
            self.assertIn("Application exited before WebView startup", run["error"])
            self.assertEqual(run["exitCodeBeforeCleanup"], 2)
            self.assertIn(b"--data-dir", (report / "process-1.log").read_bytes())
            self.assertIn(b"--data-dir", result.stderr)
