"""Verify all raw originals and bind the full engineering input set to Git."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
PREFIX = BASE.relative_to(ROOT).as_posix() + "/"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, timeout=30)


def verify_delivery(read):
    manifest = json.loads(read("delivery-manifest.json"))
    packed = read(manifest["archive"])
    assert len(packed) == manifest["archive_bytes"]
    assert digest(packed) == manifest["archive_sha256"]
    rows = manifest["files"]
    assert len(rows) == manifest["count"] == len({row["path"] for row in rows})
    assert sum(row["bytes"] for row in rows) == manifest["original_bytes"]
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert len(members) == len(rows) and all(member.isfile() for member in members)
        assert {member.name for member in members} == {row["path"] for row in rows}
        for row in rows:
            name = row["path"]
            assert not PurePosixPath(name).is_absolute() and ".." not in PurePosixPath(name).parts
            data = archive.extractfile(name).read()
            assert len(data) == row["bytes"] and digest(data) == row["sha256"], name
        delivery = json.loads(archive.extractfile("source-delivery-readback-r1.json").read())
        assert set(delivery["shas"].values()) == {manifest["source_commit"]}
        retired = json.loads(archive.extractfile("temporary-branch-absent-r1.json").read())
        assert retired["local_tracking_remote_absent"]
        diagnosis = json.loads(archive.extractfile("windows-native-diagnosis-r2.json").read())
        assert diagnosis["source"] == manifest["source_commit"]
        assert diagnosis["preflight"] == {
            "passed": 48, "failed": 16, "ignored": 0, "seconds": 0.5,
        }
        assert diagnosis["full_gate"] == "NOT_RUN" and len(diagnosis["failures"]) == 16
        log = archive.extractfile(diagnosis["raw_log"]["path"]).read()
        assert len(log) == diagnosis["raw_log"]["bytes"]
        assert digest(log) == diagnosis["raw_log"]["sha256"]
        assert manifest["source_commit"].encode() in log
    return {"originals": len(rows), "source": manifest["source_commit"], "mismatches": []}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", action="store_true")
    parser.add_argument("--source")
    parser.add_argument("--evidence")
    parser.add_argument("--delivery-only", action="store_true")
    args = parser.parse_args()
    for name in ("source", "evidence"):
        value = getattr(args, name)
        if value and not re.fullmatch("[0-9a-f]{40}", value):
            parser.error(name + " must be a complete commit SHA")
    if args.index and args.evidence:
        parser.error("--index and --evidence are mutually exclusive")
    revision = ":" if args.index else args.evidence + ":" if args.evidence else None

    def read(name):
        return git("show", revision + PREFIX + name) if revision else (BASE / name).read_bytes()

    if args.delivery_only:
        print(json.dumps({"delivery": verify_delivery(read)}))
        return

    manifest = json.loads(read("archive-manifest.json"))
    packed = read(manifest["archive"])
    assert len(packed) == manifest["archive_bytes"]
    assert digest(packed) == manifest["archive_sha256"]
    rows = manifest["files"]
    assert len(rows) == manifest["count"] == len({row["path"] for row in rows})
    assert sum(row["bytes"] for row in rows) == manifest["original_bytes"]
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert len(members) == len(rows)
        assert all(member.isfile() for member in members)
        assert {member.name for member in members} == {row["path"] for row in rows}
        for row in rows:
            name = row["path"]
            assert not PurePosixPath(name).is_absolute() and ".." not in PurePosixPath(name).parts
            data = archive.extractfile(name).read()
            assert len(data) == row["bytes"] and digest(data) == row["sha256"], name
        freeze = json.loads(archive.extractfile("batch/inputs-freeze-r1.json").read())
        aggregate = "".join(
            f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
        ).encode()
        assert freeze["count"] == manifest["engineering_inputs"] == len(freeze["inputs"])
        assert digest(aggregate) == freeze["aggregate_sha256"] == manifest["engineering_sha256"]
        gate = json.loads(archive.extractfile("batch/full-gate-r1.json").read())
        log = archive.extractfile("batch/full-gate-r1.log").read()
        assert len(log) == gate["log_bytes"] and digest(log) == gate["log_sha256"]
        assert gate["observed_exit"] == gate["controller_exit"] == 0
        assert not gate["timed_out"] and not gate["interrupted"]
        assert gate["terminal"] == {
            "direct_reaped": True, "active_processes": 0,
            "pipes_drained": True, "errors": [],
        }
        for line in archive.extractfile("batch/reviewer/SHA256SUMS").read().decode().splitlines():
            expected, name = line.split("  ", 1)
            assert digest(archive.extractfile("batch/reviewer/" + name).read()) == expected, name

    source_revision = args.source + ":" if args.source else ":" if args.index else None
    if args.source:
        paths = git("ls-tree", "-r", "--name-only", "-z", args.source).split(b"\0")
    elif args.index:
        paths = git("ls-files", "--cached", "-z").split(b"\0")
    else:
        paths = git("ls-files", "--cached", "--others", "--exclude-standard", "-z").split(b"\0")
    actual_paths = {
        name.decode() for name in paths
        if name and not name.startswith(b"docs/")
        and (source_revision or (ROOT / name.decode()).is_file())
    }
    assert actual_paths == {row["path"] for row in freeze["inputs"]}
    for row in freeze["inputs"]:
        data = git("show", source_revision + row["path"]) if source_revision else (
            ROOT / row["path"]
        ).read_bytes()
        assert digest(data) == row["sha256"], row["path"]
    print(json.dumps({
        "evidence": "index" if args.index else args.evidence or "current",
        "source": args.source or "index" if args.index else args.source or "current",
        "originals": len(rows), "engineering_inputs": freeze["count"],
        "gate_exit": gate["observed_exit"], "mismatches": [],
    }))


if __name__ == "__main__":
    main()
