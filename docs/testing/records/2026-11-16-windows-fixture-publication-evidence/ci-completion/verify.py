"""Verify exact-source CI originals and derived facts without replaying tests."""

from collections import Counter
import hashlib
import importlib.util
import io
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tarfile
import tempfile

BASE = Path(__file__).resolve().parent
SOURCE = "ee3ad7320e90f40c9d777c500543cc6e909d0484"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def settled(receipt):
    terminal = receipt["terminal"]
    assert terminal["direct_reaped"] and terminal["active_processes"] == 0
    assert terminal["pipes_drained"] and not terminal["errors"]


def main():
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    assert len(packed) == manifest["archive_bytes"]
    assert sha(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert len(members) == manifest["count"]
        assert len({row.name for row in members}) == len(members)
        assert all(
            row.isfile() and not row.issparse()
            and not PurePosixPath(row.name).is_absolute()
            and ".." not in PurePosixPath(row.name).parts
            for row in members
        )
        assert sorted(row.name for row in members) == sorted(
            row["path"] for row in manifest["entries"]
        )
        for row in manifest["entries"]:
            raw = archive.extractfile(row["path"]).read()
            assert len(raw) == row["bytes"] and sha(raw) == row["sha256"], row["path"]
            files[row["path"]] = raw
    assert files["tools/verify.py"] == Path(__file__).read_bytes()
    final = json.loads(files["ci/source-ci-snapshot-r7.log"])
    assert final["headSha"] == manifest["source"] == SOURCE
    assert final["databaseId"] == 43880245383
    assert final["status"] == "completed" and final["conclusion"] == "success"
    jobs = {key: next(job for job in final["jobs"] if key in job["name"])
            for key in ("macos", "windows", "ubuntu")}
    assert [jobs[key]["databaseId"] for key in ("macos", "windows", "ubuntu")] == [
        133343901229, 133343901451, 133343901650
    ]
    for key, job in jobs.items():
        assert job["status"] == "completed" and job["conclusion"] == "success"
        assert next(row for row in job["steps"] if row["name"] == "Rust quality gate")[
            "conclusion"
        ] == "success"
        if key != "windows":
            assert next(row for row in job["steps"]
                        if row["name"] == "OpenSSH interoperability (macOS and Linux)")[
                "conclusion"
            ] == "success"
    for label in (
        *(f"source-ci-snapshot-r{number}" for number in range(1, 8)),
        "source-ci-watch-r1", "source-macos-job-r2",
        "source-windows-job-r1", "source-ubuntu-job-r1",
    ):
        receipt = json.loads(files[f"ci/{label}.json"])
        raw = files[f"ci/{label}.log"]
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0, label
        assert not receipt["timed_out"] and not receipt["interrupted"]
        settled(receipt)
        assert len(raw) == receipt["log_bytes"] and sha(raw) == receipt["log_sha256"]
    for label, message in (
        ("source-macos-job-r1", b"--allow-escape-sequences"),
        ("source-windows-active-log-r1", b"BlobNotFound"),
    ):
        receipt = json.loads(files[f"ci/{label}.json"])
        raw = files[f"ci/{label}.log"]
        assert receipt["observed_exit"] == receipt["controller_exit"] == 1
        settled(receipt)
        assert sha(raw) == receipt["log_sha256"] and message in raw
    expected = {
        "macos": (3225, 48, 1341, 2),
        "windows": (3212, 46, 1330, 34),
        "ubuntu": (3218, 49, 1342, 2),
    }
    reports = {}
    # Only the two known parser files and three completed logs are materialized
    # into an isolated temporary directory. No test, target or app is invoked.
    with tempfile.TemporaryDirectory(prefix="keelshell-ci-record-") as scratch:
        directory = Path(scratch)
        for name in ("summarize_ci_r3.py", "summarize_source_ci.py"):
            (directory / name).write_bytes(files[f"tools/{name}"])
        spec = importlib.util.spec_from_file_location(
            "completed_source_ci", directory / "summarize_source_ci.py"
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for key, label in (
            ("macos", "source-macos-job-r2"),
            ("windows", "source-windows-job-r1"),
            ("ubuntu", "source-ubuntu-job-r1"),
        ):
            log = directory / f"{label}.log"
            log.write_bytes(files[f"ci/{label}.log"])
            report = module.summarize(log, key)
            assert report == json.loads(files[f"ci/source-{key}-summary-r1.json"])
            passed, ignored, app_passed, skipped = expected[key]
            assert report["ordinary_rust"] == {
                "passed": passed, "failed": 0, "ignored": ignored
            }
            assert report["rustdoc"] == {"passed": 14, "failed": 0, "ignored": 0}
            assert report["python"]["run"] == 293
            assert report["python"]["summary"] == f"OK (skipped={skipped})"
            assert len(report["workspace"]["suites"]) == 92
            assert report["app"]["declared_tests"] == len(report["app"]["results"]) == app_passed + 2
            assert report["app"]["summary"]["passed"] == app_passed
            assert report["app"]["summary"]["failed"] == 0
            assert report["app"]["summary"]["ignored"] == 2
            reports[key] = report
    windows = reports["windows"]
    assert windows["archive_preflight"]["counts"] == {"ok": 41}
    assert windows["session_preflight"]["counts"] == {"ok": 329, "ignored": 3}
    assert len(windows["session_preflight"]["inventory"]) == 332
    local = [(name, value) for name, value in windows["session_preflight"]["results"]
             if name.startswith("zmodem::local::")]
    assert len(local) == 45 and Counter(value for _, value in local) == {"ok": 43, "ignored": 2}
    assert len(windows["tree_controls"]) == 1
    controls = windows["tree_controls"][0]
    assert len(controls) == 7
    for control in controls:
        assert control["identity_bound_before_ack"]
        assert control["state_identity"] == "retained_native_handles"
        assert control["job_membership"] == {"parent": True, "child": True}
        assert control["post_states"] == {"parent": "TERMINAL", "child": "TERMINAL"}
        assert not control["receipt"]["errors"]
        settled(control["receipt"])
    exit7 = next(row for row in controls if row["requested_parent_exit"] == 7)
    assert exit7["receipt"]["observed_exit"] == exit7["receipt"]["controller_exit"] == 7
    assert exit7["receipt"]["timed_out"]
    assert sum(row["partial_publications_rejected"] == ["empty", "partial"] for row in controls) == 1
    assert len(windows["job_barrier"]) == 1
    barrier = windows["job_barrier"][0]
    assert barrier["evidence_kind"] == "ACTUAL_NATIVE_WINDOWS"
    assert barrier["barrier"]["before_original_handle_states"] == {"parent": "LIVE", "child": "LIVE"}
    assert barrier["barrier"]["after_original_handle_states"] == {"parent": "LIVE", "child": "LIVE"}
    assert barrier["late_createprocess"]["creation_rejected"]
    assert barrier["late_createprocess"]["nested_child_membership"]
    assert not barrier["late_marker_exists"]
    assert barrier["post_original_handle_states"] == {"parent": "TERMINAL", "child": "TERMINAL"}
    assert barrier["receipt"]["observed_exit"] == barrier["receipt"]["controller_exit"] == 0
    assert not barrier["receipt"]["errors"]
    settled(barrier["receipt"])
    subprocess.run(
        [sys.executable, str(BASE.parent / "verify.py"), "--source", SOURCE],
        check=True, timeout=180,
    )
    print(json.dumps({
        "source": SOURCE, "run": final["databaseId"], "members": len(files),
        "three_platform_quality": "SUCCESS", "macos_ubuntu_openssh": "SUCCESS",
        "windows_native_tree_controls": 7, "windows_native_job_barrier": "SETTLED_EXIT_0",
        "mismatches": 0, "tests_replayed": 0, "native_desktop_acceptance": False,
        "ci_app_inventory": "declared count and unique results; no separate --list",
        "python_inventory": "complete discovery summary; no per-case verbose log",
    }))


if __name__ == "__main__":
    main()
