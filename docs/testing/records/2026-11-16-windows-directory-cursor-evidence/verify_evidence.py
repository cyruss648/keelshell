"""Check every retained original and bind final and failed inputs separately."""
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
PREFIX = BASE.relative_to(ROOT).as_posix() + "/"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, timeout=30)


def check_freeze(freeze):
    assert len(freeze["inputs"]) == freeze["count"]
    assert len({row["path"] for row in freeze["inputs"]}) == freeze["count"]
    aggregate = "".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode()
    assert digest(aggregate) == freeze["aggregate_sha256"]


def check_gate(archive, label, expected):
    receipt = json.loads(archive.extractfile(f"batch/{label}.json").read())
    log = archive.extractfile(f"batch/{label}.log").read()
    assert len(log) == receipt["log_bytes"] and digest(log) == receipt["log_sha256"]
    assert receipt["observed_exit"] == receipt["controller_exit"] == expected
    assert not receipt["timed_out"] and not receipt["interrupted"]
    assert receipt["terminal"] == {
        "direct_reaped": True, "active_processes": 0,
        "pipes_drained": True, "errors": [],
    }
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", action="store_true")
    parser.add_argument("--source")
    parser.add_argument("--evidence")
    args = parser.parse_args()
    for name in ("source", "evidence"):
        value = getattr(args, name)
        if value and not re.fullmatch("[0-9a-f]{40}", value):
            parser.error(name + " must be a complete commit SHA")
    if args.index and args.evidence:
        parser.error("--index and --evidence are mutually exclusive")
    revision = ":" if args.index else args.evidence + ":" if args.evidence else None

    def read(name):
        return git("show", revision + PREFIX + name) if revision else (BASE / name).read_bytes()

    manifest = json.loads(read("archive-manifest.json"))
    packed = read(manifest["archive"])
    assert len(packed) == manifest["archive_bytes"]
    assert digest(packed) == manifest["archive_sha256"]
    rows = manifest["files"]
    assert len(rows) == manifest["count"] == len({row["path"] for row in rows})
    assert sum(row["bytes"] for row in rows) == manifest["original_bytes"]
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert len(members) == len(rows) and all(member.isfile() for member in members)
        assert {member.name for member in members} == {row["path"] for row in rows}
        for row in rows:
            name = row["path"]
            assert not PurePosixPath(name).is_absolute() and ".." not in PurePosixPath(name).parts
            data = archive.extractfile(name).read()
            assert len(data) == row["bytes"] and digest(data) == row["sha256"], name
        final = json.loads(archive.extractfile("batch/inputs-freeze-r2.json").read())
        previous = json.loads(archive.extractfile("batch/inputs-freeze-r1.json").read())
        check_freeze(final)
        check_freeze(previous)
        assert final["count"] == manifest["engineering_inputs"]
        assert final["aggregate_sha256"] == manifest["engineering_sha256"]
        check_gate(archive, "full-gate-r1", 1)
        check_gate(archive, "full-gate-r2", 0)
        # The failed first gate preceded both app test fixes. Its exact inputs
        # are the original Git base plus the author's two Windows candidates.
        old_base = manifest["base_commit"]
        assert re.fullmatch("[0-9a-f]{40}", old_base)
        replacements = {
            "crates/keelshell-platform/src/windows/" + name
            for name in ("native.rs", "tests.rs")
        }
        for row in previous["inputs"]:
            name = row["path"]
            data = archive.extractfile("batch/author/candidate/" + name).read() if (
                name in replacements
            ) else git("show", old_base + ":" + name)
            assert digest(data) == row["sha256"], ("first-gate-input", name)
        for review in manifest["reviews"]:
            assert digest(archive.extractfile(review["path"]).read()) == review["sha256"]
        expected = json.loads(archive.extractfile("batch/windows-expected-inventory-r1.json").read())
        assert expected["expected_count"] == len(set(expected["expected_names"])) == 67
        assert expected["new_windows_execution"] == "NOT_RUN"

    source_revision = args.source + ":" if args.source else ":" if args.index else None
    if args.source:
        paths = git("ls-tree", "-r", "--name-only", "-z", args.source).split(b"\0")
    elif args.index:
        paths = git("ls-files", "--cached", "-z").split(b"\0")
    else:
        paths = git("ls-files", "--cached", "--others", "--exclude-standard", "-z").split(b"\0")
    actual_paths = {
        name.decode() for name in paths
        if name and not name.startswith(b"docs/")
        and (source_revision or (ROOT / name.decode()).is_file())
    }
    assert actual_paths == {row["path"] for row in final["inputs"]}
    for row in final["inputs"]:
        data = git("show", source_revision + row["path"]) if source_revision else (
            ROOT / row["path"]
        ).read_bytes()
        assert digest(data) == row["sha256"], ("final-input", row["path"])
    print(json.dumps({
        "evidence": "index" if args.index else args.evidence or "current",
        "source": args.source or ("index" if args.index else "current"),
        "originals": len(rows), "engineering_inputs": final["count"],
        "first_gate_exit": 1, "final_gate_exit": 0,
        "windows_actual_execution": 0, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
