import json
import hashlib
import struct
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile
from pathlib import Path

from tool.package_desktop import (
    REQUIRED,
    ROOT,
    app_version,
    package_bundle,
    package_release,
    verify_package,
    verify_release_package,
)


def fixture_binary(platform: str) -> bytes:
    content = bytearray(128)
    if platform == "windows":
        content[:2] = b"MZ"
        struct.pack_into("<I", content, 60, 64)
        content[64:70] = b"PE\x00\x00\x64\x86"
    else:
        content[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", content, 18, 62)
    return bytes(content)


class DesktopBundleFixture:
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def bundle(self, platform: str) -> Path:
        bundle = self.directory / platform
        for relative in REQUIRED[platform]:
            path = bundle / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            binary_platform = "linux" if relative.endswith(".so") else platform
            if relative in {"install.py", "shuvpn.desktop"}:
                path.write_bytes((ROOT / "packaging/linux" / relative).read_bytes())
            else:
                path.write_bytes(fixture_binary(binary_platform))
        return bundle

class DesktopPackageTest(DesktopBundleFixture, unittest.TestCase):
    def release_bundle(self, platform: str) -> Path:
        bundle = self.bundle(platform)
        (bundle / "icon.png").write_bytes(b"icon fixture")
        (bundle / "build-info.json").write_text(json.dumps({
            "schema": 1, "product": "ShuVPN", "version": app_version(),
            "profile": "release", "frontend": "embedded", "platform": platform, "architecture": "x86_64",
        }), encoding="utf-8")
        return bundle

    def test_official_release_rejects_dirty_sources_and_extra_development_files(self):
        source = {"revision": "a" * 40, "dirty": False, "tree_sha256": "b" * 64, "file_count": 10}
        with patch("tool.package_desktop.clean_checkout", return_value=source["revision"]), \
                patch("tool.package_desktop.release_version", return_value=app_version()), \
                patch("tool.package_desktop.source_identity", return_value={**source, "dirty": True}):
            with self.assertRaisesRegex(ValueError, "clean"):
                package_release("windows", self.release_bundle("windows"), self.directory / "dirty")
        with patch("tool.package_desktop.clean_checkout", return_value=source["revision"]), \
                patch("tool.package_desktop.release_version", return_value=app_version()), \
                patch("tool.package_desktop.source_identity", return_value=source):
            bundle = self.release_bundle("windows")
            (bundle / "debug.log").write_text("development output", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Unexpected"):
                package_release("windows", bundle, self.directory / "extra")

    def test_official_package_preserves_release_identity_and_rejects_a_debug_build(self):
        source = {"revision": "a" * 40, "dirty": False, "tree_sha256": "b" * 64, "file_count": 10}
        with patch("tool.package_desktop.clean_checkout", return_value=source["revision"]), \
                patch("tool.package_desktop.release_version", return_value=app_version()), \
                patch("tool.package_desktop.source_identity", return_value=source):
            bundle = self.release_bundle("windows")
            info = json.loads((bundle / "build-info.json").read_text())
            with patch("tool.package_desktop.read_build_info", return_value=info) as query:
                archive = package_release("windows", bundle, self.directory / "official")
                query.assert_called_once_with(bundle / "shuvpn.exe")
            verify_release_package(archive)
            with zipfile.ZipFile(archive) as reader:
                manifest = json.loads(next(reader.read(name) for name in reader.namelist() if name.endswith("/package.json")))
            self.assertEqual(manifest["channel"], "stable")
            self.assertFalse(manifest["source"]["dirty"])
            info = json.loads((bundle / "build-info.json").read_text())
            (bundle / "build-info.json").write_text(json.dumps({**info, "profile": "debug"}))
            with self.assertRaisesRegex(ValueError, "release build"):
                package_release("windows", bundle, self.directory / "debug")

    def test_a_staged_identity_cannot_disguise_another_executable_or_revision(self):
        source = {"revision": "a" * 40, "dirty": False, "tree_sha256": "b" * 64, "file_count": 10}
        with patch("tool.package_desktop.clean_checkout", return_value=source["revision"]), \
                patch("tool.package_desktop.release_version", return_value=app_version()), \
                patch("tool.package_desktop.source_identity", return_value=source):
            bundle = self.release_bundle("windows")
            with self.assertRaisesRegex(ValueError, "Cannot read build identity"):
                package_release("windows", bundle, self.directory / "fake-binary")
            info = json.loads((bundle / "build-info.json").read_text())
            with patch("tool.package_desktop.read_build_info", return_value={**info, "profile": "debug"}):
                with self.assertRaisesRegex(ValueError, "release build"):
                    package_release("windows", bundle, self.directory / "swapped")
            with patch("tool.package_desktop.source_identity", return_value={**source, "revision": "c" * 40}):
                with self.assertRaisesRegex(ValueError, "actual clean checkout"):
                    package_release("windows", bundle, self.directory / "spoofed")

    def test_inventory_cannot_name_windows_drives_streams_or_ambiguous_paths(self) -> None:
        archive = package_bundle("windows", self.bundle("windows"), self.directory / "output")
        with zipfile.ZipFile(archive) as reader:
            original = {name: reader.read(name) for name in reader.namelist()}
        manifest_path = next(name for name in original if name.endswith("/package.json"))
        prefix = manifest_path.removesuffix("package.json")
        for index, name in enumerate(("C:/escape", "lib/file:stream", "lib/file.", "lib//file", "lib/NUL.txt", "lib/a\nb")):
            with self.subTest(name=name):
                files = dict(original)
                manifest = json.loads(files[manifest_path])
                content = b"payload"
                manifest["files"][name] = {"size": len(content), "sha256": hashlib.sha256(content).hexdigest()}
                files[prefix + name] = content
                files[manifest_path] = json.dumps(manifest).encode()
                broken = self.directory / f"unsafe-{index}.zip"
                with zipfile.ZipFile(broken, "w") as writer:
                    for member, data in files.items():
                        writer.writestr(member, data)
                with self.assertRaises(ValueError):
                    verify_package(broken)

    def test_archive_version_cannot_change_its_extraction_root(self) -> None:
        archive = package_bundle("windows", self.bundle("windows"), self.directory / "output")
        with zipfile.ZipFile(archive) as reader:
            files = {name: reader.read(name) for name in reader.namelist()}
        old = next(name for name in files if name.endswith("/package.json"))
        manifest = json.loads(files[old])
        manifest["version"] = "foo/../../escape"
        prefix = "ShuVPN-foo/../../escape-windows-x64/"
        broken = self.directory / "unsafe-version.zip"
        with zipfile.ZipFile(broken, "w") as writer:
            for name, content in files.items():
                writer.writestr(prefix + name.removeprefix(old.removesuffix("package.json")), json.dumps(manifest).encode() if name == old else content)
        with self.assertRaises(ValueError):
            verify_package(broken)

    def test_windows_archive_includes_every_file_and_detects_modified_library(self) -> None:
        bundle = self.bundle("windows")
        (bundle / "extra_plugin.dll").write_bytes(fixture_binary("windows"))
        archive = package_bundle("windows", bundle, self.directory / "output")
        verify_package(archive)
        with zipfile.ZipFile(archive) as reader:
            entries = {name: reader.read(name) for name in reader.namelist()}
        extra = next(name for name in entries if name.endswith("/extra_plugin.dll"))
        self.assertIn(extra, entries)
        changed = next(name for name in entries if name.endswith("/shuvpn.exe"))
        entries[changed] += b"changed"
        corrupted = self.directory / "corrupted.zip"
        with zipfile.ZipFile(corrupted, "w") as writer:
            for name, content in entries.items():
                writer.writestr(name, content)
        with self.assertRaisesRegex(ValueError, "integrity check failed"):
            verify_package(corrupted)

    def test_windows_bundle_without_helper_is_rejected_before_output(self) -> None:
        bundle = self.bundle("windows")
        (bundle / "shuvpn_tun_helper.exe").unlink()
        output = self.directory / "output"
        with self.assertRaisesRegex(ValueError, "shuvpn_tun_helper.exe"):
            package_bundle("windows", bundle, output)
        self.assertFalse(output.exists())

    def test_windows_bundle_without_driver_license_is_rejected(self) -> None:
        bundle = self.bundle("windows")
        (bundle / "data/licenses/wintun/LICENSE.txt").unlink()
        output = self.directory / "output"
        with self.assertRaisesRegex(ValueError, "data/licenses/wintun/LICENSE.txt"):
            package_bundle("windows", bundle, output)
        self.assertFalse(output.exists())

    def test_desktop_bundle_requires_open_source_notices(self) -> None:
        for platform in ("windows", "linux"):
            with self.subTest(platform=platform):
                bundle = self.bundle(platform)
                (bundle / "THIRD_PARTY_NOTICES.md").unlink()
                output = self.directory / "output"
                with self.assertRaisesRegex(ValueError, "THIRD_PARTY_NOTICES"):
                    package_bundle(platform, bundle, output)
                self.assertFalse(output.exists())

    def test_linux_archive_preserves_executable_and_records_source(self) -> None:
        archive = package_bundle("linux", self.bundle("linux"), self.directory / "output")
        verify_package(archive)
        with tarfile.open(archive) as reader:
            launcher = next(member for member in reader if member.name.endswith("/shuvpn"))
            self.assertEqual(launcher.mode & 0o111, 0o111)
            manifest_member = next(
                member for member in reader.getmembers() if member.name.endswith("/package.json")
            )
            stream = reader.extractfile(manifest_member)
            self.assertIsNotNone(stream)
            manifest = json.load(stream)
        self.assertEqual(manifest["platform"], "linux")
        self.assertEqual(len(manifest["source"]["revision"]), 40)
        self.assertIn("LICENSE", manifest["files"])
        self.assertIn("install.py", manifest["files"])
        self.assertIn("shuvpn.desktop", manifest["files"])

    def test_linux_bundle_without_installer_is_rejected_before_output(self) -> None:
        bundle = self.bundle("linux")
        (bundle / "install.py").unlink()
        output = self.directory / "output"
        with self.assertRaisesRegex(ValueError, "install.py"):
            package_bundle("linux", bundle, output)
        self.assertFalse(output.exists())

    def test_wrong_architecture_is_rejected_and_does_not_publish_archive(self) -> None:
        bundle = self.bundle("linux")
        (bundle / "shuvpn").write_bytes(fixture_binary("windows"))
        output = self.directory / "output"
        with self.assertRaisesRegex(ValueError, "x64 binary"):
            package_bundle("linux", bundle, output)
        self.assertEqual(list(output.glob("*.tar.gz")), [])

    def test_wrong_plugin_architecture_is_rejected(self) -> None:
        bundle = self.bundle("linux")
        extra = bundle / "lib/optional.so"
        extra.parent.mkdir()
        extra.write_bytes(fixture_binary("windows"))
        with self.assertRaisesRegex(ValueError, "optional.so"):
            package_bundle("linux", bundle, self.directory / "output")

    def test_linux_bundle_without_helper_is_rejected_before_output(self) -> None:
        bundle = self.bundle("linux")
        (bundle / "shuvpn_tun_helper").unlink(missing_ok=True)
        output = self.directory / "output"
        with self.assertRaisesRegex(ValueError, "shuvpn_tun_helper"):
            package_bundle("linux", bundle, output)
        self.assertFalse(output.exists())

    def test_linux_helper_has_executable_permission_and_binary_architecture(self) -> None:
        bundle = self.bundle("linux")
        (bundle / "shuvpn_tun_helper").write_bytes(fixture_binary("linux"))
        archive = package_bundle("linux", bundle, self.directory / "output")
        with tarfile.open(archive) as reader:
            helper = next(member for member in reader if member.name.endswith("/shuvpn_tun_helper"))
            self.assertEqual(helper.mode, 0o755)
        wrong = self.bundle("linux")
        (wrong / "shuvpn_tun_helper").write_bytes(fixture_binary("windows"))
        with self.assertRaisesRegex(ValueError, "shuvpn_tun_helper"):
            package_bundle("linux", wrong, self.directory / "wrong")

    def test_linux_helper_permission_tampering_is_rejected(self) -> None:
        bundle = self.bundle("linux")
        (bundle / "shuvpn_tun_helper").write_bytes(fixture_binary("linux"))
        archive = package_bundle("linux", bundle, self.directory / "output")
        for mode in (0o644, 0o4755):
            corrupted = self.directory / f"mode-{mode}.tar.gz"
            with tarfile.open(archive) as reader, tarfile.open(corrupted, "w:gz") as writer:
                for member in reader:
                    stream = reader.extractfile(member) if member.isfile() else None
                    if member.name.endswith("/shuvpn_tun_helper"):
                        member.mode = mode
                    writer.addfile(member, stream)
            with self.assertRaisesRegex(ValueError, "executable permissions"):
                verify_package(corrupted)


if __name__ == "__main__":
    unittest.main()
