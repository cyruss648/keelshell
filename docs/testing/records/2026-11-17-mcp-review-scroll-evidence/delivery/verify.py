"""Verify delivery receipts without executing or extracting archived members."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[4]


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--commit")
options = parser.parse_args()
manifest = json.loads((BASE / "manifest.json").read_text())
main = json.loads((BASE.parent / "manifest.json").read_text())
assert manifest["main_archive_sha256"] == main["sha256"]
raw = (BASE / "originals.tar.gz").read_bytes()
assert len(raw) == manifest["bytes"] and sha(raw) == manifest["sha256"]
rows = {row["member"]: row for row in manifest["files"]}
saved = {}
with tarfile.open(BASE / "originals.tar.gz", "r:gz") as archive:
    members = archive.getmembers()
    assert len(members) == len(rows) == manifest["members"]
    assert {member.name for member in members} == rows.keys()
    for member in members:
        assert member.isfile() and not Path(member.name).is_absolute() and ".." not in Path(member.name).parts
        content = archive.extractfile(member).read()
        row = rows[member.name]
        assert len(content) == row["bytes"] and sha(content) == row["sha256"]
        saved[member.name] = content
assert sum(row["bytes"] for row in rows.values()) == manifest["logical_bytes"]
readback = json.loads(saved["feature-push-readback.json"])
assert readback["all_equal"] and readback["working_tree_clean"]
assert readback["head"] == readback["tracking"] == readback["remote"] == manifest["feature_commit"]
cleanup = json.loads(saved["sealed-source-cleanup.json"])
assert cleanup["commit"] == manifest["feature_commit"] and cleanup["archive_sha256"] == main["sha256"]
assert cleanup["protected_equal"] and cleanup["protected_before"] == cleanup["protected_after"]
assert sum(row["removed_files"] for row in cleanup["removed"]) == 460
assert len(cleanup["retained_normal_package_files"]) == 6
assert sum(row["source_directory_removed"] for row in cleanup["removed"]) == 10
binding = json.loads(saved["feature-commit-binding-r1/command.log"])
assert binding["exact_git_blobs_match"] and binding["engineering_inputs"] == 777
assert binding["commit"] == manifest["feature_commit"]
ci = json.loads(saved["feature-ci-summary-r1.json"])
assert len(ci) == 3
assert all(row["head_sha"] == manifest["feature_commit"] and row["status"] == "in_progress" and row["conclusion"] is None for row in ci)
if options.commit:
    assert re.fullmatch("[0-9a-f]{40}", options.commit)
    for path in sorted(BASE.iterdir()):
        if path.is_file():
            blob = subprocess.check_output(["git", "show", f"{options.commit}:{path.relative_to(ROOT)}"], cwd=ROOT, timeout=15)
            assert path.read_bytes() == blob
print(json.dumps({
    "archive_members": len(rows), "archive_bytes": len(raw), "archive_sha256": sha(raw),
    "feature_commit": manifest["feature_commit"],
    "removed_files": 460, "removed_logical_bytes": sum(row["logical_bytes"] for row in cleanup["removed"]),
    "removed_source_directories": 10, "protected_equal": True,
    "engineering_git_blobs": 777,
    "ci_snapshot": "three in_progress at recorded GET; not terminal success",
    "commit": options.commit, "exact_delivery_git_blobs_match": True if options.commit else None,
    "boundary": "Delivery evidence only; main native wheel P2 and unfinished platform/product/release acceptance remain open.",
}, indent=2))
