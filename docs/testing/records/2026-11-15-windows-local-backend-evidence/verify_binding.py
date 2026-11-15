"""Verify frozen engineering and lossless raw evidence against Git blobs."""

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


def run(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", action="store_true")
    parser.add_argument("--commit")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert args.index != bool(args.commit), "Choose the index or one exact commit"
    prefix = evidence.relative_to(root).as_posix()
    if args.index:
        raw = run("ls-files", "--stage", "-z")
        objects = {}
        for line in raw.split(b"\0"):
            if not line:
                continue
            header, path = line.split(b"\t", 1)
            mode, object_id, stage = header.split()
            assert stage == b"0"
            objects[path.decode()] = object_id.decode()
    else:
        assert len(args.commit) == 40 and all(c in "0123456789abcdef" for c in args.commit)
        raw = run("ls-tree", "-r", "-z", args.commit)
        objects = {}
        for line in raw.split(b"\0"):
            if line:
                header, path = line.split(b"\t", 1)
                mode, kind, object_id = header.split()
                if kind == b"blob":
                    objects[path.decode()] = object_id.decode()

    freeze_path = evidence / "raw/combined-inputs-r2.json"
    freeze = json.loads(freeze_path.read_text())
    archive = json.loads((evidence / "archive-manifest.json").read_text())
    engineering = dict(freeze["files"], **freeze["scripts"])
    artifacts = {
        prefix + "/" + item["artifact"]: item
        for item in archive["files"].values()
    }
    wanted = set(engineering) | set(artifacts) | {
        prefix + "/archive-manifest.json", prefix + "/raw/combined-inputs-r2.json",
    }
    assert wanted.issubset(objects), sorted(wanted - set(objects))
    assert {p for p in objects if p.startswith(prefix + "/raw/")} == set(artifacts)
    assert {
        p.relative_to(root).as_posix()
        for p in (evidence / "raw").rglob("*") if p.is_file()
    } == set(artifacts)
    ids = list(dict.fromkeys(objects[path] for path in sorted(wanted)))
    result = subprocess.run(
        ["git", "cat-file", "--batch"], cwd=root,
        input="".join(object_id + "\n" for object_id in ids).encode(),
        stdout=subprocess.PIPE, check=True,
    ).stdout
    blobs = {}
    offset = 0
    for expected in ids:
        end = result.index(b"\n", offset)
        object_id, kind, size = result[offset:end].split()
        assert object_id.decode() == expected and kind == b"blob"
        offset = end + 1
        size = int(size)
        blobs[expected] = result[offset:offset + size]
        offset += size
        assert result[offset:offset + 1] == b"\n"
        offset += 1
    assert offset == len(result)
    for path, expected in engineering.items():
        assert sha(blobs[objects[path]]) == expected, path
        assert sha((root / path).read_bytes()) == expected, path
    current_sources = {"Cargo.toml", "Cargo.lock"} | {
        path.relative_to(root).as_posix()
        for path in (root / "crates").rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    }
    assert current_sources == set(freeze["files"])
    assert {p.relative_to(root).as_posix() for p in (root / "scripts").glob("*.py")} == set(freeze["scripts"])
    for source, item in archive["files"].items():
        path = prefix + "/" + item["artifact"]
        data = blobs[objects[path]]
        assert sha(data) == item["artifact_sha256"], path
        assert (root / path).read_bytes() == data, path
        compressed = item["artifact"] != "raw/" + source
        decoded = gzip.decompress(data) if compressed else data
        assert len(decoded) == item["source_bytes"] and sha(decoded) == item["source_sha256"], source
    for path in (prefix + "/archive-manifest.json", prefix + "/raw/combined-inputs-r2.json"):
        assert blobs[objects[path]] == (root / path).read_bytes()
    record = {
        "recorded_utc": datetime.now(timezone.utc).isoformat(),
        "source_commit": args.commit,
        "git_index": args.index,
        "freeze_sha256": sha(freeze_path.read_bytes()),
        "engineering_inputs": len(engineering),
        "raw_originals": len(archive["files"]),
        "git_blobs_current_files_and_lossless_sources_match": True,
        "complete_engineering_path_sets_match": True,
        "mismatches": [],
    }
    with args.output.open("x") as stream:
        json.dump(record, stream, indent=2)
        stream.write("\n")
    print(json.dumps(record))


if __name__ == "__main__":
    main()
