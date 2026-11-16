"""Recompute archived receipts and exact source binding without replaying tests."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

import summarize_tests

BASE = Path(__file__).resolve().parent


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Bind to an exact Git source rather than working files")
    args = parser.parse_args()
    manifest = json.loads((BASE / "manifest.json").read_text())
    compressed = (BASE / "originals.tar.gz").read_bytes()
    assert len(compressed) == manifest["archive_bytes"]
    assert digest(compressed) == manifest["archive_sha256"]
    with tarfile.open(BASE / "originals.tar.gz") as archive:
        members = archive.getmembers()
        assert all(
            member.isfile()
            and not PurePosixPath(member.name).is_absolute()
            and ".." not in PurePosixPath(member.name).parts
            for member in members
        )
        files = {member.name: archive.extractfile(member).read() for member in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and digest(data) == row["sha256"], row["path"]

    def document(name):
        return json.loads(files[name])

    def integration(name):
        return document("integration/" + name)

    def command(label, expected=0):
        receipt = integration(label + ".json")
        assert receipt["controller_exit"] == receipt["observed_exit"] == expected, label
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0, label
        assert terminal["pipes_drained"] and not terminal["errors"], label
        log = files["integration/" + label + ".log"]
        assert len(log) == receipt["log_bytes"] and digest(log) == receipt["log_sha256"]
        return receipt

    for label in (
        "full-gate-r3", "normal-release-build-r1", "package-r1",
        "package-inspection-r1", "darwin-openssh-r2", "linux-recovery-r2",
        "channel-admission-regression-r1", "linux-fixture-regression-r1",
        "app-inventory-final-r3", "session-inventory-final-r3",
        "windows-routing-inventory-final-r3",
    ):
        command(label)
    for label in ("full-gate-r1", "full-gate-r2", "linux-recovery-r1"):
        command(label, expected=1)

    freeze = integration("full-gate-inputs-r3.json")
    assert freeze["count"] == len(freeze["inputs"]) == 758
    aggregate = digest("".join(
        f"{row['path']}\0{row['sha256']}\n" for row in freeze["inputs"]
    ).encode())
    assert aggregate == freeze["aggregate_sha256"]
    assert integration("full-gate-inputs-after-r3.json")["inputs"] == freeze["inputs"]
    gate_raw = files["integration/full-gate-r3.log"]
    actual_gate = summarize_tests.summarize(gate_raw)
    recorded_gate = integration("gate-summary-r3.json")
    assert {key: value for key, value in recorded_gate.items() if key != "log"} == actual_gate
    suites = actual_gate["suites"]
    assert all(row["summary"]["failed"] == 0 for row in suites)
    assert all(row["summary"]["failed"] == 0
               for row in actual_gate["doctest_runs_unassigned"])
    inventory = integration("inventory-bindings-r3.json")
    for label, entry in inventory["targets"].items():
        listed = re.findall(
            r"^(\S+): test$",
            files["integration/" + entry["inventory_label"] + ".log"].decode(), re.M,
        )
        rows = [row for row in suites if row["suite"] == entry["suite"]]
        assert len(rows) == 1, label
        results = rows[0]["results"]
        assert len(listed) == len(set(listed)) == len(results) == len(dict(results))
        assert set(listed) == set(dict(results)), label
        assert len(listed) == entry["compiled_test_count"]
        assert all(dict(results)[name] == "ok" for name in entry["critical_regressions"])

    python_section = gate_raw.decode().split(
        "Running: cargo fmt --all --check", 1
    )[0]
    python_match = re.search(
        r"Ran (\d+) tests in [\d.]+s\s+OK \(skipped=(\d+)\)", python_section
    )
    assert python_match and tuple(map(int, python_match.groups())) == (405, 2)
    for marker in ("--readiness-error-tests", "--observer-io-error-tests",
                   "--controller-small-stack"):
        assert marker in gate_raw.decode(), marker

    review = document("function-review/final-review-receipt.json")
    assert review["decision"] == "APPROVE"
    assert all(value == 0 for value in review["open_findings"].values())
    assert review["final_freeze_aggregate"] == aggregate
    assert review["engineering_files_rechecked"] == 758 and not review["engineering_drift"]
    assert review["independent_compilations"] == 0
    assert review["app_prebuilt_r3"] == [101, 0, 0]
    assert review["routing_prebuilt_r3"] == [9, 0, 1]
    assert review["python_suite"] == [111, 0, 0]
    assert review["post_prebuilt_helper_changes_statically_reviewed"] == 3
    assert digest(files["function-review/最终报告.md"]) == review["report_sha256"]
    for name, expected in review["summary_receipt_sha256"].items():
        assert digest(files["function-review/" + name]) == expected
    controls = document("function-review/oracle-omission-controls-r2.stdout.json")
    assert controls["cases"] == 11 and controls["all_expected_outcomes"]
    assert [controls[key] for key in (
        "omission_cases_rejected", "honest_or_skip_success_cases",
        "uncertain_inventory_hold_cases",
    )] == [5, 3, 3]
    assert controls["windows_api_executions"] == 0
    assert integration("agent-lifecycle-r3.json")["all_owned_agents_closed"]
    cleanup = integration("prebuilt-cleanup-r1.json")
    assert cleanup["removed_files"] == 6 and cleanup["removed_bytes"] == 886506992
    assert cleanup["all_paths_absent"] and not cleanup["shared_target_modified"]
    assert integration("release-final-input-comparison-r3.json")["production_inputs_unchanged"]
    native = integration("native-availability-r2.json")
    assert native["gui_actions"] == native["screenshots"] == 0
    assert not native["native_desktop_acceptance"]

    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10
    ).strip())
    argv = (["git", "ls-tree", "-r", "--name-only", "-z", args.source] if args.source
            else ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"])
    names = sorted({
        name.decode() for name in subprocess.check_output(argv, cwd=root, timeout=30).split(b"\0")
        if name and not name.startswith((b"docs/", b".hallmark/"))
    })
    assert names == [row["path"] for row in freeze["inputs"]]

    def verify_input(row):
        data = (subprocess.check_output(
            ["git", "show", f"{args.source}:{row['path']}"], cwd=root, timeout=20
        ) if args.source else (root / row["path"]).read_bytes())
        assert len(data) == row["bytes"] and digest(data) == row["sha256"], row["path"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    source_map = {row["path"]: row for row in freeze["inputs"]}
    selection = document("selection.json")
    for reference in selection["source_snapshot_references"]:
        if reference["kind"] == "archive":
            data = files[reference["canonical"]]
            assert len(data) == reference["bytes"] and digest(data) == reference["sha256"]
        else:
            assert reference["kind"] == "git"
            canonical = reference["canonical"]
            if canonical in source_map:
                expected = source_map[canonical]
                assert (expected["bytes"], expected["sha256"]) == (
                    reference["bytes"], reference["sha256"]
                )
            else:
                verify_input({
                    "path": canonical, "bytes": reference["bytes"],
                    "sha256": reference["sha256"],
                })
    print(json.dumps({
        "source": args.source or "working-tree", "members": len(files),
        "engineering_inputs": len(freeze["inputs"]), "aggregate_sha256": aggregate,
        "ordinary_rust": {
            key: sum(row["summary"][key] for row in suites)
            for key in ("passed", "failed", "ignored")
        },
        "python_discovery": {"run": 405, "skipped": 2},
        "tests_replayed": 0, "mismatches": [],
        "native_desktop_acceptance": False,
        "actual_windows_api_and_ssh": "not certified by this record",
    }))


if __name__ == "__main__":
    main()
