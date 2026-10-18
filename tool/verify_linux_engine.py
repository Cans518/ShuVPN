"""Check a source-built Linux engine and its test receipts against the pinned patch."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

MANIFEST = Path(__file__).resolve().parent / "flutter_engine/source.json"


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def child_path(root, name):
    relative = Path(name)
    if relative.is_absolute() or not relative.parts or any(
            part in (".", "..") for part in relative.parts) or "\\" in name:
        raise ValueError(f"Invalid engine-relative path: {name}")
    target = (root / relative).resolve()
    if not target.is_relative_to(root.resolve()):
        raise ValueError(f"Engine path escapes its root: {name}")
    return target


def load_manifest(path=MANIFEST):
    manifest = read_json(path)
    if manifest.get("schema") != 1:
        raise ValueError("Unsupported Linux engine manifest")
    patch = child_path(path.parent, manifest["patch"])
    if digest(patch) != manifest["patch_sha256"]:
        raise ValueError("Linux engine patch digest differs from its manifest")
    return manifest


def verify_engine(source, build_receipt, test_receipt, bundle=None):
    manifest = load_manifest()
    head = subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if head != manifest["framework_git"]:
        raise ValueError("Linux engine SDK Git revision differs from the pin")
    expected = {name: row["after"] for name, row in manifest["source_files"].items()}
    changed = subprocess.check_output(
        ["git", "-C", str(source), "diff", "--name-only", "HEAD"], text=True).splitlines()
    if set(changed) - set(expected):
        raise ValueError("Linux engine checkout has additional tracked modifications")
    for name, checksum in expected.items():
        if digest(child_path(source, name)) != checksum:
            raise ValueError(f"Linux engine patched source differs: {name}")
    build = read_json(build_receipt)
    if (build.get("running") is not False or build.get("exit") != 0 or
            build.get("source_unchanged") is not True or
            build.get("source_inputs") != expected):
        raise ValueError("Linux engine build receipt does not match the pinned inputs")
    outputs = build.get("outputs") or {}
    required = {"libflutter_linux_gtk.so", "flutter_linux_unittests"}
    if not required.issubset(outputs):
        raise ValueError("Linux engine build receipt lacks required outputs")
    output = child_path(source / "engine/src/out", manifest["engine_output"])
    for name, checksum in outputs.items():
        if digest(child_path(output, name)) != checksum:
            raise ValueError(f"Linux engine build output differs: {name}")
    tests = read_json(test_receipt)
    selected_filter = "--gtest_filter=" + manifest["test_filter"]
    if (tests.get("exit") != 0 or tests.get("failures") != 0 or tests.get("errors") != 0 or
            tests.get("tests") != manifest["selected_tests"] or
            tests.get("binary_sha256") != outputs["flutter_linux_unittests"] or
            (tests.get("command") or []).count(selected_filter) != 1):
        raise ValueError("Linux engine test receipt does not cover the built test binary")
    if bundle is not None and digest(bundle / "lib/libflutter_linux_gtk.so") != outputs[
            "libflutter_linux_gtk.so"]:
        raise ValueError("Linux bundle maps a different engine than the checked build")
    return {"framework_git": head, "patch_sha256": manifest["patch_sha256"],
            "source_inputs": expected, "outputs": outputs, "selected_tests": tests["tests"],
            "bundle_engine_checked": bundle is not None,
            "scope": "local source, build and selected-test receipt consistency; not a signature"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--build-receipt", required=True, type=Path)
    parser.add_argument("--test-receipt", required=True, type=Path)
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    try:
        record = verify_engine(args.source, args.build_receipt, args.test_receipt, args.bundle)
        encoded = json.dumps(record, indent=2) + "\n"
        if args.report is not None:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            with args.report.open("x", encoding="utf-8") as stream:
                stream.write(encoded)
        print(encoded, end="")
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as failure:
        parser.exit(1, f"Linux engine verification failed: {failure}\n")


if __name__ == "__main__":
    main()
