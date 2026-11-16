"""Verify the sealed delivery receipts and their exact source binding."""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[4]
SOURCE = "bbd4eaccfa8bba2feb487ff43e0423567428bc82"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert digest(raw) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as tar:
        members = tar.getmembers()
        assert len(members) == manifest["count"] == 43
        assert [m.name for m in members] == [r["path"] for r in manifest["entries"]]
        for member, row in zip(members, manifest["entries"]):
            assert member.isfile()
            content = tar.extractfile(member).read()
            assert len(content) == row["bytes"] and digest(content) == row["sha256"]
            files[member.name] = content

    def document(name):
        return json.loads(files[name])

    for name, content in files.items():
        if not name.endswith(".json"):
            continue
        receipt = json.loads(content)
        if "log_sha256" not in receipt:
            continue
        log = name[:-5] + ".log"
        assert len(files[log]) == receipt["log_bytes"]
        assert digest(files[log]) == receipt["log_sha256"]

    readback = document("feature-readback-r2.json")
    assert readback["head"] == readback["tracking"] == readback["remote"] == SOURCE
    assert readback["clean"]
    for label in (
        "feature-commit-r2", "feature-push-r2", "feature-git-binding-r2",
        "package-r2", "package-inspect-r2", "cleanup-r3-run",
    ):
        receipt = document(label + ".json")
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0
        assert receipt["terminal"]["direct_reaped"]
        assert receipt["terminal"]["active_processes"] == 0
        assert receipt["terminal"]["pipes_drained"]
    assert document("cleanup-r2-run.json")["controller_exit"] == 1
    assert document("delivery-inputs-r2.json")["unchanged"]
    binding = json.loads(files["feature-git-binding-r2.log"])
    assert binding["engineering_inputs"] == 731 and binding["mismatches"] == []
    assert binding["tests_replayed"] == 0
    package = document("normal-package-r2.json")
    assert package["manifest"]["build"]["commit"] == SOURCE
    assert package["application_files_equal_to_precommit"]
    assert package["manifest_changed_only_in_build_commit"]
    assert not any(package[k] for k in (
        "launched", "installed", "signed", "notarized", "native_desktop_acceptance",
    ))
    cleanup = document("cleanup-r2.json")
    assert cleanup["source_commit"] == SOURCE
    assert cleanup["removed_files"] == len(cleanup["entries"]) == 266
    assert cleanup["removed_bytes"] == sum(r["bytes"] for r in cleanup["entries"])
    assert cleanup["removed_bytes"] == 74562243
    assert cleanup["target_identity_unchanged"] and cleanup["normal_package_unchanged"]
    ci = json.loads(files["ci-current-r2.log"])
    assert ci["headSha"] == SOURCE and ci["databaseId"] == 43955379234
    assert ci["status"] == "in_progress" and not ci["conclusion"]
    assert {j["databaseId"] for j in ci["jobs"]} == {
        133608443190, 133608443592, 133608443616,
    }
    assert all(j["status"] == "in_progress" and not j["conclusion"] for j in ci["jobs"])
    subprocess.run(
        [sys.executable, str(BASE.parent / "verify.py"), "--source", SOURCE],
        cwd=ROOT, timeout=120, check=True,
    )
    print(json.dumps(dict(
        source=SOURCE, members=43, engineering_inputs=731, mismatches=[],
        source_pushed=True, package_application_files_unchanged=True,
        removed_files=266, removed_bytes=74562243, tests_replayed=0,
        ci_at_seal="in_progress; no terminal result certified",
        native_desktop_acceptance=False,
    )))


if __name__ == "__main__":
    main()
