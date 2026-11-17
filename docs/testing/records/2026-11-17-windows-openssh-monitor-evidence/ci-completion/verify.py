"""Recompute exact Windows CI originals, command bindings and Quality counts."""
import ast
import base64
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import runpy
import subprocess
import sys
import zipfile

BASE = Path(__file__).resolve().parent
common = runpy.run_path(str(BASE.parent / "repair/verify.py"))
digest = common["digest"]
unpack = common["unpack"]
summarize_tests = common["summarize_tests"]


def main():
    files = unpack(BASE)
    acceptance = json.loads(files["acceptance.json"])
    source = acceptance["source"]
    assert re.fullmatch(r"[0-9a-f]{40}", source)
    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10).strip())

    def git_bytes(name):
        return subprocess.check_output(["git", "show", f"{source}:{name}"],
                                       cwd=root, timeout=20)

    tree = ast.parse(git_bytes("scripts/windows_monitor_lab.py"))
    constants = {}
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1:
            target = node.targets[0]
            if isinstance(target, ast.Name) and target.id in ("ASSEMBLY", "POWERSHELL", "SUFFIX"):
                constants[target.id] = ast.literal_eval(node.value)
    assert set(constants) == {"ASSEMBLY", "POWERSHELL", "SUFFIX"}
    commands = {}
    assemblies = {}
    for operation, (prefix, body) in constants["ASSEMBLY"].items():
        parts = [(name, git_bytes("crates/keelshell-session/src/monitor/windows/" + name))
                 for name in ("common.ps1", body)]
        script = prefix + "".join(raw.decode() for _, raw in parts) + constants["SUFFIX"]
        command = constants["POWERSHELL"] + base64.b64encode(script.encode("utf-16le")).decode()
        commands[operation] = command
        assemblies[operation] = {
            "operation": operation,
            "parts": [{"file": name, "sha256": digest(raw)} for name, raw in parts],
            "join": "verbatim", "literal_prefix": prefix,
            "literal_suffix": constants["SUFFIX"],
            "script_sha256": digest(script.encode()),
            "wire_command_sha256": digest(command.encode("ascii")),
            "wire_command_ascii_bytes": len(command.encode("ascii")),
        }
    summaries = {}
    native = {}
    interop = {}
    for label, expected_id in acceptance["runs"].items():
        prefix = f"ci/{label}/"
        run = json.loads(files[prefix + "run.json"])
        receipt = json.loads(files[prefix + "fetch-receipt.json"])
        assert run["id"] == receipt["run_id"] == expected_id
        assert run["head_sha"] == receipt["source"] == source
        assert run["status"] == receipt["status"] == "completed"
        assert run["conclusion"] == receipt["conclusion"] == "success"
        jobs = json.loads(files[prefix + "jobs.json"])["jobs"]
        assert receipt["jobs"] == [
            {key: job[key] for key in ("id", "name", "status", "conclusion")}
            for job in jobs
        ]
        unpacked = {}
        for download in receipt["downloads"]:
            raw = files[prefix + download["zip"]]
            assert len(raw) == download["bytes"] and digest(raw) == download["sha256"]
            if download["github_digest"] is not None:
                assert download["github_digest"] == "sha256:" + digest(raw)
            with zipfile.ZipFile(io.BytesIO(raw)) as archive:
                members = [item.filename for item in archive.infolist() if not item.is_dir()]
                assert len(members) == len(set(members)) == len(download["members"])
                assert set(members) == {row["path"] for row in download["members"]}
                for row in download["members"]:
                    content = archive.read(row["path"])
                    assert len(content) == row["bytes"] and digest(content) == row["sha256"]
                    unpacked[(download["zip"], row["path"])] = content
        if label == "quality":
            assert {job["name"] for job in jobs} == {
                f"native ({platform})" for platform in
                ("windows-2025", "macos-26", "ubuntu-24.04")
            }
            assert all(job["status"] == "completed" and job["conclusion"] == "success"
                       for job in jobs)
            for platform in ("windows-2025", "macos-26", "ubuntu-24.04"):
                raw = unpacked[("logs.zip", f"native ({platform})/8_Rust quality gate.txt")]
                report = summarize_tests.summarize(raw)
                assert not report["inventory_result_summary_mismatches"]
                assert sum(row["summary"]["passed"]
                           for row in report["doctest_runs_unassigned"]) == 14
                assert all(row["summary"]["failed"] == 0 for row in
                           report["suites"] + report["doctest_runs_unassigned"])
                section = summarize_tests.normalize(raw).split("Running: cargo fmt --all --check", 1)[0]
                # GitHub merges stdout/stderr; child diagnostics may occur between
                # unittest's count and OK. Bind the sole pair before the fmt step.
                runs = list(re.finditer(r"^Ran (\d+) tests in [\d.]+s$", section, re.MULTILINE))
                outcomes = list(re.finditer(r"^OK(?: \(skipped=(\d+)\))?$", section, re.MULTILINE))
                assert len(runs) == len(outcomes) == 1
                assert outcomes[0].start() > runs[0].end()
                assert not re.search(r"^(?:FAIL:|ERROR:|FAILED\b)", section, re.MULTILINE)
                summaries[platform] = {
                    "ordinary_rust": {key: sum(row["summary"][key] for row in report["suites"])
                                      for key in ("passed", "failed", "ignored")},
                    "rustdoc_passed": sum(row["summary"]["passed"]
                                          for row in report["doctest_runs_unassigned"]),
                    "python": {"run": int(runs[0][1]), "skipped": int(outcomes[0][1] or 0)},
                }
            for download in receipt["downloads"]:
                if not download.get("name", "").startswith("openssh-interop-"):
                    continue
                result_name, = [row["path"] for row in download["members"]
                                if row["path"].endswith("/result.json")]
                result = json.loads(unpacked[(download["zip"], result_name)])
                assert result["outcome"] == "passed"
                assert all(step["returncode"] == 0 for step in result["steps"])
                assert result["cleanup"]["owned_processes_stopped"]
                assert result["cleanup"]["temporary_directory_removed"]
                assert not result["cleanup"]["ancestry_unverified"]
                log_name = str(PurePosixPath(result_name).with_name("tests.log"))
                text = summarize_tests.normalize(unpacked[(download["zip"], log_name)])
                declared, = re.findall(r"^running (\d+) tests$", text, re.MULTILINE)
                named = re.findall(r"^test (\S+) \.\.\. (\S+)$", text, re.MULTILINE)
                passed, = re.findall(
                    r"^test result: ok\. (\d+) passed; 0 failed; 0 ignored; "
                    r"0 measured; 0 filtered out; finished in [\d.]+s$",
                    text, re.MULTILINE,
                )
                assert int(declared) == int(passed) == len(named) == result["passed_tests"]
                assert len({name for name, _ in named}) == len(named)
                assert all(state == "ok" for _, state in named)
                interop[result["platform"]] = result["passed_tests"]
            assert set(interop) == {"Linux", "Darwin"}
            continue
        artifact = next(row for row in receipt["downloads"] if "id" in row)

        def document(name):
            return json.loads(unpacked[(artifact["zip"], name)])

        actual = document("receipt.json")
        assert actual["status"] == "PASS"
        assert actual["windows_api"] == "PASS" and actual["native_gui"] == "UNRUN"
        for operation, assembly in assemblies.items():
            assert document(operation + ".assembly.json") == assembly
        if label == "direct-api":
            assert actual["ssh"] == "UNRUN"
            continue
        assert actual["source"] == source and actual["ssh"] == "PASS"
        seal, final = document("ssh-debug-originals.json"), document("ssh-debug-cleanup.json")
        assert seal["admission_closed"] and seal["complete_original_object_census"]
        assert seal["identities_bound"] and not seal["errors"]
        assert seal["job_active_processes"] == 1
        assert final["state"] == "PASS" and final["terminal_proven"] and final["closed"]
        assert final["admission_closed"] and final["complete_original_object_census"]
        assert final["identities_bound"] and not final["errors"]
        for name in ("job_active_processes", "pending_process_objects", "owned_handles_remaining",
                     "debug_connections", "fallback_detaches"):
            assert final[name] == 0, name
        assert final["pending_debug_event"] is False
        assert final["debug_events_received"] == final["debug_events_continued"]
        identities = lambda rows: {(row["pid"], row["creation_time"]) for row in rows}
        assert identities(seal["admissions"]) == identities(final["admissions"])
        assert len(final["admissions"]) == len(identities(final["admissions"])) == final["job_total_processes"]
        assert sum(row["is_master"] for row in final["admissions"]) == 1
        assert all(row["natural_terminal_before_cleanup"] for row in final["admissions"]
                   if not row["is_master"])
        powershell = [row for row in final["admissions"] if row["image"].lower().endswith("\\powershell.exe")]
        found = {}
        for row in powershell:
            assert row["exit_code"] == row["exit_event_code"] == 0
            assert row["original_signaled"] and row["exit_event_continued"]
            assert row["identity_bound_before_first_continue"] and row["retained_before_first_instruction"]
            match = re.fullmatch(r'(?:"[^"\r\n]+"|[^"\s]+) +(-NoLogo -NoProfile -NonInteractive -EncodedCommand [A-Za-z0-9+/]+=*)',
                                 row["command_line"])
            assert match
            command = "powershell.exe " + match[1]
            matches = [name for name, value in commands.items() if value == command]
            assert len(matches) == 1 and matches[0] not in found
            found[matches[0]] = digest(command.encode("ascii"))
        assert set(found) == {"probe", "resources", "processes", "sockets"}
        binding = document("ssh-production-binding.json")
        assert binding["source_exact"] and binding["original_processes_naturally_terminal"]
        assert {row["operation"]: row["wire_sha256"] for row in binding["operations"]} == found
        cleanup = document("ssh-cleanup.json")
        assert cleanup["temporary_directory_removed"] and not cleanup["keys_uploaded"]
        assert not cleanup["private_config_uploaded"]
        client = document("rust-client.json")
        assert client["status"] == "PASS" and not client["errors"]
        native[label] = {
            "run": expected_id, "admitted": len(final["admissions"]),
            "natural_nonmasters": len(final["admissions"]) - 1,
            "powershell_natural_exit0": len(powershell), "closed": True,
            "incomplete_census_observations": final["incomplete_census_observations"],
        }
    assert summaries == acceptance["quality_counts"]
    review = json.loads(files["review/decision.json"])
    assert review["approved"] and not any(review["findings_by_priority"].values())
    assert review["source"] == source
    assert json.loads(files["agent-lifecycle.json"])["all_owned_agents_closed"]
    binding = subprocess.check_output([
        sys.executable, "-B", str(BASE.parent / "repair/verify.py"), "--source", source,
    ], text=True, cwd=root, timeout=90)
    assert json.loads(binding)["mismatches"] == []
    print(json.dumps({"source": source, "members": len(files), "quality": summaries,
                      "native_ssh": native, "openssh_interop": interop, "tests_replayed": 0,
                      "native_desktop_acceptance": False, "mismatches": []}))


if __name__ == "__main__":
    main()
