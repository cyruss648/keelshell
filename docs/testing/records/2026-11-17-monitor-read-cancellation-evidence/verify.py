"""Bind this batch's archived inputs and results to exact Git objects offline."""
import argparse
import io
import json
from pathlib import Path
import re
import runpy
import subprocess
import tarfile

MAIN = "docs/testing/records/2026-11-17-monitor-read-cancellation-evidence"
PREVIOUS = "docs/testing/records/2026-11-17-mcp-client-history-evidence"
SOURCE = "f910e0013832643f09b112b08443675a956d5551"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commit", required=True)
    options = parser.parse_args()
    assert re.fullmatch(r"[0-9a-f]{40}", options.commit)
    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10
    ).strip())
    helpers = runpy.run_path(str(root / PREVIOUS / "verify_git_binding.py"))
    blobs = helpers["git_blobs"]
    sha = helpers["digest"]

    def archive(directory):
        data = blobs(root, options.commit, [
            f"{directory}/originals.tar.gz", f"{directory}/manifest.json",
        ])
        raw = data[f"{directory}/originals.tar.gz"]
        manifest_raw = data[f"{directory}/manifest.json"]
        assert raw == (root / directory / "originals.tar.gz").read_bytes()
        assert manifest_raw == (root / directory / "manifest.json").read_bytes()
        manifest = json.loads(manifest_raw)
        assert len(raw) == manifest["bytes"] and sha(raw) == manifest["sha256"]
        rows = {row["member"]: row for row in manifest["files"]}
        assert len(rows) == len(manifest["files"]) == manifest["members"]
        contents = {}
        with tarfile.open(fileobj=io.BytesIO(raw)) as content:
            members = content.getmembers()
            assert len(members) == len(rows)
            assert {member.name for member in members} == set(rows)
            for member in members:
                assert member.isfile()
                value = content.extractfile(member).read()
                row = rows[member.name]
                assert len(value) == row["bytes"] and sha(value) == row["sha256"]
                contents[member.name] = value
        assert sum(len(value) for value in contents.values()) == manifest["logical_bytes"]
        return {
            "directory": directory, "members": len(contents),
            "sha256": manifest["sha256"],
        }, contents

    sealed = []
    main_archive, current = archive(MAIN)
    sealed.append(main_archive)
    candidate = json.loads(current["parent/candidate-inputs-r3.json"])
    current_source = helpers["verify_inputs"](root, options.commit, candidate, 777)
    r2 = json.loads(current["parent/candidate-inputs-r2.json"])
    assert [row["path"] for row in r2["inputs"]] == [
        row["path"] for row in candidate["inputs"]
    ]
    assert [new["path"] for old, new in zip(r2["inputs"], candidate["inputs"]) if old != new] == [
        "crates/keelshell-session/src/network_diagnostic/tests/test_traceroute.py"
    ]
    after = json.loads(current["parent/after-final-gate-r3.json"])
    assert after["inputs"] == candidate["inputs"]
    gate = json.loads(current["parent/final-gate-r3/result.json"])
    assert gate["exit_code"] == 0 and gate["reaped"] and not gate["timed_out"]
    inventory = json.loads(current["parent/final-gate-r3-inventory/summary.json"])
    assert sha(current["parent/final-gate-r3/command.log"]) == inventory["gate_log_sha256"]
    assert inventory["failed"] == 0 and inventory["all_inventory_names_match"]
    assert inventory["all_binary_digests_unchanged"]
    for directory in [
        f"{MAIN}/review", f"{MAIN}/fixture-review", f"{MAIN}/peer-diagnosis",
        f"{PREVIOUS}/ci-completion", f"{PREVIOUS}/ci-completion/review",
    ]:
        result, contents = archive(directory)
        sealed.append(result)
        if directory == f"{MAIN}/review":
            decision = json.loads(contents["review/decision.json"])
            assert decision["decision"] == "APPROVE"
            assert decision["source_aggregate_sha256"] == r2["aggregate_sha256"]
            assert decision["findings"] == {"P0": 0, "P1": 0, "P2": 0, "P3": 0}
        if directory == f"{MAIN}/fixture-review":
            decision = json.loads(contents["review/decision.json"])
            assert decision["decision"] == "APPROVE"
            assert decision["candidate"]["aggregate_sha256"] == candidate["aggregate_sha256"]
            assert decision["finding_counts"] == {"P0": 0, "P1": 0, "P2": 0, "P3": 0}
            assert decision["validation"]["passed"] == 14
            assert decision["validation"]["driver_reaped"]
            assert decision["validation"]["driver_natural_exit_code"] == 0
        if directory == f"{PREVIOUS}/ci-completion/review":
            decision = json.loads(contents["review/decision.json"])
            assert decision["engineering_sha"] == SOURCE
            assert decision["decision"] == "APPROVE_LIMITED_CI_ORIGINALS"
            assert decision["findings"] == {"P0": 0, "P1": 0, "P2": 0, "P3": 0}

    # The earlier CI is signed by its engineering SHA, never by the new HEAD.
    old_raw = blobs(root, SOURCE, [f"{PREVIOUS}/originals.tar.gz"])[
        f"{PREVIOUS}/originals.tar.gz"
    ]
    with tarfile.open(fileobj=io.BytesIO(old_raw)) as content:
        old_candidate = json.load(content.extractfile("parent/candidate-inputs-r1.json"))
    previous_source = helpers["verify_inputs"](root, SOURCE, old_candidate, 777)
    print(json.dumps({
        "commit": options.commit, "sources": [current_source, previous_source],
        "archives": sealed, "members": sum(item["members"] for item in sealed),
        "gate_exit": gate["exit_code"], "gate_reaped": gate["reaped"],
        "ordinary_rust_passed_failed_ignored": [
            inventory["passed"], inventory["failed"], inventory["ignored"],
        ],
        "ordinary_binary_inventories": inventory["suite_count"],
        "boundary": "Offline exact Git/archive binding. No test rerun, target access, network, GUI, Windows execution or release acceptance.",
    }))


if __name__ == "__main__":
    main()
