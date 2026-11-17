"""Verify exact committed source inputs and sealed originals, without running tests."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

MAIN = "docs/testing/records/2026-11-17-mcp-client-history-evidence"
CI = "docs/testing/records/2026-11-17-tunnel-transport-lifetime-evidence/ci-completion"
CI_SOURCE = "cdf1e171f2f9b179f09db230f4739696d02fe80d"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def git_blobs(root, source, paths):
    assert len(paths) == len(set(paths))
    assert re.fullmatch(r"[0-9a-f]{40}", source)
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
        raw = stream.read(size)
        assert len(raw) == size and stream.read(1) == b"\n"
        blobs[path] = raw
    assert stream.read() == b""
    return blobs


def verify_inputs(root, source, candidate, expected_count):
    rows = candidate["inputs"]
    assert candidate["count"] == len(rows) == expected_count
    canonical = json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()
    assert digest(canonical) == candidate["aggregate_sha256"]
    blobs = git_blobs(root, source, [row["path"] for row in rows])
    for row in rows:
        raw = blobs[row["path"]]
        assert len(raw) == row["bytes"] and digest(raw) == row["sha256"], row["path"]
    return {
        "source": source, "engineering_inputs": len(rows),
        "aggregate_sha256": digest(canonical), "all_exact_git_blobs_equal": True,
    }


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
    sources = []
    archives = []
    for directory, input_member, source, count in [
        (MAIN, "parent/candidate-inputs-r1.json", options.commit, 777),
        (CI, "source-inputs.json", CI_SOURCE, 769),
    ]:
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
            logical = 0
            candidate = None
            for member in members:
                assert member.isfile()
                content = archive.extractfile(member).read()
                row = expected[member.name]
                assert len(content) == row["bytes"] and digest(content) == row["sha256"]
                logical += len(content)
                if member.name == input_member:
                    candidate = json.loads(content)
            assert logical == manifest["logical_bytes"]
        assert candidate is not None
        sources.append(verify_inputs(root, source, candidate, count))
        archives.append({
            "path": directory, "members": len(members), "sha256": digest(raw),
            "all_member_digests_equal": True,
        })
    print(json.dumps({
        "commit": options.commit, "sources": sources, "archives": archives,
        "members": sum(row["members"] for row in archives),
        "boundary": "Offline exact Git/archive binding; no product tests, network or target access.",
    }))


if __name__ == "__main__":
    main()
