#!/usr/bin/env python3
"""Validate local release inputs and archive staged packages; never execute or publish.

Python 3.11+, standard library only. Binary checks establish format/architecture,
not native execution, ABI compatibility, signing, notarization or installation.
Archive roots mirror package.py's staging directory, including its unchanged
package-manifest.json. Verification streams members without extracting them.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import struct
import sys
import tarfile
import tempfile
import tomllib
from typing import BinaryIO, Iterator
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "aarch64-apple-darwin": ("macos", "aarch64"),
    "x86_64-apple-darwin": ("macos", "x86_64"),
    "aarch64-unknown-linux-gnu": ("linux", "aarch64"),
    "x86_64-unknown-linux-gnu": ("linux", "x86_64"),
    "aarch64-pc-windows-msvc": ("windows", "aarch64"),
    "x86_64-pc-windows-msvc": ("windows", "x86_64"),
}
BINARIES = {
    "macos": "KeelShell.app/Contents/MacOS/keelshell-app",
    "linux": "usr/bin/keelshell-app",
    "windows": "keelshell-app.exe",
}
MCP_BINARIES = {
    "macos": "KeelShell.app/Contents/MacOS/keelshell-mcp",
    "linux": "usr/bin/keelshell-mcp",
    "windows": "keelshell-mcp.exe",
}
MANIFEST = "package-manifest.json"
MAX_MANIFEST = 16 * 1024 * 1024
MAX_MEMBERS = 20_000
MAX_UNPACKED = 4 * 1024 * 1024 * 1024
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?", re.ASCII)
HEX = re.compile(r"[0-9a-f]{64}", re.ASCII)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def version(manifest: Path = ROOT / "Cargo.toml") -> str:
    value = tomllib.loads(manifest.read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    require(isinstance(value, str), "Workspace version must be a string")
    match = VERSION.fullmatch(value)
    require(match is not None, "Workspace version must be SemVer without build metadata")
    prerelease = match.group(4)
    require(not prerelease or all(not part.isdigit() or part == "0" or not part.startswith("0")
                                for part in prerelease.split(".")), "Prerelease numeric identifiers cannot have leading zeros")
    return value


def validate_ref(tag: str, manifest: Path = ROOT / "Cargo.toml") -> str:
    value = version(manifest)
    require(tag == f"v{value}", "Release tag must exactly match v + workspace.package.version")
    return value


def target_info(target: str, platform: str | None = None) -> tuple[str, str]:
    require(target in TARGETS, "Unsupported release target")
    result = TARGETS[target]
    require(platform is None or platform == result[0], "Platform and target disagree")
    return result


def package_name(value: str, target: str) -> str:
    platform, _ = target_info(target)
    return f"KeelShell-{value}-{target}" + (".tar.gz" if platform == "linux" else ".zip")


def safe_path(name: str) -> str:
    require(isinstance(name, str) and bool(name), "Empty or non-string package path")
    require(not any(ord(char) < 32 or ord(char) == 127 for char in name)
            and not any(char in name for char in "\\:"), "Unsafe characters in package path")
    parts = name.split("/")
    require(not name.startswith("/") and all(part not in ("", ".", "..") for part in parts),
            "Package paths must be canonical relative paths")
    devices = {"CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"} | {
        f"{prefix}{digit}" for prefix in ("COM", "LPT") for digit in "123456789¹²³"
    }
    require(all(not part.endswith((".", " ")) and not any(char in part for char in '<>"|?*')
                and part.split(".")[0].upper() not in devices for part in parts),
            "Package path is unsafe after Windows filename normalization")
    require(PurePosixPath(name).as_posix() == name, "Noncanonical package path")
    return name


def digest(stream: BinaryIO) -> str:
    result = hashlib.sha256()
    for block in iter(lambda: stream.read(1024 * 1024), b""):
        result.update(block)
    return result.hexdigest()


def file_digest(path: Path) -> str:
    with path.open("rb") as stream:
        return digest(stream)


def no_duplicate_keys(pairs: list[tuple[str, object]]) -> dict:
    value: dict = {}
    for key, item in pairs:
        require(key not in value, "Duplicate key in package manifest")
        value[key] = item
    return value


def receipt(data: bytes, platform: str, value: str, target: str) -> dict:
    require(len(data) <= MAX_MANIFEST, "Package manifest exceeds size limit")
    result = json.loads(data, object_pairs_hook=no_duplicate_keys)
    require(isinstance(result, dict), "Package manifest must be an object")
    require(result.get("schema_version") == 1, "Unsupported package manifest schema")
    require(result.get("platform") == platform and result.get("version") == value,
            "Package manifest platform/version mismatch")
    require("target" not in result or result["target"] == target, "Package manifest target mismatch")
    require(result.get("installed") is False and result.get("signed_by_packaging_script") is False
            and result.get("native_acceptance") == "not performed by this script",
            "Package receipt must retain package.py's explicit acceptance boundaries")
    for field in ("binary_sha256", "mcp_binary_sha256", "icon_source_sha256"):
        require(isinstance(result.get(field), str) and HEX.fullmatch(result[field]) is not None,
                f"Invalid {field} in package manifest")
    files = result.get("files")
    require(isinstance(files, dict) and 0 < len(files) < MAX_MEMBERS, "Invalid package manifest file list")
    for name, checksum in files.items():
        safe_path(name)
        require(name != MANIFEST, "Package manifest cannot checksum itself")
        require(isinstance(checksum, str) and HEX.fullmatch(checksum) is not None, "Invalid file checksum")
    file_names = set(files) | {MANIFEST}
    directory_names = ancestors(file_names)
    require(not file_names & directory_names, "A file cannot also be a parent directory")
    all_names = file_names | directory_names
    require(len({name.casefold() for name in all_names}) == len(all_names),
            "Case-colliding package paths are not portable")
    require(BINARIES[platform] in files, "Package manifest does not contain the executable")
    require(files[BINARIES[platform]] == result["binary_sha256"], "Executable checksum disagrees with receipt")
    require(MCP_BINARIES[platform] in files, "Package manifest does not contain the MCP companion")
    require(files[MCP_BINARIES[platform]] == result["mcp_binary_sha256"], "MCP companion checksum disagrees with receipt")
    return result


def ancestors(files: set[str]) -> set[str]:
    return {str(parent) for name in files for parent in PurePosixPath(name).parents if str(parent) != "."}


def check_mode(mode: int, executable: bool) -> None:
    require(not mode & 0o7000, "Special permission bits are not allowed in packages")
    if executable:
        require(mode & 0o111 != 0, "Unix executable permission is missing")


def read_at(stream: BinaryIO, offset: int, length: int) -> bytes:
    require(0 <= offset <= 16 * 1024 * 1024 and length <= 64 * 1024,
            "Binary header offset or size is unreasonable")
    stream.seek(offset)
    data = stream.read(length)
    require(len(data) == length, "Binary header is truncated")
    return data


def check_binary(stream: BinaryIO, target: str) -> None:
    """Check native executable headers without claiming launch or ABI acceptance."""
    platform, architecture = target_info(target)
    if platform == "macos":
        header = read_at(stream, 0, 32)
        require(header[:4] == b"\xcf\xfa\xed\xfe", "Expected a thin little-endian 64-bit Mach-O executable")
        cpu, _, kind, commands, command_bytes = struct.unpack_from("<IIIII", header, 4)
        require(cpu == {"x86_64": 0x1000007, "aarch64": 0x100000C}[architecture], "Mach-O architecture mismatch")
        require(kind == 2 and commands > 0 and command_bytes >= 8 * commands,
                "Mach-O is not an executable with load commands")
        # Fat/universal binaries are deliberately excluded from single-target assets.
        require(command_bytes <= 16 * 1024 * 1024 and commands <= 16_384,
                "Mach-O load command table is unreasonable")
        position = 32
        for _ in range(commands):
            _, length = struct.unpack("<II", read_at(stream, position, 8))
            require(length >= 8 and length % 8 == 0 and position + length <= 32 + command_bytes,
                    "Malformed Mach-O load command")
            read_at(stream, position + length - 1, 1)
            position += length
        require(position == 32 + command_bytes, "Mach-O load command sizes disagree")
    elif platform == "linux":
        header = read_at(stream, 0, 64)
        require(header[:7] == b"\x7fELF\x02\x01\x01", "Expected a little-endian 64-bit ELF executable")
        kind, machine, elf_version = struct.unpack_from("<HHI", header, 16)
        require(machine == {"x86_64": 62, "aarch64": 183}[architecture], "ELF architecture mismatch")
        require(kind in (2, 3) and elf_version == 1 and struct.unpack_from("<H", header, 52)[0] == 64,
                "Invalid ELF executable header")
        entry, program_offset = struct.unpack_from("<QQ", header, 24)
        program_size, program_count = struct.unpack_from("<HH", header, 54)
        require(entry != 0 and program_offset >= 64 and program_size == 56 and program_count > 0,
                "ELF has no executable entry/program headers")
        read_at(stream, program_offset + program_size * program_count - 1, 1)
    else:
        require(read_at(stream, 0, 2) == b"MZ", "Expected a PE executable")
        offset = struct.unpack("<I", read_at(stream, 0x3C, 4))[0]
        require(offset >= 64, "Invalid PE header offset")
        header = read_at(stream, offset, 26)
        require(header[:4] == b"PE\0\0", "Invalid PE signature")
        machine, sections = struct.unpack_from("<HH", header, 4)
        optional_size, flags, optional_magic = struct.unpack_from("<HHH", header, 20)
        require(machine == {"x86_64": 0x8664, "aarch64": 0xAA64}[architecture], "PE architecture mismatch")
        require(sections > 0 and optional_size >= 112 and optional_magic == 0x20B
                and flags & 2 != 0 and flags & 0x2000 == 0,
                "PE is not a 64-bit executable image")
        read_at(stream, offset + 24, optional_size)


def stage_files(stage: Path) -> tuple[dict[str, Path], set[str]]:
    require(not stage.is_symlink() and stage.is_dir(), "Stage must be a real directory")
    files, directories = {}, set()
    for base, dirs, names in os.walk(stage, followlinks=False):
        for name in dirs + names:
            path = Path(base) / name
            relative = safe_path(path.relative_to(stage).as_posix())
            mode = path.lstat().st_mode
            require(not stat.S_ISLNK(mode), "Symbolic links are forbidden in staged packages")
            require(stat.S_ISDIR(mode) or stat.S_ISREG(mode), "Only regular files/directories can be packaged")
            if stat.S_ISDIR(mode):
                directories.add(relative)
            else:
                files[relative] = path
            require(len(files) + len(directories) <= MAX_MEMBERS, "Too many staged members")
    return files, directories


def validate_stage(stage: Path, platform: str, target: str, value: str) -> dict:
    target_info(target, platform)
    files, directories = stage_files(stage)
    require(MANIFEST in files and files[MANIFEST].stat().st_size <= MAX_MANIFEST, "Missing or oversized package manifest")
    manifest = receipt(files[MANIFEST].read_bytes(), platform, value, target)
    expected = set(manifest["files"]) | {MANIFEST}
    require(set(files) == expected, "Stage has missing or unrecorded files")
    require(directories == ancestors(expected), "Stage has missing or unrecorded directories")
    require(sum(path.stat().st_size for path in files.values()) <= MAX_UNPACKED, "Stage exceeds size limit")
    for name, path in files.items():
        check_mode(path.stat().st_mode, platform != "windows" and name in (BINARIES[platform], MCP_BINARIES[platform]))
        if name != MANIFEST:
            require(file_digest(path) == manifest["files"][name], f"Staged checksum mismatch: {name}")
    for name in (BINARIES[platform], MCP_BINARIES[platform]):
        with files[name].open("rb") as stream:
            check_binary(stream, target)
    return manifest


@contextmanager
def archive_members(path: Path) -> Iterator[tuple[dict, set[str], object]]:
    """Yield regular member metadata and a read-only opener, never extract paths."""
    if path.name.endswith(".zip"):
        with zipfile.ZipFile(path) as archive:
            members, directories = {}, set()
            for member in archive.infolist():
                name = safe_path(member.filename.removesuffix("/") if member.is_dir() else member.filename)
                require(name not in members and name not in directories, "Duplicate archive path")
                mode = member.external_attr >> 16
                require(stat.S_IFMT(mode) in (stat.S_IFREG, stat.S_IFDIR), "ZIP member has unknown type or is a symlink")
                require(not member.flag_bits & 1, "Encrypted ZIP members are not supported")
                if member.is_dir():
                    require(stat.S_ISDIR(mode), "ZIP directory type mismatch")
                    check_mode(mode, False)
                    directories.add(name)
                else:
                    require(stat.S_ISREG(mode), "ZIP file type mismatch")
                    members[name] = (member.file_size, mode, member)
            yield members, directories, lambda member: archive.open(member, "r")
    else:
        with tarfile.open(path, "r:gz") as archive:
            members, directories = {}, set()
            for member in archive:
                name = safe_path(member.name)
                require(name not in members and name not in directories, "Duplicate archive path")
                require(member.isfile() or member.isdir(), "TAR links and special members are forbidden")
                if member.isdir():
                    check_mode(member.mode, False)
                    directories.add(name)
                else:
                    members[name] = (member.size, member.mode, member)
                require(len(members) + len(directories) <= MAX_MEMBERS, "Too many archive members")
            yield members, directories, archive.extractfile


def validate_archive(path: Path, target: str, value: str) -> None:
    platform, _ = target_info(target)
    require(path.name == package_name(value, target), "Archive filename does not match target/version")
    require(path.is_file() and not path.is_symlink(), "Archive must be a regular file")
    with archive_members(path) as (members, directories, open_member):
        require(len(members) + len(directories) <= MAX_MEMBERS, "Too many archive members")
        require(sum(record[0] for record in members.values()) <= MAX_UNPACKED, "Archive exceeds expanded size limit")
        require(MANIFEST in members and members[MANIFEST][0] <= MAX_MANIFEST, "Missing or oversized archived manifest")
        with open_member(members[MANIFEST][2]) as stream:
            manifest = receipt(stream.read(MAX_MANIFEST + 1), platform, value, target)
        expected = set(manifest["files"]) | {MANIFEST}
        require(set(members) == expected, "Archive has missing or unrecorded files")
        require(directories <= ancestors(expected), "Archive has unrecorded directories")
        for name, (_, mode, member) in members.items():
            check_mode(mode, platform != "windows" and name in (BINARIES[platform], MCP_BINARIES[platform]))
            with open_member(member) as stream:
                if name != MANIFEST:
                    require(digest(stream) == manifest["files"][name], f"Archived checksum mismatch: {name}")
        for name in (BINARIES[platform], MCP_BINARIES[platform]):
            with open_member(members[name][2]) as stream:
                check_binary(stream, target)


def archive_stage(platform: str, target: str, stage: Path, output: Path, manifest: Path = ROOT / "Cargo.toml") -> Path:
    value = version(manifest)
    validate_stage(stage, platform, target, value)
    require(not output.resolve().is_relative_to(stage.resolve()), "Output must be outside the stage")
    output.mkdir(parents=True, exist_ok=True)
    require(not output.is_symlink(), "Output directory cannot be a symlink")
    destination = output / package_name(value, target)
    checksum = destination.with_name(destination.name + ".sha256")
    require(not destination.exists() and not checksum.exists(), "Release output already exists")
    files, _ = stage_files(stage)
    with tempfile.TemporaryDirectory(prefix=".release-", dir=output) as temporary:
        candidate = Path(temporary) / destination.name
        if platform == "linux":
            with tarfile.open(candidate, "w:gz", format=tarfile.PAX_FORMAT) as archive:
                for name, path in sorted(files.items()):
                    info = archive.gettarinfo(str(path), arcname=name)
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    with path.open("rb") as stream:
                        archive.addfile(info, stream)
        else:
            with zipfile.ZipFile(candidate, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
                for name, path in sorted(files.items()):
                    archive.write(path, arcname=name)
        validate_archive(candidate, target, value)
        line = f"{file_digest(candidate)}  {destination.name}\n"
        # Exclusive creation preserves previous evidence even under an accidental
        # concurrent publisher. Only remove files created by this invocation.
        created = []
        try:
            with destination.open("xb") as out, candidate.open("rb") as source:
                created.append(destination)
                shutil.copyfileobj(source, out)
            with checksum.open("x", encoding="ascii", newline="\n") as out:
                created.append(checksum)
                out.write(line)
        except BaseException:
            for path in created:
                path.unlink(missing_ok=True)
            raise
    return destination


def verify_collection(source: Path, targets: list[str], manifest: Path = ROOT / "Cargo.toml") -> dict[str, tuple[Path, str]]:
    value = version(manifest)
    require(targets and len(targets) == len(set(targets)), "Expected targets must be nonempty and unique")
    expected = {package_name(value, target): target for target in targets}
    files, _ = stage_files(source)
    found: dict[str, Path] = {}
    for relative, path in files.items():
        name = PurePosixPath(relative).name
        require(name in expected or name.removesuffix(".sha256") in expected or relative == "SHA256SUMS",
                f"Unexpected release collection file: {relative}")
        require(name not in found, "Duplicate release artifact basename")
        found[name] = path
    require(set(found) - {"SHA256SUMS"} == set(expected) | {name + ".sha256" for name in expected},
            "Release collection is incomplete")
    verified = {}
    for name, target in sorted(expected.items()):
        path = found[name]
        checksum_path = found[name + ".sha256"]
        require(checksum_path.stat().st_size <= 1024, "Checksum file is oversized")
        checksum = file_digest(path)
        line = f"{checksum}  {name}\n"
        require(checksum_path.read_bytes() == line.encode("ascii"), f"Checksum mismatch: {name}")
        validate_archive(path, target, value)
        verified[name] = (path, line)
    if "SHA256SUMS" in found:
        expected_sums = "".join(line for _, line in verified.values()).encode("ascii")
        require(found["SHA256SUMS"].stat().st_size == len(expected_sums)
                and found["SHA256SUMS"].read_bytes() == expected_sums,
                "SHA256SUMS does not match artifacts")
    return verified


def collect(source: Path, output: Path, targets: list[str], manifest: Path = ROOT / "Cargo.toml") -> Path:
    verified = verify_collection(source, targets, manifest)
    require(not output.exists() and not output.is_symlink(), "Collection output must be a new directory")
    require(not output.resolve().is_relative_to(source.resolve()), "Collection output must be outside input")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".release-collect-", dir=output.parent) as temporary:
        stage = Path(temporary) / "complete"
        stage.mkdir()
        for name, (path, line) in verified.items():
            shutil.copyfile(path, stage / name)
            (stage / (name + ".sha256")).write_text(line, encoding="ascii", newline="\n")
        (stage / "SHA256SUMS").write_text("".join(line for _, line in verified.values()), encoding="ascii", newline="\n")
        verify_collection(stage, targets, manifest)
        os.rename(stage, output)
    return output / "SHA256SUMS"


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    validate = commands.add_parser("validate-ref", help="Require exact v + workspace SemVer")
    validate.add_argument("--tag", required=True)
    archive = commands.add_parser("archive", help="Validate a stage and produce one archive/checksum")
    archive.add_argument("--platform", required=True, choices=("macos", "linux", "windows"))
    archive.add_argument("--target", required=True, choices=TARGETS)
    archive.add_argument("--stage", required=True, type=Path)
    archive.add_argument("--output", required=True, type=Path)
    for name in ("verify", "collect"):
        command = commands.add_parser(name, help="Verify the complete expected release matrix")
        command.add_argument("--input", required=True, type=Path)
        command.add_argument("--expected-target", action="append", choices=TARGETS)
        if name == "collect":
            command.add_argument("--output", required=True, type=Path)
    for command in (validate, archive, commands.choices["verify"], commands.choices["collect"]):
        command.add_argument("--manifest", type=Path, default=ROOT / "Cargo.toml")
    args = parser.parse_args(argv)
    if args.command == "validate-ref":
        print(validate_ref(args.tag, args.manifest))
    elif args.command == "archive":
        print(archive_stage(args.platform, args.target, args.stage, args.output, args.manifest))
    elif args.command == "collect":
        print(collect(args.input, args.output, args.expected_target or list(TARGETS), args.manifest))
    else:
        results = verify_collection(args.input, args.expected_target or list(TARGETS), args.manifest)
        print(f"Verified {len(results)} packages and checksums; no native acceptance is implied.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, struct.error, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f"Release validation failed: {error}", file=sys.stderr)
        sys.exit(1)
