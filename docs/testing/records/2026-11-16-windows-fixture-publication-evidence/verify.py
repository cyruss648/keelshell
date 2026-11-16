"""Verify fixture-only inputs, bounded gate receipts and preserved counterexamples."""

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
PRIOR = "95813a76f0ec9b59fd043cd982e33cdc01dd69f8"
CHANGED = "scripts/test_windows_native_crash.py"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source")
    options = parser.parse_args()
    if options.source:
        assert re.fullmatch(r"[0-9a-f]{40}", options.source)
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    assert len(packed) == manifest["archive_bytes"]
    assert sha(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(row.isfile() and not row.issparse() for row in members)
        assert len(members) == manifest["count"]
        assert sorted(row.name for row in members) == sorted(row["path"] for row in manifest["entries"])
        for row in manifest["entries"]:
            data = archive.extractfile(row["path"]).read()
            assert len(data) == row["bytes"] and sha(data) == row["sha256"], row["path"]
            files[row["path"]] = data
    assert files["tools/verify.py"] == Path(__file__).read_bytes()
    freeze = json.loads(files["batch/inputs-freeze-r1.json"])
    expected = {row["path"]: row["sha256"] for row in freeze["inputs"]}
    assert len(expected) == freeze["count"] == 719 and list(expected) == sorted(expected)
    aggregate = sha("".join(f"{name}\0{digest}\n" for name, digest in expected.items()).encode())
    assert aggregate == freeze["aggregate_sha256"] == manifest["engineering_sha256"]
    command = (
        ["git", "ls-tree", "-rz", "--name-only", options.source] if options.source else
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]
    )
    listing = subprocess.check_output(command, cwd=ROOT, timeout=60)
    names = sorted({
        name.decode() for name in listing.split(b"\0")
        if name and not name.startswith(b"docs/")
    })
    assert names == list(expected)
    for name in names:
        data = (
            subprocess.check_output(["git", "show", f"{options.source}:{name}"], cwd=ROOT, timeout=30)
            if options.source else (ROOT / name).read_bytes()
        )
        assert sha(data) == expected[name], name
    diff_command = ["git", "diff", "--name-only", "--no-renames", PRIOR]
    if options.source:
        diff_command.append(options.source)
    diff = subprocess.check_output(diff_command, cwd=ROOT, timeout=30).decode().splitlines()
    assert [name for name in diff if not name.startswith("docs/")] == [CHANGED]
    baseline = subprocess.check_output(["git", "show", f"{PRIOR}:{CHANGED}"], cwd=ROOT, timeout=30)
    assert baseline == files[f"batch/baseline/{CHANGED}"]
    assert sha(files[f"batch/source-r1/{CHANGED}"]) == expected[CHANGED]
    for label in ("scoped-tests-r1", "scoped-gate-r1", "negative-controls-r1"):
        receipt = json.loads(files[f"batch/{label}.json"])
        raw = files[f"batch/{label}.log"]
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0
        assert receipt["terminal"]["direct_reaped"] and not receipt["timed_out"]
        assert len(raw) == receipt["log_bytes"] and sha(raw) == receipt["log_sha256"]
    scoped = files["batch/scoped-tests-r1.log"].decode()
    gate = files["batch/scoped-gate-r1.log"].decode()
    assert re.findall(r"Ran (\d+) tests? in", scoped) == ["72"]
    assert re.findall(r"Ran (\d+) tests? in", gate) == ["293"]
    assert "OK (skipped=2)" in scoped and "OK (skipped=2)" in gate
    controls = json.loads(files["batch/negative-summary-r1.json"])
    assert controls["engineering_mutations"] == 0 and len(controls["controls"]) == 2
    for row in controls["controls"]:
        assert row["exit"] == 1
        assert row["exact_failure"] in files[f"batch/negative-{row['control']}-r1.log"].decode()
    windows = json.loads(files["batch/prior-source-windows-summary-r1.json"])
    assert windows["source"] == PRIOR
    assert len(windows["session_inventory"]) == len(set(windows["session_inventory"])) == 332
    assert windows["session_counts"] == {"ok": 329, "ignored": 3}
    assert windows["zmodem_counts"] == {"ok": 43, "ignored": 2}
    assert windows["full_gate"]["full_workspace_rust_tests"] == "NOT_RUN"
    print(json.dumps({
        "source": options.source or "current", "members": len(files),
        "engineering_inputs": len(names), "mismatches": 0, "changed_inputs": [CHANGED],
        "python_discovery_run": 293, "python_discovery_skipped": 2,
        "prior_ci_source": PRIOR, "whole_Rust_tests_repeated": False,
        "tests_replayed": 0, "windows_candidate_execution_at_seal": 0,
    }))


if __name__ == "__main__":
    main()
