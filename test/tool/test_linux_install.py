import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

if sys.platform == "linux":
    specification = importlib.util.spec_from_file_location(
        "linux_install", Path(__file__).resolve().parents[2] / "packaging/linux/install.py"
    )
    installer = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(installer)
else:
    installer = None


@unittest.skipUnless(sys.platform == "linux", "Linux installer")
class LinuxInstallTest(unittest.TestCase):
    def test_managed_bundle_cannot_be_republished_by_portable_installer(self) -> None:
        (self.bundle / ".package-manager").write_text("dpkg\n")
        with self.assertRaisesRegex(RuntimeError, "apt/dpkg"):
            installer.install(self.bundle, "nobody")

    def test_managed_destination_cannot_be_removed_by_portable_installer(self) -> None:
        managed = self.directory / "managed"
        managed.mkdir()
        (managed / ".package-manager").write_text("dpkg\n")
        with patch.object(installer, "APPLICATION", managed):
            with self.assertRaisesRegex(RuntimeError, "managed by dpkg"):
                installer.uninstall()
        self.assertTrue((managed / ".package-manager").exists())

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.bundle = self.directory / "bundle"
        self.bundle.mkdir()
        inventory = {}
        for name in installer.REQUIRED:
            content = name.encode()
            path = self.bundle / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            inventory[name] = {
                "size": len(content), "sha256": hashlib.sha256(content).hexdigest(),
            }
        self.manifest = {
            "schema": 1, "product": "ShuVPN", "platform": "linux",
            "architecture": "x64", "files": inventory,
        }
        self.write_manifest()

    def write_manifest(self) -> None:
        (self.bundle / "package.json").write_text(json.dumps(self.manifest))

    def destination(self, name: str, content: str) -> Path:
        path = self.directory / name
        path.mkdir()
        (path / "version").write_text(content)
        return path

    def test_copy_validates_installed_bytes_and_limits_executable_modes(self) -> None:
        manifest = installer.read_manifest(self.bundle)
        target = self.directory / "copied"
        target.mkdir()
        installer.verify_copy(self.bundle, target, manifest)
        self.assertEqual((target / "shuvpn").stat().st_mode & 0o777, 0o755)
        self.assertEqual((target / "install.py").stat().st_mode & 0o777, 0o644)
        self.assertEqual(installer.read_manifest(target), manifest)

    def test_corrupted_source_is_rejected_after_copy(self) -> None:
        (self.bundle / "shuvpn").write_bytes(b"changed")
        target = self.directory / "copied"
        target.mkdir()
        with self.assertRaisesRegex(ValueError, "integrity check failed"):
            installer.verify_copy(self.bundle, target, self.manifest)

    def test_unsafe_inventory_paths_are_rejected(self) -> None:
        for name in ("../escape", "/absolute", "lib/../escape", "lib\\escape", "lib//escape"):
            with self.subTest(name=name):
                self.manifest["files"][name] = {"size": 0, "sha256": ""}
                self.write_manifest()
                with self.assertRaisesRegex(ValueError, "Unsafe package path"):
                    installer.read_manifest(self.bundle)
                del self.manifest["files"][name]

    def test_symlinked_parent_in_source_is_rejected(self) -> None:
        nested = self.bundle / "lib/optional.dat"
        nested.parent.mkdir()
        nested.write_bytes(b"optional")
        self.manifest["files"]["lib/optional.dat"] = {
            "size": 8, "sha256": hashlib.sha256(b"optional").hexdigest(),
        }
        (self.bundle / "lib").rename(self.bundle / "linked-lib")
        (self.bundle / "lib").symlink_to(self.bundle / "linked-lib", target_is_directory=True)
        target = self.directory / "copied"
        target.mkdir()
        with self.assertRaisesRegex(ValueError, "symlink"):
            installer.verify_copy(self.bundle, target, self.manifest)

    def test_failed_second_publish_restores_both_previous_directories(self) -> None:
        first = self.destination("first", "old-first")
        second = self.destination("second", "old-second")
        staged = self.destination("staged", "new-first")
        with self.assertRaises(FileNotFoundError):
            with installer.replace_installation([
                (staged, first), (self.directory / "missing", second),
            ]):
                self.fail("Publishing a nonexistent directory succeeded")
        self.assertEqual((first / "version").read_text(), "old-first")
        self.assertEqual((second / "version").read_text(), "old-second")

    def test_failed_authorization_restores_previous_installation(self) -> None:
        destination = self.destination("installed", "old")
        staged = self.destination("staged", "new")
        with self.assertRaisesRegex(RuntimeError, "authorization"):
            with installer.replace_installation([(staged, destination)]):
                self.assertEqual((destination / "version").read_text(), "new")
                raise RuntimeError("authorization failed")
        self.assertEqual((destination / "version").read_text(), "old")

    def test_failed_removal_rename_restores_preceding_path(self) -> None:
        destination = self.destination("installed", "old")
        with patch.object(Path, "rename", autospec=True, side_effect=OSError("rename failed")):
            with self.assertRaisesRegex(OSError, "rename failed"):
                with installer.replace_installation([(None, destination)]):
                    self.fail("rename failure was hidden")
        self.assertEqual((destination / "version").read_text(), "old")

    def test_successful_removal_leaves_no_backup(self) -> None:
        destination = self.destination("installed", "old")
        with installer.replace_installation([(None, destination)]):
            self.assertFalse(destination.exists())
        self.assertEqual(list(self.directory.glob(".shuvpn-backup-*")), [])

    def test_cleanup_failure_reports_committed_paths_and_preserves_backup(self) -> None:
        destination = self.destination("installed", "old")
        staged = self.destination("staged", "new")
        with (
            patch.object(installer.shutil, "rmtree", side_effect=OSError("cleanup failed")),
            patch.object(installer.sys, "stderr") as stderr,
        ):
            with installer.replace_installation([(staged, destination)]):
                pass
        self.assertEqual((destination / "version").read_text(), "new")
        self.assertEqual(len(list(self.directory.glob(".shuvpn-backup-*"))), 1)
        self.assertIn(
            "backup cleanup failed",
            "".join(call.args[0] for call in stderr.write.call_args_list),
        )


if __name__ == "__main__":
    unittest.main()
