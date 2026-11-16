"""Verify original evidence bytes and bind the three-file batch to exact source."""

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


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, timeout=60)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="complete source commit; default: current files")
    args = parser.parse_args()
    if args.source:
        require(re.fullmatch(r"[0-9a-f]{40}", args.source), "expected complete SHA")
        require(git("rev-parse", args.source + "^{commit}").decode().strip() == args.source,
                "source is not the specified commit")
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    require(len(packed) == manifest["archive_bytes"]
            and digest(packed) == manifest["archive_sha256"], "archive identity differs")
    rows = manifest["entries"]
    names = [row["path"] for row in rows]
    require(names == sorted(set(names)) and len(names) == manifest["count"],
            "archive inventory differs")
    for name in names:
        path = PurePosixPath(name)
        require(not path.is_absolute() and path.as_posix() == name
                and all(part not in ("", ".", "..") for part in path.parts)
                and "\\" not in name and "\0" not in name, "unsafe archive path")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        require(sorted(member.name for member in members) == names
                and all(member.isfile() and not member.issparse() for member in members),
                "extra, missing, repeated or non-regular member")
        for row in rows:
            data = archive.extractfile(row["path"]).read()
            require(len(data) == row["bytes"] and digest(data) == row["sha256"],
                    "member differs: " + row["path"])
            files[row["path"]] = data
    require(sum(map(len, files.values())) == manifest["uncompressed_bytes"],
            "uncompressed size differs")
    require(files["tools/verify.py"] == Path(__file__).read_bytes(),
            "verifier differs from archived bytes")
    freeze = json.loads(files["batch/inputs-freeze-r1.json"])
    frozen = {row["path"]: row["sha256"] for row in freeze["inputs"]}
    require(len(frozen) == freeze["count"] == 719 and list(frozen) == sorted(frozen),
            "engineering inventory differs")
    aggregate = digest("".join(f"{path}\0{sha}\n" for path, sha in frozen.items()).encode())
    require(aggregate == freeze["aggregate_sha256"] == manifest["engineering_sha256"],
            "engineering aggregate differs")
    listing = (git("ls-tree", "-rz", "--name-only", args.source) if args.source else
               git("ls-files", "--cached", "--others", "--exclude-standard", "-z"))
    actual = sorted({name.decode() for name in listing.split(b"\0")
                     if name and not name.startswith(b"docs/")})
    require(actual == sorted(frozen), "source paths differ")
    for name in actual:
        data = git("show", f"{args.source}:{name}") if args.source else (ROOT / name).read_bytes()
        require(digest(data) == frozen[name], "source differs: " + name)
    snapshots = [name for name in files if name.startswith("batch/source-r1/")]
    require(len(snapshots) == 3, "expected three source snapshots")
    for name in snapshots:
        require(digest(files[name]) == frozen[name.removeprefix("batch/source-r1/")],
                "source snapshot differs")
    for name, expected_exit in (("scoped-gate-r1", 0), ("windows-cross-clippy-r1", 101)):
        receipt = json.loads(files[f"batch/{name}.json"])
        log = files[f"batch/{name}.log"]
        require(receipt["observed_exit"] == receipt["controller_exit"] == expected_exit
                and not receipt["timed_out"] and not receipt["interrupted"]
                and receipt["terminal"] == {
                    "direct_reaped": True, "active_processes": 0,
                    "pipes_drained": True, "errors": [],
                } and receipt["log_bytes"] == len(log)
                and receipt["log_sha256"] == digest(log), "command receipt differs")
    scoped = json.loads(files["batch/scoped-summary-r1.json"])
    require(scoped["engineering_sha256"] == aggregate and scoped["mismatches"] == []
            and scoped["inventories_match"] and len(scoped["listed_names"]) == 14
            and scoped["app_executed"] == {"passed": 13, "failed": 0, "ignored": 0}
            and scoped["host_session_executed"] == {"passed": 1, "failed": 0, "ignored": 0}
            and scoped["native_windows_executed"] == 0, "scoped summary differs")
    text = files["batch/scoped-gate-r1.log"].decode()
    listed = re.findall(r"^(\S+): test$", text, re.MULTILINE)
    executed = re.findall(r"^test (\S+) \.\.\. (ok|ignored|FAILED)$", text, re.MULTILINE)
    require(len(listed) == len(set(listed)) == len(executed) == 14
            and listed == scoped["listed_names"]
            and set(listed) == {name for name, _ in executed}
            and all(status == "ok" for _, status in executed),
            "compiled and executed inventories differ")
    ci = json.loads(files["ci/source-ci-summary-r1.json"])
    require(ci["source"] == "8c07d61009dba14f989dd0c00a2a370dc7a2d37b"
            and ci["run"] == 43848390542 and ci["status"] == "completed"
            and ci["conclusion"] == "failure", "prior CI identity differs")
    for job in ci["jobs"]:
        log = files[f"ci/{job['platform']}-job-log-r1.log"]
        require(len(log) == job["raw_bytes"] and digest(log) == job["raw_sha256"],
                "CI log identity differs")
    print(json.dumps({
        "source": args.source or "current", "members": len(files),
        "engineering_inputs": len(frozen), "engineering_sha256": aggregate,
        "source_mismatches": 0, "archive_mismatches": 0, "tests_replayed": 0,
        "prior_ci": "failure", "windows_native": "NOT_RUN",
    }))


if __name__ == "__main__":
    main()
