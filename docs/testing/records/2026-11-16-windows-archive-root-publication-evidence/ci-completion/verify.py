"""Verify the later, completed CI evidence separately from the original seal."""

import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

BASE = Path(__file__).resolve().parent


def main():
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    assert len(packed) == manifest["archive_bytes"]
    assert hashlib.sha256(packed).hexdigest() == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(row.isfile() and not row.issparse() for row in members)
        assert sorted(row.name for row in members) == sorted(row["path"] for row in manifest["entries"])
        for row in manifest["entries"]:
            data = archive.extractfile(row["path"]).read()
            assert len(data) == row["bytes"]
            assert hashlib.sha256(data).hexdigest() == row["sha256"], row["path"]
            files[row["path"]] = data
    assert files["tools/verify.py"] == Path(__file__).read_bytes()
    report = json.loads(files["batch/source-ci-summary-final-r1.json"])
    assert report["source"] == manifest["source"] == "f094de6ad2789d168f5a24bacb316c98b83608ac"
    assert report["run"] == 43859165104 and report["status"] == "completed"
    assert report["conclusion"] == "failure"
    for job in report["jobs"]:
        label = {
            "windows": "source-windows-job-r2",
            "ubuntu": "source-ubuntu-job-r2",
            "macos": "source-macos-job-r1",
        }[job["platform"]]
        data = files[f"batch/{label}.log"]
        receipt = json.loads(files[f"batch/{label}.json"])
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0
        assert receipt["terminal"]["direct_reaped"] and not receipt["timed_out"]
        assert len(data) == receipt["log_bytes"] == job["raw_bytes"]
        assert hashlib.sha256(data).hexdigest() == receipt["log_sha256"] == job["raw_sha256"]
        assert all(outcomes == ["ok"] for outcomes in job["prior_app_cases"].values())
    windows = next(row for row in report["jobs"] if row["platform"] == "windows")
    assert windows["archive_preflight"]["counts"] == {"ok": 29, "FAILED": 0, "ignored": 1}
    assert len(windows["archive_preflight"]["compiled"]) == 30
    assert windows["app"]["passed"] == 1330 and windows["app"]["failed"] == 0
    assert windows["failed"] == ["reencoding_discards_dangerous_incoming_permissions_and_ownership"]
    subprocess.run(
        [sys.executable, str(BASE.parent / "verify.py"), "--source", manifest["source"]],
        check=True,
    )
    print(json.dumps({
        "source": manifest["source"], "run": report["run"],
        "members": len(files), "evidence_mismatches": 0,
        "windows_archive": "29 passed / 1 ignored",
        "windows_app": "1330 passed / 2 ignored",
        "windows_complete_gate": "FAILURE_AFTER_APP", "tests_replayed": 0,
    }))


if __name__ == "__main__":
    main()
