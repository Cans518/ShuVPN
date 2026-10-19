#!/usr/bin/env python3
"""Wrap a verified desktop archive in an Inno Setup installer or Debian package."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path, PurePosixPath

if __package__:
    from .package_desktop import ROOT, archive_contents, source_identity, verify_package, verify_release_package
    from .release import clean_checkout, release_version
else:
    from package_desktop import ROOT, archive_contents, source_identity, verify_package, verify_release_package
    from release import clean_checkout, release_version


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_installer_release(archive: Path) -> None:
    revision = clean_checkout(ROOT)
    official = verify_release_package(archive)
    source = source_identity()
    if source["revision"] != revision:
        raise ValueError("Installer source identity must match the actual clean checkout")
    if official["version"] != release_version(ROOT) or official["source"] != source:
        raise ValueError("Official installer and payload must use the same clean source checkout")


def unpack(archive: Path, target: Path) -> dict:
    verify_package(archive)
    files = archive_contents(archive)
    manifests = [name for name in files if name.endswith("/package.json")]
    if len(manifests) != 1:
        raise ValueError("Expected one desktop package manifest")
    prefix = manifests[0].removesuffix("package.json")
    manifest = json.loads(files[manifests[0]])
    if not re.fullmatch(r"\d+\.\d+\.\d+", manifest["version"]):
        raise ValueError("Expected a numeric desktop version")
    for name, content in files.items():
        if not name.startswith(prefix):
            raise ValueError("Archive has multiple roots")
        relative = PurePosixPath(name[len(prefix):])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("Unsafe package path")
        destination = target / relative
        if not destination.resolve().is_relative_to(target.resolve()):
            raise ValueError("Package path escapes the extraction directory")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
        destination.chmod(0o755 if relative.as_posix() in {"shuvpn", "shuvpn_tun_helper"} else 0o644)
    if {path.relative_to(target).as_posix() for path in target.rglob("*") if path.is_file()} != {
        name[len(prefix):] for name in files
    }:
        raise ValueError("Extracted filenames do not match the archive")
    for name, expected in manifest["files"].items():
        path = target / name
        if path.stat().st_size != expected["size"] or sha256(path) != expected["sha256"]:
            raise ValueError(f"Extracted package integrity check failed: {name}")
    return manifest


def build_windows(bundle: Path, manifest: dict, output: Path, iscc: Path) -> Path:
    if os.name != "nt":
        raise ValueError("Build the Windows installer on Windows")
    destination = output / f"ShuVPN-{manifest['version']}-windows-x64-setup.exe"
    if destination.exists():
        raise FileExistsError(destination)
    subprocess.run([
        str(iscc.resolve(strict=True)), "--quiet-progress",
        f"--define=BundleDir={bundle}", f"--define=AppVersion={manifest['version']}",
        f"--define=OutputDir={output}", str(ROOT / "packaging/windows/shuvpn.iss"),
    ], check=True)
    if not destination.is_file():
        raise RuntimeError("Inno Setup did not produce the expected installer")
    return destination


def write_file(path: Path, text: str, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")
    path.chmod(mode)


def build_deb(bundle: Path, manifest: dict, output: Path, staging: Path) -> Path:
    if os.name != "posix" or shutil.which("dpkg-deb") is None:
        raise ValueError("Build the Debian package on Linux with dpkg-deb")
    version = manifest["version"]
    destination = output / f"ShuVPN-{version}-linux-x64.deb"
    if destination.exists():
        raise FileExistsError(destination)
    application = staging / "opt/shuvpn"
    application.parent.mkdir(parents=True)
    shutil.copytree(bundle, application)
    write_file(application / ".package-manager", "dpkg\n")
    readme = (
        f"ShuVPN {version} — Debian/Ubuntu x64\n\n"
        "Install or upgrade: sudo apt install ./ShuVPN-" + version + "-linux-x64.deb\n"
        "Launch ShuVPN from the application menu; do not run the GUI as root.\n"
        "Optional TUN authorization: sudo usermod -aG shuvpn YOUR_LOGIN_NAME\n"
        "Sign out and sign in after authorization. System proxy needs no TUN authorization.\n"
        "Remove: sudo apt remove shuvpn\n"
        "Account/settings data and authorization group membership are retained.\n"
        "Exit ShuVPN from its tray menu before installing, upgrading or removing it.\n"
        "Do not overwrite this installation with the portable install.py script.\n"
    )
    write_file(application / "README.txt", readme)
    installed = dict(manifest)
    installed["distribution"] = "deb"
    installed["files"] = dict(manifest["files"])
    for name in (".package-manager", "README.txt"):
        path = application / name
        installed["files"][name] = {"size": path.stat().st_size, "sha256": sha256(path)}
    write_file(application / "package.json", json.dumps(installed, indent=2) + "\n")
    helper = staging / "usr/lib/shuvpn"
    helper.mkdir(parents=True)
    shutil.copyfile(application / "shuvpn_tun_helper", helper / "shuvpn_tun_helper")
    (helper / "shuvpn_tun_helper").chmod(0o750)
    shutil.copyfile(application / "package.json", helper / "package.json")
    write_file(helper / ".package-manager", "dpkg\n")
    launcher = staging / "usr/share/applications/work.shuvpn.app.desktop"
    launcher.parent.mkdir(parents=True)
    shutil.copyfile(application / "shuvpn.desktop", launcher)
    documentation = staging / "usr/share/doc/shuvpn"
    write_file(documentation / "README.Debian", readme)
    shutil.copyfile(application / "LICENSE", documentation / "copyright")
    control = staging / "DEBIAN"
    write_file(control / "control", (
        f"Package: shuvpn\nVersion: {version}\nArchitecture: amd64\n"
        "Maintainer: ShuVPN contributors <798061533@qq.com>\n"
        "Section: net\nPriority: optional\n"
        "Depends: libc6 (>= 2.35), libgcc-s1, libstdc++6, libgtk-3-0, "
        "libwebkit2gtk-4.1-0, libayatana-appindicator3-1, libcap2-bin, adduser, passwd, python3\n"
        f"Installed-Size: {sum(p.stat().st_size for p in staging.rglob('*') if p.is_file()) // 1024 + 1}\n"
        "Description: Shanghai University desktop VPN and proxy\n"
        " Tauri desktop interface with a native Rust networking core.\n"
        " The GUI runs without privilege; TUN access requires explicit group authorization.\n"
    ))
    lifecycle = (ROOT / "packaging/linux/lifecycle.sh").read_text(encoding="utf-8")
    for operation in ("preinst", "postinst", "prerm", "postrm"):
        write_file(control / operation, f"#!/bin/sh\nset -eu\noperation={operation}\n{lifecycle}", 0o755)
    checksums = []
    for path in sorted(staging.rglob("*")):
        if path.is_file() and control not in path.parents:
            checksums.append(f"{hashlib.md5(path.read_bytes(), usedforsecurity=False).hexdigest()}  {path.relative_to(staging).as_posix()}\n")
    write_file(control / "md5sums", "".join(checksums))
    for directory in staging.rglob("*"):
        if directory.is_dir():
            directory.chmod(0o755)
    subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(staging), str(destination)], check=True)
    return destination


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--iscc", type=Path)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    archive = args.archive.resolve(strict=True)
    if args.release:
        validate_installer_release(archive)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    # A WSL output directory may be on drvfs, which cannot preserve DEB modes.
    with tempfile.TemporaryDirectory(prefix="shuvpn-installer-", dir=output if os.name == "nt" else None) as temporary:
        root = Path(temporary)
        bundle = root / "bundle"
        bundle.mkdir()
        manifest = unpack(archive, bundle)
        if manifest["platform"] == "windows":
            if args.iscc is None:
                parser.error("--iscc is required for Windows")
            installer = build_windows(bundle, manifest, output, args.iscc)
        else:
            installer = build_deb(bundle, manifest, output, root / "debian")
    checksum = sha256(installer)
    write_file(installer.with_name(installer.name + ".sha256"), f"{checksum}  {installer.name}\n")
    metadata = {
        "schema": 1, "product": "ShuVPN", "format": installer.suffix,
        "version": manifest["version"], "platform": manifest["platform"],
        "sha256": checksum, "size": installer.stat().st_size,
        "payload_source": manifest["source"], "builder_source": source_identity(),
        "input_archive": archive.name, "input_sha256": sha256(archive),
    }
    if manifest["platform"] == "windows":
        metadata["compiler"] = {
            "sha256": sha256(args.iscc),
            "version": subprocess.check_output([str(args.iscc), "--version"], text=True).strip(),
        }
    else:
        metadata["compiler"] = {
            "version": subprocess.check_output(["dpkg-deb", "--version"], text=True).splitlines()[0],
        }
    write_file(installer.with_name(installer.name + ".json"), json.dumps(metadata, indent=2) + "\n")
    print(installer)


if __name__ == "__main__":
    main()
