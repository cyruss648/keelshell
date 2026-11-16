"""Verify archived originals and engineering inputs against exact Git trees."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
PREFIX = BASE.relative_to(ROOT).as_posix()


def git(*args, data=None):
    return subprocess.run(
        ["git", *args], cwd=ROOT, input=data, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, check=True, timeout=30,
    ).stdout


def blobs(revisions):
    """Read blobs in one bounded process without printing their contents."""
    if any("\n" in revision for revision in revisions):
        raise ValueError("invalid revision")
    data = git("cat-file", "--batch", data=("\n".join(revisions) + "\n").encode())
    result = {}
    cursor = 0
    for revision in revisions:
        end = data.index(b"\n", cursor)
        fields = data[cursor:end].split()
        if len(fields) != 3 or fields[1] != b"blob":
            raise ValueError("required Git blob is missing")
        size = int(fields[2])
        start = end + 1
        result[revision] = data[start:start + size]
        if len(result[revision]) != size or data[start + size:start + size + 1] != b"\n":
            raise ValueError("incomplete Git blob response")
        cursor = start + size + 1
    if cursor != len(data):
        raise ValueError("unexpected Git blob data")
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", action="store_true")
    parser.add_argument("--source-commit")
    parser.add_argument("--evidence-commit")
    parser.add_argument("--check-current", action="store_true")
    args = parser.parse_args()
    if args.index:
        if args.source_commit or args.evidence_commit:
            parser.error("index cannot be combined with commits")
        source = evidence = ":"
    else:
        if not all(
            value and re.fullmatch(r"[0-9a-f]{40}", value)
            for value in [args.source_commit, args.evidence_commit]
        ):
            parser.error("two exact 40-character commits are required")
        source, evidence = args.source_commit + ":", args.evidence_commit + ":"

    def evidence_blob(path):
        revision = evidence + PREFIX + "/" + path
        return blobs([revision])[revision]

    manifest = json.loads(evidence_blob("archive-manifest.json"))
    rows = manifest["files"]
    revisions = [evidence + PREFIX + "/" + row["archive"] for row in rows]
    archived = blobs(revisions)
    for row, revision in zip(rows, revisions):
        data = archived[revision]
        assert len(data) == row["gzip_bytes"] and digest(data) == row["gzip_sha256"]
        original = gzip.decompress(data)
        assert len(original) == row["bytes"] and digest(original) == row["sha256"]
    assert len(rows) == manifest["count"]
    assert sum(row["bytes"] for row in rows) == manifest["original_bytes"]
    freeze = json.loads(gzip.decompress(evidence_blob(manifest["inputs_freeze"])))

    source_paths = (
        git("ls-files", "-z") if args.index
        else git("ls-tree", "-r", "--name-only", "-z", args.source_commit)
    )
    actual_paths = {
        name.decode() for name in source_paths.split(b"\0")
        if name and not name.startswith(b"docs/")
    }
    expected_paths = {row["path"] for row in freeze["inputs"]}
    assert actual_paths == expected_paths and len(expected_paths) == freeze["count"]
    revisions = [source + row["path"] for row in freeze["inputs"]]
    sources = blobs(revisions)
    for row, revision in zip(freeze["inputs"], revisions):
        assert digest(sources[revision]) == row["sha256"], row["path"]
        if args.check_current:
            assert digest((ROOT / row["path"]).read_bytes()) == row["sha256"], row["path"]

    previews = json.loads(evidence_blob("previews.json"))
    by_path = {row["path"]: row for row in rows}
    for preview in previews:
        assert digest(evidence_blob(preview["path"])) == preview["sha256"]
        assert by_path[preview["original"]]["sha256"] == preview["sha256"]
    print(json.dumps({
        "source": "index" if args.index else args.source_commit,
        "evidence": "index" if args.index else args.evidence_commit,
        "originals": len(rows), "engineering_inputs": freeze["count"],
        "previews": len(previews), "mismatches": [],
        "checked_current_bytes": args.check_current,
    }))


if __name__ == "__main__":
    main()
