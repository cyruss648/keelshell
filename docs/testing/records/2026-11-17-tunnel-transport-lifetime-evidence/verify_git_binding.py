"""Read-only verification of this batch's exact Git inputs and evidence archives."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

MAIN = "docs/testing/records/2026-11-17-tunnel-transport-lifetime-evidence"
CI = "docs/testing/records/2026-11-17-independent-zmodem-peers-evidence/ci-completion"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def git_blobs(root, source, paths):
    assert len(paths) == len(set(paths))
    assert all(path and not any(ord(char) < 32 for char in path) for path in paths)
    result = subprocess.run(
        ["git", "cat-file", "--batch"], cwd=root,
        input="".join(f"{source}:{path}\n" for path in paths).encode(),
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True,
    )
    stream = io.BytesIO(result.stdout)
    blobs = {}
    for path in paths:
        header = stream.readline().split()
        assert len(header) == 3 and header[1] == b"blob", path
        size = int(header[2])
        content = stream.read(size)
        assert len(content) == size and stream.read(1) == b"\n", path
        blobs[path] = content
    assert stream.read() == b""
    return blobs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commit", required=True)
    options = parser.parse_args()
    assert re.fullmatch(r"[0-9a-f]{40}", options.commit)
    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10
    ).strip())
    paths = [f"{directory}/{name}" for directory in (MAIN, CI)
             for name in ("manifest.json", "originals.tar.gz")]
    blobs = git_blobs(root, options.commit, paths)
    archives = []
    candidate = None
    for directory in (MAIN, CI):
        raw = blobs[directory + "/originals.tar.gz"]
        manifest_raw = blobs[directory + "/manifest.json"]
        assert raw == (root / directory / "originals.tar.gz").read_bytes()
        assert manifest_raw == (root / directory / "manifest.json").read_bytes()
        manifest = json.loads(manifest_raw)
        assert len(raw) == manifest["bytes"] and digest(raw) == manifest["sha256"]
        expected = {row["member"]: row for row in manifest["files"]}
        with tarfile.open(fileobj=io.BytesIO(raw)) as archive:
            members = archive.getmembers()
            assert len(members) == len(expected) == manifest["members"]
            assert len({member.name for member in members}) == len(members)
            assert {member.name for member in members} == set(expected)
            for member in members:
                assert member.isfile()
                content = archive.extractfile(member).read()
                row = expected[member.name]
                assert len(content) == row["bytes"] and digest(content) == row["sha256"]
                if directory == MAIN and member.name == "parent/candidate-inputs-r2.json":
                    candidate = json.loads(content)
        archives.append({"path": directory, "members": len(members), "sha256": digest(raw)})
    assert candidate is not None and candidate["count"] == len(candidate["inputs"]) == 769
    canonical = json.dumps(candidate["inputs"], sort_keys=True, separators=(",", ":")).encode()
    assert digest(canonical) == candidate["aggregate_sha256"]
    inputs = git_blobs(root, options.commit, [row["path"] for row in candidate["inputs"]])
    for row in candidate["inputs"]:
        raw = inputs[row["path"]]
        assert len(raw) == row["bytes"] and digest(raw) == row["sha256"], row["path"]
    print(json.dumps({
        "source": options.commit, "engineering_inputs": len(inputs),
        "aggregate_sha256": digest(canonical), "archives": archives,
        "members": sum(row["members"] for row in archives),
        "all_exact_git_blobs_equal": True,
        "boundary": "Offline Git/archive verification; no product tests, network or target access.",
    }))


if __name__ == "__main__":
    main()
