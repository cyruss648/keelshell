"""Recompute local evidence and Git binding without replaying Windows or tests."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

import summarize_tests

BASE = Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Exact Git commit whose engineering inputs must match")
    args = parser.parse_args()
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"] and sha(raw) == manifest["archive_sha256"]
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() and not PurePosixPath(member.name).is_absolute()
                   and ".." not in PurePosixPath(member.name).parts for member in members)
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and sha(data) == row["sha256"], row["path"]

    def document(name):
        return json.loads(files[name])

    acceptance = document("acceptance.json")
    for entry in acceptance["commands"]:
        prefix = entry["path"]
        receipt = document(prefix + ".json")
        assert receipt["observed_exit"] == receipt["controller_exit"] == entry["expected_exit"]
        assert not receipt["timed_out"] and not receipt["interrupted"]
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0
        assert terminal["pipes_drained"] and not terminal["errors"]
        log = files[prefix + ".log"]
        assert len(log) == receipt["log_bytes"] and sha(log) == receipt["log_sha256"]
    freeze = document(acceptance["freeze"])
    after = document(acceptance["after"])
    assert freeze["inputs"] == after["inputs"] and after["unchanged"]
    assert freeze["count"] == len(freeze["inputs"])
    aggregate = sha("".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode())
    assert aggregate == freeze["aggregate_sha256"] == after["aggregate_sha256"]
    inputs = {row["path"]: row for row in freeze["inputs"]}
    gate_raw = files[acceptance["gate_log"]]
    summary = summarize_tests.summarize(gate_raw)
    recorded = document(acceptance["gate_summary"])
    assert {key: value for key, value in recorded.items() if key != "log"} == summary
    assert not summary["inventory_result_summary_mismatches"]
    assert all(row["summary"]["failed"] == 0 for row in (
        summary["suites"] + summary["doctest_runs_unassigned"]))
    inventory = document(acceptance["inventory"])
    for entry in inventory["targets"].values():
        listed = re.findall(
            r"^(\S+): test$", files[entry["log"]].decode(), re.MULTILINE)
        suites = [row for row in summary["suites"] if row["suite"] == entry["suite"]]
        assert len(suites) == 1
        results = dict(suites[0]["results"])
        assert len(listed) == len(set(listed)) == len(results) == entry["count"]
        assert set(listed) == set(results)
        for name, state in entry["required"].items():
            assert results[name] == state, name
    python_section = gate_raw.decode().split("Running: cargo fmt --all --check", 1)[0]
    runs = re.findall(r"Ran (\d+) tests in [\d.]+s\s+OK(?: \(skipped=(\d+)\))?",
                      python_section)
    assert runs
    python_run = {"run": int(runs[-1][0]), "skipped": int(runs[-1][1] or 0)}
    assert python_run == acceptance["python_discovery"]
    for marker in ("--readiness-error-tests", "--observer-io-error-tests",
                   "--controller-small-stack"):
        assert marker in gate_raw.decode(), marker
    negative = files[acceptance["nonwindows_negative_log"]]
    assert b"WindowsRequired" in negative
    assert b"test result: FAILED. 0 passed; 1 failed; 0 ignored;" in negative

    reviews = document(acceptance["review_binding"])
    assert reviews["independent_reviewers"] >= 2
    assert not any(reviews["open_findings"].values())
    for entry in reviews["reports"]:
        assert sha(files[entry["path"]]) == entry["sha256"]
        assert entry["decision"] == "APPROVE_SCOPED_SOURCE"
    for row in reviews["reviewed_inputs"]:
        expected = inputs[row["path"]]
        assert row["sha256"] == expected["sha256"], row["path"]
    assert document(acceptance["agent_lifecycle"])["all_owned_agents_closed"]
    native = document(acceptance["native_availability"])
    assert native["gui_actions"] == native["screenshots"] == 0
    assert not native["native_desktop_acceptance"]
    assert acceptance["local_windows_ssh_executions"] == 0

    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10).strip())
    if args.source:
        assert re.fullmatch(r"[0-9a-f]{40}", args.source)
        argv = ["git", "ls-tree", "-r", "--name-only", "-z", args.source]
    else:
        argv = ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]
    names = sorted({
        name.decode() for name in subprocess.check_output(argv, cwd=root, timeout=30).split(b"\0")
        if name and not name.startswith((b"docs/", b".hallmark/"))
    })
    assert names == [row["path"] for row in freeze["inputs"]]

    def verify_input(row):
        data = (subprocess.check_output(
            ["git", "show", f"{args.source}:{row['path']}"], cwd=root, timeout=20)
            if args.source else (root / row["path"]).read_bytes())
        assert len(data) == row["bytes"] and sha(data) == row["sha256"], row["path"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    print(json.dumps({
        "source": args.source or "working-tree", "members": len(files),
        "engineering_inputs": len(inputs), "aggregate_sha256": aggregate,
        "ordinary_rust": {
            key: sum(row["summary"][key] for row in summary["suites"])
            for key in ("passed", "failed", "ignored")
        },
        "python_discovery": python_run, "tests_replayed": 0,
        "local_windows_ssh_executions": 0, "native_desktop_acceptance": False,
        "mismatches": [],
    }))


if __name__ == "__main__":
    main()
