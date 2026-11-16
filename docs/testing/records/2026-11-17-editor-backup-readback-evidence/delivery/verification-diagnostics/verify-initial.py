"""Verify delivery and the Windows-test-only follow-up without replaying tests."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[4]
SOURCE = "f810f674c57b2c25dac5b987d77bf38656e3af56"
MAIN = "docs/testing/records/2026-11-17-editor-backup-readback-evidence"
CHANGED = "crates/keelshell-session/src/sftp/archive_transfer/local_windows/tests.rs"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def unpack(raw, manifest):
    assert len(raw) == manifest["archive_bytes"] and digest(raw) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        for row in archive:
            name = PurePosixPath(row.name)
            assert row.isfile() and not name.is_absolute() and ".." not in name.parts
            assert row.name not in files
            files[row.name] = archive.extractfile(row).read()
    assert len(files) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        assert len(files[row["path"]]) == row["bytes"]
        assert digest(files[row["path"]]) == row["sha256"]
    return files


def git_blob(source, path):
    return subprocess.check_output(["git", "show", f"{source}:{path}"], cwd=ROOT, timeout=30)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Read this exact commit rather than the working tree")
    args = parser.parse_args()
    if args.source:
        assert re.fullmatch(r"[0-9a-f]{40}", args.source)

    def read(path):
        return git_blob(args.source, path.relative_to(ROOT).as_posix()) if args.source else path.read_bytes()

    files = unpack(read(BASE / "originals.tar.gz"), json.loads(read(BASE / "manifest.json")))
    original = unpack(git_blob(SOURCE, MAIN + "/originals.tar.gz"),
                      json.loads(git_blob(SOURCE, MAIN + "/manifest.json")))

    def doc(name):
        return json.loads(files[name])

    def command(label, code=0):
        receipt = doc(label + ".json")
        assert receipt["observed_exit"] == receipt["controller_exit"] == code
        assert not receipt["timed_out"] and not receipt["interrupted"]
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0
        assert terminal["pipes_drained"] and not terminal["errors"]
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"] and digest(raw) == receipt["log_sha256"]
        return raw

    freeze = doc("inputs-freeze-r5.json")
    old = json.loads(original["inputs-freeze-r4.json"])
    assert freeze["count"] == len(freeze["inputs"]) == old["count"] == 727
    assert freeze["inputs"] == doc("windows-inputs-after-r1.json")["inputs"]
    assert [row["path"] for row, prior in zip(freeze["inputs"], old["inputs"])
            if row != prior] == [CHANGED]
    assert digest("".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode()) == freeze["aggregate_sha256"]
    names = sorted({
        name.decode() for name in subprocess.check_output(
            ["git", "ls-tree", "-rz", "--name-only", args.source] if args.source else
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
            cwd=ROOT, timeout=30,
        ).split(b"\0") if name and not name.startswith((b"docs/", b".hallmark/"))
    })
    assert names == [row["path"] for row in freeze["inputs"]]
    for row in freeze["inputs"]:
        raw = read(ROOT / row["path"])
        assert len(raw) == row["bytes"] and digest(raw) == row["sha256"], row["path"]
    old_source = git_blob(SOURCE, CHANGED)
    anchor = b'        SessionError::MutationUncertain => "MutationUncertain",\n'
    addition = b'        SessionError::EditorBackupUnverified => "EditorBackupUnverified",\n'
    assert old_source.count(anchor) == 1
    assert read(ROOT / CHANGED) == old_source.replace(anchor, anchor + addition)
    review = doc("windows-review-r1/receipt.json")
    assert review["decision"] == "APPROVE"
    assert review["severity_counts"] == dict.fromkeys(("P0", "P1", "P2", "P3"), 0)
    assert not review["findings"] and not review["unfinished_static_points"]
    assert review["source_binding"]["head"] == SOURCE
    assert review["source_binding"]["exact_one_line_change_verified"]
    assert review["source_binding"]["working_file_sha256"] == digest(read(ROOT / CHANGED))
    assert review["integrity_readback"]["head_unchanged"]
    assert review["execution_counts"]["independent_compilation_executions"] == 0
    assert review["execution_counts"]["independent_test_executions"] == 0
    assert review["execution_counts"]["target_writes"] == 0
    assert digest(files["windows-review-r1/report.md"]) == review["report_sha256"]
    assert doc("windows-review-closed-r1.json")["closed"]

    namespace = {"__name__": "sealed_windows_followup", "__file__": str(BASE / "parser.py")}
    exec(compile(files["summarize_windows_fix.py"], "sealed_windows_followup", "exec"), namespace)
    summary = namespace["derive"](
        command("windows-fix-gate-r1"), original["session-inventory-r4.log"],
        json.loads(original["author-handoff.json"])["new_tests"],
    )
    saved = doc("windows-fix-summary-r1.json")
    assert summary == {key: value for key, value in saved.items() if key != "binary"}
    command("windows-fix-derive-r1", 1)
    command("windows-fix-derive-r2")
    command("current-ci-windows-log-r1", 1)
    failed = command("current-ci-windows-log-r2")
    assert b"error[E0004]" in failed and b"EditorBackupUnverified" in failed
    command("feature-commit-r1")
    command("feature-push-r1")
    delivery = json.loads(command("feature-readback-r1"))
    assert delivery["local"] == delivery["tracking"] == delivery["remote"] == SOURCE
    assert delivery["equal"] and delivery["working_tree_clean"] and not delivery["forced_push"]
    binding = json.loads(command("source-binding-r1"))
    assert binding["source"] == SOURCE and binding["source_inputs"] == 727
    package = doc("normal-package-committed-r1.json")
    assert package["source"] == SOURCE and package["all_application_files_equal_precommit"]
    assert package["only_manifest_build_provenance_changed"]
    assert not package["installed"] and not package["native_desktop_acceptance"]
    command("package-r1")
    command("package-final-inspect-r1")
    command("cleanup-command-r1", 1)
    command("cleanup-command-r2")
    cleanup = doc("cleanup-r2.json")
    assert cleanup["prior_r1_verified_files_now_absent"] == 242
    assert cleanup["prior_r1_removed_bytes"] == 70086612
    assert not cleanup["r1_before_after_target_comparison_available"]
    assert cleanup["current_normal_package_six_files_equal_committed_provenance_receipt"]
    print(json.dumps({
        "source": args.source or "working-tree", "delivered_source": SOURCE,
        "members": len(files), "engineering_inputs": 727,
        "aggregate_sha256": freeze["aggregate_sha256"],
        "only_changed_input": CHANGED,
        "session": summary["session"], "session_inventory": 343,
        "new_backup_tests_passed": 30, "tests_replayed": 0,
        "independent_static_review": "APPROVE",
        "windows_fix_native_compilation_or_execution": False,
        "native_desktop_acceptance": False, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
