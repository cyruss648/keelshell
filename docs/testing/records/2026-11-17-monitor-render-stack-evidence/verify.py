"""Verify the frozen UI extraction and recorded results without test replay."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Read an exact Git source instead of working files")
    args = parser.parse_args()
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert digest(raw) == manifest["archive_sha256"]
    with tarfile.open(BASE / "originals.tar.gz") as archive:
        members = archive.getmembers()
        assert all(member.isfile() for member in members)
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"] == 192
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and digest(data) == row["sha256"]

    def document(name):
        return json.loads(files[name])

    for label in ("format-r2", "compile-app-r2", "scoped-gate-r2",
                  "app-gate-inventory-r2", "release-build-r2",
                  "package-precommit-r2", "package-precommit-inspect-r2"):
        receipt = document(label + ".json")
        assert receipt["controller_exit"] == receipt["observed_exit"] == 0
        assert receipt["terminal"]["direct_reaped"]
        assert receipt["terminal"]["active_processes"] == 0
        assert receipt["terminal"]["pipes_drained"] and not receipt["terminal"]["errors"]
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"] and digest(raw) == receipt["log_sha256"]
    stopped = document("scoped-gate-r1.json")
    assert stopped["controller_exit"] == 130 and stopped["observed_exit"] is None
    assert stopped["interrupted"] and stopped["terminal"]["direct_reaped"]
    assert stopped["terminal"]["active_processes"] == 0 and stopped["terminal"]["pipes_drained"]

    freeze = document("inputs-freeze-r2.json")
    assert freeze["count"] == len(freeze["inputs"]) == 731
    assert digest("".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode()) == freeze["aggregate_sha256"]
    for name in ("gate-inputs-after-r2.json", "release-inputs-after-r2.json"):
        assert document(name)["inputs"] == freeze["inputs"]
    gate = document("gate-summary-r2.json")
    names = re.findall(r"^(\S+): test$",
                       files["app-gate-inventory-r2.log"].decode(), re.M)
    results = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:,.*)?$",
                         files["scoped-gate-r2.log"].decode(), re.M)
    assert len(names) == len(set(names)) == len(results) == len(dict(results)) == 1377
    assert set(names) == set(dict(results)) and dict(results) == dict(gate["results"])
    assert [sum(outcome == value for _, outcome in results)
            for value in ("ok", "FAILED", "ignored")] == [1375, 0, 2]
    assert gate["configured_rust_min_stack"] == 2097152
    assert gate["actual_binary"] == document("binaries-r2.json")
    assert all(dict(results)[name] == "ok" for name in gate["critical_regressions"])

    source = document("source-review/commands-r2.json")
    assert source["r2_count"] == 731 and source["r2_aggregate"] == freeze["aggregate_sha256"]
    assert set(source["finding_counts"].values()) == {0}
    assert source["final_frozen_inputs_unchanged"]
    function = document("function-review/r2/review-summary-r2.json")
    assert function["recommendation"] == "APPROVE_SCOPED_R2"
    assert set(function["findings"].values()) == {0}
    assert function["freeze_count"] == 731
    assert function["freeze_aggregate_sha256"] == freeze["aggregate_sha256"]
    assert function["independent_compilations"] == function["shared_target_accesses"] == 0
    assert all(function["identity_checks"].values())
    assert function["controlled_child_exit_codes_r2"] == [0] * 12
    independent = re.findall(
        r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:,.*)?$",
        files["function-review/r2/monitor-2m-r2.stdout.log"].decode(), re.M,
    )
    expected = {name for name in names if name.startswith("monitor::")}
    assert len(independent) == len(dict(independent)) == len(expected) == 78
    assert set(dict(independent)) == expected and {value for _, value in independent} == {"ok"}
    assert function["monitor"]["passed"] == 78 and function["monitor"]["RUST_MIN_STACK"] == "2097152"

    prologue = document("stack-prologues-r3.json")
    assert len(prologue["functions"]) == len({row["file"] for row in prologue["functions"]}) == 8
    for row in prologue["functions"]:
        assert len(files[row["file"]]) == row["bytes"]
        assert digest(files[row["file"]]) == row["sha256"]
    assert prologue["original_render_frame_bytes"] == 1576336
    assert prologue["current_render_frame_bytes"] == 70992
    cleanup = document("review-prebuilt-cleanup-r1.json")
    assert cleanup["removed_files"] == 2 and cleanup["removed_bytes"] == 548027216
    assert not cleanup["shared_target_modified"]
    assert all(row["closed"] for row in document("agent-lifecycle-r1.json")["agents"])
    package = document("normal-package-precommit-r2.json")
    assert package["manifest"]["build"]["commit"] is None
    assert len(package["files"]) == 6
    assert package["aggregate_sha256"] == freeze["aggregate_sha256"]
    assert not any(package[key] for key in (
        "native_desktop_acceptance", "launched", "installed", "signed", "notarized",
    ))

    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10,
    ).strip())
    command = (["git", "ls-tree", "-r", "--name-only", "-z", args.source]
               if args.source else ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"])
    actual_names = [
        name.decode() for name in subprocess.check_output(command, cwd=root, timeout=20).split(b"\0")
        if name and not name.startswith((b"docs/", b".hallmark/"))
    ]
    assert sorted(set(actual_names)) == [row["path"] for row in freeze["inputs"]]

    def verify_input(row):
        raw = (subprocess.check_output(
            ["git", "show", f"{args.source}:{row['path']}"], cwd=root, timeout=15,
        ) if args.source else (root / row["path"]).read_bytes())
        assert len(raw) == row["bytes"] and digest(raw) == row["sha256"], row["path"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    print(json.dumps({
        "source": args.source or "working-tree", "members": len(files),
        "engineering_inputs": 731, "aggregate_sha256": freeze["aggregate_sha256"],
        "app": {"passed": 1375, "failed": 0, "ignored": 2},
        "independent_prebuilt": {"passed": 78, "failed": 0, "ignored": 0},
        "tests_replayed": 0, "mismatches": [], "native_desktop_acceptance": False,
        "fixed_source_linux_windows_ci": "not certified by this record",
    }))


if __name__ == "__main__":
    main()
