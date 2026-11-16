"""Recompute recorded delivery evidence without rebuilding or replaying tests."""
import hashlib
import json
from pathlib import Path
import tarfile

BASE = Path(__file__).resolve().parent
SOURCE = "16703293012d798f885e1590f02c348c47e81a10"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def payloads(base):
    manifest = json.loads((base / "manifest.json").read_text())
    raw = (base / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert digest(raw) == manifest["archive_sha256"]
    with tarfile.open(base / "originals.tar.gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() for member in members)
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and digest(data) == row["sha256"]
    return manifest, files


def main():
    _, files = payloads(BASE)
    master_manifest, master = payloads(BASE.parent)

    def document(name):
        return json.loads(files[name])

    commands = (
        "feature-commit-r1", "feature-push-r1", "feature-git-binding-r1",
        "package-r1", "package-inspect-r1", "ci-initial-r1", "ci-current-r1",
        "rust-environment-r1", "sealed-verify-r2",
    )
    for label in commands:
        receipt = document(label + ".json")
        assert receipt["controller_exit"] == receipt["observed_exit"] == 0
        assert receipt["terminal"]["direct_reaped"]
        assert receipt["terminal"]["active_processes"] == 0
        assert receipt["terminal"]["pipes_drained"]
        assert not receipt["terminal"]["errors"]
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"]
        assert digest(raw) == receipt["log_sha256"]

    readback = document("feature-readback-r1.json")
    assert readback["local"] == readback["tracking"] == readback["remote"] == SOURCE
    assert readback["equal"] and not readback["status_porcelain"]
    binding = document("feature-git-binding-r1.log")
    assert binding["engineering_inputs"] == 729 and not binding["mismatches"]
    freeze = json.loads(master["inputs-freeze-r3.json"])
    assert document("commit-inputs-before-r1.json")["inputs"] == freeze["inputs"]
    assert binding["aggregate_sha256"] == freeze["aggregate_sha256"]

    package = document("normal-package-r1.json")
    manifest = document("package-manifest-r1.json")
    precommit = json.loads(master["normal-package-precommit-r1.json"])
    assert package["source_commit"] == manifest["build"]["commit"] == SOURCE
    assert package["aggregate_sha256"] == freeze["aggregate_sha256"]
    assert package["manifest"] == manifest
    assert manifest["files"] == precommit["manifest"]["files"]
    assert package["all_five_application_files_equal_precommit"]
    assert package["source_manifest_changed_only"]
    rows = {row["path"]: row for row in package["files"]}
    assert len(rows) == 6
    for path, sha256 in manifest["files"].items():
        assert rows[path]["sha256"] == sha256
    manifest_row = rows["package-manifest.json"]
    assert manifest_row["bytes"] == len(files["package-manifest-r1.json"])
    assert manifest_row["sha256"] == digest(files["package-manifest-r1.json"])
    assert not any(package[key] for key in (
        "native_desktop_acceptance", "launched", "installed", "signed", "notarized",
    ))

    cleanup = document("cleanup-r1.json")
    assert cleanup["source_commit"] == SOURCE
    assert cleanup["removed_files"] == len(cleanup["entries"]) == 235
    assert cleanup["removed_bytes"] == sum(row["bytes"] for row in cleanup["entries"])
    assert cleanup["removed_bytes"] == 70_913_865
    assert cleanup["target_before"] == cleanup["target_after"]
    assert cleanup["normal_package_before"] == cleanup["normal_package_after"]
    assert cleanup["normal_package_after"] == {
        path: row["sha256"] for path, row in rows.items()
    }
    assert not cleanup["other_task_resources_modified"]
    archives = cleanup["archives_verified_from_git"]
    assert [row["members_verified"] for row in archives] == [199, 27]
    assert archives[0]["archive_sha256"] == master_manifest["archive_sha256"]
    assert len({row["path"] for row in cleanup["entries"]}) == 235

    rust = document("rust-environment-r1.log")
    assert rust["rustc"].startswith("rustc 1.100.0 ")
    assert rust["cargo"].startswith("cargo 1.100.0 ")
    assert not rust["rust_version_constraint"] and not rust["toolchain_pin"]
    initial = document("ci-initial-r1.log")
    current = document("ci-current-r1.log")
    assert len(initial) == 1 and initial[0]["headSha"] == SOURCE
    assert initial[0]["databaseId"] == current["databaseId"] == 43945988599
    assert current["headSha"] == SOURCE
    assert current["status"] in ("queued", "in_progress")
    assert not current["conclusion"]
    print(json.dumps({
        "source": SOURCE, "members": len(files), "engineering_inputs": 729,
        "package_application_files_equal": 5,
        "cleaned_files": 235, "cleaned_bytes": 70_913_865,
        "ci_snapshot_only": True, "tests_replayed": 0, "mismatches": [],
        "native_desktop_acceptance": False,
    }))


if __name__ == "__main__":
    main()
