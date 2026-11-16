"""Recompute frozen UDP evidence and optional Git binding without test replay."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Verify exact Git blobs instead of working files")
    args = parser.parse_args()
    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10,
    ).strip())
    manifest = json.loads((BASE / "manifest.json").read_text())
    archive_raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(archive_raw) == manifest["archive_bytes"]
    assert digest(archive_raw) == manifest["archive_sha256"]
    with tarfile.open(BASE / "originals.tar.gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() for member in members)
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        raw = files[row["path"]]
        assert len(raw) == row["bytes"] and digest(raw) == row["sha256"], row["path"]

    def document(name):
        return json.loads(files[name])

    def command(label, exit_code=0):
        receipt = document(label + ".json")
        assert receipt["controller_exit"] == exit_code
        if exit_code != 124:
            assert receipt["observed_exit"] == exit_code
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0
        assert terminal["pipes_drained"] and not terminal["errors"]
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"]
        assert digest(raw) == receipt["log_sha256"]
        return raw

    for label in ("owned-socket-probe-r1", "compile-app-r1", "compile-app-r2",
                  "compile-session-r1", "focused-r2", "darwin-ssh-r1",
                  "full-gate-r2", "release-build-r1", "package-precommit-r1",
                  "package-precommit-inspect-r1"):
        command(label)
    for label in ("focused-r1", "full-gate-r1", "dependencies-r1", "dependencies-r2"):
        command(label, 1)
    command("dependencies-r3", 124)
    timeout = document("dependencies-r3.json")
    assert timeout["timed_out"] and timeout["observed_exit"] is None
    freeze = document("inputs-freeze-r3.json")
    assert freeze["count"] == len(freeze["inputs"]) == 729
    assert digest("".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode()) == freeze["aggregate_sha256"]
    assert document("gate-inputs-after-r2.json")["inputs"] == freeze["inputs"]
    assert document("release-inputs-after-r1.json")["inputs"] == freeze["inputs"]
    r2 = document("inputs-freeze-r2.json")
    prior = {row["path"]: row for row in r2["inputs"]}
    assert [row["path"] for row in freeze["inputs"]
            if prior.get(row["path"]) != row] == ["scripts/monitor_recovery/fixture.py"]
    if args.source:
        names = [name.decode() for name in subprocess.check_output(
            ["git", "ls-tree", "-r", "--name-only", "-z", args.source],
            cwd=root, timeout=20,
        ).split(b"\0") if name and not name.startswith((b"docs/", b".hallmark/"))]
    else:
        names = [name.decode() for name in subprocess.check_output(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
            cwd=root, timeout=20,
        ).split(b"\0") if name and not name.startswith((b"docs/", b".hallmark/"))]
    assert sorted(set(names)) == [row["path"] for row in freeze["inputs"]]

    def verify_input(row):
        raw = (subprocess.check_output(
            ["git", "show", f"{args.source}:{row['path']}"], cwd=root, timeout=15,
        ) if args.source else (root / row["path"]).read_bytes())
        assert len(raw) == row["bytes"] and digest(raw) == row["sha256"], row["path"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    namespace = {"__name__": "sealed_test_parser"}
    exec(compile(files["summarize_tests.py"], "sealed_test_parser", "exec"), namespace)
    parsed = namespace["summarize"](
        b"##[group]Run python scripts/check.py\n" + files["full-gate-r2.log"]
    )
    assert parsed == document("gate-suites-r1.json")
    summary = document("gate-summary-r1.json")
    suites, docs = parsed["suites"], parsed["doctest_runs_unassigned"]
    assert len(suites) == 92 and not parsed["inventory_result_summary_mismatches"]
    for label, rows in (("ordinary_rust", suites), ("rustdoc", docs)):
        assert summary[label] == {
            key: sum(row["summary"][key] for row in rows)
            for key in ("passed", "failed", "ignored")
        }
        assert all(row["summary"]["state"] == "ok" and row["summary"]["failed"] == 0
                   for row in rows)
    for kind, stem, expected, new in (("app", "keelshell_app", 1377, 3),
                                      ("session", "keelshell_session", 358, 15)):
        row = next(row for row in suites if stem in row["suite"] and "unittests" in row["suite"])
        names = re.findall(r"^(\S+): test$",
                           files[f"gate-{kind}-inventory-r1.log"].decode(), re.M)
        outcomes = dict(row["results"])
        assert len(names) == len(set(names)) == expected
        assert set(names) == set(outcomes)
        required = summary["binaries"][kind]["required_new_tests"]
        assert len(required) == new and all(outcomes[name] == "ok" for name in required)
    ssh = document("darwin-ssh-r1/result.json")
    assert ssh["outcome"] == "passed" and ssh["passed_tests"] == 15
    assert all(step["returncode"] == 0 for step in ssh["steps"])
    assert ssh["cleanup"]["owned_processes_stopped"]
    assert ssh["cleanup"]["observed_processes_stopped"]
    assert ssh["cleanup"]["temporary_directory_removed"]
    assert not ssh["cleanup"]["ancestry_unverified"]
    actual_ssh = dict(re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)",
                                files["darwin-ssh-r1/tests.log"].decode(), re.M))
    assert len(actual_ssh) == 15 and set(actual_ssh.values()) == {"ok"}
    real_new = document("focused-summary-r2.json")["interop"]["required_new_tests"]
    assert len(real_new) == 6 and all(actual_ssh[name] == "ok" for name in real_new)
    source_review = document("source-review/summary.json")
    function_review = document("function-review/summary.json")
    assert source_review["decision"] == function_review["recommendation"] == "APPROVE"
    assert set(source_review["finding_counts"].values()) == {0}
    assert set(function_review["severity_counts"].values()) == {0}
    assert source_review["aggregate_sha256"] == freeze["aggregate_sha256"]
    assert function_review["source"]["aggregate_sha256"] == freeze["aggregate_sha256"]
    assert function_review["independent_tests"]["passed"] == 69
    assert function_review["independent_tests"]["failed"] == 0
    assert function_review["independent_tests"]["ignored"] == 0
    assert function_review["prebuilt"]["independent_compilations"] == 0
    assert function_review["prebuilt"]["before_after_identical_including_device_inode_mtime"]
    print(json.dumps({
        "source": args.source or "working-tree", "members": len(files),
        "engineering_inputs": freeze["count"], "aggregate_sha256": freeze["aggregate_sha256"],
        "ordinary_rust": summary["ordinary_rust"], "rustdoc": summary["rustdoc"],
        "new_default_passed": 18, "real_darwin_ssh_passed": 15,
        "independent_prebuilt_passed": 69, "tests_replayed": 0, "mismatches": [],
        "online_dependency_freshness": "UNKNOWN", "native_desktop_acceptance": False,
    }))


if __name__ == "__main__":
    main()
