"""Verify the complete CI bundle and its source files without extracting it."""
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


def git_blob(revision):
    return subprocess.check_output(
        ["git", "show", revision], cwd=ROOT, timeout=30,
    )


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group()
    choice.add_argument("--index", action="store_true")
    choice.add_argument("--commit")
    args = parser.parse_args()
    if args.commit and not re.fullmatch("[0-9a-f]{40}", args.commit):
        parser.error("commit must be a complete SHA")
    revision = ":" if args.index else args.commit + ":" if args.commit else None

    def read(name):
        return git_blob(revision + PREFIX + name) if revision else (BASE / name).read_bytes()

    manifest = json.loads(read("archive-manifest.json"))
    source = manifest["source_commit"]
    assert re.fullmatch("[0-9a-f]{40}", source)
    packed = read(manifest["archive"])
    assert len(packed) == manifest["archive_bytes"]
    assert digest(packed) == manifest["archive_sha256"]
    rows = manifest["files"]
    assert len(rows) == manifest["count"] == len({row["path"] for row in rows})
    assert sum(row["bytes"] for row in rows) == manifest["original_bytes"]
    sources = 0
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert len(members) == len(rows)
        assert {member.name for member in members} == {row["path"] for row in rows}
        assert all(member.isfile() for member in members)
        for row in rows:
            name = row["path"]
            assert not PurePosixPath(name).is_absolute() and ".." not in PurePosixPath(name).parts
            original = archive.extractfile(name).read()
            assert len(original) == row["bytes"] and digest(original) == row["sha256"], name
            for prefix in ("originals/source/", "originals/context/"):
                if name.startswith(prefix):
                    assert git_blob(source + ":" + name.removeprefix(prefix)) == original, name
                    sources += 1
        original_manifest = archive.extractfile("report/created-files-manifest.json").read()
        assert digest(original_manifest) == manifest["agent_manifest_sha256"]
        assert read("diagnosis.md") == archive.extractfile("report/diagnosis.md").read()
        for name in (
            "originals/0045-windows-logs.raw.log",
            "originals/0024-macos-logs.raw.log",
            "originals/0021-ubuntu-logs.raw.log",
        ):
            assert source.encode() in archive.extractfile(name).read()
    print(json.dumps({
        "evidence": "index" if args.index else args.commit or "current",
        "source": source, "run": manifest["run_id"], "attempt": manifest["attempt"],
        "originals": len(rows), "source_files_exact": sources, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
