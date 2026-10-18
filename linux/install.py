#!/usr/bin/env python3
"""Install or remove the verified ShuVPN Linux bundle (run with sudo)."""

import argparse
import fcntl
import grp
import hashlib
import json
import os
import pwd
import shutil
import stat
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path, PurePosixPath

APPLICATION = Path("/opt/shuvpn")
HELPER = Path("/usr/lib/shuvpn")
LAUNCHER = Path("/usr/share/applications/work.shuvpn.app.desktop")
GROUP = "shuvpn"
COMMAND_PATH = "/usr/sbin:/usr/bin:/sbin:/bin"
REQUIRED = {
    "shuvpn", "shuvpn_tun_helper", "lib/libshuvpn_core.so",
    "lib/libflutter_linux_gtk.so", "lib/libapp.so", "data/icudtl.dat",
    "install.py", "shuvpn.desktop",
}


def read_manifest(bundle: Path) -> dict:
    manifest = json.loads((bundle / "package.json").read_text(encoding="utf-8"))
    if (
        manifest.get("schema") != 1
        or manifest.get("product") != "ShuVPN"
        or manifest.get("platform") != "linux"
        or manifest.get("architecture") != "x64"
        or not isinstance(manifest.get("files"), dict)
        or not REQUIRED <= manifest["files"].keys()
    ):
        raise ValueError("Expected a complete ShuVPN Linux x64 package")
    for name in manifest["files"]:
        relative = PurePosixPath(name)
        if (
            not name or relative.is_absolute() or "\\" in name
            or ".." in relative.parts or relative.as_posix() != name
            or name == "package.json"
        ):
            raise ValueError(f"Unsafe package path: {name}")
    return manifest


def verify_copy(bundle: Path, destination: Path, manifest: dict) -> None:
    """Validate the copied bytes, so source changes cannot bypass integrity."""
    for name, expected in manifest["files"].items():
        source = bundle / name
        for component in (source, *source.parents):
            if component == bundle:
                break
            if component.is_symlink():
                raise ValueError(f"Package contains a symlink: {name}")
        if not source.is_file():
            raise ValueError(f"Missing package file: {name}")
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        content = target.read_bytes()
        if (
            len(content) != expected["size"]
            or hashlib.sha256(content).hexdigest() != expected["sha256"]
        ):
            raise ValueError(f"Package integrity check failed: {name}")
        target.chmod(0o755 if name in {"shuvpn", "shuvpn_tun_helper"} else 0o644)
    (destination / "package.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    for directory in destination.rglob("*"):
        if directory.is_dir():
            directory.chmod(0o755)
    destination.chmod(0o755)


def trusted_directory(path: Path) -> None:
    for component in reversed((path, *path.parents)):
        metadata = component.lstat()
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != 0 or metadata.st_mode & 0o022
        ):
            raise PermissionError(f"Untrusted installation directory: {component}")


def command(name: str, *arguments: str) -> str:
    executable = shutil.which(name, path=COMMAND_PATH)
    if executable is None:
        raise RuntimeError(f"Missing command: {name} (setcap requires libcap2-bin)")
    result = subprocess.run(
        [executable, *arguments], check=True,
        env={"PATH": COMMAND_PATH, "LC_ALL": "C"},
        text=True, stdout=subprocess.PIPE,
    )
    return result.stdout


@contextmanager
def installation_lock():
    trusted_directory(APPLICATION.parent)
    descriptor = os.open(
        APPLICATION.parent / ".shuvpn-install.lock",
        os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600,
    )
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o077:
            raise PermissionError("Untrusted installation lock")
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield
    finally:
        os.close(descriptor)


def check_existing() -> None:
    for destination in (APPLICATION, HELPER):
        if (destination / ".package-manager").exists():
            raise RuntimeError("This installation is managed by dpkg; use apt to upgrade or remove shuvpn")
        trusted_directory(destination.parent)
        if destination.is_symlink():
            raise PermissionError(f"Refusing a symlink: {destination}")
        if destination.exists():
            trusted_directory(destination)
            read_manifest(destination)
    trusted_directory(LAUNCHER.parent)
    if LAUNCHER.is_symlink():
        raise PermissionError(f"Refusing a symlink: {LAUNCHER}")
    if LAUNCHER.exists():
        metadata = LAUNCHER.stat()
        if not APPLICATION.exists() or metadata.st_uid != 0 or metadata.st_mode & 0o022:
            raise PermissionError(f"Refusing an unrelated launcher: {LAUNCHER}")


@contextmanager
def replace_installation(replacements: list[tuple[Path | None, Path]]):
    """Rollback each published path if any rename or later operation fails."""
    originals = []
    published = []
    recovered = False
    try:
        for index, (staged, destination) in enumerate(replacements):
            # Backups must stay on the destination filesystem.
            holder = Path(tempfile.mkdtemp(prefix=".shuvpn-backup-", dir=destination.parent))
            original = holder / str(index)
            originals.append((original, destination, holder))
            if destination.exists():
                destination.rename(original)
            if staged is not None:
                staged.rename(destination)
                published.append(destination)
        yield
    except BaseException:
        for destination in reversed(published):
            if destination.is_dir():
                shutil.rmtree(destination)
            else:
                destination.unlink()
        for original, destination, _ in reversed(originals):
            if original.exists():
                original.rename(destination)
        recovered = True
        raise
    else:
        recovered = True
    finally:
        # If rollback itself failed, leave backups for administrator recovery.
        if recovered:
            for _, _, holder in originals:
                try:
                    shutil.rmtree(holder)
                except OSError as error:
                    print(
                        f"Installation paths are settled; backup cleanup failed at {holder}: {error}",
                        file=sys.stderr,
                    )


def install(bundle: Path, username: str) -> None:
    if (bundle / ".package-manager").exists():
        raise RuntimeError("Install this distribution using apt/dpkg, not install.py")
    account = pwd.getpwnam(username)
    if account.pw_uid == 0 or username.startswith("-"):
        raise ValueError("--user must name an ordinary local account")
    for name in ("setcap", "getcap", "groupadd", "usermod"):
        if shutil.which(name, path=COMMAND_PATH) is None:
            raise RuntimeError(f"Missing command: {name} (install libcap2-bin for setcap/getcap)")
    manifest = read_manifest(bundle)
    check_existing()
    try:
        authorization = grp.getgrnam(GROUP)
    except KeyError:
        command("groupadd", "--system", GROUP)
        authorization = grp.getgrnam(GROUP)
    if authorization.gr_gid == 0:
        raise PermissionError("The shuvpn authorization group must not be root")
    with (
        tempfile.TemporaryDirectory(prefix=".shuvpn-stage-", dir=APPLICATION.parent) as app_stage,
        tempfile.TemporaryDirectory(prefix=".shuvpn-stage-", dir=HELPER.parent) as helper_stage,
        tempfile.TemporaryDirectory(prefix=".shuvpn-stage-", dir=LAUNCHER.parent) as launcher_stage,
    ):
        application = Path(app_stage) / "application"
        application.mkdir()
        verify_copy(bundle, application, manifest)
        privileged = Path(helper_stage) / "helper"
        privileged.mkdir(mode=0o755)
        executable = privileged / "shuvpn_tun_helper"
        shutil.copyfile(application / "shuvpn_tun_helper", executable)
        os.chown(executable, 0, authorization.gr_gid)
        executable.chmod(0o750)
        command("setcap", "cap_net_admin,cap_net_raw+ep", str(executable))
        # Carry an ownership marker for safe upgrades/removal.
        shutil.copyfile(application / "package.json", privileged / "package.json")
        launcher = Path(launcher_stage) / "launcher"
        shutil.copyfile(application / "shuvpn.desktop", launcher)
        launcher.chmod(0o644)
        with replace_installation([
            (application, APPLICATION), (privileged, HELPER), (launcher, LAUNCHER),
        ]):
            capabilities = command("getcap", str(HELPER / "shuvpn_tun_helper"))
            if not capabilities.strip().endswith(" cap_net_admin,cap_net_raw=ep"):
                raise PermissionError("Installed helper capabilities were not retained")
            command("usermod", "--append", "--groups", GROUP, "--", username)
    print(f"Installed ShuVPN for {username}. Sign out and sign in before using TUN.")
    print("Launch ShuVPN from the application menu. Account/settings files were not changed.")


def uninstall() -> None:
    check_existing()
    # Renames can roll back. Backup deletion happens only after commit.
    with replace_installation([
        (None, destination) for destination in (LAUNCHER, HELPER, APPLICATION)
    ]):
        pass
    print("Removed ShuVPN. User data and shuvpn group membership were retained.")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--user", help="Install and authorize this ordinary account")
    action.add_argument("--uninstall", action="store_true")
    arguments = parser.parse_args()
    if os.geteuid() != 0:
        parser.error("run with sudo; the desktop application itself stays unprivileged")
    os.umask(0o022)
    with installation_lock():
        if arguments.uninstall:
            uninstall()
        else:
            install(Path(__file__).resolve().parent, arguments.user)


if __name__ == "__main__":
    main()
