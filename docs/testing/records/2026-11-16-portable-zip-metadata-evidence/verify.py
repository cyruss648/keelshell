"""Check archived batch bytes, test inventories and current or exact Git inputs."""

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source")
    options = parser.parse_args()
    if options.source:
        assert re.fullmatch(r"[0-9a-f]{40}", options.source)
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    assert len(packed) == manifest["archive_bytes"]
    assert hashlib.sha256(packed).hexdigest() == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(row.isfile() and not row.issparse() for row in members)
        assert sorted(row.name for row in members) == sorted(row["path"] for row in manifest["entries"])
        for row in manifest["entries"]:
            data = archive.extractfile(row["path"]).read()
            assert len(data) == row["bytes"]
            assert hashlib.sha256(data).hexdigest() == row["sha256"], row["path"]
            files[row["path"]] = data
    assert len(files) == manifest["count"]
    assert files["tools/verify.py"] == Path(__file__).read_bytes()
    freeze = json.loads(files["batch/inputs-freeze-r1.json"])
    expected = {row["path"]: row["sha256"] for row in freeze["inputs"]}
    assert len(expected) == freeze["count"] == 719
    aggregate = hashlib.sha256(
        "".join(f"{name}\0{sha}\n" for name, sha in expected.items()).encode()
    ).hexdigest()
    assert aggregate == freeze["aggregate_sha256"] == manifest["engineering_sha256"]
    command = (
        ["git", "ls-tree", "-rz", "--name-only", options.source] if options.source else
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]
    )
    listing = subprocess.check_output(command, cwd=ROOT, timeout=60)
    names = sorted({name.decode() for name in listing.split(b"\0") if name and not name.startswith(b"docs/")})
    assert names == list(expected)
    for name in names:
        data = (
            subprocess.check_output(["git", "show", f"{options.source}:{name}"], cwd=ROOT, timeout=30)
            if options.source else (ROOT / name).read_bytes()
        )
        assert hashlib.sha256(data).hexdigest() == expected[name], name
    for label in ("format-r1", "core-archive-green-r1", "dos-metadata-red-r1", "scoped-gate-r1"):
        receipt = json.loads(files[f"batch/{label}.json"])
        log = files[f"batch/{label}.log"]
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0
        assert not receipt["timed_out"] and receipt["terminal"]["direct_reaped"]
        assert receipt["log_bytes"] == len(log)
        assert receipt["log_sha256"] == hashlib.sha256(log).hexdigest()
    text = files["batch/scoped-gate-r1.log"].decode()
    listed = re.findall(r"^([A-Za-z0-9_:]+): test$", text, re.MULTILINE)
    results = re.findall(
        r"\btest ([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*) \.\.\. (ok|ignored|FAILED)(?:,.*)?$",
        text, re.MULTILINE,
    )
    assert len(listed) == len(set(listed)) == len(results) == 346
    assert set(listed) == {name for name, _ in results}
    assert sum(outcome == "ok" for _, outcome in results) == 342
    assert sum(outcome == "ignored" for _, outcome in results) == 4
    print(json.dumps({
        "source": options.source or "current", "engineering_inputs": len(names),
        "members": len(files), "mismatches": 0, "tests_replayed": 0,
        "windows_new_source_execution": "NOT_RUN_AT_SEAL",
    }))


if __name__ == "__main__":
    main()
