"""Recompute successful and aborted CI evidence without test replay."""
import ast
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
from types import SimpleNamespace

BASE = Path(__file__).resolve().parent
SOURCE = "16703293012d798f885e1590f02c348c47e81a10"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert digest(raw) == manifest["archive_sha256"]
    with tarfile.open(BASE / "originals.tar.gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() for member in members)
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and digest(data) == row["sha256"]

    def document(name):
        return json.loads(files[name])

    state = document("ci-final-r1.log")
    assert state["headSha"] == SOURCE and state["databaseId"] == 43945988599
    assert state["status"] == "completed" and state["conclusion"] == "failure"
    jobs = {job["databaseId"]: job for job in state["jobs"]}
    assert set(jobs) == {133575666472, 133575666946, 133575666982}
    assert all(job["status"] == "completed" for job in jobs.values())
    assert jobs[133575666472]["conclusion"] == "success"
    assert jobs[133575666946]["conclusion"] == jobs[133575666982]["conclusion"] == "failure"
    for job in jobs.values():
        gate = next(step for step in job["steps"] if step["name"] == "Rust quality gate")
        assert gate["conclusion"] == job["conclusion"]
    mac_ssh = next(step for step in jobs[133575666472]["steps"]
                   if step["name"] == "OpenSSH interoperability (macOS and Linux)")
    assert mac_ssh["conclusion"] == "success"

    for label, expected in (
        ("ci-watch-r1", 1), ("ci-final-r1", 0), ("download-terminal-r1", 0),
        ("partial-derive-r1", 1), ("partial-derive-r2", 0),
    ):
        receipt = document(label + ".json")
        assert receipt["controller_exit"] == receipt["observed_exit"] == expected
        assert receipt["terminal"]["direct_reaped"]
        assert receipt["terminal"]["active_processes"] == 0
        assert receipt["terminal"]["pipes_drained"]
        assert not receipt["terminal"]["errors"]
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"] and digest(raw) == receipt["log_sha256"]

    namespace = {"__name__": "sealed_strict_parser"}
    exec(compile(files["summarize_tests.py"], "sealed_strict_parser", "exec"), namespace)
    mac_raw = files["job-133575666472.log"]
    parsed = namespace["summarize"](mac_raw)
    assert parsed == document("job-133575666472-suites.json")
    mac = document("macos-summary-r1.json")
    assert mac["raw_bytes"] == len(mac_raw) and mac["raw_sha256"] == digest(mac_raw)
    assert len(parsed["suites"]) == 92 and not parsed["inventory_result_summary_mismatches"]
    for label, rows in (("ordinary_rust", parsed["suites"]),
                        ("rustdoc", parsed["doctest_runs_unassigned"])):
        assert mac[label] == {
            key: sum(row["summary"][key] for row in rows)
            for key in ("passed", "failed", "ignored")
        }
    assert mac["ordinary_rust"] == {"passed": 3325, "failed": 0, "ignored": 58}
    assert mac["app"]["passed"] == 1375 and mac["app"]["failed"] == 0
    assert mac["rustdoc"] == {"passed": 14, "failed": 0, "ignored": 0}
    discovery = namespace["normalize"](mac_raw).split(
        " -m unittest discover -s scripts -p test_*.py", 1,
    )[1].split("Running: cargo fmt", 1)[0]
    assert re.findall(r"Ran (\d+) tests? in", discovery) == ["293"]
    assert re.findall(r"OK \(skipped=(\d+)\)", discovery) == ["2"]

    tree = ast.parse(files["summarize_partial.py"])
    function = next(node for node in tree.body
                    if isinstance(node, ast.FunctionDef) and node.name == "summarize_failure")
    partial_namespace = {
        "parser": SimpleNamespace(normalize=namespace["normalize"], summarize=namespace["summarize"]),
        "re": re, "Counter": Counter, "hashlib": hashlib,
    }
    exec(compile(ast.Module(body=[function], type_ignores=[]), "sealed_partial", "exec"),
         partial_namespace)
    for job in (133575666946, 133575666982):
        raw = files[f"job-{job}.log"]
        assert raw == files[f"job-{job}-failure-r1.log"]
        partial = partial_namespace["summarize_failure"](raw)
        # JSON represents regex result tuples as arrays; compare that exact wire form.
        assert json.loads(json.dumps(partial)) == document(f"job-{job}-partial-r2.json")
        app = partial["aborted_app"]
        assert not app["complete_summary_present"] and not app["full_app_passed"]
        assert app["unreported_count"] > 0 and not partial["workspace_gate_passed"]
        assert app["observed_result_count"] + app["unreported_count"] == app["declared"]

    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10,
    ).strip())
    freeze = document("inputs-freeze-r3.json")
    names = [
        name.decode() for name in subprocess.check_output(
            ["git", "ls-tree", "-r", "--name-only", "-z", SOURCE], cwd=root, timeout=20,
        ).split(b"\0") if name and not name.startswith((b"docs/", b".hallmark/"))
    ]
    assert sorted(names) == [row["path"] for row in freeze["inputs"]]
    assert len(names) == 729
    assert digest("".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode()) == freeze["aggregate_sha256"]

    def verify_input(row):
        raw = subprocess.check_output(
            ["git", "show", f"{SOURCE}:{row['path']}"], cwd=root, timeout=15,
        )
        assert len(raw) == row["bytes"] and digest(raw) == row["sha256"], row["path"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    print(json.dumps({
        "source": SOURCE, "run": 43945988599, "members": len(files),
        "engineering_inputs": 729, "conclusion": "failure",
        "macos": "success", "ubuntu_windows": "aborted app; no complete summary",
        "tests_replayed": 0, "mismatches": [], "native_desktop_acceptance": False,
    }))


if __name__ == "__main__":
    main()
