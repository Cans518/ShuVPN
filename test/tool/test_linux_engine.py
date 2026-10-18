import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tool"))
import verify_linux_engine as engine


class LinuxEngineTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "sdk"
        name = "engine/src/flutter/shell/platform/linux/fl_accessible_node.cc"
        path = self.source / name
        path.parent.mkdir(parents=True)
        path.write_bytes(b"patched source")
        self.manifest = {
            "framework_git": "a" * 40, "patch_sha256": "b" * 64,
            "source_files": {name: {"after": engine.digest(path)}},
            "engine_output": "host_release", "test_filter": "Selected.*", "selected_tests": 56,
        }
        output = self.source / "engine/src/out/host_release"
        output.mkdir(parents=True)
        outputs = {}
        for name in ("libflutter_linux_gtk.so", "flutter_linux_unittests"):
            (output / name).write_bytes(name.encode())
            outputs[name] = engine.digest(output / name)
        self.build = {"running": False, "exit": 0, "source_unchanged": True,
                      "source_inputs": {name: row["after"] for name, row in
                                        self.manifest["source_files"].items()},
                      "outputs": outputs}
        self.tests = {"exit": 0, "failures": 0, "errors": 0, "tests": 56,
                      "binary_sha256": outputs["flutter_linux_unittests"],
                      "command": ["test", "--gtest_filter=Selected.*"]}
        self.bundle = self.root / "bundle"
        (self.bundle / "lib").mkdir(parents=True)
        (self.bundle / "lib/libflutter_linux_gtk.so").write_bytes(b"libflutter_linux_gtk.so")
        self.head = self.manifest["framework_git"]
        self.changed = ""
        self.addCleanup(patch.stopall)
        patch.object(engine, "load_manifest", return_value=self.manifest).start()
        patch.object(engine.subprocess, "check_output", side_effect=self.git).start()

    def git(self, command, **kwargs):
        return self.head if command[-1] == "HEAD" and "rev-parse" in command else self.changed

    def verify(self):
        build, tests = self.root / "build.json", self.root / "tests.json"
        build.write_text(json.dumps(self.build), encoding="utf-8")
        tests.write_text(json.dumps(self.tests), encoding="utf-8")
        return engine.verify_engine(self.source, build, tests, self.bundle)

    def test_accepts_exact_sources_outputs_selected_tests_and_bundle(self):
        result = self.verify()
        self.assertEqual(result["selected_tests"], 56)
        self.assertTrue(result["bundle_engine_checked"])

    def test_rejects_wrong_sdk_and_unreviewed_source_edits(self):
        self.head = "c" * 40
        with self.assertRaisesRegex(ValueError, "revision"):
            self.verify()
        self.head = self.manifest["framework_git"]
        self.changed = "engine/src/flutter/unreviewed.cc\n"
        with self.assertRaisesRegex(ValueError, "additional tracked"):
            self.verify()
        self.changed = ""
        source = self.source / next(iter(self.manifest["source_files"]))
        source.write_bytes(b"other")
        with self.assertRaisesRegex(ValueError, "patched source"):
            self.verify()

    def test_rejects_partial_failed_stale_or_different_build_receipts(self):
        original = copy.deepcopy(self.build)
        for key, value in (("running", True), ("exit", 1), ("source_unchanged", False),
                           ("source_inputs", {}), ("outputs", {})):
            self.build = {**original, key: value}
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.verify()

    def test_rejects_replaced_output_or_bundle_engine(self):
        output = self.source / "engine/src/out/host_release/libflutter_linux_gtk.so"
        output.write_bytes(b"stock engine")
        with self.assertRaisesRegex(ValueError, "build output"):
            self.verify()
        output.write_bytes(b"libflutter_linux_gtk.so")
        (self.bundle / "lib/libflutter_linux_gtk.so").write_bytes(b"stock engine")
        with self.assertRaisesRegex(ValueError, "different engine"):
            self.verify()

    def test_rejects_test_failures_wrong_filter_count_or_test_binary(self):
        original = copy.deepcopy(self.tests)
        for key, value in (("exit", 1), ("failures", 1), ("errors", 1), ("tests", 54),
                           ("binary_sha256", "d" * 64), ("command", ["test"]),
                           ("command", ["test", "--gtest_filter=Selected.*",
                                        "--gtest_filter=Selected.*"])):
            self.tests = {**original, key: value}
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "test receipt"):
                self.verify()

    def test_engine_paths_cannot_escape_checkout(self):
        for name in ("../other", "/absolute", r"..\other"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                engine.child_path(self.source, name)
        external = self.root / "external"
        external.mkdir()
        link = self.source / "link"
        try:
            link.symlink_to(external, target_is_directory=True)
        except OSError:
            return
        with self.assertRaises(ValueError):
            engine.child_path(self.source, "link/file")


class EngineManifestTest(unittest.TestCase):
    def test_committed_patch_matches_manifest(self):
        manifest = engine.load_manifest()
        self.assertEqual(manifest["selected_tests"], 56)
        self.assertEqual(len(manifest["source_files"]), 12)

    def test_changed_patch_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "engine.patch").write_bytes(b"modified")
            expected = hashlib.sha256(b"expected").hexdigest()
            manifest = {"schema": 1, "patch": "engine.patch", "patch_sha256": expected}
            (root / "source.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "patch digest"):
                engine.load_manifest(root / "source.json")


if __name__ == "__main__":
    unittest.main()
