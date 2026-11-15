"""Verify the complete engineering freeze and lossless evidence against Git blobs."""

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import subprocess

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=root, timeout=30)


def engineering_paths():
    paths = [root / "Cargo.toml", root / "Cargo.lock"]
    paths += list((root / "crates").rglob("*.rs"))
    paths += list((root / "crates").rglob("*.py"))
    paths += list((root / "crates").rglob("Cargo.toml"))
    paths += list((root / "scripts").glob("*.py"))
    if (root / ".cargo").exists():
        paths += list((root / ".cargo").rglob("*.toml"))
    return {path.relative_to(root).as_posix() for path in paths}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", action="store_true")
    parser.add_argument("--commit")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert args.index != bool(args.commit), "Choose the index or one exact full commit"
    assert not args.output.exists(), "Keep previous receipts; use a new output name"
    prefix = evidence.relative_to(root).as_posix()
    if args.index:
        rows = git("ls-files", "--stage", "-z")
        objects = {}
        for row in rows.split(b"\0"):
            if row:
                header, path = row.split(b"\t", 1)
                _mode, oid, stage = header.split()
                assert stage == b"0", path
                objects[path.decode()] = oid.decode()
    else:
        assert len(args.commit) == 40 and all(c in "0123456789abcdef" for c in args.commit)
        objects = {}
        for row in git("ls-tree", "-r", "-z", args.commit).split(b"\0"):
            if row:
                header, path = row.split(b"\t", 1)
                _mode, kind, oid = header.split()
                if kind == b"blob":
                    objects[path.decode()] = oid.decode()

    freeze_path = evidence / "raw/frozen-inputs-r2.json.gz"
    freeze = json.loads(gzip.decompress(freeze_path.read_bytes()))
    engineering = freeze["files"]
    encoded = json.dumps(engineering, sort_keys=True, separators=(",", ":")).encode()
    assert len(engineering) == freeze["count"] and sha(encoded) == freeze["sha256"]
    assert engineering_paths() == set(engineering), "Complete engineering path set drifted"
    archive = json.loads((evidence / "archive-manifest.json").read_bytes())
    artifacts = {
        prefix + "/" + item["artifact"]: item for item in archive["files"].values()
    }
    assert len(artifacts) == archive["count"]
    metadata = {prefix + "/archive-manifest.json", prefix + "/omitted-artifacts.json"}
    wanted = set(engineering) | set(artifacts) | metadata
    assert wanted.issubset(objects), sorted(wanted - set(objects))
    assert {path for path in objects if path.startswith(prefix + "/raw/")} == set(artifacts)
    assert {
        path.relative_to(root).as_posix()
        for path in (evidence / "raw").rglob("*") if path.is_file()
    } == set(artifacts)
    ids = list(dict.fromkeys(objects[path] for path in sorted(wanted)))
    payload = subprocess.run(
        ["git", "cat-file", "--batch"], cwd=root,
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
        assert sha((root / path).read_bytes()) == expected, path
    for source, item in archive["files"].items():
        path = prefix + "/" + item["artifact"]
        data = blobs[objects[path]]
        assert data == (root / path).read_bytes(), path
        assert sha(data) == item["artifact_sha256"], path
        decoded = gzip.decompress(data) if item["artifact"] != "raw/" + source else data
        assert len(decoded) == item["source_bytes"], source
        assert sha(decoded) == item["source_sha256"], source
    for path in metadata:
        assert blobs[objects[path]] == (root / path).read_bytes(), path
    record = {
        "recorded_utc": datetime.now(timezone.utc).isoformat(),
        "source_commit": args.commit, "git_index": args.index,
        "engineering_inputs": len(engineering), "engineering_sha256": freeze["sha256"],
        "raw_originals": len(archive["files"]),
        "git_blobs_current_files_and_lossless_sources_match": True,
        "complete_engineering_path_sets_match": True, "mismatches": [],
    }
    with args.output.open("x") as stream:
        json.dump(record, stream, indent=2)
        stream.write("\n")
    print(json.dumps(record))


if __name__ == "__main__":
    main()
