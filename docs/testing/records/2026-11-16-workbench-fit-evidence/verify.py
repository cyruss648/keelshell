"""Check archived bytes and bind the frozen inputs to current or committed source."""

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


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def safe_path(name):
    path = PurePosixPath(name)
    require(not path.is_absolute() and name == path.as_posix()
            and all(part not in ("", ".", "..") for part in path.parts)
            and "\\" not in name and "\0" not in name, "unsafe evidence path")
    return name


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, timeout=60)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="exact complete source commit; default: current files")
    args = parser.parse_args()
    if args.source:
        require(re.fullmatch(r"[0-9a-f]{40}", args.source), "expected complete commit SHA")
        require(git("rev-parse", args.source + "^{commit}").decode().strip() == args.source,
                "not the specified commit")

    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    require(len(packed) == manifest["archive_bytes"]
            and digest(packed) == manifest["archive_sha256"], "archive identity mismatch")
    rows = manifest["entries"]
    expected_names = [safe_path(row["path"]) for row in rows]
    require(expected_names == sorted(set(expected_names))
            and len(rows) == manifest["count"], "archive inventory mismatch")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        require(sorted(member.name for member in members) == expected_names
                and all(member.isfile() and not member.issparse() for member in members),
                "non-regular, extra, missing or repeated member")
        for row in rows:
            data = archive.extractfile(row["path"]).read()
            require(len(data) == row["bytes"] and digest(data) == row["sha256"],
                    "member identity mismatch: " + row["path"])
            files[row["path"]] = data
    require(sum(len(data) for data in files.values()) == manifest["uncompressed_bytes"],
            "uncompressed size mismatch")
    require(files["tools/verify.py"] == Path(__file__).read_bytes(),
            "verifier differs from archived bytes")

    freeze = json.loads(files["batch/inputs-freeze-r2.json"])
    frozen = {row["path"]: row["sha256"] for row in freeze["inputs"]}
    require(len(frozen) == len(freeze["inputs"]) == freeze["count"] == 719,
            "frozen inventory mismatch")
    require(list(frozen) == sorted(frozen), "frozen inventory not sorted")
    aggregate = digest("".join(f"{name}\0{sha}\n" for name, sha in frozen.items()).encode())
    require(aggregate == freeze["aggregate_sha256"] == manifest["engineering_sha256"],
            "frozen aggregate mismatch")

    if args.source:
        names = git("ls-tree", "-rz", "--name-only", args.source).split(b"\0")
    else:
        names = git("ls-files", "--cached", "--others", "--exclude-standard", "-z").split(b"\0")
    names = sorted({name.decode() for name in names if name and not name.startswith(b"docs/")})
    require(names == sorted(frozen), "source path inventory differs")
    for name in names:
        data = git("show", f"{args.source}:{name}") if args.source else (ROOT / name).read_bytes()
        require(digest(data) == frozen[name], "source bytes differ: " + name)
    snapshots = [name for name in files if name.startswith("batch/source-r2/")]
    require(len(snapshots) == 5, "expected five changed source snapshots")
    for name in snapshots:
        require(digest(files[name]) == frozen[name.removeprefix("batch/source-r2/")],
                "snapshot differs: " + name)

    receipt = json.loads(files["batch/full-gate-r1.json"])
    gate_log = files["batch/full-gate-r1.log"]
    require(receipt["observed_exit"] == receipt["controller_exit"] == 0
            and not receipt["timed_out"] and not receipt["interrupted"]
            and receipt["terminal"] == {
                "direct_reaped": True, "active_processes": 0,
                "pipes_drained": True, "errors": [],
            }
            and receipt["log_bytes"] == len(gate_log)
            and receipt["log_sha256"] == digest(gate_log), "gate receipt mismatch")
    summary = json.loads(files["batch/full-gate-summary-r1.json"])
    require(summary["engineering_sha256"] == aggregate
            and summary["engineering_mismatches"] == []
            and summary["app_inventory_count"] == 1343
            and summary["app_inventory_missing"] == summary["app_inventory_extra"] == []
            and summary["rust"] == {"passed": 3224, "failed": 0, "ignored": 48}
            and summary["rustdoc"] == {"passed": 14, "failed": 0, "ignored": 0},
            "gate summary mismatch")
    print(json.dumps({
        "source": args.source or "current", "members": len(files),
        "engineering_inputs": len(frozen), "engineering_sha256": aggregate,
        "source_mismatches": 0, "archive_mismatches": 0, "tests_replayed": 0,
    }))


if __name__ == "__main__":
    main()
