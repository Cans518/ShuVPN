import json
from pathlib import Path
import tempfile
import unittest

from tool.release import release_version, release_notes, validate_tag, validate_source, validate_build


class ReleaseMetadataTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.write("desktop/package.json", json.dumps({"version": "0.6.0"}))
        self.write("desktop/package-lock.json", json.dumps({
            "version": "0.6.0", "packages": {"": {"version": "0.6.0"}},
        }))
        self.write("desktop/src-tauri/tauri.conf.json", '{"version":"0.6.0"}')
        self.write("desktop/src-tauri/Cargo.toml", '[package]\nname = "shuvpn_desktop"\nversion = "0.6.0"\n')
        self.write("Cargo.lock", '[[package]]\nname = "shuvpn_desktop"\nversion = "0.6.0"\n')
        self.write("CHANGELOG.md", "# Changes\n\n## 0.6.0 - 2026-10-19\n\n- Desktop release.\n\n## 0.5.1 - 2026-10-18\n\n- Older release.\n")

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def test_all_application_versions_and_lock_entries_must_agree(self):
        self.assertEqual(release_version(self.root), "0.6.0")
        for name in ("desktop/src-tauri/tauri.conf.json", "desktop/package-lock.json",
                     "desktop/src-tauri/Cargo.toml", "Cargo.lock"):
            with self.subTest(name=name):
                path = self.root / name
                original = path.read_text(encoding="utf-8")
                path.write_text(original.replace("0.6.0", "0.5.1"), encoding="utf-8")
                with self.assertRaises(ValueError):
                    release_version(self.root)
                path.write_text(original, encoding="utf-8")

    def test_tag_must_be_the_exact_stable_version(self):
        validate_tag("0.6.0", "v0.6.0")
        for tag in ("v0.5.1", "0.6.0", "v0.6.0-beta.1"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                validate_tag("0.6.0", tag)

    def test_release_notes_do_not_include_another_version(self):
        self.assertEqual(release_notes(self.root), "- Desktop release.\n")
        self.write("CHANGELOG.md", "# Work in progress\n")
        with self.assertRaises(ValueError):
            release_notes(self.root)

    def test_dirty_or_incomplete_source_cannot_be_a_formal_release(self):
        source = {"revision": "a" * 40, "dirty": False, "tree_sha256": "b" * 64, "file_count": 10}
        validate_source(source)
        for broken in ({**source, "dirty": True}, {**source, "revision": "HEAD"}, {}, {**source, "tree_sha256": ""}):
            with self.subTest(source=broken), self.assertRaises(ValueError):
                validate_source(broken)

    def test_binary_must_identify_a_release_build_for_the_requested_platform(self):
        info = {"schema": 1, "product": "ShuVPN", "version": "0.6.0", "profile": "release", "frontend": "embedded",
                "platform": "windows", "architecture": "x86_64"}
        validate_build(info, "windows", "0.6.0")
        for key, value in (("profile", "debug"), ("frontend", "dev-server"), ("version", "0.4.2"), ("platform", "linux"), ("architecture", "aarch64")):
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_build({**info, key: value}, "windows", "0.6.0")
