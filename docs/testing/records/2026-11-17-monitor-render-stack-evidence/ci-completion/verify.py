"""Verify sealed exact-source CI bytes with local Git only and no test replay."""

import io
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
SOURCE = "bbd4eaccfa8bba2feb487ff43e0423567428bc82"
RUN = 43955379234
EXPECTED = {
    133608443190: (3312, 56, 1364, 1366, 34, 53, 4, 11, 3),
    133608443592: (3325, 58, 1375, 1377, 2, 57, 0, 10, 4),
    133608443616: (3318, 59, 1376, 1378, 2, 57, 0, 11, 3),
}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    # Missing promisor objects must fail locally instead of causing a lazy fetch.
    os.environ.update(
        GIT_NO_LAZY_FETCH="1", GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0",
    )
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert digest(raw) == manifest["archive_sha256"]
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() for member in members)
        assert all(
            not PurePosixPath(member.name).is_absolute()
            and ".." not in PurePosixPath(member.name).parts
            for member in members
        )
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"] == 37
    entries = manifest["entries"]
    assert len(entries) == len({row["path"] for row in entries}) == 37
    assert set(files) == {row["path"] for row in entries}
    for row in entries:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and digest(data) == row["sha256"], row["path"]
    assert (BASE / "verify.py").read_bytes() == files["sealed-verifier-r1.py"]

    def document(name):
        return json.loads(files[name])

    original = document("original-inputs-r1.json")
    assert original["original_count"] == len(original["entries"]) == 29
    assert len({row["path"] for row in original["entries"]}) == 29
    for row in original["entries"]:
        if row["archive_member"]:
            data = files[row["path"]]
            assert len(data) == row["bytes"] and digest(data) == row["sha256"], row["path"]
        else:
            assert row["path"] == "__pycache__/summarize_tests.cpython-315.pyc"
            assert row["path"] not in files
    assert sum(row["archive_member"] for row in original["entries"]) == 28

    namespace = {"__name__": "sealed_offline_derivation"}
    exec(compile(files["derive_terminal_offline.py"], "sealed_offline_derivation", "exec"),
         namespace)
    summary = namespace["derive"](files)
    assert summary == document("terminal-summary-offline-r1.json")
    assert summary["source_commit"] == SOURCE and summary["run_id"] == RUN
    assert summary["status"] == "completed" and summary["conclusion"] == "success"
    assert summary["steps_total"] == 42
    for job in summary["jobs"]:
        passed, ignored, app_passed, app_declared, skipped, package_passed, package_skipped, good_steps, skipped_steps = EXPECTED[job["job_id"]]
        assert job["ordinary_rust"] == {"passed": passed, "failed": 0, "ignored": ignored}
        assert job["ordinary_declared"] == passed + ignored
        assert job["ordinary_suites"] == 92 and job["rustdoc_subruns"] == 6
        assert job["app_declared"] == app_declared
        assert [job["app"][key] for key in ("passed", "failed", "ignored")] == [app_passed, 0, 2]
        assert job["rustdoc"] == {"passed": 14, "failed": 0, "ignored": 0}
        assert [job["python_discovery"][key] for key in ("run", "passed", "failed", "skipped")] == [
            293, 293 - skipped, 0, skipped,
        ]
        assert [job["python_packaging"][key] for key in ("run", "passed", "failed", "skipped")] == [
            57, package_passed, 0, package_skipped,
        ]
        for helper in job["python_workspace_helpers"]:
            assert [helper[key] for key in ("run", "passed", "failed", "skipped")] == [30, 30, 0, 0]
        assert job["step_counts"] == {"success": good_steps, "skipped": skipped_steps}
        assert len(job["original_failure_results"]) == 2
        assert set(job["original_failure_results"].values()) == {"ok"}
    namespace["check_receipt"](files, "derive-terminal-offline-r1", 0)
    before = document("scope-before-r1.json")
    assert before["head"] == "4209e272260aff6374e6216423ce8e0157bea2f4"
    assert before["ci_source"] == SOURCE
    root = Path(subprocess.check_output(
        ["git", "--no-optional-locks", "rev-parse", "--show-toplevel"],
        cwd=BASE, text=True, timeout=10,
    ).strip())
    binding = namespace["bind_git"](root, document("inputs-freeze-r2.json"))
    assert binding == document("exact-source-binding-offline-r1.json")
    assert binding["source"] == SOURCE and binding["engineering_inputs"] == 731
    print(json.dumps({
        "source": SOURCE, "run": RUN, "members": len(files),
        "archive_bytes": manifest["archive_bytes"], "archive_sha256": manifest["archive_sha256"],
        "engineering_inputs": 731, "aggregate_sha256": binding["aggregate_sha256"],
        "all_three_platforms": "completed/success",
        "original_regression_results": "two names times three platforms: six ok",
        "steps": {"total": 42, "success": 32, "skipped": 10},
        "original_collector_failures": "r1=1 and r2=1 preserved",
        "tests_replayed": 0, "network_requests": 0, "cargo_commands": 0,
        "target_accesses": 0, "current_working_engineering_files_read": 0,
        "current_dirty_windows_implementation_certified": False,
        "native_desktop_acceptance": False, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
