#!/usr/bin/env python3
"""Resolve rustc dep-info paths against the recorded Cargo cwd."""
import datetime
import hashlib
import json
from pathlib import Path
import re

OUTPUT = Path(__file__).resolve().parent
SOURCE = Path("/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell")
start = json.loads((OUTPUT / "source-start.json").read_text())
text = (OUTPUT / "app-inventory.log").read_text()
binary = Path(re.search(r"Running unittests .*?\(([^\n]+)\)", text).group(1))
depinfo = binary.with_suffix(".d")
deptext = depinfo.read_text()
raw_inputs = sorted(set(re.findall(r"(?:^|\s)(crates/[^\s:]+\.rs)(?=\s|$)", deptext)))
resolved = {
    path: str((SOURCE / path).resolve().relative_to(SOURCE))
    for path in raw_inputs
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


hashes = {path: sha(SOURCE / path) for path in sorted(set(resolved.values()))}
data = {
    "recorded_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "compile_receipt": str(OUTPUT / "app-inventory.json"),
    "test_receipt": str(OUTPUT / "app-parallel-tests.json"),
    "binary": str(binary),
    "binary_sha256": sha(binary),
    "binary_size_bytes": binary.stat().st_size,
    "dep_info": str(depinfo),
    "dep_info_sha256": sha(depinfo),
    "dep_info_source_resolution_cwd": str(SOURCE),
    "unique_resolved_rust_inputs": len(hashes),
    "normalized_paths": {raw: normalized for raw, normalized in resolved.items() if raw != normalized},
    "candidate_files_in_dep_info": {p: p in hashes for p in start["candidate_file_sha256"]},
    "input_mismatches_vs_start": {
        p: {"start": start["tracked_file_sha256"].get(p), "actual": h}
        for p, h in hashes.items()
        if start["tracked_file_sha256"].get(p) != h
    },
    "foreign_absolute_workspace_rs_paths": sorted(set(
        p for p in re.findall(r"/[^\s:]*crates/[^\s:]+\.rs", deptext)
        if not p.startswith(str(SOURCE) + "/")
    )),
    "prior_record": str(OUTPUT / "compiled-provenance.json"),
    "prior_record_sha256": sha(OUTPUT / "compiled-provenance.json"),
    "prior_check_failure": (
        "The first ad hoc provenance check raised AssertionError because two "
        "relative fixture paths containing ../../ were compared to canonical "
        "tracked-file keys without normalization. The prior JSON is retained; "
        "this receipt resolves paths before comparing the same bytes."
    ),
    "source_sha256": hashes,
}
with (OUTPUT / "compiled-provenance-r2.json").open("x") as stream:
    json.dump(data, stream, ensure_ascii=False, indent=2)
    stream.write("\n")
print(json.dumps({k: v for k, v in data.items() if k != "source_sha256"}, ensure_ascii=False, indent=2))
assert not data["input_mismatches_vs_start"]
assert all(data["candidate_files_in_dep_info"].values())
assert not data["foreign_absolute_workspace_rs_paths"]
author_records = json.loads((OUTPUT / "author-evidence-readback.json").read_text())
assert all(record["hash_matches_receipt"] for record in author_records.values())
