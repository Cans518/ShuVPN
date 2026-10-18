"""Prepare a production-AOT copy whose two EXE resource names select fresh storage."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import sys
import uuid

from package_desktop import ROOT, MANIFEST, archive_contents, source_identity, verify_package


def validate_names(company: str, product: str) -> None:
    if not all(re.fullmatch(r"[0-9a-f]{6}", name) for name in (company, product)):
        raise ValueError("Private profile names must be six lowercase hexadecimal characters")


def resource_span(content: bytes) -> tuple[int, int]:
    try:
        if content[:2] != b"MZ":
            raise ValueError("Not a PE executable")
        header = struct.unpack_from("<I", content, 60)[0]
        if content[header:header + 4] != b"PE\0\0":
            raise ValueError("Missing PE header")
        machine, count = struct.unpack_from("<HH", content, header + 4)
        optional_size = struct.unpack_from("<H", content, header + 20)[0]
        optional = header + 24
        if machine != 0x8664 or struct.unpack_from("<H", content, optional)[0] != 0x20B:
            raise ValueError("Expected a Windows x64 executable")
        if any(struct.unpack_from("<II", content, optional + 112 + 8 * 4)):
            raise ValueError("Do not edit a signed executable")
        matches = []
        for index in range(count):
            section = optional + optional_size + 40 * index
            if content[section:section + 8].rstrip(b"\0") != b".rsrc":
                continue
            size, offset = struct.unpack_from("<II", content, section + 16)
            flags = struct.unpack_from("<I", content, section + 36)[0]
            if offset <= section or size == 0 or offset + size > len(content) or flags & 0x20000000:
                raise ValueError("Invalid or executable resource section")
            matches.append((offset, offset + size))
        if len(matches) != 1:
            raise ValueError("Expected one non-executable PE resource section")
        return matches[0]
    except struct.error as error:
        raise ValueError("Truncated PE executable") from error


def validate_resource_overlay(original: bytes, patched: bytes,
                              ranges: list[tuple[int, int]]) -> None:
    start, end = resource_span(original)
    if len(original) != len(patched) or not ranges:
        raise ValueError("Resource overlay must preserve file size")
    if any(not start <= left < right <= end for left, right in ranges):
        raise ValueError("Overlay ranges must stay in the non-executable resource section")
    for index, (before, after) in enumerate(zip(original, patched)):
        if before != after and not any(left <= index < right for left, right in ranges):
            raise ValueError("Executable changed outside the two declared resource strings")


def digest(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def known_folder(identifier: str) -> Path:
    shell = c.WinDLL("shell32", use_last_error=True)
    shell.SHGetKnownFolderPath.argtypes = [c.c_void_p, w.DWORD, w.HANDLE, c.POINTER(w.LPWSTR)]
    shell.SHGetKnownFolderPath.restype = c.c_long
    guid, value = c.create_string_buffer(uuid.UUID(identifier).bytes_le), w.LPWSTR()
    status = shell.SHGetKnownFolderPath(guid, 0, None, c.byref(value))
    if status != 0:
        raise RuntimeError(f"Cannot read the actual KnownFolder: {status}")
    try:
        return Path(value.value).resolve(strict=True)
    finally:
        ole = c.WinDLL("ole32")
        ole.CoTaskMemFree.argtypes = [c.c_void_p]
        ole.CoTaskMemFree(c.cast(value, c.c_void_p))


def folder_roots() -> list[Path]:
    return [known_folder(identifier) for identifier in (
        "3EB685DB-65F9-4CF6-A03A-E3EF65729F3D",  # RoamingAppData
        "F1B32785-6FBA-4FCF-9D55-7B8E7F157091",  # LocalAppData
    )]


def version_buffer(executable: Path):
    library = c.WinDLL("version", use_last_error=True)
    library.GetFileVersionInfoSizeW.argtypes = [w.LPCWSTR, c.POINTER(w.DWORD)]
    library.GetFileVersionInfoSizeW.restype = w.DWORD
    library.GetFileVersionInfoW.argtypes = [w.LPCWSTR, w.DWORD, w.DWORD, c.c_void_p]
    library.GetFileVersionInfoW.restype = w.BOOL
    library.VerQueryValueW.argtypes = [c.c_void_p, w.LPCWSTR, c.POINTER(c.c_void_p), c.POINTER(w.UINT)]
    library.VerQueryValueW.restype = w.BOOL
    unused = w.DWORD()
    size = library.GetFileVersionInfoSizeW(str(executable), c.byref(unused))
    if size == 0:
        raise c.WinError(c.get_last_error())
    buffer = c.create_string_buffer(size)
    if not library.GetFileVersionInfoW(str(executable), 0, size, buffer):
        raise c.WinError(c.get_last_error())
    return library, buffer


def version_names(executable: Path) -> dict[str, str]:
    library, buffer = version_buffer(executable)
    result = {}
    for key in ("CompanyName", "ProductName"):
        pointer, length = c.c_void_p(), w.UINT()
        if not library.VerQueryValueW(buffer, f"\\StringFileInfo\\040904e4\\{key}",
                                     c.byref(pointer), c.byref(length)):
            raise RuntimeError(f"Missing CP1252 version field: {key}")
        result[key] = c.wstring_at(pointer.value)
    return result


def patch_profile_names(executable: Path, company: str, product: str) -> dict:
    validate_names(company, product)
    original = executable.read_bytes()
    start, end = resource_span(original)
    key = "VS_VERSION_INFO\0".encode("utf-16-le")
    key_offset = original.find(key, start, end)
    if key_offset < start + 6 or original.find(key, key_offset + 1, end) != -1:
        raise ValueError("Expected a unique version resource root")
    resource_offset = key_offset - 6
    size = struct.unpack_from("<H", original, resource_offset)[0]
    if resource_offset + size > end:
        raise ValueError("Version root exceeds its resource section")
    library, buffer = version_buffer(executable)
    if buffer.raw[:size] != original[resource_offset:resource_offset + size]:
        raise ValueError("Win32 version data differs from the file resource")
    patched, ranges = bytearray(original), []
    for key, value in (("CompanyName", company), ("ProductName", product)):
        pointer, length = c.c_void_p(), w.UINT()
        if not library.VerQueryValueW(buffer, f"\\StringFileInfo\\040904e4\\{key}",
                                     c.byref(pointer), c.byref(length)):
            raise RuntimeError(f"Cannot locate original version field {key}")
        offset = pointer.value - c.addressof(buffer)
        expected = "ShuVPN\0".encode("utf-16-le")
        if length.value != 7 or not 0 <= offset <= size - len(expected):
            raise ValueError("Unexpected original version field size")
        left = resource_offset + offset
        if original[left:left + len(expected)] != expected:
            raise ValueError("Only the original ShuVPN names can be isolated")
        replacement = (value + "\0").encode("utf-16-le")
        patched[left:left + len(expected)] = replacement
        ranges.append((left, left + len(expected)))
    validate_resource_overlay(original, patched, ranges)
    executable.write_bytes(patched)
    if version_names(executable) != {"CompanyName": company, "ProductName": product}:
        raise RuntimeError("Win32 did not read back the isolated profile names")
    return {"original_sha256": digest(original), "patched_sha256": digest(patched),
            "resource_ranges": ranges, "unchanged_outside_ranges": True,
            "changed_byte_count": sum(a != b for a, b in zip(original, patched))}


def claim_profile(roots: list[Path], ownership: dict) -> None:
    claimed = []
    try:
        for root in roots:
            root.mkdir()
            claimed.append(root)
            (root / ".shuvpn-acceptance-owner.json").write_text(
                json.dumps(ownership), encoding="utf-8")
    except OSError:
        # Nothing has launched yet: rollback uses only nonrecursive operations
        # on directories this call created exclusively.
        for root in reversed(claimed):
            owner = root / ".shuvpn-acceptance-owner.json"
            if root.resolve() != root or any(path != owner for path in root.iterdir()):
                raise RuntimeError(f"Cannot safely roll back private profile claim: {root}")
            if owner.exists():
                if owner.is_symlink() or getattr(owner.stat(), "st_file_attributes", 0) & 0x400:
                    raise RuntimeError("Profile owner was replaced during claim")
                owner.unlink()
            root.rmdir()
        raise


def prepare(archive: Path, output: Path) -> dict:
    preparer_source = source_identity()
    output, archive = output.resolve(), archive.resolve(strict=True)
    boundary = (ROOT / "build/desktop").resolve()
    if not output.is_relative_to(boundary) or output == boundary:
        raise ValueError("Prepared bundle must be new and inside build/desktop")
    verify_package(archive)
    contents = archive_contents(archive)
    name = next(name for name in contents if name.endswith("/" + MANIFEST))
    manifest = json.loads(contents[name])
    if manifest["platform"] != "windows":
        raise ValueError("Expected the Windows production package")
    configuration = (ROOT / "windows/flutter/ephemeral/generated_config.cmake").read_text()
    if '"FLUTTER_TARGET=lib/main.dart"' not in configuration:
        raise ValueError("The current reference build must have restored production main")
    prefix = name.removesuffix(MANIFEST)
    aot = contents[prefix + "data/app.so"]
    if aot != (ROOT / "build/windows/x64/runner/Release/data/app.so").read_bytes():
        raise ValueError("Archive AOT does not match the restored production AOT")
    company, product = uuid.uuid4().hex[:6], uuid.uuid4().hex[:6]
    validate_names(company, product)
    roots = [folder / company for folder in folder_roots()]
    if any(root.exists() for root in roots):
        raise ValueError("Random profile company directory already exists")
    output.mkdir()
    bundle = output / "bundle"
    bundle.mkdir()
    for relative in manifest["files"]:
        destination = bundle / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(contents[prefix + relative])
    overlay = patch_profile_names(bundle / "shuvpn.exe", company, product)
    token = uuid.uuid4().hex
    ownership = {"token": token, "company": company, "product": product,
                 "output": str(output)}
    products = {relative: digest((bundle / relative).read_bytes()) for relative in manifest["files"]}
    for relative, expected in manifest["files"].items():
        actual = products[relative]
        if relative == "shuvpn.exe":
            actual = overlay["original_sha256"]
        if actual != expected["sha256"]:
            raise RuntimeError(f"Copied production payload changed: {relative}")
    result = {"schema": 1, "entrypoint": "lib/main.dart",
              "preparer_source": preparer_source,
              "kind": "production-aot-private-resource-copy",
              "archive_sha256": digest(archive.read_bytes()), "archive_manifest": manifest,
              "company": company, "product": product, "ownership": ownership,
              "profile_roots": [str(root) for root in roots],
              "profile": str(roots[0] / product), "overlay": overlay, "products": products,
              "production_aot_unchanged": True, "exe_code_unchanged": True,
              "independent_windows_user_or_network_isolation": False}
    (bundle / "private-profile.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output / "verification.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    claim_profile(roots, ownership)
    if source_identity() != preparer_source:
        cleanup_profile(result)
        raise RuntimeError("Preparation source changed during the copy")
    return result


def cleanup_profile(marker: dict) -> None:
    validate_names(marker["company"], marker["product"])
    expected = [folder / marker["company"] for folder in folder_roots()]
    if [str(root) for root in expected] != marker["profile_roots"]:
        raise ValueError("Profile cleanup roots differ from the current KnownFolders")
    for root in expected:
        if not root.exists() and not root.is_symlink():
            continue
        if root.resolve() != root or root.parent not in folder_roots():
            raise ValueError("Profile cleanup escaped its explicitly owned directory")
        owner = root / ".shuvpn-acceptance-owner.json"
        if json.loads(owner.read_text(encoding="utf-8")) != marker["ownership"]:
            raise ValueError("Profile directory ownership marker differs")
        for path in (root, *root.rglob("*")):
            if path.is_symlink() or getattr(path.stat(), "st_file_attributes", 0) & 0x400:
                raise ValueError("Do not delete a profile containing reparse points")
        # Keep the owner while deleting contents so a partial cleanup can retry.
        for child in root.iterdir():
            if child == owner:
                continue
            if child.is_dir():
                shutil.rmtree(child)
            else:
                child.unlink()
        owner.unlink()
        root.rmdir()


def main() -> None:
    if sys.platform != "win32":
        raise RuntimeError("Prepare and clean Windows profiles on Windows")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    result = prepare(**vars(parser.parse_args()))
    print("WINDOWS_PRODUCTION_AOT_PRIVATE_RESOURCE_COPY_PREPARED", result["profile"])


if __name__ == "__main__":
    main()
