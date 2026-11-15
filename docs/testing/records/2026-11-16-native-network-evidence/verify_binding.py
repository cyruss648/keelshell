"""Verify this batch's frozen inputs and lossless evidence against Git blobs."""

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import subprocess


EVIDENCE = Path(__file__).resolve().parent
ROOT = EVIDENCE.parents[3]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, timeout=30)


def engineering_paths():
    """Match this batch's full Git-visible non-document input selection."""
    names = git("ls-files", "--cached", "--others", "--exclude-standard", "-z")
    return {
        name.decode() for name in names.split(b"\0")
        if name and not name.startswith(b"docs/") and (ROOT / name.decode()).is_file()
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--index", action="store_true")
    source.add_argument("--commit")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert not args.output.exists(), "Keep earlier receipts; choose a new output"
    prefix = EVIDENCE.relative_to(ROOT).as_posix()
    objects = {}
    if args.index:
        for row in git("ls-files", "--stage", "-z").split(b"\0"):
            if row:
                header, path = row.split(b"\t", 1)
                _mode, oid, stage = header.split()
                assert stage == b"0", path
                objects[path.decode()] = oid.decode()
    else:
        assert len(args.commit) == 40 and all(c in "0123456789abcdef" for c in args.commit)
        for row in git("ls-tree", "-r", "-z", args.commit).split(b"\0"):
            if row:
                header, path = row.split(b"\t", 1)
                _mode, kind, oid = header.split()
                if kind == b"blob":
                    objects[path.decode()] = oid.decode()

    freeze = json.loads(gzip.decompress((EVIDENCE / "raw/final-inputs-r3.json.gz").read_bytes()))
    rows = freeze["inputs"]
    engineering = {row["path"]: row["sha256"] for row in rows}
    aggregate = "".join(f"{row['path']}\0{row['sha256']}\n" for row in rows).encode()
    assert len(rows) == len(engineering) == freeze["count"]
    assert list(engineering) == sorted(engineering)
    assert sha(aggregate) == freeze["aggregate_sha256"]
    assert engineering_paths() == set(engineering), "Complete engineering path set drifted"
    assert {path for path in objects if not path.startswith("docs/")} == set(engineering)
    archive = json.loads((EVIDENCE / "archive-manifest.json").read_bytes())
    artifacts = {prefix + "/" + item["artifact"]: item for item in archive["files"].values()}
    assert len(artifacts) == archive["count"]
    metadata = {prefix + "/archive-manifest.json", prefix + "/omitted-artifacts.json"}
    wanted = set(engineering) | set(artifacts) | metadata
    assert wanted.issubset(objects), sorted(wanted - set(objects))
    assert {path for path in objects if path.startswith(prefix + "/raw/")} == set(artifacts)
    assert {
        path.relative_to(ROOT).as_posix()
        for path in (EVIDENCE / "raw").rglob("*") if path.is_file()
    } == set(artifacts)

    ids = list(dict.fromkeys(objects[path] for path in sorted(wanted)))
    payload = subprocess.run(
        ["git", "cat-file", "--batch"], cwd=ROOT,
        input="".join(oid + "\n" for oid in ids).encode(),
        stdout=subprocess.PIPE, check=True, timeout=30,
    ).stdout
    blobs = {}
    offset = 0
    for expected in ids:
        end = payload.index(b"\n", offset)
        oid, kind, size = payload[offset:end].split()
        assert oid.decode() == expected and kind == b"blob"
        offset = end + 1
        size = int(size)
        blobs[expected] = payload[offset:offset + size]
        offset += size
        assert payload[offset:offset + 1] == b"\n"
        offset += 1
    assert offset == len(payload)
    for path, expected in engineering.items():
        assert sha(blobs[objects[path]]) == expected, path
        assert sha((ROOT / path).read_bytes()) == expected, path
    for original, item in archive["files"].items():
        path = prefix + "/" + item["artifact"]
        data = blobs[objects[path]]
        assert data == (ROOT / path).read_bytes(), path
        assert sha(data) == item["artifact_sha256"], path
        decoded = gzip.decompress(data)
        assert len(decoded) == item["source_bytes"], original
        assert sha(decoded) == item["source_sha256"], original
    for path in metadata:
        assert blobs[objects[path]] == (ROOT / path).read_bytes(), path

    record = {
        "recorded_utc": datetime.now(timezone.utc).isoformat(),
        "source_commit": args.commit, "git_index": args.index,
        "engineering_inputs": len(engineering),
        "engineering_sha256": freeze["aggregate_sha256"],
        "raw_originals": archive["count"],
        "git_blobs_current_files_and_lossless_sources_match": True,
        "complete_engineering_path_sets_match": True, "mismatches": [],
    }
    with args.output.open("x") as stream:
        json.dump(record, stream, indent=2)
        stream.write("\n")
    print(json.dumps(record))


if __name__ == "__main__":
    main()
