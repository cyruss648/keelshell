"""Verify delivery, cleanup and review receipts without accessing old work paths."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import tarfile

BASE = Path(__file__).resolve().parent
SOURCE = "5d2df2fc5ac8566fe11182b7f4337076296dfa54"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default=SOURCE)
    args = parser.parse_args()
    assert args.source == SOURCE
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"] and sha(raw) == manifest["archive_sha256"]
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(m.isfile() and not PurePosixPath(m.name).is_absolute()
                   and ".." not in PurePosixPath(m.name).parts for m in members)
        files = {m.name: archive.extractfile(m).read() for m in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {r["path"] for r in manifest["entries"]}
    for row in manifest["entries"]:
        assert len(files[row["path"]]) == row["bytes"] and sha(files[row["path"]]) == row["sha256"]

    def doc(name):
        return json.loads(files[name])

    def command(label, exit_code=0):
        receipt = doc(label + ".json")
        assert receipt["observed_exit"] == receipt["controller_exit"] == exit_code
        end = receipt["terminal"]
        assert end["direct_reaped"] and end["active_processes"] == 0
        assert end["pipes_drained"] and not end["errors"]
        data = files[label + ".log"]
        assert len(data) == receipt["log_bytes"] and sha(data) == receipt["log_sha256"]

    for label in (
        "git-stage-r1", "git-commit-r1", "git-push-r1",
        "evidence-verify-working-r2", "evidence-verify-git-r1",
        "source-package-r2", "source-package-inspection-r2",
        "cleanup-archived-copies-r1", "exact-ci-watch-r1",
        "derive-completed-jobs-r1", "derive-completed-posix-r1",
        "derive-terminal-ci-r1", "seal-ci-originals-r1",
        "ci-evidence-verify-r1", "engineering-docs-freeze-r1",
    ):
        command(label)
    command("evidence-verify-working-r1", exit_code=1)
    assert b"KeyError: 'integration/package-r1.json'" in files["evidence-verify-working-r1.log"]
    readback = doc("git-readback-r1.json")
    assert readback["expected_source"] == SOURCE
    assert readback["all_commands_passed"] and readback["three_shas_match"]
    assert readback["working_tree_clean"]
    cleanup = doc("cleanup-result-r1.json")
    assert cleanup["source"] == SOURCE and cleanup["removed_files"] == 1968
    assert cleanup["removed_bytes"] == 106004384 and cleanup["all_roots_absent"]
    assert cleanup["normal_package_unchanged"] and cleanup["shared_target_identity_unchanged"]
    assert not cleanup["shared_target_cleaned"] and not cleanup["other_worktrees_or_containers_touched"]
    plan = doc("cleanup-plan-r1.json")
    assert len(plan["entries"]) == cleanup["removed_files"]
    assert sum(r["bytes"] for r in plan["entries"]) == cleanup["removed_bytes"]
    for row in plan["retained"]:
        data = files[row["retained_as"]]
        assert len(data) == row["bytes"] and sha(data) == row["sha256"]
    package = doc("normal-source-package-r2.json")
    assert package["source"] == package["manifest"]["build"]["commit"] == SOURCE
    assert package["all_five_application_files_equal_precommit"] and len(package["files"]) == 6
    assert not package["native_desktop_acceptance"] and not package["installed"]
    assert package["files"] == cleanup["normal_package_files"]
    for label, expected in (
        ("windows-artifact-list-r1", 0),
        ("evidence-verify-git-r1", 0),
    ):
        command(label, expected)
    lifecycle = doc("agent-lifecycle-final.json")
    assert lifecycle["all_owned_agents_closed"]
    assert lifecycle["additional_windows_api_reviewer"]["closed"]
    report = doc("windows-api-independent-review/parent-binding.json")
    assert report["parent_delivery_decision"] == "APPROVE_SCOPED_API_EVIDENCE"
    assert all(n == 0 for n in report["parent_open_findings"].values())
    assert sha(files["windows-api-independent-review/report.md"]) == report["parent_report_sha256"]
    assert sha(files["windows-api-independent-review/receipt.json"]) == report["original_receipt_sha256"]
    independent = doc("windows-api-independent-review/receipt.json")
    assert independent["source"] == SOURCE and independent["conclusion"] == "APPROVE"
    assert all(n == 0 for n in independent["severity_counts"].values())
    checks = independent["independent_recomputation"]
    assert checks["r2_assertions"] == checks["r2_passed"] == 102
    assert checks["r2_failed"] == checks["windows_API_executions"] == checks["tests_executed"] == 0
    for name in ("recompute-attempt-r1.json", "recompute-failure-r1.stderr",
                 "read-path-error-r1.stderr"):
        assert files["windows-api-independent-review/" + name]
    freeze = doc("engineering-docs-inputs-r1.json")
    assert freeze["unchanged_from_final_r3"] and freeze["count"] == len(freeze["inputs"]) == 758
    aggregate = sha("".join(f"{r['path']}\0{r['sha256']}\n" for r in freeze["inputs"]).encode())
    assert aggregate == freeze["aggregate_sha256"]
    assert aggregate == "6759f672ae1efb83f6a400145109ee9bdc75ed192e2b7647813a252a1eded210"
    native = doc("native-availability-r3.json")
    assert native["gui_actions"] == native["screenshots"] == 0
    assert not native["native_apps_available"] and not native["native_desktop_acceptance"]
    print(json.dumps({
        "source": SOURCE, "members": len(files), "tests_replayed": 0,
        "git_readback_equal": True, "removed_files": 1968,
        "removed_bytes": 106004384, "prebuilt_cleanup_in_main_archive": 886506992,
        "all_owned_agents_closed": True, "native_desktop_acceptance": False,
        "mismatches": [],
    }))


if __name__ == "__main__":
    main()
