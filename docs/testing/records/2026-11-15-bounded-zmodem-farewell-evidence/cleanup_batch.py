"""Remove only this completed batch after verifying its persistent evidence."""

from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

batch = Path(__file__).resolve().parent
root = batch.parents[1]
archive = root / "docs/testing/records/2026-11-15-bounded-zmodem-farewell-evidence"
target = root / "work/recovery-batch-integration-20261115/target"


def sha_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


assert batch == root / "work/pty-farewell-20261115"
assert not target.is_relative_to(batch) and target.is_dir()
assert not (archive / "cleanup-final.json").exists()
target_identity = (target.stat().st_dev, target.stat().st_ino)
manifest = json.loads((archive / "archive-manifest.json").read_text())
omitted = json.loads((archive / "omitted-artifacts.json").read_text())
assert manifest["source"] == str(batch.relative_to(root))
verified_source_paths = set()
for relative, item in manifest["files"].items():
    source = batch / relative
    artifact = archive / item["artifact"]
    assert not source.is_symlink() and not artifact.is_symlink()
    assert sha_file(source) == item["source_sha256"], relative
    assert sha_file(artifact) == item["artifact_sha256"], item["artifact"]
    data = artifact.read_bytes()
    if item["artifact"] == relative + ".gz":
        data = gzip.decompress(data)
    assert len(data) == item["source_bytes"]
    assert hashlib.sha256(data).hexdigest() == item["source_sha256"]
    verified_source_paths.add(relative)
for item in omitted["files"]:
    source = batch / item["path"]
    assert not source.is_symlink()
    assert source.stat().st_size == item["bytes"]
    assert sha_file(source) == item["sha256"], item["path"]
    verified_source_paths.add(item["path"])
actual_paths = {
    path.relative_to(batch).as_posix() for path in batch.rglob("*") if path.is_file()
}
assert verified_source_paths == actual_paths
assert json.loads((batch / "logs/full-gate-r5.json").read_text())["exit"] == 0
assert json.loads((batch / "container-absence-r5.json").read_text())[
    "all_owned_containers_absent"
]

processes = []
for line in subprocess.check_output(
    ["ps", "-axo", "pid=,ppid=,args="], text=True, timeout=15
).splitlines():
    parts = line.strip().split(None, 2)
    if len(parts) == 3:
        processes.append((int(parts[0]), int(parts[1]), parts[2]))
parents = {pid: parent for pid, parent, _ in processes}
own_ancestry = set()
pid = os.getpid()
while pid and pid not in own_ancestry:
    own_ancestry.add(pid)
    pid = parents.get(pid, 0)
matches = [
    pid
    for pid, _, args in processes
    if pid not in own_ancestry
    and (
        str(batch) in args
        or "work/pty-farewell-20261115/" in args
    )
]
assert not matches, ("Owned-batch processes still present", matches)

files_before = len(actual_paths)
bytes_before = sum(path.stat().st_size for path in batch.rglob("*") if path.is_file())
manifest_sha256 = sha_file(archive / "archive-manifest.json")
agent_record = json.loads((batch / "agent-closeout-r5.json").read_text())
fixture_record = json.loads((batch / "container-absence-r5.json").read_text())
shutil.rmtree(batch)
assert not batch.exists()
assert target_identity == (target.stat().st_dev, target.stat().st_ino)
record = {
    "recorded_utc": datetime.now(timezone.utc).isoformat(),
    "scope": "Only this completed batch's duplicate evidence, executables, upstream extraction and scratch data.",
    "persistent_manifest": {
        "artifact": "archive-manifest.json",
        "sha256": manifest_sha256,
        "count": manifest["count"],
        "all_artifact_and_decoded_hashes_verified": True,
        "all_source_and_omitted_hashes_verified_before_removal": True,
    },
    "removed": {
        "path": str(batch.relative_to(root)),
        "files_before": files_before,
        "bytes_before": bytes_before,
        "absent_after": True,
    },
    "effective_cargo_target": {
        "path": str(target.relative_to(root)),
        "exists": True,
        "directory_identity_preserved": True,
        "source_owner": "Current checkout throughout this batch; no source ownership switch.",
    },
    "closed_reviewers": agent_record["closed_reviewers"],
    "prior_author_closure": agent_record["implementation_author_closure"],
    "new_user_chats_created_in_this_batch": 0,
    "new_worktrees_created_in_this_batch": 0,
    "fixture_cleanup": fixture_record,
    "owned_batch_process_matches_before_removal": matches,
    "full_gate": {
        "receipt": "logs/full-gate-r5.json",
        "exit": 0,
        "parent_tool_session_reaped": True,
    },
    "preserved": "Current checkout and effective target; unrelated worktrees, branches, user chats, containers and WIP; all original failures and scoped reviews.",
    "git_delivery": "Local feature commit and source binding pending; no push.",
}
with (archive / "cleanup-final.json").open("x") as stream:
    json.dump(record, stream, indent=2)
    stream.write("\n")
print(
    json.dumps(
        {
            "removed": record["removed"],
            "preserved_originals": manifest["count"],
            "effective_target_preserved": True,
            "owned_process_matches": matches,
        }
    )
)
