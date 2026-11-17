"""Verify the pushed documentation and ownership-aware cleanup receipts offline."""
import hashlib
import json
from pathlib import Path
import runpy
import subprocess

BASE = Path(__file__).resolve().parent
common = runpy.run_path(str(BASE.parent / "repair/verify.py"))


def main():
    files = common["unpack"](BASE)
    acceptance = json.loads(files["acceptance.json"])
    source = acceptance["documentation_source"]
    assert source == "c088062baacc0c44ccfbc98de2a817fbc2a62869"
    assert acceptance["engineering_source"] == "660e90d6f7c7111853db7b73f4ca2344f633700d"

    def document(name):
        return json.loads(files[name])

    readback = document("finalize/docs-readback-state-r1.json")
    assert readback["expected_source"] == source
    assert readback["all_commands_passed"] and readback["three_shas_match"]
    assert readback["working_tree_clean"]
    assert set(readback["shas"].values()) == {source}
    staged = document("finalize/staged-r1.json")
    assert staged["engineering_inputs"] == 765
    assert staged["source_aggregate_sha256"] == "fb78496b8d0b22bd6fc38da46df994d16c144d1f040f83ff468a98c49ff633af"
    assert staged["all_git_index_blobs_match"] and staged["unstaged_tracked_changes"] == 0
    for prefix in (
        "finalize/verify-staged-r1", "finalize/docs-commit-r1", "finalize/docs-push-r1",
        "finalize/docs-readback-r1", "finalize/cleanup-plan-r1", "finalize/cleanup-apply-r1",
        "supplemental/work/windows-ssh-integration-20261117/ci-completion-seal-r1",
        "supplemental/work/windows-ssh-integration-20261117/ci-completion-verify-r1",
        "supplemental/work/windows-ssh-integration-20261117/ci-completion-verify-r2",
        "supplemental/work/windows-ssh-integration-20261117/ci-completion-verify-r3",
    ):
        receipt = document(prefix + ".json")
        expected = 1 if prefix.endswith("ci-completion-verify-r2") else 0
        assert receipt["observed_exit"] == receipt["controller_exit"] == expected
        assert not receipt["timed_out"] and not receipt["interrupted"]
        assert not receipt["errors"]
        assert receipt["terminal"] == {
            "direct_reaped": True, "active_processes": 0, "pipes_drained": True, "errors": [],
        }
        raw = files[prefix + ".log"]
        assert len(raw) == receipt["log_bytes"]
        assert hashlib.sha256(raw).hexdigest() == receipt["log_sha256"]
    cleanup = document("finalize/cleanup-result-state-r1.json")
    assert cleanup["source"] == source and cleanup["apply"]
    assert cleanup["protected_before"] == cleanup["protected_after"]
    assert {key: cleanup[key] for key in acceptance["first_cleanup"]} == acceptance["first_cleanup"]
    removed = [row for row in cleanup["directories"] if row["removed"]]
    assert len(removed) == cleanup["directories_removed"] == 5
    assert all(not row["unmatched"] for row in removed)
    assert sum(len(row["preserved"]) + len(row["derived_bytecode"]) for row in removed) == cleanup["files_removed"] == 827
    assert sum(item["bytes"] for row in removed
               for key in ("preserved", "derived_bytecode") for item in row[key]) == cleanup["bytes_removed"] == 17535741
    for row in acceptance["supplemental_files"]:
        raw = files["supplemental/" + row["path"]]
        assert len(raw) == row["bytes"] and hashlib.sha256(raw).hexdigest() == row["sha256"]
    assert len(acceptance["supplemental_files"]) == 42
    assert len(acceptance["remaining_owned_work_copies"]) == 4
    for directory in (
        BASE.parent, BASE.parent / "repair", BASE.parent / "ci-completion",
        BASE.parent.parent / "2026-11-17-native-toolbar-evidence",
    ):
        for name in ("manifest.json", "originals.tar.gz"):
            relative = str((directory / name).relative_to(Path.cwd()))
            assert subprocess.check_output(
                ["git", "show", source + ":" + relative], timeout=20,
            ) == (directory / name).read_bytes()
    print(json.dumps({
        "source": source, "engineering_source": acceptance["engineering_source"],
        "members": len(files), "supplemental_files": 42,
        "three_pushed_shas_match": True, "first_cleanup": acceptance["first_cleanup"],
        "package_and_target_unchanged": True, "tests_replayed": 0, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
