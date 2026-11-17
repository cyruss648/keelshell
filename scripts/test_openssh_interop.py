"""Peer-input, deadline and cleanup controls; never start or signal a real process."""

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

import openssh_interop as interop


@unittest.skipUnless(os.name == "posix", "The OpenSSH harness requires POSIX")
class ForwardingPolicyTests(unittest.TestCase):
    def _run_without_processes(self, forwarding):
        with tempfile.TemporaryDirectory(prefix="keelshell-forwarding-policy-") as scratch:
            root = Path(scratch)
            output = root / "receipt"
            server = MagicMock(pid=101, returncode=None)
            server.poll.return_value = None
            tests = MagicMock(pid=102, returncode=0)
            tests.poll.return_value = 0
            observed = {}

            def start(_runner, name, command, environment=None):
                observed[name] = list(command)
                if name == "sshd":
                    observed["config"] = Path(command[-1]).read_text()
                    return server
                self.assertEqual(name, "tests")
                (output / "tests.log").write_text(
                    "test result: ok. 6 passed; 0 failed; 0 ignored;\n"
                )
                return tests

            arguments = ["openssh_interop.py", "--output", str(output)]
            if forwarding:
                arguments.append("--forwarding")
            with patch.object(interop, "ROOT", root), \
                    patch.object(interop.platform, "system", return_value="Linux"), \
                    patch.object(sys, "argv", arguments), \
                    patch.object(interop.os, "umask"), \
                    patch.object(interop.signal, "signal", return_value=None), \
                    patch.object(interop, "executable", side_effect=lambda name, *rest: "/fixture-" + name), \
                    patch.object(interop.Runner, "run"), \
                    patch.object(interop.Runner, "start", autospec=True, side_effect=start), \
                    patch.object(interop.Runner, "wait"), \
                    patch.object(interop, "wait_for_server"), \
                    patch.object(interop.subprocess, "Popen") as popen, \
                    contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(interop.main(), 0)
                popen.assert_not_called()
            self.assertTrue(json.loads((output / "result.json").read_text())["cleanup"]["temporary_directory_removed"])
            return observed

    def test_forwarding_suite_enables_only_the_loopback_forwarding_policy(self):
        observed = self._run_without_processes(True)
        config = observed["config"].splitlines()
        self.assertIn("ListenAddress 127.0.0.1", config)
        self.assertIn("AllowTcpForwarding yes", config)
        self.assertIn("PermitOpen 127.0.0.1:*", config)
        self.assertIn("PermitListen 127.0.0.1:*", config)
        self.assertIn("GatewayPorts no", config)
        self.assertIn("AllowStreamLocalForwarding no", config)
        self.assertIn("PermitTunnel no", config)
        self.assertIn("PermitTTY no", config)
        command = observed["tests"]
        self.assertEqual(command[command.index("--test") + 1], "forwarding_openssh")
        self.assertIn("--ignored", command)
        self.assertNotIn("zmodem_openssh", command)

    def test_ordinary_suite_keeps_tcp_forwarding_disabled(self):
        observed = self._run_without_processes(False)
        self.assertIn("AllowTcpForwarding no", observed["config"].splitlines())
        self.assertEqual(
            observed["tests"][observed["tests"].index("--test") + 1],
            "openssh_interop",
        )

    def test_forwarding_cannot_be_combined_with_another_suite(self):
        for other in ("--zmodem", "--monitor-darwin"):
            with self.subTest(other=other), \
                    patch.object(sys, "argv", ["openssh_interop.py", "--forwarding", other]), \
                    patch.object(interop, "create_output") as output, \
                    contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as result:
                    interop.main()
                self.assertEqual(result.exception.code, 2)
                output.assert_not_called()


class ZmodemPeerArgsTests(unittest.TestCase):
    def test_only_undefined_arguments_get_the_legacy_default(self):
        first = interop.zmodem_peer_args(None, "PEER_ARGS")
        self.assertEqual(first, ["-b", "-vv"])
        first.append("changed")
        self.assertEqual(interop.zmodem_peer_args(None, "PEER_ARGS"), ["-b", "-vv"])
        self.assertEqual(interop.zmodem_peer_args("[]", "PEER_ARGS"), [])
        with self.assertRaises(interop.InteropFailure):
            interop.zmodem_peer_args("", "PEER_ARGS")

    def test_empty_unicode_quotes_spaces_and_shell_syntax_remain_literal(self):
        arguments = ["", "目录 😀", "\u0085", 'a "quoted" value', "a 'quoted' value", r"a\b", "$HOME", "$(touch marker)", "`id`", "*;|&<>"]
        self.assertEqual(
            interop.zmodem_peer_args(json.dumps(arguments, ensure_ascii=False), "PEER_ARGS"),
            arguments,
        )

    def test_json_requires_an_array_containing_only_strings(self):
        for raw in ("null", "{}", '"-b"', "[1]", "[true]", "[null]", "[[]]", "[NaN]", '["x",]', "["):
            with self.subTest(raw=raw), self.assertRaises(interop.InteropFailure):
                interop.zmodem_peer_args(raw, "PEER_ARGS")

    def test_json_byte_budget_accepts_exact_limit_and_rejects_before_parsing(self):
        raw = '["é"]'
        raw += " " * (16_384 - len(raw.encode("utf-8")))
        self.assertEqual(interop.zmodem_peer_args(raw, "PEER_ARGS"), ["é"])
        with patch.object(interop.json, "loads") as loads:
            with self.assertRaisesRegex(interop.InteropFailure, "16384-byte"):
                interop.zmodem_peer_args(raw + " ", "PEER_ARGS")
        loads.assert_not_called()

    def test_argument_count_accepts_32_empty_values_and_rejects_33(self):
        self.assertEqual(interop.zmodem_peer_args(json.dumps([""] * 32), "PEER_ARGS"), [""] * 32)
        with self.assertRaisesRegex(interop.InteropFailure, "32-argument"):
            interop.zmodem_peer_args(json.dumps([""] * 33), "PEER_ARGS")

    def test_individual_argument_budget_counts_utf8_bytes(self):
        for value in ("x" * 2048, "é" * 1024, "😀" * 512):
            with self.subTest(bytes=len(value.encode("utf-8"))):
                self.assertEqual(interop.zmodem_peer_args(json.dumps([value]), "PEER_ARGS"), [value])
        for value in ("x" * 2049, "é" * 1025, "😀" * 513):
            with self.subTest(bytes=len(value.encode("utf-8"))), self.assertRaisesRegex(interop.InteropFailure, "2048-byte"):
                interop.zmodem_peer_args(json.dumps([value]), "PEER_ARGS")

    def test_total_argument_budget_counts_utf8_bytes(self):
        for value in ("x" * 2048, "é" * 1024, "😀" * 512):
            arguments = [value] * 4
            with self.subTest(bytes=len(value.encode("utf-8"))):
                self.assertEqual(interop.zmodem_peer_args(json.dumps(arguments, ensure_ascii=False), "PEER_ARGS"), arguments)
                with self.assertRaisesRegex(interop.InteropFailure, "8192-byte"):
                    interop.zmodem_peer_args(json.dumps([*arguments, "x"], ensure_ascii=False), "PEER_ARGS")

    def test_every_decoded_c0_and_del_control_is_rejected(self):
        for code in (*range(32), 127):
            with self.subTest(code=code), self.assertRaisesRegex(interop.InteropFailure, "control character"):
                interop.zmodem_peer_args(json.dumps(["before" + chr(code) + "after"]), "PEER_ARGS")

    def test_invalid_utf8_and_escaped_lone_surrogates_are_rejected(self):
        for raw in ('["\ud800"]', '["\\ud800"]', '["\\udfff"]'):
            with self.subTest(raw=ascii(raw)), self.assertRaisesRegex(interop.InteropFailure, "UTF-8"):
                interop.zmodem_peer_args(raw, "PEER_ARGS")

    def test_nested_input_and_parser_diagnostics_do_not_escape_as_raw_exceptions(self):
        for raw in ("[" * 1100 + "]" * 1100, '["private-control-marker'):
            with self.subTest(length=len(raw)), self.assertRaises(interop.InteropFailure) as failure:
                interop.zmodem_peer_args(raw, "PEER_ARGS")
            self.assertNotIn("private-control-marker", str(failure.exception))


@unittest.skipUnless(os.name == "posix", "The OpenSSH harness requires POSIX")
class ZmodemPeerPreflightTests(unittest.TestCase):
    def setUp(self):
        self.scratch = self.enterContext(tempfile.TemporaryDirectory(prefix="keelshell-zmodem-inputs-"))
        self.root = Path(self.scratch)
        self.environment = {}
        self.contents = {}
        for role in ("RZ", "SZ"):
            path = self.root / (role.lower() + ' 空格 "peer" \'quote\'')
            content = b"#!/fixture-not-executed\n" + role.encode("ascii") * (1024 * 1024 + 3)
            path.write_bytes(content)
            path.chmod(0o700)
            self.environment["KEELSHELL_ZMODEM_" + role] = str(path)
            self.contents[role.lower()] = content

    def _preflight(self, environment):
        return interop.zmodem_peers(environment, interop.time.monotonic() + 10, lambda: None)

    def test_exact_peer_paths_args_and_streamed_executable_bytes_are_bound(self):
        self.environment["KEELSHELL_ZMODEM_RZ_ARGS"] = '["", "目录", "a \\"quote\\""]'
        self.environment["KEELSHELL_ZMODEM_SZ_ARGS"] = "[]"
        peers = self._preflight(self.environment)
        for role, arguments in (("rz", ["", "目录", 'a "quote"']), ("sz", [])):
            self.assertEqual(peers[role], {
                "path": self.environment["KEELSHELL_ZMODEM_" + role.upper()],
                "args": arguments,
                "sha256": hashlib.sha256(self.contents[role]).hexdigest(),
                "bytes": len(self.contents[role]),
            })

    def test_executable_symlink_name_is_preserved(self):
        alias = self.root / "multicall-rz"
        alias.symlink_to(self.environment["KEELSHELL_ZMODEM_RZ"])
        self.environment["KEELSHELL_ZMODEM_RZ"] = str(self.root) + "/./multicall-rz"
        peer = self._preflight(self.environment)["rz"]
        self.assertEqual(peer["path"], str(self.root) + "/./multicall-rz")
        self.assertEqual(peer["sha256"], hashlib.sha256(self.contents["rz"]).hexdigest())

    def test_missing_empty_relative_nonfile_and_nonexecutable_peers_are_rejected(self):
        nonexecutable = self.root / "not-executable"
        nonexecutable.write_bytes(b"fixture")
        nonexecutable.chmod(0o600)
        for role in ("RZ", "SZ"):
            variable = "KEELSHELL_ZMODEM_" + role
            for value in (None, "", "relative-peer", str(self.root / "missing"), str(self.root), str(nonexecutable)):
                environment = dict(self.environment)
                if value is None:
                    del environment[variable]
                else:
                    environment[variable] = value
                with self.subTest(role=role, value=value), self.assertRaises(interop.InteropFailure):
                    self._preflight(environment)

    def test_peer_paths_reject_c0_del_and_c1_before_filesystem_lookup(self):
        for code in (*range(32), *range(127, 160)):
            environment = dict(self.environment, KEELSHELL_ZMODEM_RZ="/peer" + chr(code))
            with self.subTest(code=code), patch.object(interop.Path, "is_file") as is_file:
                with self.assertRaisesRegex(interop.InteropFailure, "control characters"):
                    self._preflight(environment)
                is_file.assert_not_called()

    def test_unreadable_peer_is_a_sanitized_preflight_failure(self):
        error = PermissionError("private-control-marker")
        with patch.object(interop.Path, "open", side_effect=error):
            with self.assertRaises(interop.InteropFailure) as failure:
                self._preflight(self.environment)
        self.assertIs(failure.exception.__cause__, error)
        self.assertNotIn("private-control-marker", str(failure.exception))

    def test_hashing_observes_the_existing_work_deadline(self):
        with patch.object(interop.time, "monotonic", return_value=10):
            with self.assertRaisesRegex(interop.InteropFailure, "deadline"):
                interop.zmodem_peers(self.environment, 10, lambda: None)

    def test_pending_interrupt_is_observed_before_opening_peer_contents(self):
        check = MagicMock(side_effect=interop.Interrupted("requested"))
        with patch.object(interop.Path, "open") as opened:
            with self.assertRaises(interop.Interrupted):
                interop.zmodem_peers(self.environment, interop.time.monotonic() + 10, check)
        check.assert_called_once_with()
        opened.assert_not_called()

    def test_interrupt_during_hashing_preserves_interrupted_receipt_without_ssh_setup(self):
        original_open = Path.open
        for requested in (interop.signal.SIGINT, interop.signal.SIGTERM):
            handlers, closed, reads = {}, [], []
            output = self.root / ("interrupted-" + str(requested))

            def register(signum, handler):
                if callable(handler):
                    handlers[signum] = handler
                return None

            def read_chunk(_size):
                reads.append(True)
                self.assertEqual(len(reads), 1, "interrupt must stop the next hash read")
                handlers[requested](requested, None)
                return b"partial peer contents"

            @contextlib.contextmanager
            def peer_contents():
                try:
                    yield type("Peer", (), {"read": staticmethod(read_chunk)})()
                finally:
                    closed.append(True)

            def open_path(path, mode="r", *args, **kwargs):
                if mode == "rb" and str(path) == self.environment["KEELSHELL_ZMODEM_RZ"]:
                    return peer_contents()
                return original_open(path, mode, *args, **kwargs)

            with self.subTest(signal=requested), \
                    patch.dict(os.environ, self.environment, clear=True), \
                    patch.object(interop, "ROOT", self.root), \
                    patch.object(interop.platform, "system", return_value="Linux"), \
                    patch.object(sys, "argv", ["openssh_interop.py", "--zmodem", "--output", str(output)]), \
                    patch.object(interop.os, "umask"), \
                    patch.object(interop.signal, "signal", side_effect=register), \
                    patch.object(Path, "open", open_path), \
                    patch.object(interop.tempfile, "mkdtemp") as mkdtemp, \
                    patch.object(interop, "executable") as executable, \
                    patch.object(interop.subprocess, "Popen") as popen, \
                    contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(interop.main(), 130)
                mkdtemp.assert_not_called()
                executable.assert_not_called()
                popen.assert_not_called()
            self.assertEqual(reads, [True])
            self.assertEqual(closed, [True])
            receipt = json.loads((output / "result.json").read_text())
            self.assertEqual(receipt["outcome"], "interrupted")
            self.assertEqual(receipt["processes"], [])
            self.assertNotIn("temporary_directory", receipt)
            self.assertTrue(receipt["cleanup"]["owned_processes_stopped"])
            self.assertTrue(receipt["cleanup"]["temporary_directory_removed"])

    def test_bad_inputs_never_create_ssh_resources_or_start_keygen_or_server(self):
        invalid = []
        for role in ("RZ", "SZ"):
            variable = "KEELSHELL_ZMODEM_" + role
            invalid.extend((
                {variable: None},
                {variable: ""},
                {variable: str(self.root / "missing")},
                {variable: "relative-peer"},
                {variable: "/peer\nprivate-control-marker"},
                {variable + "_ARGS": '["private-control-marker'},
                {variable + "_ARGS": json.dumps(["\0"])},
                {variable + "_ARGS": json.dumps(["x" * 2049])},
                {variable + "_ARGS": json.dumps(["x" * 2048] * 4 + ["x"])},
                {variable + "_ARGS": json.dumps([""] * 33)},
                {variable + "_ARGS": " " * 16_385},
            ))
        nonexecutable = self.root / "nonexecutable"
        nonexecutable.write_bytes(b"fixture")
        nonexecutable.chmod(0o600)
        invalid.append({"KEELSHELL_ZMODEM_RZ": str(nonexecutable)})
        invalid.append({"KEELSHELL_ZMODEM_SZ": str(nonexecutable)})
        for index, changes in enumerate(invalid):
            output = self.root / ("failed-" + str(index))
            environment = dict(self.environment)
            for variable, value in changes.items():
                if value is None:
                    del environment[variable]
                else:
                    environment[variable] = value
            stdout, stderr = io.StringIO(), io.StringIO()
            with self.subTest(changes=changes), \
                    patch.dict(os.environ, environment, clear=True), \
                    patch.object(interop, "ROOT", self.root), \
                    patch.object(interop.platform, "system", return_value="Linux"), \
                    patch.object(sys, "argv", ["openssh_interop.py", "--zmodem", "--output", str(output)]), \
                    patch.object(interop.os, "umask"), \
                    patch.object(interop.signal, "signal", return_value=None), \
                    patch.object(interop.tempfile, "mkdtemp") as mkdtemp, \
                    patch.object(interop, "executable") as executable, \
                    patch.object(interop.Runner, "run") as run, \
                    patch.object(interop.Runner, "start") as start, \
                    patch.object(interop.subprocess, "Popen") as popen, \
                    patch.object(interop.subprocess, "run") as subprocess_run, \
                    contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                self.assertEqual(interop.main(), 1)
                mkdtemp.assert_not_called()
                executable.assert_not_called()
                run.assert_not_called()
                start.assert_not_called()
                popen.assert_not_called()
                subprocess_run.assert_not_called()
            self.assertFalse((self.root / "work" / ".openssh-private").exists())
            text = (output / "result.json").read_text(encoding="utf-8")
            receipt = json.loads(text)
            self.assertEqual(receipt["outcome"], "failed")
            self.assertEqual(receipt["processes"], [])
            self.assertNotIn("temporary_directory", receipt)
            self.assertTrue(receipt["cleanup"]["owned_processes_stopped"])
            self.assertTrue(receipt["cleanup"]["temporary_directory_removed"])
            self.assertNotIn("private-control-marker", text + stdout.getvalue() + stderr.getvalue())

    def _run_mocked_zmodem(self, overrides, mutate_environment=False):
        environment = dict(self.environment, **overrides, UNRELATED_CREDENTIAL="private-control-marker")
        expected = self._preflight(environment)
        output = self.root / "passed"
        server = MagicMock(pid=101, returncode=None)
        server.poll.return_value = None
        tests = MagicMock(pid=102, returncode=0)
        tests.poll.return_value = 0

        def find_executable(name, fallbacks=()):
            if mutate_environment:
                # A later environment read must not replace the inputs already
                # checked and bound to the receipt.
                os.environ["KEELSHELL_ZMODEM_RZ"] = "/different-peer"
                os.environ["KEELSHELL_ZMODEM_RZ_ARGS"] = '["changed"]'
            return "/fixture-" + name

        def popen(command, **kwargs):
            if command[0] == "/fixture-cargo":
                kwargs["stdout"].write(b"test result: ok. 8 passed; 0 failed; 0 ignored;\n")
                kwargs["stdout"].flush()
                return tests
            self.assertEqual(command[0], "/fixture-sshd")
            return server

        with patch.dict(os.environ, environment, clear=True), \
                patch.object(interop, "ROOT", self.root), \
                patch.object(interop.platform, "system", return_value="Linux"), \
                patch.object(sys, "argv", ["openssh_interop.py", "--zmodem", "--output", str(output)]), \
                patch.object(interop.os, "umask"), \
                patch.object(interop.signal, "signal", return_value=None), \
                patch.object(interop, "executable", side_effect=find_executable), \
                patch.object(interop.Runner, "run"), \
                patch.object(interop.Runner, "capture_owned"), \
                patch.object(interop, "process_info", return_value=(1, 101, ("kernel", 7), "R")), \
                patch.object(interop, "wait_for_server"), \
                patch.object(interop, "stop_owned", return_value=True), \
                patch.object(interop.subprocess, "run") as subprocess_run, \
                patch.object(interop.subprocess, "Popen", side_effect=popen) as start, \
                contextlib.redirect_stdout(io.StringIO()), \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(interop.main(), 0)
            subprocess_run.assert_not_called()
        self.assertEqual(start.call_count, 2)
        call = start.call_args
        command = list(interop.TEST_COMMAND)
        command[0] = "/fixture-cargo"
        command[command.index("openssh_interop")] = "zmodem_openssh"
        command.append("--show-output")
        self.assertEqual(call.args[0], command)
        self.assertNotIn("shell", call.kwargs)
        child = call.kwargs["env"]
        receipt_text = (output / "result.json").read_text(encoding="utf-8")
        receipt = json.loads(receipt_text)
        self.assertEqual(receipt["zmodem_peers"], expected)
        for role, peer in expected.items():
            variable = "KEELSHELL_ZMODEM_" + role.upper()
            self.assertEqual(child[variable], peer["path"])
            self.assertEqual(child[variable + "_ARGS"], json.dumps(peer["args"], ensure_ascii=False, separators=(",", ":")))
            self.assertEqual(interop.zmodem_peer_args(child[variable + "_ARGS"], variable + "_ARGS"), peer["args"])
        self.assertEqual(receipt["passed_tests"], 8)
        self.assertTrue(receipt["cleanup"]["owned_processes_stopped"])
        self.assertTrue(receipt["cleanup"]["temporary_directory_removed"])
        self.assertFalse(Path(receipt["temporary_directory"]).exists())
        self.assertNotIn("private-control-marker", receipt_text)
        return child

    def test_child_receives_both_normalized_legacy_default_arrays(self):
        child = self._run_mocked_zmodem({})
        for role in ("RZ", "SZ"):
            self.assertEqual(child["KEELSHELL_ZMODEM_" + role + "_ARGS"], '["-b","-vv"]')

    def test_independent_peer_arrays_survive_dispatch_without_shell_expansion(self):
        arguments = ["", "目录 😀", 'space "quote"', "space 'quote'", "$(touch marker)", "`id`", "$HOME", "*;|&<>", r"a\b"]
        child = self._run_mocked_zmodem({
            "KEELSHELL_ZMODEM_RZ_ARGS": json.dumps(arguments, ensure_ascii=True, indent=2),
            "KEELSHELL_ZMODEM_SZ_ARGS": "[]",
        })
        self.assertEqual(json.loads(child["KEELSHELL_ZMODEM_RZ_ARGS"]), arguments)
        self.assertEqual(child["KEELSHELL_ZMODEM_SZ_ARGS"], "[]")
        self.assertFalse((self.root / "marker").exists())

    def test_dispatch_uses_preflight_paths_and_args_after_environment_changes(self):
        self._run_mocked_zmodem({"KEELSHELL_ZMODEM_RZ_ARGS": ' [ "original" ] '}, mutate_environment=True)


class ProcessInventoryTests(unittest.TestCase):
    def test_darwin_monitor_suite_rejects_linux_before_creating_resources(self):
        with patch.object(sys, "argv", ["openssh_interop.py", "--monitor-darwin"]), \
                patch.object(interop.platform, "system", return_value="Linux"), \
                patch.object(interop, "create_output") as create, \
                contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as failure:
                interop.main()
        self.assertEqual(failure.exception.code, 2)
        create.assert_not_called()

    def test_monitor_and_zmodem_suites_cannot_share_an_acceptance_run(self):
        with patch.object(sys, "argv", ["openssh_interop.py", "--monitor-darwin", "--zmodem"]), \
                patch.object(interop, "create_output") as create, \
                contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as failure:
                interop.main()
        self.assertEqual(failure.exception.code, 2)
        create.assert_not_called()

    def test_loaded_host_has_a_bounded_three_second_probe(self):
        result = subprocess.CompletedProcess([], 0, stdout="101\n")
        identity = (1, 101, ("kernel", 7), "R")
        with patch.object(interop.time, "monotonic", return_value=10), \
                patch.object(interop.subprocess, "run", return_value=result) as run, \
                patch.object(interop, "process_info", return_value=identity):
            self.assertEqual(interop.process_table(20), {101: identity})
        self.assertEqual(run.call_args.kwargs["timeout"], 3.0)

    def test_short_remaining_deadline_is_not_extended(self):
        result = subprocess.CompletedProcess([], 0, stdout="")
        with patch.object(interop.time, "monotonic", return_value=10), \
                patch.object(interop.subprocess, "run", return_value=result) as run:
            self.assertEqual(interop.process_table(10.2), {})
        self.assertAlmostEqual(run.call_args.kwargs["timeout"], 0.2)

    def test_expired_deadline_does_not_start_ps(self):
        with patch.object(interop.time, "monotonic", return_value=10), \
                patch.object(interop.subprocess, "run") as run:
            with self.assertRaises(interop.InteropFailure):
                interop.process_table(10)
        run.assert_not_called()

    def test_timeout_fails_closed_with_original_cause(self):
        timeout = subprocess.TimeoutExpired(["ps", "-eo", "pid="], 3)
        with patch.object(interop.time, "monotonic", return_value=10), \
                patch.object(interop.subprocess, "run", side_effect=timeout):
            with self.assertRaises(interop.InteropFailure) as failure:
                interop.process_table(20)
        self.assertIs(failure.exception.__cause__, timeout)

    def test_inventory_identity_reads_observe_the_same_deadline(self):
        result = subprocess.CompletedProcess([], 0, stdout="101\n")
        with patch.object(interop.time, "monotonic", side_effect=[10, 20]), \
                patch.object(interop.subprocess, "run", return_value=result), \
                patch.object(interop, "process_info") as identity:
            with self.assertRaises(interop.InteropFailure):
                interop.process_table(20)
        identity.assert_not_called()

    @unittest.skipUnless(os.name == "posix", "The OpenSSH harness requires POSIX")
    def test_timeout_preserves_owned_cleanup_and_failed_receipt(self):
        process = MagicMock(pid=101)
        process.poll.return_value = None
        identity = (1, 101, ("kernel", 7), "R")
        failure = interop.InteropFailure("Process inventory exceeded its bounded deadline")
        with tempfile.TemporaryDirectory(prefix="keelshell-interop-test-") as scratch:
            root = Path(scratch)
            output = root / "receipt"
            with patch.object(interop, "ROOT", root), \
                    patch.object(sys, "argv", ["openssh_interop.py", "--output", str(output)]), \
                    patch.object(interop.os, "umask"), \
                    patch.object(interop.signal, "signal", return_value=None), \
                    patch.object(interop, "executable", return_value="owned-fixture"), \
                    patch.object(interop.subprocess, "Popen", return_value=process), \
                    patch.object(interop, "process_info", return_value=identity), \
                    patch.object(interop, "process_table", side_effect=failure), \
                    patch.object(interop, "signal_owned", return_value=True) as signal_owned, \
                    contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(interop.main(), 1)
            receipt = json.loads((output / "result.json").read_text(encoding="utf-8"))
            self.assertEqual(receipt["outcome"], "failed")
            self.assertFalse(receipt["cleanup"]["owned_processes_stopped"])
            self.assertTrue(receipt["cleanup"]["temporary_directory_removed"])
            self.assertFalse(Path(receipt["temporary_directory"]).exists())
            signal_owned.assert_called_once_with(101, identity, interop.signal.SIGKILL)
            process.wait.assert_called_once()


if __name__ == "__main__":
    unittest.main()
