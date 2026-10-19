import json
import shutil
import subprocess
import tarfile
import unittest
import zipfile
from unittest.mock import patch

from tool.package_desktop import package_bundle
from tool.package_installers import build_deb, unpack, validate_installer_release
import test_package_desktop as fixtures


class InstallerPackageTest(fixtures.DesktopBundleFixture, unittest.TestCase):
    def test_release_installer_cannot_relabel_a_payload_or_checkout(self):
        source = {"revision": "a" * 40, "dirty": False, "tree_sha256": "b" * 64, "file_count": 10}
        with patch("tool.package_installers.clean_checkout", return_value=source["revision"]), \
                patch("tool.package_installers.release_version", return_value="0.6.0"), \
                patch("tool.package_installers.verify_release_package", return_value={"version": "0.6.0", "source": source}):
            with patch("tool.package_installers.source_identity", return_value=source):
                validate_installer_release(self.directory / "archive.zip")
            with patch("tool.package_installers.source_identity", return_value={**source, "revision": "c" * 40}):
                with self.assertRaisesRegex(ValueError, "actual clean checkout"):
                    validate_installer_release(self.directory / "archive.zip")
            with patch("tool.package_installers.source_identity", return_value={**source, "tree_sha256": "d" * 64}):
                with self.assertRaisesRegex(ValueError, "same clean source"):
                    validate_installer_release(self.directory / "archive.zip")

    def test_verified_archive_is_required(self):
        archive = self.directory / "bad.zip"
        archive.write_bytes(b"not a package")
        with self.assertRaises(zipfile.BadZipFile):
            unpack(archive, self.directory / "unpacked")
        self.assertFalse((self.directory / "unpacked").exists())

    @unittest.skipUnless(shutil.which("dpkg-deb"), "Debian tooling required")
    def test_deb_owns_payload_and_configures_only_the_separate_helper(self):
        output = self.directory / "output"
        archive = package_bundle("linux", self.bundle("linux"), output)
        bundle = self.directory / "unpacked"
        bundle.mkdir()
        manifest = unpack(archive, bundle)
        deb = build_deb(bundle, manifest, output, self.directory / "stage")
        fields = subprocess.check_output(
            ["dpkg-deb", "-f", str(deb), "Package", "Version", "Architecture"], text=True
        )
        self.assertIn("Package: shuvpn", fields)
        self.assertIn("Architecture: amd64", fields)
        data = self.directory / "data.tar"
        with data.open("wb") as stream:
            subprocess.run(["dpkg-deb", "--fsys-tarfile", str(deb)], stdout=stream, check=True)
        with tarfile.open(data) as reader:
            helper = reader.getmember("./usr/lib/shuvpn/shuvpn_tun_helper")
            gui = reader.getmember("./opt/shuvpn/shuvpn")
            self.assertEqual((helper.uid, helper.gid, helper.mode), (0, 0, 0o750))
            self.assertEqual(gui.mode, 0o755)
            marker = reader.extractfile("./opt/shuvpn/.package-manager").read()
            self.assertEqual(marker, b"dpkg\n")
            installed = json.load(reader.extractfile("./opt/shuvpn/package.json"))
            self.assertEqual(installed["distribution"], "deb")
            self.assertIn(".package-manager", installed["files"])
            self.assertEqual(installed["source"], manifest["source"])
            self.assertEqual(
                reader.extractfile("./opt/shuvpn/shuvpn").read(),
                (bundle / "shuvpn").read_bytes(),
            )
        for script in ("preinst", "postinst", "prerm", "postrm"):
            subprocess.run(["sh", "-n", str(self.directory / "stage/DEBIAN" / script)], check=True)


if __name__ == "__main__":
    unittest.main()
