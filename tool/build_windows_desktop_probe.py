"""Build a classified developer window bundle and restore the production target."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

from package_desktop import ROOT, source_identity
from windows_desktop_probe import file_hash

PROBE_ENTRYPOINT = "tool/desktop_window_smoke.dart"
CONNECTED_ENTRYPOINT = "tool/connected_desktop_app.dart"
PROBE_PRODUCTS = ("shuvpn.exe", "shuvpn_core.dll", "data/app.so", "system_tray_plugin.dll")


def main() -> None:
    if sys.platform != "win32":
        raise RuntimeError("Build the Windows developer probe on Windows")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--flutter", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--entrypoint", choices=(PROBE_ENTRYPOINT, CONNECTED_ENTRYPOINT),
                        default=PROBE_ENTRYPOINT)
    args = parser.parse_args()
    output, flutter = args.output.resolve(), args.flutter.resolve(strict=True)
    boundary = (ROOT / "build/desktop").resolve()
    if not output.is_relative_to(boundary) or output == boundary:
        raise RuntimeError("Probe output must be a new directory inside build/desktop")
    output.mkdir()
    original = source_identity()
    entrypoint = args.entrypoint
    result = {"passed": False, "entrypoint": entrypoint, "source": original}
    failure = restore_failure = None
    try:
        with (output / "build.log").open("wb") as log:
            subprocess.run([str(flutter), "build", "windows", "--release",
                            "--target", entrypoint], cwd=ROOT,
                           stdout=log, stderr=subprocess.STDOUT, check=True)
        configuration = (ROOT / "windows/flutter/ephemeral/generated_config.cmake").read_text()
        target_pattern = re.escape(entrypoint).replace("/", r"[/\\]+")
        if not re.search(f'"FLUTTER_TARGET={target_pattern}"', configuration):
            raise RuntimeError("Flutter generated configuration is not the developer probe target")
        if source_identity() != original:
            raise RuntimeError("Source changed during the probe build")
        bundle = output / "bundle"
        shutil.copytree(ROOT / "build/windows/x64/runner/Release", bundle)
        products = {name: file_hash(bundle / name) for name in PROBE_PRODUCTS}
        marker = {"schema": 1, "entrypoint": entrypoint,
                  "source": original, "products": products}
        (bundle / "window-probe.json").write_text(
            json.dumps(marker, indent=2) + "\n", encoding="utf-8")
        result.update(bundle=str(bundle), products=products)
    except BaseException as error:
        failure = error
        result["build_error"] = repr(error)
    finally:
        try:
            with (output / "restore-production.log").open("wb") as log:
                subprocess.run([str(flutter), "build", "windows", "--release",
                                "--target", "lib/main.dart"], cwd=ROOT,
                               stdout=log, stderr=subprocess.STDOUT, check=True)
            configuration = (ROOT / "windows/flutter/ephemeral/generated_config.cmake").read_text()
            if not re.search(r'"FLUTTER_TARGET=lib[/\\]+main\.dart"', configuration):
                raise RuntimeError("Flutter did not restore the production target")
            result["production_restored"] = True
        except BaseException as error:
            restore_failure = error
            result["restore_error"] = repr(error)
        result["passed"] = failure is None and restore_failure is None
        (output / "verification.json").write_text(
            json.dumps(result, indent=2) + "\n", encoding="utf-8")
    if failure is not None:
        raise failure
    if restore_failure is not None:
        raise restore_failure
    print("WINDOWS_DEVELOPER_PROBE_BUILT_AND_PRODUCTION_RESTORED", output / "bundle")


if __name__ == "__main__":
    main()
