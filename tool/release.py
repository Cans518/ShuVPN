"""Validate application release metadata and extract the public release notes."""

import argparse
import json
from pathlib import Path
import re
import subprocess
from typing import Mapping

ROOT = Path(__file__).resolve().parents[1]
STABLE_VERSION = r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)"


def _toml_value(block: str, key: str) -> str:
    match = re.search(rf'(?m)^{re.escape(key)}\s*=\s*"([^"]+)"\s*$', block)
    if match is None:
        raise ValueError(f"Missing literal TOML {key}")
    return match[1]


def release_version(root: Path = ROOT) -> str:
    package = json.loads((root / "desktop/package.json").read_text(encoding="utf-8"))
    version = package["version"]
    if not isinstance(version, str) or re.fullmatch(STABLE_VERSION, version) is None:
        raise ValueError("An official release requires a stable numeric application version")
    npm_lock = json.loads((root / "desktop/package-lock.json").read_text(encoding="utf-8"))
    configuration = json.loads((root / "desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    manifest = (root / "desktop/src-tauri/Cargo.toml").read_text(encoding="utf-8")
    block = re.search(r"(?ms)^\[package\]\s*\n(.*?)(?=^\[|\Z)", manifest)
    if block is None:
        raise ValueError("Missing desktop Cargo package")
    package_name = _toml_value(block[1], "name")
    cargo_lock = (root / "desktop/src-tauri/Cargo.lock").read_text(encoding="utf-8")
    resolved = next((entry for entry in re.split(r"(?m)^\[\[package\]\]\s*$", cargo_lock)[1:]
                     if re.search(rf'(?m)^name = "{re.escape(package_name)}"$', entry)), None)
    if resolved is None:
        raise ValueError("Desktop package is missing from Cargo.lock")
    versions = {
        "tauri.conf.json": configuration["version"],
        "Cargo.toml": _toml_value(block[1], "version"),
        "Cargo.lock": _toml_value(resolved, "version"),
        "package-lock.json": npm_lock["version"],
        "package-lock.json root": npm_lock["packages"][""]["version"],
    }
    for name, found in versions.items():
        if found != version:
            raise ValueError(f"Version mismatch: {name} has {found}, expected {version}")
    return version


def validate_tag(version: str, tag: str) -> None:
    if tag != f"v{version}":
        raise ValueError(f"Release tag must be v{version}, got {tag}")


def release_notes(root: Path = ROOT) -> str:
    version = release_version(root)
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    section = re.search(rf"(?ms)^## {re.escape(version)} - \d{{4}}-\d{{2}}-\d{{2}}\s*\n(.*?)(?=^## |\Z)", changelog)
    if section is None or not section[1].strip():
        raise ValueError(f"CHANGELOG.md is missing public release notes for {version}")
    return section[1].strip() + "\n"


def clean_checkout(root: Path = ROOT) -> str:
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    status = subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True)
    if status.strip():
        raise ValueError("Official releases require a clean, committed source checkout")
    return revision


def validate_source(source: Mapping[str, object]) -> None:
    if (
        source.get("dirty") is not False
        or re.fullmatch(r"[0-9a-f]{40}", str(source.get("revision", ""))) is None
        or re.fullmatch(r"[0-9a-f]{64}", str(source.get("tree_sha256", ""))) is None
        or not isinstance(source.get("file_count"), int)
        or source["file_count"] <= 0
    ):
        raise ValueError("Official release source must be clean and include a full revision and fingerprint")


def validate_build(info: Mapping[str, object], platform: str, version: str) -> None:
    expected = {"schema": 1, "product": "ShuVPN", "version": version,
                "profile": "release", "frontend": "embedded", "platform": platform, "architecture": "x86_64"}
    if set(info) != set(expected) or any(info.get(key) != value for key, value in expected.items()):
        raise ValueError(f"Expected a {platform} x64 release build of ShuVPN {version}")


def read_build_info(executable: Path) -> dict:
    try:
        info = json.loads(subprocess.check_output(
            [str(executable.resolve(strict=True)), "--build-info"], text=True, timeout=15,
        ))
    except (OSError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        raise ValueError(f"Cannot read build identity from {executable.name}") from error
    if not isinstance(info, dict):
        raise ValueError("Build identity must be a JSON object")
    return info


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check")
    check.add_argument("--tag")
    check.add_argument("--clean", action="store_true")
    notes = commands.add_parser("notes")
    notes.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    version = release_version()
    contents = release_notes()
    if args.command == "check":
        if args.tag:
            validate_tag(version, args.tag)
        revision = clean_checkout() if args.clean else None
        print(json.dumps({"version": version, "tag": f"v{version}", "revision": revision}, indent=2))
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(contents, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
