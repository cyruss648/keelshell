"""Verify delivery receipts without replaying tests or requiring the local app."""

import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

BASE = Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    assert len(packed) == manifest["archive_bytes"]
    assert sha(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(row.isfile() and not row.issparse() for row in members)
        assert len(members) == manifest["count"]
        assert sorted(row.name for row in members) == sorted(
            row["path"] for row in manifest["entries"]
        )
        for row in manifest["entries"]:
            raw = archive.extractfile(row["path"]).read()
            assert len(raw) == row["bytes"] and sha(raw) == row["sha256"], row["path"]
            files[row["path"]] = raw
    assert files["tools/verify.py"] == Path(__file__).read_bytes()
    source = manifest["source"]
    assert source == "ee3ad7320e90f40c9d777c500543cc6e909d0484"
    delivery = json.loads(files["batch/source-delivery-r1.json"])
    assert delivery["source"] == source and delivery["three_way_equal"]
    assert delivery["clean"]
    assert [row["exit"] for row in delivery["results"]] == [0, 0, 0, 0]
    assert all(
        row["stdout"].split()[0] == source
        for row in delivery["results"][:3]
    )
    for label in (
        "commit-bind-r1", "source-push-r1", "prior-ci-evidence-verify-r1",
        "normal-build-r1", "package-r1", "native-package-inspect-r1",
        "evidence-verify-r1", "source-ci-runs-r1", "source-ci-snapshot-r3",
    ):
        receipt = json.loads(files[f"batch/{label}.json"])
        raw = files[f"batch/{label}.log"]
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0, label
        assert receipt["terminal"]["direct_reaped"] and not receipt["timed_out"], label
        assert len(raw) == receipt["log_bytes"] and sha(raw) == receipt["log_sha256"], label
    snapshot = json.loads(files["batch/source-ci-snapshot-r3.log"])
    assert snapshot["headSha"] == source
    assert snapshot["databaseId"] == 43880245383
    assert snapshot["status"] == "in_progress"
    windows = next(job for job in snapshot["jobs"] if "windows" in job["name"])
    assert windows["databaseId"] == 133343901451
    for name in (
        "Windows native filesystem preflight",
        "Windows archive codec preflight",
        "Windows session backend preflight",
    ):
        assert next(row for row in windows["steps"] if row["name"] == name)[
            "conclusion"
        ] == "success"
    unavailable = files["batch/source-windows-active-log-r1.log"]
    assert b"404" in unavailable and b"BlobNotFound" in unavailable
    package = json.loads(files["batch/normal-package-binding-r1.json"])
    assert package["source"] == package["build"]["commit"] == source
    assert package["engineering_sha256"] == manifest["engineering_sha256"]
    assert package["native_structure_inspection"] == "PASS"
    assert package["native_launch"] == package["GUI_actions"] == package["screenshots"] == 0
    assert not package["installed"] and not package["signed"]
    package_manifest = files["package/package-manifest.json"]
    listed_manifest = next(
        row for row in package["files"] if row["path"] == "package-manifest.json"
    )
    assert len(package_manifest) == listed_manifest["bytes"]
    assert sha(package_manifest) == listed_manifest["sha256"]
    cleanup = json.loads(files["batch/owned-cleanup-r1.json"])
    assert cleanup["archive_git_bytes_equal"]
    assert cleanup["deleted_files"] == 4 and cleanup["deleted_bytes"] == 66095
    assert not cleanup["shared_target_touched"] and not cleanup["other_resources_touched"]
    subprocess.run(
        [sys.executable, str(BASE.parent / "verify.py"), "--source", source],
        check=True, timeout=180,
    )
    print(json.dumps({
        "source": source, "members": len(files), "mismatches": 0,
        "engineering_inputs": 719, "delivery_three_way_equal": True,
        "ci_at_seal": "IN_PROGRESS", "run": snapshot["databaseId"],
        "package": "STRUCTURE_ONLY_NOT_NATIVE_ACCEPTANCE", "tests_replayed": 0,
    }))


if __name__ == "__main__":
    main()
