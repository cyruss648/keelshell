"""Read retained evidence and compare its engineering freeze with exact Git blobs."""

import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(*args, input_data=None):
    result = subprocess.run(
        ["git", *args], cwd=ROOT, input=input_data,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True,
    )
    return result.stdout


def main():
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    source = manifest["source_commit"]
    require(re.fullmatch(r"[0-9a-f]{40}", source), "expected complete source SHA")
    require(git("rev-parse", source + "^{commit}").decode().strip() == source,
            "source is not an exact commit")
    archive = (BASE / "originals.tar.gz").read_bytes()
    require(len(archive) == manifest["archive_bytes"]
            and sha(archive) == manifest["archive_sha256"], "archive changed")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as retained:
        for member in retained:
            path = PurePosixPath(member.name)
            require(member.isfile() and not path.is_absolute()
                    and member.name == path.as_posix()
                    and all(part not in ("", ".", "..") for part in path.parts)
                    and "\\" not in member.name and member.name not in files,
                    "invalid or duplicate archive member")
            stream = retained.extractfile(member)
            require(stream is not None, "unreadable archive member")
            files[member.name] = stream.read()
    expected = {entry["path"]: entry for entry in manifest["entries"]}
    require(len(expected) == len(manifest["entries"]) == len(files)
            and set(expected) == set(files), "member inventory changed")
    for name, data in files.items():
        entry = expected[name]
        require(len(data) == entry["bytes"] and sha(data) == entry["sha256"],
                "member changed: " + name)
    freeze = json.loads(files["native/engineering-inputs-r1.json"])
    rows = freeze["inputs"]
    require(len(rows) == freeze["count"] == 719
            and [row["path"] for row in rows] == sorted({row["path"] for row in rows}),
            "engineering inventory changed")
    aggregate = "".join(f"{row['path']}\0{row['sha256']}\n" for row in rows).encode()
    require(sha(aggregate) == freeze["aggregate_sha256"]
            == manifest["engineering_sha256"], "engineering freeze changed")
    source_hashes = {row["path"]: row["sha256"] for row in rows}
    objects = {}
    for entry in git("ls-tree", "-rz", "--full-tree", source).split(b"\0"):
        if entry:
            metadata, name = entry.split(b"\t", 1)
            if not name.startswith(b"docs/"):
                mode, kind, oid = metadata.decode().split()
                require(kind == "blob" and mode in ("100644", "100755"),
                        "unexpected source object")
                objects[name.decode()] = oid
    require(set(objects) == set(source_hashes), "source path inventory changed")
    identifiers = sorted(set(objects.values()))
    stream = io.BytesIO(git(
        "cat-file", "--batch",
        input_data="".join(oid + "\n" for oid in identifiers).encode(),
    ))
    digests = {}
    for oid in identifiers:
        header = stream.readline().decode().split()
        require(len(header) == 3 and header[:2] == [oid, "blob"], "missing source blob")
        data = stream.read(int(header[2]))
        require(len(data) == int(header[2]) and stream.read(1) == b"\n",
                "short source blob")
        digests[oid] = sha(data)
    require(stream.read() == b"", "extra source bytes")
    require(all(digests[oid] == source_hashes[name] for name, oid in objects.items()),
            "source content changed")
    package = json.loads(files["native/package-manifest.json"])
    require(package["build"]["commit"] == source
            and package["binary_sha256"] == manifest["app_sha256"]
            and package["mcp_binary_sha256"] == manifest["mcp_sha256"],
            "package identity changed")
    run = json.loads(files["delivery/source-ci-final-r1.json"])
    require(run["headSha"] == source and run["status"] == "completed"
            and run["conclusion"] == "failure", "CI identity or result changed")
    require({job["databaseId"]: job["conclusion"] for job in run["jobs"]}
            == {133189079006: "success", 133189079055: "failure",
                133189079924: "success"}, "CI job result changed")
    print(json.dumps({
        "source": source, "members": len(files), "engineering_inputs": len(rows),
        "source_mismatches": 0, "archive_mismatches": 0,
        "builds_or_tests_replayed": 0,
    }))


if __name__ == "__main__":
    main()
