#!/usr/bin/env python3
"""Runner/fixture safety regressions; these do not sign Linux Rust acceptance."""

from contextlib import ExitStack, redirect_stdout, redirect_stderr
import copy
import importlib.util
import io
import json
from pathlib import Path
import platform
import re
import shutil
import signal
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

import monitor_recovery_lab as lab

SPEC = importlib.util.spec_from_file_location("monitor_fixture", lab.HELPERS / "fixture.py")
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)


requires_posix_runner = unittest.skipUnless(
    platform.system() in ("Darwin", "Linux"),
    "Lab runners require macOS/Linux kernel birth identities and POSIX signals",
)


def receipt():
    return {"resources": [], "commands": [], "processes": [], "steps": [], "cleanup": {}}


def resource(kind="container", name="owned-ssh", ident="a"):
    return {"kind": kind, "name": name, "id": ident * 64,
            "mutations": [{"command": "create", "status": "COMPLETED"}]}


class SafetyTests(unittest.TestCase):
    def late_creation(self, interrupted=False, code=None):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root, record = Path(directory), receipt()
            runner = lab.ClientRunner(root, time.monotonic() + 10, stack, record, lambda: None)
            api = lab.PodmanLab(runner, "owned", record, "not-a-real-podman")
            row = api.register("container", "owned-ssh")
            actual_start, actual_wait = runner.start, runner.wait_bounded
            program = "import time; time.sleep(4)"
            if code is not None:
                program = ("import os,signal; os.kill(os.getpid(),signal.SIGTERM)"
                           if code < 0 else f"raise SystemExit({code})")
            runner.start = lambda name, argv, env=None: actual_start(
                name, [sys.executable, "-B", "-c", program], env,
            )
            runner.wait_bounded = lambda name, process, seconds: actual_wait(
                name, process, 0.025 if code is None else 2,
            )
            if interrupted:
                def interrupt():
                    if runner.processes:
                        raise lab.interop.Interrupted("owned unit-test interrupt")
                runner.check_interrupt = interrupt
            marker = root / "late-resource"
            release = threading.Event()

            def delayed_daemon():
                if release.wait(3):
                    marker.write_text("service accepted before client stopped")

            worker = threading.Thread(target=delayed_daemon)
            worker.start()
            try:
                expected = (lab.interop.Interrupted if interrupted else
                            lab.LabFailure if code is not None else lab.interop.InteropFailure)
                with self.assertRaises(expected):
                    api.command("create", ["create", "--name", row["name"]], mutation=row)
                child = runner.processes[0]
                self.assertIsNotNone(child.poll(), "real direct child must be reaped before cleanup")
                self.assertTrue(runner.owned[child], "kernel birth identity must be recorded")
                if code is not None:
                    self.assertEqual(child.returncode, code)
                self.assertEqual(row["mutations"][0]["status"], "UNKNOWN")
                runner.check_interrupt = lambda: None
                reads = []

                def command(label, argv, **kwargs):
                    reads.append((argv, child.poll() is not None))
                    if argv[0] == "rm":
                        self.assertEqual(argv[-1], "a" * 64)
                        marker.unlink()
                        kwargs["mutation"]["mutations"].append(
                            {"command": label, "status": "COMPLETED"},
                        )
                        return 0, ""
                    if argv[:2] == ["container", "exists"]:
                        return (0 if marker.exists() else 1), ""
                    return 0, ""

                api.command = command
                api.bind = lambda _: "a" * 64
                first = api.cleanup()
                self.assertFalse(marker.exists())
                release.set()
                worker.join(timeout=2)
                self.assertFalse(worker.is_alive())
                self.assertTrue(marker.exists(), "daemon creation survives the absent read")
                second = api.cleanup()
                self.assertFalse(marker.exists())
                self.assertTrue(all(stopped for _, stopped in reads))
                for results in (first, second):
                    self.assertTrue(results[0]["absence_observed"])
                    self.assertFalse(results[0]["absent"])
                    self.assertEqual(results[0]["mutation_completion"], "UNKNOWN")
                    self.assertEqual(results[0]["failure"]["type"], "MutationCompletionUnknown")
                events = [json.loads(line) for line in (root / "events.jsonl").read_text().splitlines()]
                self.assertEqual(events[-1]["resource"]["mutations"][0]["status"], "UNKNOWN")
            finally:
                release.set()
                worker.join(timeout=2)
                runner.check_interrupt = lambda: None
                lab.stop_clients(runner, time.monotonic() + 3)

    @requires_posix_runner
    def test_timeout_reaps_client_and_late_creation_cannot_be_signed_absent(self):
        self.late_creation()

    @requires_posix_runner
    def test_interrupt_reaps_client_and_late_creation_cannot_be_signed_absent(self):
        self.late_creation(interrupted=True)

    @requires_posix_runner
    def test_nonzero_mutation_is_unknown_even_after_client_reap(self):
        self.late_creation(code=125)

    @requires_posix_runner
    def test_signaled_mutation_is_unknown_even_after_client_reap(self):
        self.late_creation(code=-signal.SIGTERM)

    def test_failed_barrier_prevents_all_resource_exists_reads(self):
        record = receipt()
        record["resources"] = [resource()]
        api = lab.PodmanLab(None, "owned", record, "podman")
        api.runner = type("Runner", (), {"deadline": time.monotonic() + 3})()
        with patch.object(lab, "stop_clients", return_value=([], False)), \
                patch.object(api, "command") as command:
            rows = api.cleanup()
        command.assert_not_called()
        self.assertFalse(rows[0]["absent"])
        self.assertEqual(rows[0]["failure"]["type"], "CleanupBarrierUnconfirmed")

    @requires_posix_runner
    def test_capture_failure_after_popen_does_not_lose_the_real_child(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            record = receipt()
            runner = lab.ClientRunner(Path(directory), time.monotonic() + 8, stack, record, lambda: None)
            api = lab.PodmanLab(runner, "owned", record, "unused")
            row = api.register("container", "owned-ssh")
            with patch.object(runner, "capture_owned", side_effect=lab.interop.InteropFailure("inventory failed")):
                with self.assertRaises(lab.interop.InteropFailure):
                    api.command("create", [sys.executable, "-B", "-c", "import time; time.sleep(4)"],
                                binary=True, mutation=row)
            self.assertEqual(len(record["processes"]), 1)
            self.assertIsNotNone(runner.processes[0].poll())
            self.assertEqual(row["mutations"][0]["status"], "UNKNOWN")
            self.assertFalse(api.barrier_confirmed)

    def test_unknown_start_or_remove_is_not_laundered_by_absence_reads(self):
        for operation in ("start", "remove"):
            record = receipt()
            row = resource()
            row["mutations"].append({"command": operation, "status": "UNKNOWN"})
            record["resources"] = [row]
            api = lab.PodmanLab(type("Runner", (), {"deadline": time.monotonic() + 3})(),
                                "owned", record, "podman")
            with patch.object(lab, "stop_clients", return_value=([], True)), \
                    patch.object(api, "command", return_value=(1, "")):
                results = api.cleanup()
            self.assertFalse(results[0]["absent"])
            self.assertEqual(results[0]["mutation_completion"], "UNKNOWN")

    def test_owner_id_and_name_must_all_match_before_any_removal(self):
        record = receipt()
        ours, foreign = resource(name="owned-ours"), resource(name="owned-foreign", ident="b")
        record["resources"] = [ours, foreign]
        runner = type("Runner", (), {"deadline": time.monotonic() + 3})()
        api = lab.PodmanLab(runner, "owned", record, "podman")
        removed = set()
        calls = []

        def command(label, argv, **kwargs):
            calls.append(argv)
            if argv[:2] == ["container", "exists"]:
                return (1 if argv[-1] in removed else 0), ""
            if argv[0] == "rm":
                self.assertEqual(argv[-1], ours["id"])
                removed.update((ours["id"], ours["name"]))
                kwargs["mutation"]["mutations"].append({"command": label, "status": "COMPLETED"})
            return 0, ""

        def inspection(label, argv, **_kwargs):
            selected = ours if argv[-1] == ours["id"] else foreign
            return {"id": selected["id"], "name": selected["name"],
                    "labels": {lab.LABEL: "owned" if selected is ours else "another-owner"}}

        with patch.object(lab, "stop_clients", return_value=([], True)), \
                patch.object(api, "json", side_effect=inspection), \
                patch.object(api, "journal"), \
                patch.object(api, "command", side_effect=command):
            rows = api.cleanup()
        self.assertEqual(rows[0]["failure"]["type"], "OwnershipMismatch")
        self.assertTrue(rows[1]["absent"], "foreign resource must not skip our cleanup")
        self.assertEqual([args[-1] for args in calls if args[0] == "rm"], [ours["id"]])
        for field, value in (("id", "b" * 64), ("name", "another-name")):
            changed = {"id": ours["id"], "name": ours["name"], "labels": {lab.LABEL: "owned"}}
            changed[field] = value
            with patch.object(api, "json", return_value=changed), self.assertRaises(lab.LabFailure):
                api.bind(ours)

    def test_container_needs_no_host_mount_privileged_or_net_admin(self):
        args = lab.container_arguments("owned", "a" * 64, "b" * 64)
        for required in ("--read-only", "--pull=never", "--cap-drop=ALL", "--security-opt=no-new-privileges"):
            self.assertIn(required, args)
        for forbidden in ("--privileged", "NET_ADMIN", "--volume", "--mount", "--network=host"):
            self.assertNotIn(forbidden, args)
        self.assertEqual(args[args.index("--publish") + 1], "127.0.0.1::2222")
        caps = {args[index + 1] for index, arg in enumerate(args) if arg == "--cap-add"}
        self.assertEqual(caps, {"CHOWN", "DAC_READ_SEARCH", "SETUID", "SETGID", "SYS_CHROOT"})


class FixtureTests(unittest.TestCase):
    def test_contract_import_does_not_load_posix_runtime_dependencies(self):
        with patch.dict(sys.modules, {"fcntl": None, "pwd": None}):
            imported = importlib.util.module_from_spec(SPEC)
            SPEC.loader.exec_module(imported)
            self.assertEqual(imported.collector_command(imported.SNAPSHOT), fixture.SNAPSHOT)
            self.assertEqual(imported.COLLECTORS, fixture.COLLECTORS)

    def test_missing_runtime_dependencies_still_fail_when_used(self):
        with patch.dict(sys.modules, {"fcntl": None, "pwd": None}):
            with self.assertRaises(ModuleNotFoundError) as error:
                with fixture.locked_state():
                    self.fail("Missing fcntl must not permit unlocked access")
            self.assertEqual(error.exception.name, "fcntl")
            for operation in (fixture.guard, lambda: fixture.prepare("unused")):
                with self.subTest(operation=operation), self.assertRaises(ModuleNotFoundError) as error:
                    operation()
                self.assertEqual(error.exception.name, "pwd")

    def test_missing_portable_dependency_is_not_hidden_during_import(self):
        with patch.dict(sys.modules, {"hashlib": None}), self.assertRaises(ModuleNotFoundError) as error:
            SPEC.loader.exec_module(importlib.util.module_from_spec(SPEC))
        self.assertEqual(error.exception.name, "hashlib")

    def test_forcecommand_collectors_exactly_match_linux_api_including_df_cache(self):
        source = (lab.ROOT / "crates/keelshell-session/src/monitor.rs").read_text()
        for constant, script in (("SNAPSHOT_SCRIPT", fixture.SNAPSHOT),
                                 ("PROCESSES_SCRIPT", fixture.PROCESSES),
                                 ("SOCKETS_SCRIPT", fixture.SOCKETS), ("DF_COLLECTION", fixture.DF)):
            match = re.search(rf'const {constant}: &str = r#"(.*?)"#;', source, re.S)
            self.assertIsNotNone(match, constant)
            self.assertEqual(script, match.group(1), constant)
        self.assertNotIn("df -Pk", fixture.CACHED_SNAPSHOT)
        self.assertIn("cat /proc/stat", fixture.CACHED_SNAPSHOT)

    def test_platform_wrapper_only_delegates_an_exact_known_linux_body(self):
        for body in (fixture.SNAPSHOT, fixture.CACHED_SNAPSHOT):
            command = fixture.PLATFORM_SELECTOR + 'case "$("$ks_uname" -s)" in\nDarwin)\necho ignored; arbitrary text\n;;\nLinux)\n' + body + '\n;;\n*) exit 64 ;;\nesac\n'
            self.assertEqual(fixture.collector_command(command), body)
            with self.assertRaises(PermissionError):
                fixture.collector_command(command.replace(body, body + "echo injected\n"))
        with self.assertRaises(PermissionError):
            fixture.collector_command("echo arbitrary")

    def test_control_authenticates_token_and_admits_only_fixed_tool_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            (state / "bin").mkdir()
            value = {"modes": dict.fromkeys(fixture.TOOLS, "real"), "mode_changes": 0,
                     "tools": {tool: {"path": "/usr/bin/" + tool} for tool in fixture.TOOLS}}
            original = copy.deepcopy(value)
            token = "a" * 64
            invalid = [
                fixture.MAGIC + " " + "b" * 64 + " set-mode ps missing",
                fixture.MAGIC + " " + token + " set-mode cat missing",
                fixture.MAGIC + " " + token + " set-mode df real;id",
                fixture.MAGIC + " " + token + " set-mode df missing extra",
                "id", fixture.MAGIC + " " + token + " reset-all",
            ]
            with patch.object(fixture, "STATE", state):
                for command in invalid:
                    with self.subTest(command=command), self.assertRaises((PermissionError, ValueError)):
                        fixture.control(command, token, value)
                    self.assertEqual(value, original)
                fixture.control(fixture.MAGIC + " " + token + " set-mode ps missing", token, value)
                self.assertFalse((state / "bin" / "ps").exists())
                fixture.control(fixture.MAGIC + " " + token + " set-mode ps real", token, value)
                self.assertTrue((state / "bin" / "ps").is_symlink())
                self.assertEqual(value["mode_changes"], 2)

    def test_fixed_collector_allowlist_rejects_any_added_shell_command(self):
        for collector in fixture.COLLECTORS:
            self.assertNotIn(collector + "\nid\n", fixture.COLLECTORS)
        self.assertEqual(set(fixture.COLLECTORS.values()),
                         {("core", "df"), ("core", None), ("ps", "ps"), ("ss", "ss")})

    @requires_posix_runner
    def test_successful_wrapper_execs_an_existing_tool_and_error_modes_never_delegate(self):
        # This executes the host's real df to test exec/counter behavior only.
        # It makes no Linux, SSH, Rust, parser or native-app acceptance claim.
        actual_df = shutil.which("df")
        self.assertIsNotNone(actual_df)
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory)
            state = root / "state"
            state.mkdir()
            (state / "lock").touch()
            value = {"modes": {"df": "real"}, "tools": {"df": {"path": actual_df}},
                     "invocations": {"df": 0}, "delegations": {"df": 0}}
            program = (
                "import importlib.util,sys; from pathlib import Path; "
                "spec=importlib.util.spec_from_file_location('fixture',sys.argv[1]); "
                "f=importlib.util.module_from_spec(spec); spec.loader.exec_module(f); "
                "f.STATE=Path(sys.argv[2]); f.guard=lambda:None; "
                "sys.exit(f.tool('df',['-Pk']))"
            )
            record = receipt()
            runner = lab.ClientRunner(root, time.monotonic() + 15, stack, record, lambda: None)
            try:
                for mode, code, delegated in (("real", 0, 1), ("unsupported", 2, 0), ("permission", 126, 0)):
                    value["modes"]["df"] = mode
                    value["invocations"]["df"] = value["delegations"]["df"] = 0
                    fixture.write_json(state / "counts.json", value)
                    child = runner.start(mode, [
                        sys.executable, "-B", "-c", program, str(lab.HELPERS / "fixture.py"), str(state),
                    ])
                    self.assertEqual(runner.wait_bounded(mode, child, 3), code)
                    after = json.loads((state / "counts.json").read_text())
                    self.assertEqual(after["invocations"]["df"], 1)
                    self.assertEqual(after["delegations"]["df"], delegated)
                    if mode == "real":
                        self.assertIn("Filesystem", (root / "real.log").read_text())
            finally:
                _, stopped = lab.stop_clients(runner, time.monotonic() + 3)
                self.assertTrue(stopped)

    def test_real_ss_absence_keeps_path_missing_even_when_mode_is_real(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            (state / "bin").mkdir()
            value = {"tools": {"ss": {"path": None}}, "modes": {"ss": "missing"}}
            with patch.object(fixture, "STATE", state):
                fixture.set_mode(value, "ss", "real")
            self.assertFalse((state / "bin" / "ss").is_symlink())
            self.assertEqual(value["modes"]["ss"], "real")


class EvidenceTests(unittest.TestCase):
    def cli(self, output, binary=sys.executable, digest=None):
        return [
            "--opt-in", "localhost-owned-fixture", "--output", str(output),
            "--source-sha", "539c172389b4450570026f023ff89d50df90f910",
            "--test-binary", str(binary), "--test-binary-sha256", digest or "0" * 64,
            "--base-image", "7" * 64,
        ]

    def test_full_inventory_rejects_zero_extra_duplicate_and_substring_names(self):
        complete = "\n".join([*(test + ": test" for test in lab.TESTS), "", "2 tests, 0 benchmarks"])
        self.assertEqual(lab.inventory(complete), list(lab.TESTS))
        for invalid in ("0 tests, 0 benchmarks", complete + "\nextra: test",
                        complete.replace(lab.TESTS[0], "prefix_" + lab.TESTS[0]),
                        complete + "\n" + lab.TESTS[0] + ": test"):
            with self.subTest(invalid=invalid), self.assertRaises(lab.LabFailure):
                lab.inventory(invalid)

    def test_partial_or_wrong_owner_summary_is_never_a_successful_observation(self):
        cases = [f"{tool}-{mode}" for tool in fixture.TOOLS
                 for mode in ("missing", "unsupported", "permission")]
        complete = {"schema": 1, "test": lab.TESTS[0], "owner": "owned", "status": "PASSED",
                    "native_app": "NOT_RUN", "completed_cases": cases}
        lab.validate_summary(lab.TESTS[0], complete, "owned")
        for update in ({"completed_cases": cases[:-1]}, {"owner": "foreign"},
                       {"native_app": "PASSED"}, {"status": "FAILED"}):
            with self.assertRaises(lab.LabFailure):
                lab.validate_summary(lab.TESTS[0], dict(complete, **update), "owned")

    def test_digest_mismatch_preserves_failure_and_launches_no_process(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "failed"
            with redirect_stdout(io.StringIO()), \
                    patch.object(lab.platform, "system", return_value="Linux"), \
                    patch.object(lab.subprocess, "Popen") as start:
                self.assertEqual(lab.main(self.cli(output)), 1)
            start.assert_not_called()
            result = json.loads((output / "result.json").read_text())
            self.assertEqual(result["failure"]["type"], "PrebuiltDigestMismatch")
            self.assertFalse(result["observations_admissible"])
            self.assertTrue(result["cleanup"]["confirmed"])

    @requires_posix_runner
    def test_zero_test_real_child_is_reaped_before_any_resource_is_created(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "zero-tests"
            binary.write_text("#!" + sys.executable + "\nprint('0 tests, 0 benchmarks')\n")
            binary.chmod(0o700)
            output = root / "failed"
            arguments = self.cli(output, binary, lab.process_lab.digest(binary))
            # Reach the real inventory child after the genuine source guard;
            # a historical commit would fail earlier as this project advances.
            arguments[arguments.index("--source-sha") + 1] = lab.subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=lab.ROOT, text=True, timeout=5
            ).strip()
            with redirect_stdout(io.StringIO()), patch.object(lab.interop, "executable", return_value="unused"):
                result_code = lab.main(arguments)
            result = json.loads((output / "result.json").read_text())
            self.assertEqual(result_code, 1)
            self.assertEqual(result["failure"]["type"], "PrebuiltInventoryMismatch")
            self.assertEqual(result["resources"], [])
            self.assertTrue(result["cleanup"]["confirmed"])
            self.assertTrue(result["cleanup"]["processes"])
            self.assertTrue(all(row["reaped"] for row in result["cleanup"]["processes"]))

    def test_bad_cli_never_launches_a_child(self):
        for flag, value in (("--timeout", "nan"), ("--timeout", "299"), ("--base-image", "short"),
                            ("--source-sha", "539c172"), ("--test-binary-sha256", "short")):
            with self.subTest(flag=flag), redirect_stderr(io.StringIO()), \
                    patch.object(lab.platform, "system", return_value="Linux"), \
                    patch.object(lab.subprocess, "Popen") as start:
                with self.assertRaises(SystemExit) as error:
                    lab.main([*self.cli("/unused"), flag, value])
                self.assertEqual(error.exception.code, 2)
                start.assert_not_called()

    def test_reusing_evidence_directory_preserves_original_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            old = output / "result.json"
            old.write_text('{"status":"FAILED"}\n')
            with patch.object(lab.platform, "system", return_value="Linux"), \
                    self.assertRaises(FileExistsError):
                lab.main(self.cli(output))
            self.assertEqual(old.read_text(), '{"status":"FAILED"}\n')

    def test_unsupported_host_is_rejected_before_output_or_process_creation(self):
        for system in ("Windows", "FreeBSD"):
            with self.subTest(system=system), redirect_stderr(io.StringIO()) as errors, \
                    patch.object(lab.platform, "system", return_value=system), \
                    patch.object(lab.interop, "create_output") as output, \
                    patch.object(lab.subprocess, "Popen") as start:
                with self.assertRaises(SystemExit) as error:
                    lab.main(self.cli("/unused"))
                self.assertEqual(error.exception.code, 2)
                self.assertIn("require macOS or Linux", errors.getvalue())
                output.assert_not_called()
                start.assert_not_called()

    def test_private_cleanup_refuses_replaced_identity_or_owner(self):
        with tempfile.TemporaryDirectory() as directory:
            scratch = Path(directory) / "scratch"
            scratch.mkdir()
            (scratch / "owner").write_text("foreign")
            identity = (scratch.stat().st_dev, scratch.stat().st_ino)
            with self.assertRaises(lab.LabFailure):
                lab.private_remove(scratch, identity, "owned")
            self.assertTrue(scratch.exists())

    def test_isolation_checks_actual_kernel_caps_and_exact_network_and_port(self):
        row = {
            "network": {"Networks": {"owned": {"NetworkID": "b" * 64}},
                        "Ports": {"2222/tcp": [{"HostIp": "127.0.0.1", "HostPort": "12345"}]}},
            "security": {"privileged": False, "network_mode": "bridge", "readonly": True,
                         "security_opt": ["no-new-privileges"], "mounts": [], "image": "a" * 64},
            "fixture": {"owner": "owned", "identity": {
                "effective_caps_hex": "400c5", "bounding_caps_hex": "400c5",
                "network_namespace": "net:[100]", "pid1_birth_ticks": 123,
                "boot_id": "297a70af-36d7-4e83-a4ca-07bf3bf7fa8e",
            }},
        }
        self.assertEqual(lab.check_isolation(row, {"id": "b" * 64}, {"id": "a" * 64}, "owned"), 12345)
        for change in (
            lambda r: r["fixture"]["identity"].update(effective_caps_hex="410c5"),
            lambda r: r["fixture"]["identity"].update(effective_caps_hex="400c4"),
            lambda r: r["fixture"]["identity"].update(bounding_caps_hex="400c1"),
            lambda r: r["network"]["Networks"]["owned"].update(NetworkID="c" * 64),
            lambda r: r["network"]["Ports"]["2222/tcp"][0].update(HostIp="0.0.0.0"),
            lambda r: r["security"].update(mounts=[{"Type": "bind"}]),
            lambda r: r["security"].update(readonly=False),
        ):
            damaged = copy.deepcopy(row)
            change(damaged)
            with self.assertRaises(lab.LabFailure):
                lab.check_isolation(damaged, {"id": "b" * 64}, {"id": "a" * 64}, "owned")


if __name__ == "__main__":
    unittest.main()
