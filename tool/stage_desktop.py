"""Stage a Tauri executable and the separately built native TUN helper."""

import argparse
from pathlib import Path
import shutil
import json

if __package__:
    from .release import clean_checkout, release_version, validate_build, read_build_info
else:
    from release import clean_checkout, release_version, validate_build, read_build_info

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("windows", "linux"), required=True)
    parser.add_argument("--binaries", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    suffix = ".exe" if args.platform == "windows" else ""
    sources = {
        "shuvpn" + suffix: args.binaries / ("shuvpn" + suffix),
        "shuvpn_tun_helper" + suffix: args.binaries / ("shuvpn_tun_helper" + suffix),
        "THIRD_PARTY_NOTICES.md": ROOT / "desktop/THIRD_PARTY_NOTICES.md",
        "icon.png": ROOT / "desktop/src-tauri/icons/icon.png",
    }
    if args.platform == "windows":
        sources.update({
            "wintun.dll": ROOT / "desktop/resources/wintun/amd64/wintun.dll",
            "data/licenses/wintun/LICENSE.txt": ROOT / "desktop/resources/wintun/LICENSE.txt",
        })
    else:
        sources.update({
            name: ROOT / "packaging/linux" / name
            for name in ("install.py", "shuvpn.desktop")
        })
    for source in sources.values():
        source.resolve(strict=True)
    info = None
    if args.release:
        clean_checkout(ROOT)
        version = release_version(ROOT)
        executable = sources["shuvpn" + suffix].resolve()
        info = read_build_info(executable)
        validate_build(info, args.platform, version)
    args.output.mkdir(parents=True, exist_ok=False)
    for name, source in sources.items():
        destination = args.output / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        destination.chmod(0o755 if name in {"shuvpn", "shuvpn_tun_helper"} else 0o644)
    if info is not None:
        (args.output / "build-info.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    print(args.output.resolve())


if __name__ == "__main__":
    main()
