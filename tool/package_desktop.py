#!/usr/bin/env python3
"""Package and verify complete Flutter desktop release bundles."""

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = "package.json"
REQUIRED = {
    "windows": (
        "shuvpn.exe",
        "shuvpn_core.dll",
        "shuvpn_tun_helper.exe",
        "flutter_windows.dll",
        "wintun.dll",
        "msvcp140.dll",
        "vcruntime140.dll",
        "vcruntime140_1.dll",
        "window_manager_plugin.dll",
        "system_tray_plugin.dll",
        "url_launcher_windows_plugin.dll",
        "data/app.so",
        "data/icudtl.dat",
        "data/licenses/wintun/LICENSE.txt",
        "data/licenses/system_tray/LICENSE",
    ),
    "linux": (
        "shuvpn",
        "shuvpn_tun_helper",
        "lib/libshuvpn_core.so",
        "lib/libflutter_linux_gtk.so",
        "lib/libwindow_manager_plugin.so",
        "lib/libsystem_tray_plugin.so",
        "lib/liburl_launcher_linux_plugin.so",
        "lib/libapp.so",
        "data/icudtl.dat",
        "install.py",
        "shuvpn.desktop",
    ),
}
LINUX_EXECUTABLES = {"shuvpn", "shuvpn_tun_helper"}
FLUTTER_ASSETS = (
    "data/flutter_assets/AssetManifest.bin",
    "data/flutter_assets/FontManifest.json",
    "data/flutter_assets/NativeAssetsManifest.json",
    "data/flutter_assets/NOTICES.Z",
    "data/flutter_assets/fonts/MaterialIcons-Regular.otf",
    "data/flutter_assets/assets/fonts/LXGWWenKaiScreen.ttf",
    "data/flutter_assets/assets/fonts/OFL.txt",
)


def digest(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def app_version() -> str:
    text = (ROOT / "pubspec.yaml").read_text(encoding="utf-8")
    match = re.search(r"^version:\s*(\d+\.\d+\.\d+\+\d+)\s*$", text, re.MULTILINE)
    if match is None:
        raise ValueError("pubspec.yaml must define a version and build number")
    return match.group(1)


def validate_files(names: set[str], platform: str) -> None:
    for name in names:
        validate_relative_path(name)
    missing = set(REQUIRED[platform]) | set(FLUTTER_ASSETS)
    missing.update(
        "data/flutter_assets/" + path.relative_to(ROOT).as_posix()
        for path in (ROOT / "assets" / "images").rglob("*") if path.is_file()
    )
    missing -= names
    if missing:
        raise ValueError(f"Incomplete {platform} bundle: {', '.join(sorted(missing))}")
    debug_files = [name for name in names if name.lower().endswith((".pdb", ".ilk"))]
    if debug_files:
        raise ValueError(f"Debug artifacts in release bundle: {debug_files}")


def validate_relative_path(name: str) -> None:
    relative = PurePosixPath(name)
    reserved = {"CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$",
                *(f"COM{i}" for i in (*range(1, 10), "¹", "²", "³")),
                *(f"LPT{i}" for i in (*range(1, 10), "¹", "²", "³"))}
    if (
        not name or not relative.parts or relative.is_absolute() or relative.as_posix() != name
        or any(character in name for character in '\\:<>|"*?') or ".." in relative.parts
        or any(ord(character) < 32 for character in name)
        or any(part.endswith((" ", ".")) or part.split(".")[0].upper() in reserved for part in relative.parts)
    ):
        raise ValueError(f"Unsafe package path: {name}")


def validate_architecture(content: bytes, platform: str, name: str) -> None:
    if platform == "windows":
        if len(content) >= 64 and content[:2] == b"MZ":
            offset = struct.unpack_from("<I", content, 60)[0]
            if offset + 6 <= len(content):
                if content[offset : offset + 6] == b"PE\x00\x00\x64\x86":
                    return
    elif (
        len(content) >= 20
        and content[:6] == b"\x7fELF\x02\x01"
        and struct.unpack_from("<H", content, 18)[0] == 62
    ):
        return
    raise ValueError(f"Expected an x64 binary: {name}")


def bundle_files(bundle: Path) -> dict[str, Path]:
    files = {}
    for path in sorted(bundle.rglob("*")):
        if not path.resolve().is_relative_to(bundle):
            raise ValueError(f"Bundle link escapes its directory: {path}")
        if path.is_file():
            files[path.relative_to(bundle).as_posix()] = path
    return files


def source_identity() -> dict[str, object]:
    revision = os.environ.get("SHUVPN_SOURCE_REVISION")
    if revision is None:
        revision = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
        status = subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=ROOT, text=True
        )
        dirty = bool(status.strip())
    else:
        dirty_value = os.environ.get("SHUVPN_SOURCE_DIRTY")
        if dirty_value not in ("true", "false"):
            raise ValueError("Source snapshots must declare SHUVPN_SOURCE_DIRTY=true or false")
        dirty = dirty_value == "true"
    if re.fullmatch(r"[0-9a-f]{40}", revision) is None:
        raise ValueError("Source revision must be a full Git commit hash")
    fingerprint = hashlib.sha256()
    paths = []
    for directory in ("lib", "rust", "rust_builder", "windows", "linux", "assets", "tool", "test", ".github"):
        for current, directories, filenames in os.walk(ROOT / directory):
            directories[:] = sorted(
                name for name in directories
                if name not in ("target", "build", "ephemeral", ".plugin_symlinks", "__pycache__")
            )
            paths.extend(Path(current) / name for name in filenames)
    paths.extend(ROOT / name for name in (
        "pubspec.yaml", "pubspec.lock", "flutter_rust_bridge.yaml", "analysis_options.yaml"
    ))
    for path in sorted(paths, key=lambda path: path.relative_to(ROOT).as_posix()):
        fingerprint.update(path.relative_to(ROOT).as_posix().encode("utf-8") + b"\0")
        fingerprint.update(bytes.fromhex(digest(path.read_bytes())))
    return {
        "revision": revision, "dirty": dirty,
        "tree_sha256": fingerprint.hexdigest(), "file_count": len(paths),
    }


def linux_archive_member(member: tarfile.TarInfo) -> tarfile.TarInfo:
    member.uid = member.gid = 0
    member.uname = member.gname = ""
    member.mode = (
        0o755 if member.isdir() or member.name.rsplit("/", 1)[-1] in LINUX_EXECUTABLES
        else 0o644
    )
    return member


def package_bundle(platform: str, bundle: Path, output: Path) -> Path:
    bundle = bundle.resolve(strict=True)
    output = output.resolve()
    if output.is_relative_to(bundle):
        raise ValueError("Package output must be outside the input bundle")
    files = bundle_files(bundle)
    validate_files(set(files), platform)
    version = app_version()
    name = f"ShuVPN-{version}-{platform}-x64"
    output.mkdir(parents=True, exist_ok=True)
    extension = ".zip" if platform == "windows" else ".tar.gz"
    destination = output / f"{name}{extension}"
    if destination.exists():
        raise FileExistsError(f"Package already exists: {destination}")
    with tempfile.TemporaryDirectory(prefix="desktop-package-", dir=output) as temporary:
        staging = Path(temporary) / name
        staging.mkdir()
        for relative, source in files.items():
            target = staging / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            target.chmod(0o755 if relative in LINUX_EXECUTABLES else 0o644)
        shutil.copyfile(ROOT / "LICENSE", staging / "LICENSE")
        executable = "shuvpn.exe" if platform == "windows" else "./shuvpn"
        (staging / "README.txt").write_text(
            f"ShuVPN {version} — {platform} x64\n\n"
            f"Extract this entire directory, then run {executable}.\n"
            "Keep data/ and all bundled libraries beside the executable.\n"
            "Account and settings data are stored in the user application directory.\n"
            "Disconnect and exit the application before replacing this directory.\n"
            + (
                "\nLinux requires a graphical desktop with GTK 3, libstdc++ and "
                "Ayatana AppIndicator. This bundle targets Ubuntu 22.04 or a "
                "compatible system. The separate shuvpn_tun_helper is included "
                "without file capabilities. To install, exit ShuVPN and run:\n"
                "  sudo python3 ./install.py --user YOUR_LOGIN_NAME\n"
                "The installer requires Python 3 and libcap2-bin, installs the GUI "
                "to /opt/shuvpn and the restricted helper to /usr/lib/shuvpn, "
                "and authorizes the named account through the shuvpn group. "
                "Sign out and sign in, then use the application menu. "
                "To remove: sudo python3 /opt/shuvpn/install.py --uninstall\n"
                "Removal retains account/settings data and group membership. "
                "Use system proxy mode until the application reports TUN available. "
                "Do not grant capabilities "
                "to the GTK executable. System proxy support depends on the desktop environment.\n"
                if platform == "linux"
                else "\nRun shuvpn.exe from the extracted directory. "
                "The bundled helper and Wintun driver must remain beside it.\n"
            ),
            encoding="utf-8",
        )
        inventory = {}
        for path in sorted(staging.rglob("*")):
            if path.is_file():
                content = path.read_bytes()
                inventory[path.relative_to(staging).as_posix()] = {
                    "size": len(content),
                    "sha256": digest(content),
                }
        manifest = {
            "schema": 1,
            "product": "ShuVPN",
            "version": version,
            "platform": platform,
            "architecture": "x64",
            "source": source_identity(),
            "files": inventory,
        }
        (staging / MANIFEST).write_text(
            json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
        archive = Path(temporary) / destination.name
        if platform == "windows":
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as writer:
                for path in sorted(staging.rglob("*")):
                    if path.is_file():
                        writer.write(path, f"{name}/{path.relative_to(staging).as_posix()}")
        else:
            with tarfile.open(archive, "w:gz") as writer:
                writer.add(staging, arcname=name, filter=linux_archive_member)
        verify_package(archive)
        archive.replace(destination)
    checksum = destination.with_name(destination.name + ".sha256")
    checksum.write_text(
        f"{digest(destination.read_bytes())}  {destination.name}\n", encoding="ascii"
    )
    return destination


def archive_contents(archive: Path) -> dict[str, bytes]:
    contents = {}
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as reader:
            for member in reader.infolist():
                validate_relative_path(member.filename.rstrip("/"))
                if not member.is_dir():
                    if member.filename in contents:
                        raise ValueError(f"Duplicate archive entry: {member.filename}")
                    contents[member.filename] = reader.read(member)
    elif archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as reader:
            for member in reader.getmembers():
                validate_relative_path(member.name.rstrip("/"))
                if member.isdir():
                    continue
                if not member.isfile() or member.name in contents:
                    raise ValueError(f"Unexpected archive entry: {member.name}")
                if member.name.rsplit("/", 1)[-1] in LINUX_EXECUTABLES and member.mode != 0o755:
                    raise ValueError(f"Invalid Linux executable permissions: {member.name}")
                stream = reader.extractfile(member)
                if stream is None:
                    raise ValueError(f"Cannot read archive entry: {member.name}")
                contents[member.name] = stream.read()
    else:
        raise ValueError("Expected a .zip or .tar.gz desktop package")
    return contents


def verify_package(archive: Path) -> None:
    contents = archive_contents(archive)
    manifests = [name for name in contents if name.endswith("/" + MANIFEST)]
    if len(manifests) != 1:
        raise ValueError("Package must contain exactly one manifest")
    manifest_path = manifests[0]
    prefix = manifest_path.removesuffix(MANIFEST)
    manifest = json.loads(contents.pop(manifest_path))
    platform = manifest["platform"]
    version = manifest.get("version")
    if (
        manifest.get("schema") != 1 or platform not in REQUIRED
        or manifest.get("product") != "ShuVPN"
        or not isinstance(version, str)
        or re.fullmatch(r"\d+\.\d+\.\d+\+\d+", version) is None
    ):
        raise ValueError("Unsupported desktop package manifest")
    expected_prefix = f"ShuVPN-{manifest['version']}-{platform}-x64/"
    if prefix != expected_prefix or manifest.get("architecture") != "x64":
        raise ValueError("Package directory and manifest disagree")
    files = manifest["files"]
    validate_files(set(files), platform)
    if set(contents) != {prefix + name for name in files}:
        raise ValueError("Archive files and manifest disagree")
    for name, expected in files.items():
        parts = Path(name).parts
        if "\\" in name or name.startswith("/") or ".." in parts:
            raise ValueError(f"Unsafe package path: {name}")
        content = contents[prefix + name]
        if len(content) != expected["size"] or digest(content) != expected["sha256"]:
            raise ValueError(f"Package integrity check failed: {name}")
        if name.endswith((".dll", ".exe", ".so")) or name in LINUX_EXECUTABLES:
            binary_platform = "windows" if name.endswith((".dll", ".exe")) else "linux"
            validate_architecture(content, binary_platform, name)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("build")
    build.add_argument("--platform", choices=REQUIRED, required=True)
    build.add_argument("--bundle", type=Path, required=True)
    build.add_argument("--output", type=Path, default=ROOT / "build" / "desktop")
    verify = commands.add_parser("verify")
    verify.add_argument("archive", type=Path)
    args = parser.parse_args()
    if args.command == "build":
        print(package_bundle(args.platform, args.bundle, args.output))
    else:
        verify_package(args.archive)
        print(f"DESKTOP_PACKAGE_INTEGRITY_PASSED {args.archive}")


if __name__ == "__main__":
    main()
