#!/usr/bin/env python3
"""Check the external Windows API against this review's independent metadata."""

import hashlib
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
WINDOWS_TARGET = HERE / "target" / "x86_64-pc-windows-msvc"
PROBES = HERE / "probes"
PROBES.mkdir(exist_ok=True)
OUTPUT = HERE / "target" / "reviewer-probes"
OUTPUT.mkdir(exist_ok=True)
libraries = [
    path
    for path in WINDOWS_TARGET.rglob("libkeelshell_platform-*.rmeta")
    if path.stat().st_size
]
if len(libraries) != 1:
    raise SystemExit(f"expected one independent Windows library, got {len(libraries)}")
dependency_paths = sorted({path.parent for path in WINDOWS_TARGET.rglob("*.rmeta")})

probes = [
    (
        "public_composition_control",
        None,
        """use keelshell_platform::{Directory, FileInfo, PrivateEntry, metadata};
use std::fs::File;
use std::io;

pub fn observe_external_file(file: &File) -> io::Result<FileInfo> {
    metadata(file)
}

pub fn independent_read_while_source_is_held(parent: &Directory) -> io::Result<()> {
    let source = parent.create_file("review-sharing-stage")?;
    let _reader = parent.open_file("review-sharing-stage")?;
    let _ = metadata(source.as_file())?;
    Ok(())
}

// This valid API composition is the notice-01 native regression candidate.
// This probe typechecks it only. It does not run Windows syscalls.
pub fn retained_private_target_regression(parent: &Directory) -> io::Result<()> {
    let destination = parent.create_directory("review-private-target")?;
    let target = destination.as_directory()?;
    let source: PrivateEntry = parent.create_file("review-rename-stage")?;
    target.rename_no_replace(&source, "published")
}
""",
    ),
    (
        "external_file_directory_constructor",
        "E0624",
        """use keelshell_platform::Directory;
use std::fs::File;
use std::io;

pub fn inject(file: File) -> io::Result<Directory> {
    Directory::from_file(file)
}
""",
    ),
    (
        "external_file_private_fields",
        "E0451",
        """use keelshell_platform::{Directory, PrivateEntry};
use std::fs::File;
use std::sync::Arc;

pub fn inject_directory(file: File) -> Directory {
    Directory { file: Arc::new(file) }
}

pub fn inject_publication(file: File) -> PrivateEntry {
    PrivateEntry { file, directory: false }
}
""",
    ),
]

results = []
for label, expected_error, source in probes:
    path = PROBES / f"{label}.rs"
    if path.exists():
        raise SystemExit("refusing to overwrite a probe")
    path.write_text(source)
    argv = [
        "rustc",
        "--edition=2024",
        "--crate-type=lib",
        "--crate-name",
        label,
        "--target",
        "x86_64-pc-windows-msvc",
        "--emit=metadata",
        "--error-format=json",
        "--extern",
        f"keelshell_platform={libraries[0]}",
        "--out-dir",
        str(OUTPUT),
    ]
    for dependency in dependency_paths:
        argv.extend(["-L", f"dependency={dependency}"])
    argv.append(str(path))
    process = subprocess.run(argv, capture_output=True, timeout=30)
    (PROBES / f"{label}.stdout.txt").write_bytes(process.stdout)
    (PROBES / f"{label}.stderr.jsonl").write_bytes(process.stderr)
    codes = []
    for line in process.stderr.splitlines():
        diagnostic = json.loads(line)
        if diagnostic.get("level") == "error" and diagnostic.get("code"):
            codes.append(diagnostic["code"]["code"])
    passed = (
        process.returncode == 0
        if expected_error is None
        else process.returncode != 0 and codes and set(codes) == {expected_error}
    )
    results.append(
        {
            "label": label,
            "argv": argv,
            "exit_code": process.returncode,
            "expected_error": expected_error,
            "actual_error_codes": codes,
            "expected_result_observed": bool(passed),
            "source_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "native_execution": "NOT_RUN",
        }
    )

receipt = {
    "library": str(libraries[0]),
    "library_sha256": hashlib.sha256(libraries[0].read_bytes()).hexdigest(),
    "probes": results,
    "all_expected_results_observed": all(
        result["expected_result_observed"] for result in results
    ),
}
(PROBES / "results.json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt, indent=2))
raise SystemExit(0 if receipt["all_expected_results_observed"] else 1)
