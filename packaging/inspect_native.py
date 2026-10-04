#!/usr/bin/env python3
"""Inspect a staged package on its native build host without launching its UI."""
from __future__ import annotations

import argparse
import ctypes
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys

try:
    from . import release
except ImportError:
    import release


def run(*command: str) -> str:
    result = subprocess.run(command, check=True, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=60)
    print(result.stdout, end="")
    return result.stdout


def windows_resources(binary: Path) -> None:
    # Loading as data reads resources without running DllMain or application code.
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
    kernel.LoadLibraryExW.restype = wintypes.HMODULE
    kernel.FindResourceW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p]
    kernel.FindResourceW.restype = ctypes.c_void_p
    kernel.SizeofResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
    kernel.SizeofResource.restype = wintypes.DWORD
    kernel.LoadResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
    kernel.LoadResource.restype = ctypes.c_void_p
    kernel.LockResource.argtypes = [ctypes.c_void_p]
    kernel.LockResource.restype = ctypes.c_void_p
    kernel.FreeLibrary.argtypes = [wintypes.HMODULE]
    kernel.FreeLibrary.restype = wintypes.BOOL
    module = kernel.LoadLibraryExW(str(binary.resolve()), None, 0x02 | 0x20)
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        for kind in (14, 24):  # RT_GROUP_ICON and RT_MANIFEST, both ID 1.
            resource = kernel.FindResourceW(module, 1, kind)
            if not resource:
                raise ValueError(f"Missing Windows resource type {kind}, ID 1")
            if kind == 24:
                size = kernel.SizeofResource(module, resource)
                address = kernel.LockResource(kernel.LoadResource(module, resource))
                if not address or not size:
                    raise ValueError("Empty Windows application manifest")
                manifest = ctypes.string_at(address, size).decode("utf-8-sig")
                if "asInvoker" not in manifest or "PerMonitorV2" not in manifest:
                    raise ValueError("Windows manifest lacks expected privilege/DPI declarations")
        print("Windows icon and GPUI manifest resources: present")
    finally:
        kernel.FreeLibrary(module)


def windows_dependencies(binary: Path) -> None:
    """Leave native PE import evidence for release runtime prerequisites."""
    llvm = shutil.which("llvm-readobj")
    if llvm:
        run(llvm, "--coff-imports", str(binary))
        return
    candidates = []
    for variable in ("ProgramFiles", "ProgramFiles(x86)"):
        directory = os.environ.get(variable)
        if directory:
            candidates.extend((Path(directory) / "Microsoft Visual Studio").glob(
                "*/*/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe"))
    if not candidates:
        raise ValueError("Windows PE dependency inspection needs llvm-readobj or Visual Studio dumpbin")
    run(str(sorted(candidates)[-1]), "/DEPENDENTS", str(binary))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=("macos", "linux", "windows"))
    parser.add_argument("--target", required=True, help="Build label; archive validation checks binary architecture")
    parser.add_argument("--stage", required=True, type=Path)
    args = parser.parse_args()
    expected_os = {"macos": "darwin", "linux": "linux", "windows": "win32"}[args.platform]
    if sys.platform != expected_os:
        raise ValueError("Package inspection must run on the native target OS")
    release.validate_stage(args.stage, args.platform, args.target, release.version())
    binaries = [args.stage / name for name in (release.BINARIES[args.platform], release.MCP_BINARIES[args.platform])]
    if args.platform == "macos":
        contents = args.stage / "KeelShell.app" / "Contents"
        info = contents / "Info.plist"
        run("plutil", "-lint", str(info))
        if plistlib.loads(info.read_bytes())["LSMinimumSystemVersion"] != "15.0":
            raise ValueError("Unexpected macOS minimum version")
        for binary in binaries:
            if not os.access(binary, os.X_OK):
                raise ValueError("macOS packaged executable is not executable")
            run("otool", "-L", str(binary))
            build = run("xcrun", "vtool", "-show-build", str(binary))
            minimums = re.findall(r"^\s*minos\s+(\d+)\.(\d+)(?:\.(\d+))?\s*$", build, re.MULTILINE)
            if not minimums or any(tuple(int(n or "0") for n in value) > (15, 0, 0) for value in minimums):
                raise ValueError("Mach-O deployment target exceeds the advertised macOS 15.0 minimum")
    elif args.platform == "linux":
        run("desktop-file-validate", str(args.stage / "usr/share/applications/keelshell.desktop"))
        for binary in binaries:
            run("readelf", "-h", str(binary))
            dependencies = run("ldd", str(binary))
            if "not found" in dependencies:
                raise ValueError("Linux runtime libraries are missing on the baseline runner")
    else:
        windows_resources(args.stage / "keelshell-app.exe")
        for binary in binaries:
            windows_dependencies(binary)
    print(f"Native package structure inspected: {args.target}; GUI acceptance remains separate")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"Native inspection failed: {error}", file=sys.stderr)
        sys.exit(1)
