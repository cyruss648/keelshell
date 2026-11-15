#!/usr/bin/env python3
"""Bind historical r1 evidence and remove only this review's redundant outputs."""

import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def store(path, value):
    if path.exists():
        raise RuntimeError(f"refusing to overwrite: {path}")
    path.write_text(json.dumps(value, indent=2) + "\n")


manifest_bytes = (HERE / "platform-inputs-r1.original.json").read_bytes()
manifest = json.loads(manifest_bytes)
observed = []
for relative, expected in manifest["files"].items():
    frozen = (HERE / "frozen-platform-r1" / relative).read_bytes()
    assert digest(frozen) == expected, relative
    live = (ROOT / relative).read_bytes()
    observed.append(
        {
            "path": relative,
            "r1_sha256": expected,
            "retained_r1_sha256": digest(frozen),
            "live_sha256": digest(live),
            "live_matches_r1": digest(live) == expected,
        }
    )
store(
    HERE / "freeze-final.json",
    {
        "observed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "original_manifest_sha256": digest(manifest_bytes),
        "current_r1_manifest_sha256": digest(
            (ROOT / "work/windows-local-backend-20261115/platform-inputs-r1.json").read_bytes()
        ),
        "retained_r1_count": len(observed),
        "retained_r1_match_count": len(observed),
        "live_changed_count": sum(not entry["live_matches_r1"] for entry in observed),
        "live_drift_context": "parent authorized r2 implementation; no r2 approval in r1",
        "files": observed,
    },
)
target = HERE / "target"
assert target.is_dir() and not target.is_symlink()
assert target.resolve().parent == HERE
artifact_inventory = []
copied = []
for path in sorted(target.rglob("*")):
    assert not path.is_symlink(), path
    if not path.is_file():
        continue
    content = path.read_bytes()
    relative = path.relative_to(target)
    artifact_inventory.append(
        {"path": str(relative), "bytes": len(content), "sha256": digest(content)}
    )
    if "keelshell-platform" in relative.parts and (
        path.suffix == ".d" or "fingerprint" in relative.parts
    ):
        destination = HERE / "compiler-evidence" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
        assert destination.read_bytes() == content
        copied.append(
            {
                "original": str(path),
                "retained": str(destination),
                "bytes": len(content),
                "sha256": digest(content),
            }
        )
store(
    HERE / "compiler-artifacts-r1.json",
    {
        "description": "artifact hashes, not retained binaries; zero byte Windows test rmeta is typecheck output",
        "target": str(target),
        "files": artifact_inventory,
        "retained_dep_info_and_fingerprints": copied,
    },
)
receipts = []
for receipt_path in sorted(HERE.glob("*.json")):
    receipt = json.loads(receipt_path.read_text())
    if not isinstance(receipt, dict) or "pid" not in receipt:
        continue
    assert receipt["waited_and_reaped"]
    try:
        os.kill(receipt["pid"], 0)
    except ProcessLookupError:
        pass
    else:
        raise RuntimeError(f"captured pid still present: {receipt['pid']}")
    output = Path(receipt["output_path"])
    assert digest(output.read_bytes()) == receipt["output_sha256"]
    receipts.append(receipt_path.name)
assert len(receipts) == 11
for entry in copied:
    assert digest(Path(entry["retained"]).read_bytes()) == entry["sha256"]
assert copied
source_lines = HERE / "source-lines"
assert source_lines.is_dir() and not source_lines.is_symlink()
assert source_lines.resolve().parent == HERE
source_line_files = []
for path in sorted(source_lines.rglob("*")):
    assert not path.is_symlink(), path
    if path.is_file():
        source_line_files.append(
            {"path": str(path.relative_to(HERE)), "bytes": path.stat().st_size}
        )
shutil.rmtree(target)
shutil.rmtree(source_lines)
store(
    HERE / "cleanup.json",
    {
        "finished_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "evidence_verified_before_deletion": True,
        "retained_r1_source_files_verified": len(observed),
        "captured_receipts_verified": receipts,
        "retained_compiler_evidence_files": len(copied),
        "removed": [
            {
                "path": str(target),
                "file_count": len(artifact_inventory),
                "logical_bytes": sum(entry["bytes"] for entry in artifact_inventory),
                "exists_after": target.exists(),
            },
            {
                "path": str(source_lines),
                "file_count": len(source_line_files),
                "logical_bytes": sum(entry["bytes"] for entry in source_line_files),
                "exists_after": source_lines.exists(),
            },
        ],
        "deletion_scope": "exact two non-symlink direct children of review-platform",
        "parent_target_touched": False,
    },
)
store(
    HERE / "result-r1.json",
    {
        "review": "historical r1; source contract only",
        "decision": "HOLD",
        "severity_counts": {"P0": 0, "P1": 0, "P2": 2, "P3": 0},
        "findings": ["R1-P2-01", "R1-P2-02"],
        "notice_03": "supplement to R1-P2-01, not a third finding",
        "engineering_commands": {"total": 7, "cargo": 3, "rustc": 4},
        "engineering_expected_exit_zero": 5,
        "engineering_expected_exit_one_negative_probes": 2,
        "environment_commands_separate": 4,
        "captured_wrapper_receipts": len(receipts),
        "host_tests": {"passed": 15, "failed": 0, "ignored": 0},
        "windows_conditional_typechecked": 42,
        "windows_tests_executed": 0,
        "windows_native": "NOT_RUN",
        "frozen_inputs": {"count": 9, "retained_match_count": 9},
        "live_changed_from_r1": sum(not entry["live_matches_r1"] for entry in observed),
        "subsequent_r2": "separate differential review and decision",
        "own_r1_target_deleted": not target.exists(),
    },
)
print(
    json.dumps(
        {
            "r1": "HOLD",
            "retained_r1_matches": len(observed),
            "live_differences": sum(not entry["live_matches_r1"] for entry in observed),
            "compiler_evidence_copies": len(copied),
            "target_deleted": not target.exists(),
            "target_logical_bytes_removed": sum(entry["bytes"] for entry in artifact_inventory),
        },
        indent=2,
    )
)
