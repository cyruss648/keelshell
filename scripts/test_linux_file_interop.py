#!/usr/bin/env python3
"""Fixture ownership, literal command construction, and real process cleanup.

These tests use only isolated temporary directories and short-lived Python
children. They do not invoke Podman, Cargo, SSH, network access, or shared targets.
The real Linux/Python/lrzsz/ACL checks belong to run_linux_file_interop.py.
"""

import contextlib
import io
import json
from pathlib import Path
import platform
import shlex
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

sys.dont_write_bytecode = True
import run_linux_file_interop as fixture


requires_posix_runner = unittest.skipUnless(
    platform.system() in ("Darwin", "Linux"),
    "Lab runners require macOS/Linux kernel birth identities and POSIX signals",
)


class OwnershipTests(unittest.TestCase):
    def setUp(self):
        self.owner = "a" * 32
        self.identifier = "b" * 64
        self.info = {
            "Id": self.identifier,
            "Name": fixture.CONTAINER_PREFIX + self.owner[:12],
            "Config": {"Labels": {
                fixture.LABEL: fixture.LABEL_VALUE,
                fixture.OWNER_LABEL: self.owner,
            }},
        }

    def test_exact_container_identity_is_required(self):
        self.assertTrue(fixture.container_matches(self.info, self.identifier, self.owner))
        for field, replacement in (
            ("Id", "c" * 64), ("Name", "unrelated-container"),
            ("Config", {"Labels": {fixture.LABEL: fixture.LABEL_VALUE}}),
            ("Config", {"Labels": {fixture.OWNER_LABEL: self.owner}}),
            ("Config", {"Labels": []}), ("Config", None),
        ):
            with self.subTest(field=field, replacement=replacement):
                info = dict(self.info, **{field: replacement})
                self.assertFalse(fixture.container_matches(info, self.identifier, self.owner))
        self.assertFalse(fixture.container_matches(self.info, self.identifier[:12], self.owner))
        self.assertFalse(fixture.container_matches(self.info, self.identifier, "d" * 32))

    def test_only_exact_owner_token_releases_lease(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "release"
            self.assertFalse(fixture.release_requested(path, self.owner))
            for value in ("", "d" * 32, self.owner + "\nextra", "\N{SNOWMAN}"):
                path.write_text(value, encoding="utf-8")
                self.assertFalse(fixture.release_requested(path, self.owner))
            path.write_text(self.owner + "\n", encoding="ascii")
            self.assertTrue(fixture.release_requested(path, self.owner))


class CommandConstructionTests(unittest.TestCase):
    def test_package_bridge_decodes_versions_and_refuses_arbitrary_paths(self):
        token = "a" * 32
        path = "/" + token + "/debian/pool/main/p/python3/python3_3.11.2-1%2bb1_arm64.deb"
        self.assertEqual(
            fixture.debian_upstream(path, token),
            "https://deb.debian.org/debian/pool/main/p/python3/python3_3.11.2-1+b1_arm64.deb",
        )
        for bad in (
            "/" + token + "/debian/../../etc/passwd",
            "/" + token + "/debian/pool/%2e%2e/etc/passwd",
            "/" + token + "/debian/pool/%252e%252e/etc/passwd",
            "/" + token + "/debian/pool//file.deb",
            "/" + token + "/debian/pool/file.deb?host=other",
            "/" + token + "/other/pool/file.deb",
            "/stale/debian/pool/file.deb", "https://unrelated.example/pool/file.deb",
        ):
            with self.subTest(path=bad):
                self.assertIsNone(fixture.debian_upstream(bad, token))

    def test_pty_is_explicit_and_config_is_an_argv_element(self):
        config = Path("/tmp/a quoted ' fixture/ssh_config")
        for pty, flag in ((False, "-T"), (True, "-tt")):
            command = fixture.ssh_argv(config, "printf literal", pty=pty)
            self.assertEqual(command, ["ssh", "-F", str(config), flag, "fixture", "printf literal"])

    def test_remote_pty_quoting_preserves_shell_metacharacters_as_data(self):
        directory = fixture.REMOTE_ROOT + "/a quoted ' directory"
        name = "source '$(printf never); 中文.bin"
        command = fixture.raw_pty_command(directory, "sz", (name,))
        before, after = command.split(" && stty raw -echo -onlcr && exec ")
        self.assertEqual(shlex.split(before), ["cd", directory])
        self.assertEqual(shlex.split(after), ["sz", "-b", "--", name])
        receive = fixture.raw_pty_command(directory, "rz")
        self.assertEqual(shlex.split(receive.rsplit(" && exec ", 1)[1]), ["rz", "-b"])

    def test_remote_pty_stays_in_owned_tree(self):
        for path in (
            "/", fixture.REMOTE_ROOT, fixture.REMOTE_ROOT + "2/receive",
            fixture.REMOTE_ROOT + "/../outside", fixture.REMOTE_ROOT + "//receive",
            fixture.REMOTE_ROOT + "/./receive", fixture.REMOTE_ROOT + "/bad\npath",
        ):
            with self.subTest(path=path), self.assertRaises(fixture.FixtureFailure):
                fixture.raw_pty_command(path, "rz")
        for program, names in (
            ("sh", ()), ("rz", ("filename",)), ("sz", ()),
            ("sz", ("../outside",)), ("sz", ("a/b",)), ("sz", ("\0",)),
        ):
            with self.subTest(program=program, names=names), self.assertRaises(fixture.FixtureFailure):
                fixture.raw_pty_command(fixture.REMOTE_ROOT + "/safe", program, names)

    def test_adoption_and_timeouts_must_be_explicit(self):
        for args in (
            ["--prepared-container", "b" * 64], ["--owner", "a" * 32],
            ["--prepared-container", "b" * 12, "--owner", "a" * 32],
            ["--hold-seconds", "-1"], ["--hold-seconds", "1801"],
            ["--command-timeout", "0"], ["--command-timeout", "601"],
            ["--", "cargo", "test"], ["--", "/usr/bin/rustc", "input.rs"],
        ):
            with self.subTest(args=args), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                fixture.arguments(args)
        args = fixture.arguments(["--", "/tmp/copied-test", "--ignored", "--exact", "some_test"])
        self.assertEqual(args.command, ["/tmp/copied-test", "--ignored", "--exact", "some_test"])


class CommandDeadlineTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.output = Path(directory.name)
        self.probe_timeout = fixture.ownership.InteropFailure(
            "Process inventory exceeded its bounded deadline"
        )
        self.probe_timeout.__cause__ = fixture.subprocess.TimeoutExpired(
            ["ps", "-eo", "pid="], 0.05
        )

    def run_inventory(self, completed_at, error=None, clean=True):
        """Complete one inventory at a controlled time, with an already-exited child."""
        self.receipt = {"steps": []}
        commands = fixture.Commands(self.output, self.receipt, lambda: None)
        process = MagicMock(pid=101, returncode=None)
        process.poll.return_value = None
        identity = (1, 101, ("kernel", 7), "R")

        def inventory(deadline):
            self.assertEqual(deadline, 10.8)
            clock.return_value = completed_at
            process.returncode = 0
            process.poll.return_value = 0
            if error is not None:
                raise error
            return {process.pid: identity}

        def stop(child, deadline, identities):
            self.assertIs(child, process)
            self.assertEqual(deadline, completed_at + 7)
            self.assertEqual(identities, {process.pid: identity})
            return clean

        with patch.object(fixture.time, "monotonic", return_value=10) as clock, \
                patch.object(fixture.subprocess, "Popen", return_value=process), \
                patch.object(fixture.ownership, "process_info", return_value=identity), \
                patch.object(fixture.ownership, "process_table", side_effect=inventory) as capture, \
                patch.object(fixture.ownership, "stop_owned", side_effect=stop) as cleanup:
            try:
                return commands.run("inventory-boundary", ["controlled-child"], timeout=0.8)
            finally:
                capture.assert_called_once_with(10.8)
                cleanup.assert_called_once()

    def assert_recorded_step(self, timed_out, interrupted, clean=True):
        self.assertEqual(len(self.receipt["steps"]), 1)
        step = self.receipt["steps"][0]
        self.assertEqual(step["timed_out"], timed_out)
        self.assertEqual(step["interrupted"], interrupted)
        self.assertEqual(step["owned_processes_stopped"], clean)
        self.assertEqual(step["returncode"], 0)
        self.assertEqual(
            json.loads((self.output / "progress.json").read_text()), self.receipt
        )

    def test_inventory_probe_timeout_at_command_deadline_is_a_timeout(self):
        for completed_at in (10.8, 10.9):
            with self.subTest(completed_at=completed_at):
                with self.assertRaisesRegex(fixture.FixtureFailure, "inventory-boundary failed"):
                    self.run_inventory(completed_at, self.probe_timeout)
                self.assert_recorded_step(timed_out=True, interrupted=False)

    def test_inventory_internal_deadline_failures_are_command_timeouts(self):
        for message in (
            "Deadline reached while inspecting owned processes",
            "Deadline reached while reading process identities",
        ):
            with self.subTest(message=message):
                with self.assertRaises(fixture.FixtureFailure):
                    self.run_inventory(10.8, fixture.ownership.InteropFailure(message))
                self.assert_recorded_step(timed_out=True, interrupted=False)

    def test_inventory_completion_at_or_past_deadline_cannot_report_success(self):
        for completed_at in (10.8, 10.9):
            with self.subTest(completed_at=completed_at):
                with self.assertRaisesRegex(fixture.FixtureFailure, "inventory-boundary failed"):
                    self.run_inventory(completed_at)
                self.assert_recorded_step(timed_out=True, interrupted=False)

    def test_inventory_failures_before_deadline_are_not_timeouts(self):
        for error in (
            self.probe_timeout,
            fixture.ownership.InteropFailure("Deadline reached while inspecting owned processes"),
            fixture.ownership.InteropFailure("Deadline reached while reading process identities"),
            fixture.ownership.InteropFailure("Cannot verify process identity for PID 101"),
            PermissionError("Process inventory is not permitted"),
            fixture.subprocess.CalledProcessError(1, ["ps", "-eo", "pid="]),
        ):
            with self.subTest(error=str(error)):
                with self.assertRaises(type(error)) as caught:
                    self.run_inventory(10.4, error)
                self.assertIs(caught.exception, error)
                self.assert_recorded_step(timed_out=False, interrupted=True)

    def test_unrelated_inventory_failure_after_deadline_is_not_a_timeout(self):
        for error in (
            fixture.ownership.InteropFailure("Cannot verify process identity for PID 101"),
            PermissionError("Process inventory is not permitted"),
            fixture.subprocess.CalledProcessError(1, ["ps", "-eo", "pid="]),
        ):
            with self.subTest(error=str(error)):
                with self.assertRaises(type(error)) as caught:
                    self.run_inventory(10.9, error)
                self.assertIs(caught.exception, error)
                self.assert_recorded_step(timed_out=False, interrupted=True)

    def test_deadline_does_not_override_unverified_cleanup(self):
        with self.assertRaisesRegex(fixture.FixtureFailure, "verify owned command cleanup"):
            self.run_inventory(10.9, self.probe_timeout, clean=False)
        self.assert_recorded_step(timed_out=True, interrupted=False, clean=False)

    def test_inventory_completion_before_deadline_preserves_success(self):
        self.assertEqual(self.run_inventory(10.4), (0, b""))
        self.assert_recorded_step(timed_out=False, interrupted=False)


@requires_posix_runner
class ProcessTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.output = Path(self.directory.name)
        self.receipt = {"steps": []}
        self.commands = fixture.Commands(self.output, self.receipt, lambda: None)

    def tearDown(self):
        self.assertTrue(self.commands.finish())
        self.directory.cleanup()

    def test_private_stdin_does_not_appear_in_receipts_or_logs(self):
        private_input = b"fixture-private-stdin-must-never-be-logged"
        result, output = self.commands.run(
            "private-stdin",
            [sys.executable, "-c", "import sys; assert sys.stdin.buffer.read(); print('received')"],
            data=private_input, timeout=3,
        )
        self.assertEqual((result, output), (0, b"received\n"))
        self.assertTrue(self.receipt["steps"][0]["owned_processes_stopped"])
        for path in self.output.iterdir():
            self.assertNotIn(private_input, path.read_bytes())

    def test_nonzero_exit_is_preserved_before_exception(self):
        with self.assertRaises(fixture.FixtureFailure):
            self.commands.run("expected-failure", [
                sys.executable, "-c", "import sys; print('bounded failure',file=sys.stderr); sys.exit(7)",
            ], timeout=3)
        step = self.receipt["steps"][0]
        self.assertEqual(step["returncode"], 7)
        self.assertIn(b"bounded failure", (self.output / step["stderr_log"]).read_bytes())
        self.assertTrue(step["owned_processes_stopped"])

    def test_timeout_drains_birth_verified_child_and_grandchild(self):
        child_pid = self.output / "child.pid"
        script = (
            "import subprocess,sys,time,pathlib; "
            "p=subprocess.Popen([sys.executable,'-c','import time;time.sleep(30)']); "
            "pathlib.Path(sys.argv[1]).write_text(str(p.pid)); time.sleep(30)"
        )
        with self.assertRaises(fixture.FixtureFailure):
            self.commands.run("expected-timeout", [
                sys.executable, "-c", script, str(child_pid),
            ], timeout=0.8)
        step = self.receipt["steps"][0]
        self.assertTrue(step["timed_out"])
        self.assertFalse(step["interrupted"])
        self.assertTrue(step["owned_processes_stopped"])
        self.assertIsNotNone(step["returncode"])
        grandchild_pid = int(child_pid.read_text())
        self.assertIn(grandchild_pid, self.commands.identities[self.commands.processes[0]])
        identity = fixture.ownership.process_info(grandchild_pid)
        self.assertTrue(identity is None or identity[3].startswith("Z"))
        saved = json.loads((self.output / "progress.json").read_text())
        self.assertEqual(saved["steps"][0], step)


if __name__ == "__main__":
    unittest.main()
