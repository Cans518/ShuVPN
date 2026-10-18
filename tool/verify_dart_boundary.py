"""Check the reviewed production-Dart inventory; this is not a semantic proof."""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROLES = {"composition", "view", "view-model", "native-adapter", "platform-ui", "generated-frb"}
BACKEND_PACKAGES = {"http", "dio", "crypto", "cryptography", "html"}
BACKEND_DART = {"dart:html", "dart:js", "dart:js_interop", "dart:mirrors"}
ROOT = Path(__file__).resolve().parent.parent
GENERATOR = "flutter_rust_bridge_codegen 2.11.1"


class BoundaryError(ValueError):
    pass


def text_hash(path):
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n")
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def production_files(root):
    return {path.relative_to(root).as_posix(): path
            for directory in (root / "lib", root / "rust_builder/lib")
            if directory.is_dir() for path in directory.rglob("*.dart")}


def _layout(text, position):
    while position < len(text):
        if text[position].isspace() or text[position] == "\ufeff":
            position += 1
        elif text.startswith("//", position):
            end = text.find("\n", position)
            position = len(text) if end < 0 else end + 1
        elif text.startswith("/*", position):
            depth = 1
            position += 2
            while position < len(text) and depth:
                if text.startswith("/*", position):
                    depth += 1
                    position += 2
                elif text.startswith("*/", position):
                    depth -= 1
                    position += 2
                else:
                    position += 1
            if depth:
                raise BoundaryError("Unterminated Dart header comment")
        else:
            break
    return position


def _string_end(text, position, raw=False):
    quote = text[position]
    delimiter = quote * 3 if text.startswith(quote * 3, position) else quote
    cursor = position + len(delimiter)
    while cursor < len(text):
        if text.startswith(delimiter, cursor):
            return cursor + len(delimiter)
        cursor += 2 if text[cursor] == "\\" and not raw else 1
    raise BoundaryError("Unterminated Dart header string")


def _metadata_end(text, position):
    position += 1
    match = re.match(r"[A-Za-z_$][A-Za-z0-9_$.]*", text[position:])
    if not match:
        raise BoundaryError("Unsupported Dart header metadata")
    position = _layout(text, position + match.end())
    if text[position:position + 1] == "<":
        raise BoundaryError("Review generic Dart header metadata explicitly")
    if position >= len(text) or text[position] != "(":
        return position
    depth = 1
    position += 1
    while position < len(text) and depth:
        position = _layout(text, position)
        if position >= len(text):
            break
        raw = text[position] in "rR" and text[position + 1:position + 2] in ("'", '"')
        if raw:
            position = _string_end(text, position + 1, raw=True)
        elif text[position] in "'\"":
            position = _string_end(text, position)
        else:
            if text[position] == "(":
                depth += 1
            elif text[position] == ")":
                depth -= 1
            position += 1
    if depth:
        raise BoundaryError("Unterminated Dart header metadata")
    return position


def import_records(text):
    """Read URI and its actual conditional selector from header directives."""
    result = []
    position = 0
    while True:
        position = _layout(text, position)
        while text[position:position + 1] == "@":
            position = _layout(text, _metadata_end(text, position))
        match = re.match(r"(import|export|part|library)\b", text[position:])
        if not match:
            return result
        position += match.end()
        prefix = ""
        while True:
            position = _layout(text, position)
            if position >= len(text):
                raise BoundaryError("Unterminated Dart directive")
            character = text[position]
            if character == ";":
                position += 1
                break
            if character in "'\"":
                end = text.find(character, position + 1)
                if end < 0:
                    raise BoundaryError("Unterminated Dart URI")
                uri = text[position + 1:end]
                if "\\" in uri or "$" in uri:
                    raise BoundaryError("Review escaped or interpolated directive literals explicitly")
                if ":" in uri or uri.endswith(".dart"):
                    result.append({"uri": uri, "js_only": prefix == "if(dart.library.js_interop)"})
                prefix = ""
                position = end + 1
            else:
                prefix += character
                position += 1


def directives(text):
    return [entry["uri"] for entry in import_records(text)]


def dependencies(root):
    collecting = False
    names = []
    for line in (root / "pubspec.yaml").read_text(encoding="utf-8").splitlines():
        if line == "dependencies:":
            collecting = True
            continue
        if not collecting or not line.strip() or line.lstrip().startswith("#"):
            continue
        if not line.startswith(" "):
            break
        if line.startswith("  ") and not line.startswith("   "):
            match = re.fullmatch(r"  ([a-z][a-z0-9_]*):.*", line)
            if not match:
                raise BoundaryError("Unsupported production dependency mapping")
            names.append(match[1])
    return sorted(names)


def verify(root, manifest):
    root = root.resolve()
    if manifest.get("schema") != 1:
        raise BoundaryError("Unsupported boundary manifest")
    if manifest.get("generator") != GENERATOR:
        raise BoundaryError("Review the pinned bridge generator version")
    if manifest.get("pubspec_sha256") != text_hash(root / "pubspec.yaml"):
        raise BoundaryError("pubspec.yaml changed; review dependency scope")
    if manifest.get("lockfile_sha256") != text_hash(root / "pubspec.lock"):
        raise BoundaryError("pubspec.lock changed; review resolved dependencies")
    packages = dependencies(root)
    if packages != manifest.get("runtime_dependencies"):
        raise BoundaryError("Production dependency list differs from review")
    actual = production_files(root)
    reviewed = {}
    for entry in manifest["files"]:
        path = entry["path"]
        if path in reviewed or path not in actual:
            raise BoundaryError(f"Duplicate, missing or nonproduction review path: {path}")
        reviewed[path] = entry
    if actual.keys() != reviewed.keys():
        raise BoundaryError(f"Unreviewed production files: {sorted(actual.keys() - reviewed.keys())}")
    counts = {}
    for relative, path in actual.items():
        entry = reviewed[relative]
        role = entry.get("role")
        if role not in ROLES or not entry.get("review"):
            raise BoundaryError(f"Missing role or review rationale: {relative}")
        if text_hash(path) != entry.get("sha256"):
            raise BoundaryError(f"Reviewed source changed: {relative}")
        source = path.read_text(encoding="utf-8")
        if role == "generated-frb" and (
            not relative.startswith("lib/core/rust/generated/") or
            not ("automatically generated" in source[:250] or
                 (relative.endswith(".freezed.dart") and
                  source.startswith("// GENERATED CODE - DO NOT MODIFY BY HAND")))
        ):
            raise BoundaryError(f"Generated classification lacks its expected source header: {relative}")
        for directive in import_records(source):
            uri = directive["uri"]
            if uri in BACKEND_DART:
                raise BoundaryError(f"Backend/platform escape import: {relative}: {uri}")
            if uri == "dart:io" and role != "generated-frb" and not entry.get("io_review"):
                raise BoundaryError(f"Unreviewed platform I/O: {relative}")
            if uri.startswith("package:"):
                package = uri.removeprefix("package:").split("/", 1)[0]
                if package in BACKEND_PACKAGES or package.startswith("flutter_sangfor"):
                    raise BoundaryError(f"Dart backend dependency: {relative}: {uri}")
                if package != "shuvpn" and package not in packages:
                    raise BoundaryError(f"Nonproduction or undeclared import: {relative}: {uri}")
                if package == "shuvpn":
                    target = "lib/" + uri.split("/", 1)[1]
                    if target not in actual:
                        raise BoundaryError(f"Unknown app import: {relative}: {uri}")
            elif not uri.startswith("dart:"):
                target = (path.parent / uri).resolve()
                if (relative == "lib/core/rust/generated/frb_generated.dart"
                        and role == "generated-frb" and uri == "frb_generated.web.dart"
                        and directive["js_only"]
                        and re.search(r"(?m)^web:\s*false\s*$",
                                      (root / "flutter_rust_bridge.yaml").read_text(encoding="utf-8"))):
                    # The pinned native-only generator retains an unreachable JS branch.
                    continue
                if not target.is_relative_to(root) or target.relative_to(root).as_posix() not in actual:
                    raise BoundaryError(f"Import leaves reviewed production source: {relative}: {uri}")
        counts[role] = counts.get(role, 0) + 1
    auxiliary_files = manifest.get("generation_inputs", [])
    if {entry["path"] for entry in auxiliary_files} != {
        "rust/src/frb_generated.rs", "flutter_rust_bridge.yaml"
    } or len(auxiliary_files) != 2:
        raise BoundaryError("Generation input coverage differs from review")
    for auxiliary in auxiliary_files:
        if auxiliary["path"] not in {
            "rust/src/frb_generated.rs", "flutter_rust_bridge.yaml"
        } or text_hash(root / auxiliary["path"]) != auxiliary["sha256"]:
            raise BoundaryError("Reviewed generation inputs or Rust binding changed")
    return {"production_dart_files": len(actual), "roles": counts,
            "runtime_dependencies": packages,
            "scope": "reviewed source inventory and import boundaries; runtime acceptance is separate"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--manifest", type=Path,
                        default=ROOT / "docs/dart_backend_boundary_manifest.json")
    args = parser.parse_args()
    result = verify(args.root, json.loads(args.manifest.read_text(encoding="utf-8")))
    print("REVIEWED_DART_BOUNDARY_INVENTORY_PASSED " + json.dumps(result, sort_keys=True))
