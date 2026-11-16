"""Verify sealed bytes, host gate inventory and current or exact Git inputs."""

import argparse
from collections import Counter
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]


def digest(data):
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
    assert digest(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(row.isfile() and not row.issparse() for row in members)
        assert len(members) == manifest["count"]
        assert sorted(row.name for row in members) == sorted(row["path"] for row in manifest["entries"])
        for row in manifest["entries"]:
            data = archive.extractfile(row["path"]).read()
            assert len(data) == row["bytes"] and digest(data) == row["sha256"], row["path"]
            files[row["path"]] = data
    assert files["tools/verify.py"] == Path(__file__).read_bytes()
    freeze = json.loads(files["batch/inputs-freeze-r1.json"])
    expected = {row["path"]: row["sha256"] for row in freeze["inputs"]}
    assert len(expected) == freeze["count"] == 719
    assert list(expected) == sorted(expected)
    aggregate = digest("".join(f"{name}\0{sha}\n" for name, sha in expected.items()).encode())
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
        assert digest(data) == expected[name], name
    for name in manifest["changed_engineering_files"]:
        assert digest(files[f"batch/source-r1/{name}"]) == expected[name]
    for label in ("format-r1", "app-inventory-r1", "full-gate-r1",
                  "prior-source-windows-job-r1", "prior-macos-job-r1",
                  "prior-ubuntu-job-r1", "prior-ci-final-r1"):
        receipt = json.loads(files[f"batch/{label}.json"])
        log = files[f"batch/{label}.log"]
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0
        assert not receipt["timed_out"] and receipt["terminal"]["direct_reaped"]
        assert len(log) == receipt["log_bytes"] and digest(log) == receipt["log_sha256"]
    gate = files["batch/full-gate-r1.log"].decode()
    workspace = gate.split("Running: cargo test --workspace --locked -- --test-threads=4\n", 1)[1]
    workspace = workspace.split("Running: cargo test -p keelshell-ai ", 1)[0]
    ordinary, doctests = workspace.split("Doc-tests ", 1)
    summary_pattern = re.compile(
        r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        re.MULTILINE,
    )
    ordinary_rows = summary_pattern.findall(ordinary)
    doc_rows = summary_pattern.findall(doctests)
    assert [sum(int(row[i]) for row in ordinary_rows) for i in (1, 2, 3)] == [3225, 0, 48]
    assert [sum(int(row[i]) for row in doc_rows) for i in (1, 2, 3)] == [14, 0, 0]
    app_start = re.search(r"Running unittests src/main.rs \([^\n]*keelshell_app[^\n]*\)", workspace)
    assert app_start
    app = workspace[app_start.end():]
    app_end = re.search(r"Running (unittests|tests/)", app)
    assert app_end
    app = app[:app_end.start()]
    results = re.findall(r"\btest ([A-Za-z0-9_:]+) \.\.\. (ok|ignored|FAILED)(?:,.*)?$", app, re.MULTILINE)
    listed = re.findall(r"^(\S+): test$", files["batch/app-inventory-r1.log"].decode(), re.MULTILINE)
    assert len(listed) == len(set(listed)) == len(results) == 1343
    assert set(listed) == {name for name, _ in results}
    assert Counter(outcome for _, outcome in results) == {"ok": 1341, "ignored": 2}
    contract = json.loads(files["batch/contract-checks-r1.json"])
    assert contract["old_test_count"] == contract["new_test_count"] == 45
    assert contract["unchanged_count"] == 43
    assert contract["third_empty_parent_attack_body_unchanged"]
    assert contract["ci_only_changes_exact_guard_mapping"]
    final = json.loads(files["batch/prior-ci-final-r1.log"])
    assert final["headSha"] == manifest["prior_ci_source"]
    assert final["status"] == "completed" and final["conclusion"] == "failure"
    for platform in ("macos", "ubuntu"):
        report = json.loads(files[f"batch/prior-{platform}-summary-r2.json"])
        raw = files[f"batch/prior-{platform}-job-r1.log"]
        assert report["raw_bytes"] == len(raw) and report["raw_sha256"] == digest(raw)
        assert len(report["suites"]) == 92
        for suite in report["suites"]:
            outcomes = Counter(value for _, value in suite["results"])
            assert len(suite["results"]) == len({name for name, _ in suite["results"]}) == suite["declared_tests"]
            assert [outcomes[x] for x in ("ok", "FAILED", "ignored")] == [
                suite["summary"][x] for x in ("passed", "failed", "ignored")
            ]
        archive = next(row for row in report["suites"] if "tests/archive.rs " in row["suite"])
        assert archive["declared_tests"] == 41
        assert ["zip_records_private_file_and_directory_modes_in_unix_metadata", "ok"] in archive["results"]
    print(json.dumps({
        "source": options.source or "current",
        "engineering_inputs": len(names), "members": len(files), "mismatches": 0,
        "host_ordinary": [3225, 0, 48], "host_app_inventory": 1343,
        "prior_ci_source": final["headSha"], "tests_replayed": 0,
        "windows_candidate_execution_at_seal": 0,
    }))


if __name__ == "__main__":
    main()
