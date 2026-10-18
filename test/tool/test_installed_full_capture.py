import os
from pathlib import Path
import sys
import tempfile
import unittest

if sys.platform == "linux":
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
    import installed_full_capture_smoke as smoke


@unittest.skipUnless(sys.platform == "linux", "Linux root namespace verifier")
class InstalledFullCaptureProtocolTest(unittest.TestCase):
    def test_local_fixture_environment_does_not_inherit_host_proxies(self):
        environment = smoke.fixture_environment({
            "HTTP_PROXY": "http://fixture:secret@127.0.0.1:7897",
            "https_proxy": "http://127.0.0.1:7897", "all_proxy": "socks5://127.0.0.1:7897",
            "NO_PROXY": "old-value", "SHUVPN_INSTALLED_FULL_CONFIG": "/tmp/config",
        })
        self.assertNotIn("HTTP_PROXY", environment)
        self.assertNotIn("https_proxy", environment)
        self.assertNotIn("all_proxy", environment)
        self.assertEqual(environment["NO_PROXY"], "*")
        self.assertEqual(environment["no_proxy"], "*")
        self.assertEqual(environment["SHUVPN_INSTALLED_FULL_CONFIG"], "/tmp/config")

    def test_exact_order_is_required(self):
        completed = []
        for stage in smoke.STAGES:
            self.assertEqual(smoke.validate_stage(completed, stage), stage)
            completed.append(stage)
        smoke.validate_complete(completed)
        with self.assertRaises(ValueError):
            smoke.validate_stage(completed, smoke.STAGES[0])

    def test_fault_and_audit_stages_cannot_be_omitted(self):
        for missing in ("before-blocked", "after-blocked", "coverage-change", "helper-crash"):
            incomplete = [stage for stage in smoke.STAGES if stage != missing]
            with self.subTest(missing=missing), self.assertRaises(ValueError):
                smoke.validate_complete(incomplete)
        with self.assertRaises(ValueError):
            smoke.validate_stage([smoke.STAGES[0]], "coverage-change")

    def test_root_barriers_reject_absolute_and_traversal_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            victim = directory / "victim-cleanup"
            victim.write_text("retained")
            descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                for stage in (str(victim), "../victim-cleanup", "arbitrary-cleanup"):
                    with self.subTest(stage=stage), self.assertRaises(ValueError):
                        smoke.acknowledge(descriptor, stage, "replacement")
                self.assertEqual(victim.read_text(), "retained")
            finally:
                os.close(descriptor)

    def test_barrier_symlinks_cannot_overwrite_other_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            victim = directory / "victim"
            victim.write_text("retained")
            (directory / smoke.STAGES[0]).symlink_to(victim)
            descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                with self.assertRaises(FileExistsError):
                    smoke.acknowledge(descriptor, smoke.STAGES[0], "replacement")
                self.assertEqual(victim.read_text(), "retained")
            finally:
                os.close(descriptor)

    def test_barriers_are_anchored_to_the_opened_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            original = parent / "barriers"
            original.mkdir()
            descriptor = os.open(original, os.O_RDONLY | os.O_DIRECTORY)
            try:
                original.rename(parent / "retained")
                original.mkdir()
                smoke.acknowledge(descriptor, smoke.STAGES[0], "token")
                self.assertFalse((original / smoke.STAGES[0]).exists())
                self.assertEqual((parent / "retained" / smoke.STAGES[0]).read_text(), "token")
            finally:
                os.close(descriptor)
