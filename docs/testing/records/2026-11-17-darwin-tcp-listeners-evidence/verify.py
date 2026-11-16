"""Verify the sealed listener batch without replaying Cargo or tests."""

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


def sha(data):
    return hashlib.sha256(data).hexdigest()


def gate_summary(full, inventory, new_app, new_session):
    workspace = full.split("Running: cargo test --workspace", 1)[1].split(
        "Running: cargo test -p keelshell-ai", 1
    )[0]
    headers = list(re.finditer(
        r"^\s*((?:Running (?:unittests |tests/)|Doc-tests ).+)$", workspace, re.M
    ))
    suites = []
    controllers = []
    app = None
    session_new = set()
    for index, header in enumerate(headers):
        section = workspace[
            header.end():headers[index + 1].start() if index + 1 < len(headers) else None
        ]
        summaries = re.findall(
            r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;",
            section,
        )
        rustdoc = header[1].startswith("Doc-tests ")
        if not summaries:
            markers = {
                "local_agent_directory_process.rs":
                    "directory owned process controller: 20 cases passed",
                "local_agent_process.rs":
                    "local agent process integration: all assertions/scenario groups passed; no supplier CLI or model calls",
            }
            matched = [
                name for name, marker in markers.items()
                if f"Running tests/{name} " in header[1] and marker in section
            ]
            assert len(matched) == 1, header[1]
            controllers.extend(matched)
            continue
        assert rustdoc or len(summaries) == 1, header[1]
        for status, passed, failed, ignored in summaries:
            assert status == "ok" and int(failed) == 0, header[1]
            suites.append({
                "label": header[1], "passed": int(passed), "failed": int(failed),
                "ignored": int(ignored), "rustdoc": rustdoc,
            })
        if rustdoc:
            continue
        # ZipWriter's expected budget diagnostic can precede a libtest result
        # on the same line. Keep the raw log and still account for that result.
        executed = re.findall(
            r"\btest (\S+) \.\.\. (ok|ignored)(?:,.*)?$", section, re.M
        )
        assert len(executed) == len(set(name for name, _ in executed)), header[1]
        assert len(executed) == int(passed) + int(ignored), header[1]
        assert re.findall(r"^running (\d+) tests?$", section, re.M) == [
            str(len(executed))
        ], header[1]
        outcomes = dict(executed)
        if "keelshell_app-" in header[1]:
            assert app is None
            assert sorted(outcomes) == sorted(inventory)
            assert all(outcomes[name] == "ok" for name in new_app)
            app = {
                "inventory": len(inventory), "passed": int(passed),
                "failed": int(failed), "ignored": int(ignored),
            }
        if "keelshell_session-" in header[1]:
            assert all(outcomes[name] == "ok" for name in new_session)
            session_new.update(new_session)
    assert app is not None and session_new == set(new_session)
    assert sorted(controllers) == [
        "local_agent_directory_process.rs", "local_agent_process.rs"
    ]
    python = full.split(" -m unittest discover -s scripts -p test_*.py", 1)[1].split(
        "Running: cargo fmt", 1
    )[0]
    assert re.findall(r"Ran (\d+) tests? in", python) == ["293"]
    assert "OK (skipped=2)" in python
    assert re.findall(r"Ran (\d+) tests? in", full) == ["293", "30"]
    for name in ("readiness-error-tests", "observer-io-error-tests", "controller-small-stack"):
        assert name in full
    normal = [suite for suite in suites if not suite["rustdoc"]]
    docs = [suite for suite in suites if suite["rustdoc"]]
    return {
        "ordinary_suites": len(normal),
        "ordinary_rust": {
            key: sum(suite[key] for suite in normal)
            for key in ("passed", "failed", "ignored")
        },
        "rustdoc": {
            key: sum(suite[key] for suite in docs)
            for key in ("passed", "failed", "ignored")
        },
        "app": app, "new_app_names_passed": len(new_app),
        "new_session_names_passed": len(new_session),
        "workspace_harness_free_controllers": controllers,
        "python_run": 293, "python_skipped": 2,
        "archive_helper_python_run": 30,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Verify a full Git SHA instead of the working tree")
    args = parser.parse_args()
    if args.source:
        assert re.fullmatch(r"[0-9a-f]{40}", args.source)

    def read(path):
        return (
            subprocess.check_output(
                ["git", "show", f"{args.source}:{path.relative_to(ROOT).as_posix()}"],
                cwd=ROOT, timeout=30,
            ) if args.source else path.read_bytes()
        )

    manifest = json.loads(read(BASE / "manifest.json"))
    packed = read(BASE / "originals.tar.gz")
    assert len(packed) == manifest["archive_bytes"]
    assert sha(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        for member in archive.getmembers():
            name = PurePosixPath(member.name)
            assert member.isfile() and not name.is_absolute() and ".." not in name.parts
            assert member.name not in files
            files[member.name] = archive.extractfile(member).read()
    assert len(files) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and sha(data) == row["sha256"], row["path"]

    def document(name):
        return json.loads(files[name])

    def run(label, exit_code=0):
        receipt = document(label + ".json")
        assert receipt["observed_exit"] == receipt["controller_exit"] == exit_code, label
        assert not receipt["timed_out"] and not receipt["interrupted"], label
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0, label
        assert terminal["pipes_drained"] and not terminal["errors"], label
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"] and sha(raw) == receipt["log_sha256"], label
        return raw.decode()

    freeze = document("inputs-freeze-r4.json")
    expected = {row["path"]: row for row in freeze["inputs"]}
    assert len(expected) == freeze["count"] == 724
    assert list(expected) == sorted(expected)
    assert sha("".join(
        f"{name}\0{row['sha256']}\n" for name, row in expected.items()
    ).encode()) == freeze["aggregate_sha256"]
    command = (
        ["git", "ls-tree", "-rz", "--name-only", args.source] if args.source else
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]
    )
    names = sorted({
        name.decode() for name in subprocess.check_output(command, cwd=ROOT, timeout=60).split(b"\0")
        if name and not name.startswith((b"docs/", b".hallmark/"))
    })
    assert names == list(expected), "engineering input set differs"
    for name in names:
        raw = read(ROOT / name)
        assert len(raw) == expected[name]["bytes"] and sha(raw) == expected[name]["sha256"], name

    inventory = re.findall(
        r"^(\S+): test$", run("gate-app-inventory-r1"), re.M
    )
    assert len(inventory) == len(set(inventory)) == 1369
    binary = document("test-binaries-r4.json")
    review = document("function-review/codex-function-20261117/execution-summary.json")
    new_app = binary["new_app_names"]
    new_session = list(review["runs"]["session"]["required_new_tests"])
    assert len(new_app) == 5 and len(new_session) == 21
    full = run("full-gate-r1")
    assert Path(document("gate-app-inventory-r1.json")["command"][0]).name in full
    summary = gate_summary(full, inventory, new_app, new_session)
    assert summary == document("gate-summary-r1.json")
    assert review["valid"] and review["freeze_before_after_valid"]
    assert review["aggregate"] == freeze["aggregate_sha256"]
    assert review["cargo_invocations"] == review["target_writes"] == 0
    assert review["parent_prebuilt"] and not review["independent_compilation"]
    for key, count in (("app", 75), ("session", 102)):
        result = review["runs"][key]
        assert result["actual_passed"] == count
        assert result["actual_failed"] == result["actual_ignored"] == 0
        assert not result["duplicates"] and not result["missing"] and not result["unexpected"]
    assert "102 passed; 0 failed; 0 ignored;" in run("session-monitor-r4")
    assert "Ran 27 tests" in run("forcecommand-contract-r3")
    assert "running 0 tests" in run("app-darwin-r1")
    assert "9 passed; 0 failed; 0 ignored;" in run("app-darwin-r2")
    assert "1 failed;" in run("missing-frame-negative-r1", 101)
    assert "101 passed; 1 failed;" in run("session-monitor-r3", 101)
    assert document("dependency-followup-report-r1.json")["status"] == "passed"
    assert len(document("dependency-followup-report-r1.json")["rust"]) == 113
    for revision, passed, outcome in (("r1", 6, "failed"), ("r2", 9, "passed"), ("r4", 9, "passed")):
        result = document(f"openssh-darwin-{revision}/result.json")
        assert result["outcome"] == outcome, revision
        if outcome == "passed":
            assert result["passed_tests"] == passed
        else:
            assert "6 passed; 2 failed;" in files[f"openssh-darwin-{revision}/tests.log"].decode()
        assert result["cleanup"]["owned_processes_stopped"]
        assert result["cleanup"]["temporary_directory_removed"]
    linux = document("linux-recovery-r4/result.json")
    assert linux["status"] == "PASSED" and len(linux["tests"]) == 2
    assert all(test["status"] == "PASSED" for test in linux["tests"])
    assert all(row["absent"] and row["absence_observed"] for row in linux["cleanup"]["resources"])
    binding = document("linux-recovery-binding-r4.json")
    assert binding["source_sha_argument_is_base_head_not_committed_working_diff"]
    assert binding["base_head"] == linux["source_sha"] == freeze["base"]
    assert binding["dirty_source_aggregate"] == freeze["aggregate_sha256"]
    assert binding["test_binary"]["sha256"] == binary["binaries"]["linux_recovery"]["sha256"]
    cleanup = document("linux-base-cleanup-after.json")
    for key in ("owned_base_image_absent", "owned_intermediate_parent_absent", "protected_public_base_present"):
        assert cleanup[key]
    assert cleanup["owned_containers_remaining"] == 0
    native = document("native-availability-r1.json")
    assert native["gui_actions"] == native["screenshots"] == 0
    assert native["native_acceptance"] == "NOT_RUN"
    for label in ("release-build-r1", "package-candidate-r1", "package-inspect-r1"):
        run(label)
    package = document("normal-package-candidate-manifest-r1.json")
    assert package["target"] == "aarch64-apple-darwin"
    assert package["build"]["commit"] is None
    assert not package["installed"] and not package["signed_by_packaging_script"]
    assert package["files"]["KeelShell.app/Contents/MacOS/keelshell-app"] == package["binary_sha256"]
    assert package["files"]["KeelShell.app/Contents/MacOS/keelshell-mcp"] == package["mcp_binary_sha256"]
    inspection = files["package-inspect-r1.log"].decode()
    assert inspection.count("minos 15.0") == 2
    assert "aarch64-apple-darwin; GUI acceptance remains separate" in inspection
    print(json.dumps({
        "source": args.source or "working tree", "source_inputs": len(names),
        "aggregate_sha256": freeze["aggregate_sha256"], "mismatches": 0,
        "sealed_members": len(files), **summary,
        "independent_prebuilt": {"app_passed": 75, "session_passed": 102},
        "real_darwin_openssh_passed": 9, "real_linux_recovery_passed": 2,
        "normal_package": {
            "gui_sha256": package["binary_sha256"],
            "mcp_sha256": package["mcp_binary_sha256"],
            "precommit_build_commit": None,
        },
        "tests_replayed": 0, "native_desktop_accepted": False,
    }))


if __name__ == "__main__":
    main()
