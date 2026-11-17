"""Verify the bounded-census repair evidence without replaying tests or CI."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import zipfile

BASE = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE.parent))
import summarize_tests


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def unpack(directory):
    manifest = json.loads((directory / "manifest.json").read_text())
    raw = (directory / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert digest(raw) == manifest["archive_sha256"]
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() and not PurePosixPath(member.name).is_absolute()
                   and ".." not in PurePosixPath(member.name).parts for member in members)
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and digest(data) == row["sha256"], row["path"]
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Exact repair commit; otherwise verify the working tree")
    args = parser.parse_args()
    files = unpack(BASE)
    original = unpack(BASE.parent)

    def document(name):
        return json.loads(files[name])

    acceptance = document("acceptance.json")
    baseline = json.loads(original["acceptance.json"])
    before = json.loads(original[baseline["freeze"]])
    freeze = document("integration/full-gate-inputs-r2.json")
    after = document("integration/full-gate-inputs-after-r2.json")
    assert freeze["inputs"] == after["inputs"] and after["unchanged"]
    assert freeze["count"] == len(freeze["inputs"]) == 765
    aggregate = digest("".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode())
    assert aggregate == freeze["aggregate_sha256"] == after["aggregate_sha256"]
    old = {row["path"]: row for row in before["inputs"]}
    new = {row["path"]: row for row in freeze["inputs"]}
    assert set(old) == set(new)
    changed = sorted(name for name in old if old[name] != new[name])
    assert changed == acceptance["changed_engineering_inputs"]
    assert len(changed) == 4 and all(name.endswith(".py") for name in changed)

    for entry in acceptance["commands"]:
        prefix = entry["path"]
        receipt = document(prefix + ".json")
        assert receipt["observed_exit"] == receipt["controller_exit"] == entry["expected_exit"]
        assert not receipt["timed_out"] and not receipt["interrupted"]
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0
        assert terminal["pipes_drained"] and not terminal["errors"]
        raw = files[prefix + ".log"]
        assert len(raw) == receipt["log_bytes"] and digest(raw) == receipt["log_sha256"]

    raw = files["integration/full-gate-r2.log"]
    summary = summarize_tests.summarize(raw)
    recorded = document("integration/gate-summary-r2.json")
    assert {key: value for key, value in recorded.items() if key != "log"} == summary
    assert not summary["inventory_result_summary_mismatches"]
    assert all(row["summary"]["failed"] == 0 for row in (
        summary["suites"] + summary["doctest_runs_unassigned"]))
    inventory = json.loads(original[baseline["inventory"]])
    for entry in inventory["targets"].values():
        names = re.findall(r"^(\S+): test$", original[entry["log"]].decode(), re.MULTILINE)
        rows = [row for row in summary["suites"] if row["suite"] == entry["suite"]]
        assert len(rows) == 1
        results = dict(rows[0]["results"])
        assert len(names) == len(set(names)) == len(results) == entry["count"]
        assert set(names) == set(results)
        for name, state in entry["required"].items():
            assert results[name] == state
    python_section = summarize_tests.normalize(raw).split("Running: cargo fmt --all --check", 1)[0]
    runs = re.findall(r"Ran (\d+) tests in [\d.]+s\s+OK(?: \(skipped=(\d+)\))?",
                      python_section)
    assert runs
    python_run = {"run": int(runs[-1][0]), "skipped": int(runs[-1][1] or 0)}
    assert python_run == acceptance["python_discovery"]
    for marker in ("--readiness-error-tests", "--observer-io-error-tests",
                   "--controller-small-stack"):
        assert marker in raw.decode()

    review = document("review/notice/review-decision-r1.json")
    assert review["decision"] == "LIMITED_APPROVE_FOR_PARENT_FINAL_FULL_GATE"
    assert not review["blocking_findings"] and not any(review["findings_by_priority"].values())
    assert review["candidate_aggregate_sha256"] == aggregate
    assert sorted(review["scope"]) == changed
    assert review["reviewer_executed_tests"]["total"] == {
        "run": 49, "passed": 49, "failed": 0, "errors": 0, "skipped": 0,
    }
    assert document("integration/repair-agent-lifecycle.json")["repair_source_agents_closed"]

    ci = {}
    for label, expected in (("quality-r1", "success"), ("direct-api", "success"),
                            ("openssh-r1", "failure")):
        prefix = f"first-ci/{label}/"
        receipt = document(prefix + "fetch-receipt.json")
        run = document(prefix + "run.json")
        assert receipt["source"] == run["head_sha"] == acceptance["baseline_source"]
        assert receipt["status"] == run["status"] == "completed"
        assert receipt["conclusion"] == run["conclusion"] == expected
        for download in receipt["downloads"]:
            content = files[prefix + download["zip"]]
            assert len(content) == download["bytes"] and digest(content) == download["sha256"]
            if download["github_digest"] is not None:
                assert download["github_digest"] == "sha256:" + digest(content)
            with zipfile.ZipFile(io.BytesIO(content)) as archive:
                names = [item.filename for item in archive.infolist() if not item.is_dir()]
                assert len(names) == len(set(names)) == len(download["members"])
                assert set(names) == {row["path"] for row in download["members"]}
                for row in download["members"]:
                    data = archive.read(row["path"])
                    assert len(data) == row["bytes"] and digest(data) == row["sha256"]
        ci[label] = {"run_id": run["id"], "conclusion": expected}

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
        assert len(data) == row["bytes"] and digest(data) == row["sha256"], row["path"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    print(json.dumps({
        "source": args.source or "working-tree", "members": len(files),
        "engineering_inputs": len(new), "changed_python_inputs": changed,
        "aggregate_sha256": aggregate,
        "ordinary_rust": {
            key: sum(row["summary"][key] for row in summary["suites"])
            for key in ("passed", "failed", "ignored")
        },
        "python_discovery": python_run, "first_ci": ci, "tests_replayed": 0,
        "repair_windows_ssh_acceptance": False, "native_desktop_acceptance": False,
        "mismatches": [],
    }))


if __name__ == "__main__":
    main()
