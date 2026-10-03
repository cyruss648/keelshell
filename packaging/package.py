#!/usr/bin/env python3
"""Stage local macOS/Linux/Windows application folders; never install or sign."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "assets" / "icons"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def check_binary(binary: Path, platform: str) -> None:
    with binary.open("rb") as stream:
        magic = stream.read(4)
        if platform == "macos":
            require(magic in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca"),
                    "macOS packaging requires a real Mach-O executable")
        elif platform == "linux":
            require(magic == b"\x7fELF", "Linux packaging requires a real ELF executable")
        else:
            require(magic[:2] == b"MZ", "Windows packaging requires a real PE executable")
            stream.seek(0x3C)
            offset = stream.read(4)
            require(len(offset) == 4, "PE header is truncated")
            stream.seek(struct.unpack("<I", offset)[0])
            require(stream.read(4) == b"PE\0\0", "Invalid Windows PE signature")


def copy(source: Path, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)


def check_icons() -> dict:
    manifest = json.loads((ICONS / "manifest.json").read_text())
    for name, record in manifest["files"].items():
        path = Path(name)
        require(not path.is_absolute() and ".." not in path.parts, "Unsafe icon manifest path")
        data = (ICONS / path).read_bytes()
        require(hashlib.sha256(data).hexdigest() == record["sha256"], f"Icon hash mismatch: {name}")
    require(hashlib.sha256((ICONS / "source.png").read_bytes()).hexdigest() == manifest["source"]["sha256"],
            "Icon source changed; run scripts/build-icons.py")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=("macos", "linux", "windows"))
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--target", help="Rust target triple for release provenance")
    parser.add_argument("--output", required=True, type=Path, help="New staging directory; existing destination is rejected")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    check_binary(binary, args.platform)
    manifest = check_icons()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    destination = args.output.resolve()
    require(not destination.exists(), "Output already exists; choose a new directory to preserve prior evidence")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".keelshell-stage-", dir=destination.parent) as temporary:
        stage = Path(temporary) / "output"
        stage.mkdir()
        if args.platform == "macos":
            contents = stage / "KeelShell.app" / "Contents"
            executable = contents / "MacOS" / "keelshell-app"
            copy(binary, executable)
            executable.chmod(0o755)
            copy(ICONS / "macos" / "KeelShell.icns", contents / "Resources" / "KeelShell.icns")
            info = {
                "CFBundleIdentifier": "app.keelshell.desktop", "CFBundleName": "KeelShell",
                "CFBundleDisplayName": "KeelShell", "CFBundleExecutable": "keelshell-app",
                "CFBundleIconFile": "KeelShell.icns", "CFBundlePackageType": "APPL",
                "CFBundleShortVersionString": version.split("-")[0],
                "CFBundleVersion": version.split("-")[0],
                "CFBundleDevelopmentRegion": "zh_CN", "CFBundleLocalizations": ["zh_CN", "en"],
                "NSHighResolutionCapable": True, "NSPrincipalClass": "NSApplication",
                "LSMinimumSystemVersion": "15.0",
            }
            (contents / "Info.plist").write_bytes(plistlib.dumps(info))
            (contents / "PkgInfo").write_bytes(b"APPL????")
        elif args.platform == "linux":
            prefix = stage / "usr"
            executable = prefix / "bin" / "keelshell-app"
            copy(binary, executable)
            executable.chmod(0o755)
            copy(ROOT / "packaging" / "linux" / "keelshell.desktop", prefix / "share" / "applications" / "keelshell.desktop")
            shutil.copytree(ICONS / "linux" / "hicolor", prefix / "share" / "icons" / "hicolor")
        else:
            copy(binary, stage / "keelshell-app.exe")
            copy(ICONS / "windows" / "keelshell.ico", stage / "keelshell.ico")
        files = {}
        for path in sorted(stage.rglob("*")):
            if path.is_file():
                files[path.relative_to(stage).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
        receipt = {
            "schema_version": 1, "platform": args.platform, "version": version,
            "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "icon_source_sha256": manifest["source"]["sha256"], "files": files,
            "installed": False, "signed_by_packaging_script": False,
            "native_acceptance": "not performed by this script",
        }
        if args.target:
            receipt["target"] = args.target
            receipt["build"] = {
                "commit": os.environ.get("KEELSHELL_BUILD_COMMIT"),
                "workflow_run": os.environ.get("GITHUB_RUN_ID"),
                "rustc": subprocess.check_output(["rustc", "-V"], text=True, timeout=30).strip(),
            }
        (stage / "package-manifest.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        os.rename(stage, destination)
    print(f"Staged {args.platform} application at {destination}")
    print("No installation, signing, publication, or native execution was performed.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, struct.error) as error:
        print(f"Packaging failed: {error}", file=sys.stderr)
        sys.exit(1)
