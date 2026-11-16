"""Recompute exact completed CI evidence without Cargo, extraction or test replay."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
SOURCE = "a1a97b3b2ee0f0ccf91ea53ca96623aa8867fe8d"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default=SOURCE)
    args = parser.parse_args()
    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10,
    ).strip())
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert hashlib.sha256(raw).hexdigest() == manifest["archive_sha256"]
    with tarfile.open(BASE / "originals.tar.gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() for member in members)
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"] == 27
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"]
        assert hashlib.sha256(data).hexdigest() == row["sha256"]

    def document(name):
        return json.loads(files[name])

    state = document("ci-current-r3.log")
    summary = document("ci-summary-r1.json")
    assert state["status"] == "completed" and state["conclusion"] == "success"
    assert state["headSha"] == summary["source"] == SOURCE
    assert len(state["jobs"]) == 3 and all(
        job["status"] == "completed" and job["conclusion"] == "success"
        for job in state["jobs"]
    )
    namespace = {"__name__": "sealed_ci_parser"}
    exec(compile(files["summarize_tests.py"], "sealed_ci_parser", "exec"), namespace)
    for job in summary["platforms"]:
        log = files[job["file"]]
        assert hashlib.sha256(log).hexdigest() == job["sha256"]
        parsed = namespace["summarize"](log)
        assert parsed == document(f"job-{job['id']}-suites.json")
        suites, docs = parsed["suites"], parsed["doctest_runs_unassigned"]
        assert len(suites) == 92 and not parsed["inventory_result_summary_mismatches"]
        assert all(row["summary"]["failed"] == 0 and row["summary"]["state"] == "ok"
                   for row in suites + docs)
        for label, rows in (("ordinary_rust", suites), ("rustdoc", docs)):
            assert job[label] == {
                key: sum(row["summary"][key] for row in rows)
                for key in ("passed", "failed", "ignored")
            }
        assert job["app"] == next(
            row["summary"] for row in suites if "keelshell_app" in row["suite"]
        )
    for label in ("ci-watch-r1", "download-final-r1", "summarize-final-r1",
                  "exact-source-binding-r1"):
        receipt = document(label + ".json")
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0
        assert receipt["terminal"]["direct_reaped"]
        assert receipt["terminal"]["active_processes"] == 0
        assert receipt["terminal"]["pipes_drained"]
        log = files[label + ".log"]
        assert len(log) == receipt["log_bytes"]
        assert hashlib.sha256(log).hexdigest() == receipt["log_sha256"]
    freeze = document("inputs-freeze-r5.json")
    names = [
        name.decode() for name in subprocess.check_output(
            ["git", "ls-tree", "-r", "--name-only", "-z", args.source],
            cwd=root, timeout=20,
        ).split(b"\0")
        if name and not name.startswith((b"docs/", b".hallmark/"))
    ]
    assert sorted(names) == [row["path"] for row in freeze["inputs"]]

    def verify_input(row):
        data = subprocess.check_output(
            ["git", "show", f"{args.source}:{row['path']}"], cwd=root, timeout=15,
        )
        assert len(data) == row["bytes"], row["path"]
        assert hashlib.sha256(data).hexdigest() == row["sha256"], row["path"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    print(json.dumps({
        "source": args.source, "ci_source": SOURCE, "run": 43934041540,
        "members": len(files), "engineering_inputs": len(names),
        "all_three_platforms": "completed/success",
        "tests_replayed": 0, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
