"""Verify sealed editor evidence and exact engineering inputs without test replay."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import types

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--source", help="Compare with this exact Git commit instead of the working tree")
    args = cli.parse_args()
    if args.source:
        assert re.fullmatch(r"[0-9a-f]{40}", args.source)

    def read(path):
        return subprocess.check_output(
            ["git", "show", f"{args.source}:{path.relative_to(ROOT).as_posix()}"],
            cwd=ROOT, timeout=30,
        ) if args.source else path.read_bytes()

    manifest = json.loads(read(BASE / "manifest.json"))
    packed = read(BASE / "originals.tar.gz")
    assert len(packed) == manifest["archive_bytes"]
    assert digest(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        for member in archive:
            name = PurePosixPath(member.name)
            assert member.isfile() and not name.is_absolute() and ".." not in name.parts
            assert member.name not in files
            files[member.name] = archive.extractfile(member).read()
    assert len(files) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        assert len(files[row["path"]]) == row["bytes"]
        assert digest(files[row["path"]]) == row["sha256"]

    def document(name):
        return json.loads(files[name])

    def command(label, code=0, timed_out=False):
        receipt = document(label + ".json")
        assert receipt["controller_exit"] == code
        assert receipt["timed_out"] == timed_out and not receipt["interrupted"]
        if not timed_out:
            assert receipt["observed_exit"] == code
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0
        assert terminal["pipes_drained"] and not terminal["errors"]
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"] and digest(raw) == receipt["log_sha256"]
        return raw

    freeze = document("inputs-freeze-r4.json")
    assert freeze["count"] == len(freeze["inputs"]) == 727
    expected = {row["path"]: row for row in freeze["inputs"]}
    assert list(expected) == sorted(expected)
    assert digest("".join(
        f"{path}\0{row['sha256']}\n" for path, row in expected.items()
    ).encode()) == freeze["aggregate_sha256"]
    names = sorted({
        name.decode() for name in subprocess.check_output(
            ["git", "ls-tree", "-rz", "--name-only", args.source] if args.source else
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
            cwd=ROOT, timeout=60,
        ).split(b"\0")
        if name and not name.startswith((b"docs/", b".hallmark/"))
    })
    assert names == list(expected), "engineering input set differs"
    for name in names:
        raw = read(ROOT / name)
        assert len(raw) == expected[name]["bytes"] and digest(raw) == expected[name]["sha256"], name
    assert document("release-inputs-after-r2.json")["inputs"] == freeze["inputs"]
    assert document("gate-inputs-before-r1.json")["inputs"] == freeze["inputs"]
    assert document("gate-inputs-after-r1.json")["inputs"] == freeze["inputs"]

    parser_module = types.ModuleType("sealed_editor_test_parser")
    exec(compile(files["summarize_tests.py"], "sealed_editor_test_parser", "exec"),
         parser_module.__dict__)
    gate_module = types.ModuleType("sealed_editor_gate")
    gate_module.__file__ = str(BASE / "summarize_gate.py")
    # The sealed helper uses this parser; no archived main block is executed.
    gate_source = files["summarize_gate.py"].decode()
    body = gate_source[gate_source.index("def derive("):gate_source.index(
        '\n\nif __name__ == "__main__":'
    )]
    gate_module.__dict__.update(parser=parser_module, hashlib=hashlib, re=re)
    exec(compile(body, "sealed_editor_gate", "exec"), gate_module.__dict__)
    focused = document("focused-summary-r4.json")
    handoff = document("author-handoff.json")
    summary, suites = gate_module.derive(
        command("full-gate-r1"), command("gate-app-inventory-r1"), focused, handoff,
    )
    assert summary == document("gate-summary-r1.json")
    assert suites == document("gate-suites-r1.json")
    assert focused["session"]["passed"] == 30 and focused["app"]["passed"] == 5
    assert focused["app"]["names_equal"] and focused["session"]["names_equal"]
    assert "30 passed; 0 failed; 0 ignored;" in command("session-backup-r4").decode()
    assert "5 passed; 0 failed; 0 ignored;" in command("app-backup-r4").decode()

    dependencies = document("dependencies-report-r3.json")
    assert dependencies["status"] == "passed"
    assert len(dependencies["rust"]) == 113
    assert len({row["package"] for row in dependencies["rust"]}) == 37
    command("dependencies-r3")
    command("dependencies-r1", 124, timed_out=True)
    command("dependencies-r2", 1)
    command("build-test-inventory-r1", 101)
    assert "29 passed; 1 failed;" in command("session-backup-r1", 101).decode()
    assert "2 passed; 2 failed;" in command("app-backup-r2", 101).decode()
    assert "running 0 tests" in command("tooltip-negative-r2").decode()
    negative = command("tooltip-negative-r5", 101).decode()
    assert "running 1 test" in negative and "0 passed; 1 failed;" in negative
    assert "editor_backup_tests.rs:324:" in negative

    interop = document("openssh-r1/result.json")
    assert interop["outcome"] == "passed" and interop["passed_tests"] == 18
    assert interop["cleanup"]["owned_processes_stopped"]
    assert interop["cleanup"]["temporary_directory_removed"]
    assert not interop["cleanup"]["ancestry_unverified"]
    command("openssh-r1")
    binaries = document("test-binaries-r4.json")
    old_binaries = document("test-binaries-r3.json")
    gate_binaries = document("gate-test-binaries-r1.json")
    assert gate_binaries["gate_log_sha256"] == summary["full_log_sha256"]
    assert not gate_binaries["cargo_recompiled_for_inventory"]
    assert len(gate_binaries["binaries"]) == 3
    gate_text = files["full-gate-r1.log"].decode()
    gate_cwd = Path(document("full-gate-r1.json")["cwd"])
    for row in gate_binaries["binaries"]:
        original = next(item for item in binaries["binaries"] if item["target"] == row["target"])
        assert row["matches_r4"]
        assert all(row[key] == original[key] for key in ("path", "bytes", "sha256"))
        assert f"({Path(row['path']).relative_to(gate_cwd).as_posix()})" in gate_text
    app_binary = next(row for row in gate_binaries["binaries"] if row["target"] == "keelshell-app")
    assert document("gate-app-inventory-r1.json")["command"] == [app_binary["path"], "--list"]
    for name in ("keelshell_session", "openssh_interop"):
        current = next(row for row in binaries["binaries"] if row["target"] == name)
        previous = next(row for row in old_binaries["binaries"] if row["target"] == name)
        assert current["sha256"] == previous["sha256"]

    source_review = document("source-review/followup-receipt.json")
    assert source_review["recommendation"] == "APPROVE"
    assert source_review["open_severity_counts"] == dict.fromkeys(("P0", "P1", "P2", "P3"), 0)
    assert source_review["fixed_inputs"]["all_rows_equal_before_and_after"]
    assert source_review["fixed_inputs"]["expected_aggregate_sha256"] == freeze["aggregate_sha256"]
    assert source_review["execution"]["test_executions"] == 0
    assert source_review["original_review"]["both_original_files_unchanged"]
    for name, row in source_review["original_review"]["after"].items():
        assert digest(files["source-review/" + name]) == row["sha256"]

    function_review = document("function-review/followup-r4/receipt.json")
    assert function_review["verdict"] == "APPROVE"
    assert function_review["finding_counts"] == dict.fromkeys(("P0", "P1", "P2", "P3"), 0)
    assert function_review["binding"]["aggregate_sha256"] == freeze["aggregate_sha256"]
    assert function_review["binding"]["all_engineering_inputs_and_all_three_binary_hashes_unchanged"]
    assert function_review["binding"]["independent_list_equals_parent_r4_list"]
    assert not function_review["binding"]["independent_compilation"]
    assert function_review["binding"]["parent_prebuilt"]
    execution = function_review["execution"]
    assert (execution["full_inventory"], execution["passed"], execution["failed"],
            execution["ignored"]) == (1374, 88, 0, 0)
    assert all(dict(execution["results"])[name] == "ok"
               for name in focused["app"]["required_new_tests"])
    for name in ("app-list", "app-editor"):
        command("function-review/followup-r4/" + name)
    assert function_review["prior_review"]["verdict_still"] == "REQUEST_CHANGES"
    assert function_review["prior_review"]["all_r3_evidence_files_unchanged"]
    for name in ("report", "receipt"):
        row = function_review["prior_review"][name]
        suffix = ".md" if name == "report" else ".json"
        assert digest(files["function-review/" + name + suffix]) == row["sha256"]
    agents = document("agents-closed-r1.json")
    assert len(agents["owned_agents"]) == 3
    assert all(row["closed"] for row in agents["owned_agents"])

    command("release-build-r2")
    command("package-precommit-r1")
    command("package-inspect-r1")
    package = document("package-precommit-manifest-r1.json")
    assert not package["installed"] and not package["signed_by_packaging_script"]
    assert package["build"]["commit"] is None
    normal = document("normal-package-r1.json")
    assert normal["gui_sha256"] == package["binary_sha256"]
    assert normal["mcp_sha256"] == package["mcp_binary_sha256"]
    for row in normal["files"]:
        if row["path"] != "package-manifest.json":
            assert row["sha256"] == package["files"][row["path"]]
    native = document("native-availability-r1.json")
    assert not native["native_apps_available"]
    assert all(native[key] == 0 for key in (
        "gui_actions", "screenshots", "app_launches", "installations",
    ))
    print(json.dumps({
        "source": args.source or "working-tree", "source_inputs": len(names),
        "aggregate_sha256": freeze["aggregate_sha256"],
        "members": len(files), "mismatches": [], **summary,
        "real_openssh_passed": 18, "native_desktop_acceptance": False,
        "independent_source_review": "APPROVE",
        "independent_prebuilt_function_review": "APPROVE",
        "independent_prebuilt_editor_passed": 88,
        "owned_agents_closed": 3,
        "normal_gui_sha256": package["binary_sha256"],
        "normal_mcp_sha256": package["mcp_binary_sha256"],
        "tests_replayed": 0,
    }))


if __name__ == "__main__":
    main()
