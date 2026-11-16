"""Verify frozen engineering inputs and lossless evidence against Git blobs."""
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--index", action="store_true")
    source.add_argument("--commit")
    parser.add_argument("--source-commit")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert not args.output.exists(), "Keep earlier receipts"
    prefix = EVIDENCE.relative_to(ROOT).as_posix()
    objects = {}
    if args.index:
        for row in git("ls-files", "--stage", "-z").split(b"\0"):
            if row:
                header, name = row.split(b"\t", 1)
                _mode, oid, stage = header.split()
                assert stage == b"0", name
                objects[name.decode()] = oid.decode()
    else:
        for row in git("ls-tree", "-r", "-z", args.commit).split(b"\0"):
            if row:
                header, name = row.split(b"\t", 1)
                _mode, kind, oid = header.split()
                if kind == b"blob":
                    objects[name.decode()] = oid.decode()

    freeze = json.loads(gzip.decompress((EVIDENCE / "raw/engineering-inputs-r4.json.gz").read_bytes()))
    rows = freeze["inputs"]
    engineering = {row["path"]: row["sha256"] for row in rows}
    aggregate = "".join(f"{row['path']}\0{row['sha256']}\n" for row in rows).encode()
    assert len(rows) == len(engineering) == freeze["count"]
    assert list(engineering) == sorted(engineering)
    assert sha(aggregate) == freeze["aggregate_sha256"]
    current = {
        name.decode() for name in git("ls-files", "--cached", "--others", "--exclude-standard", "-z").split(b"\0")
        if name and not name.startswith(b"docs/") and (ROOT / name.decode()).is_file()
    }
    assert current == set(engineering), "Complete engineering path set drifted"
    assert {name for name in objects if not name.startswith("docs/")} == set(engineering)
    archive = json.loads((EVIDENCE / "archive-manifest.json").read_bytes())
    artifacts = {prefix + "/" + item["artifact"]: item for item in archive["files"].values()}
    assert len(artifacts) == archive["count"]
    assert {name for name in objects if name.startswith(prefix + "/raw/")} == set(artifacts)
    assert {
        path.relative_to(ROOT).as_posix() for path in (EVIDENCE / "raw").rglob("*") if path.is_file()
    } == set(artifacts)
    metadata = {
        path.relative_to(ROOT).as_posix()
        for path in EVIDENCE.iterdir()
        if path.is_file() and path.resolve() != args.output.resolve()
    }
    wanted = set(engineering) | set(artifacts) | metadata
    assert wanted.issubset(objects), sorted(wanted - set(objects))
    ids = list(dict.fromkeys(objects[name] for name in sorted(wanted)))
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
    for name, expected in engineering.items():
        assert sha(blobs[objects[name]]) == expected, name
        assert sha((ROOT / name).read_bytes()) == expected, name
    if args.source_commit:
        source_objects = {}
        for row in git("ls-tree", "-r", "-z", args.source_commit).split(b"\0"):
            if row:
                header, name = row.split(b"\t", 1)
                _mode, kind, oid = header.split()
                if kind == b"blob" and not name.startswith(b"docs/"):
                    source_objects[name.decode()] = oid.decode()
        assert set(source_objects) == set(engineering)
        assert all(source_objects[name] == objects[name] for name in engineering)
    for original, item in archive["files"].items():
        name = prefix + "/" + item["artifact"]
        data = blobs[objects[name]]
        assert data == (ROOT / name).read_bytes(), name
        assert sha(data) == item["artifact_sha256"], name
        decoded = gzip.decompress(data)
        assert len(decoded) == item["source_bytes"], original
        assert sha(decoded) == item["source_sha256"], original
    for name in metadata:
        assert blobs[objects[name]] == (ROOT / name).read_bytes(), name
    record = {
        "recorded_utc": datetime.now(timezone.utc).isoformat(),
        "source_commit": args.source_commit or args.commit,
        "evidence_commit": args.commit, "git_index": args.index,
        "engineering_inputs": len(engineering),
        "engineering_sha256": freeze["aggregate_sha256"],
        "raw_originals": archive["count"],
        "delivery_metadata": len(metadata),
        "git_blobs_current_files_and_lossless_sources_match": True,
        "complete_engineering_path_sets_match": True, "mismatches": [],
    }
    with args.output.open("x") as output:
        json.dump(record, output, indent=2)
        output.write("\n")
    print(json.dumps(record))


if __name__ == "__main__":
    main()
