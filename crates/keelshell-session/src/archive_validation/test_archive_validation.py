"""Offline helper/process regressions. Fixtures and archive payloads stay in memory."""
import sys

sys.dont_write_bytecode = True

import ast
import errno
import gzip
import io
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import tarfile
import time
import types
import unittest
from unittest import mock

SOURCE = Path(__file__).resolve().parents[1] / "archive_validation.py"
HELPER = SOURCE.read_text()
probe = types.ModuleType("keelshell_owned_archive_validation")
exec(compile(HELPER, str(SOURCE), "exec"), probe.__dict__)


def run_python(code, data=b"", seconds=0.25, budget=None):
    with mock.patch.object(probe, "COMMAND_SECONDS", seconds):
        return probe.run_owned([sys.executable, "-I", "-S", "-c", code], data,
                               probe.clean_environment(), time.monotonic() + 3,
                               [probe.TOTAL_OUTPUT] if budget is None else budget)


@unittest.skipUnless(os.name == "posix", "remote helper checks require POSIX")
class HelperTests(unittest.TestCase):
    def setUp(self):
        probe.STOPPED = False

    def test_actual_review_command_has_no_user_arguments_or_persistent_script(self):
        command = "python3 -I -S -c '" + HELPER.replace("'", "'\\''") + "'"
        argv = shlex.split(command)
        self.assertEqual(argv, ["python3", "-I", "-S", "-c", HELPER])
        # Run the actual complete review command, with the same subprocess ownership limits.
        state, code, out, err = probe.run_owned(
            ["/bin/sh", "-c", command], b"", probe.clean_environment(),
            time.monotonic() + 9, [32768])
        self.assertEqual((state, code, err), ("passed", 0, b""))
        report = json.loads(out)
        self.assertEqual(set(report), {"protocol", "tools"})
        self.assertEqual(report["protocol"], probe.PROTOCOL)
        self.assertEqual([row["tool"] for row in report["tools"]], list(probe.TOOLS))
        for row in report["tools"]:
            self.assertEqual(set(row), {"tool", "path", "version", "version_status", "stream_check"})
            if row["tool"] in ("zip", "unzip", "rz", "sz"):
                self.assertEqual(row["stream_check"], "not_tested")
            if row["version_status"] == "passed":
                self.assertTrue(probe.safe_text(row["version"], probe.MAX_VERSION))
                self.assertTrue(os.path.isabs(row["path"]))

    def test_fixed_tar_and_gzip_samples_are_regular_in_memory_payloads(self):
        with mock.patch("builtins.open", side_effect=AssertionError("file access")):
            data = probe.tar_input()
            self.assertLessEqual(len(data), 10240)
            with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
                members = archive.getmembers()
                self.assertEqual([entry.name for entry in members], [probe.MEMBER])
                self.assertTrue(members[0].isfile())
                self.assertEqual(archive.extractfile(members[0]).read(), probe.PAYLOAD)
            self.assertEqual(gzip.decompress(gzip.compress(probe.PAYLOAD, mtime=0)), probe.PAYLOAD)

    def test_actual_isolated_python_roundtrip_uses_only_bytesio(self):
        state, code, out, err = run_python(probe.PYTHON_STREAM, seconds=2)
        self.assertEqual((state, code, out, err), ("passed", 0, probe.PAYLOAD, b""))
        # Audit actual roundtrip file arguments: only BytesIO, never path-based archive APIs.
        calls = [node for node in ast.walk(ast.parse(probe.PYTHON_STREAM)) if isinstance(node, ast.Call)]
        self.assertFalse(any(isinstance(node.func, ast.Name) and node.func.id == "open" for node in calls))
        self.assertNotIn("extractall", probe.PYTHON_STREAM)
        self.assertIn("compression=zipfile.ZIP_DEFLATED", probe.PYTHON_STREAM)
        self.assertIn("compress_type != zipfile.ZIP_DEFLATED", probe.PYTHON_STREAM)

    def test_actual_helper_performs_no_file_mutations_or_network_calls(self):
        driver = """import sys
sys.dont_write_bytecode = True
import gzip,io,json,os,selectors,signal,stat,subprocess,tarfile,time
def audit(event, args):
    if event in ("os.mkdir", "os.remove", "os.rename", "os.rmdir", "tempfile.mkstemp", "tempfile.mkdtemp"):
        raise AssertionError("unexpected filesystem mutation")
    if event == "open" and (args[2] & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND)):
        if not isinstance(args[0], int) or not stat.S_ISFIFO(os.fstat(args[0]).st_mode):
            raise AssertionError("unexpected file write")
    if event.startswith("socket."):
        raise AssertionError("unexpected network call")
sys.addaudithook(audit)
""" + HELPER
        state, code, out, err = run_python(driver, seconds=3)
        self.assertEqual((state, code, err), ("passed", 0, b""))
        self.assertEqual(json.loads(out)["protocol"], probe.PROTOCOL)

    def test_tool_dispatch_is_fixed_and_function_checks_never_run_zip_or_zmodem(self):
        seen = []
        def run(argv, data, env, deadline, budget):
            seen.append((argv, data, env))
            if argv[1:] in [list(args) for args in probe.VERSION_ARGS]:
                return "passed", 0, b"observed version 1\n", b""
            return "passed", 0, probe.PAYLOAD, b""
        with mock.patch.object(probe, "resolve", side_effect=lambda tool, env: "/fixed/" + tool), \
                mock.patch.object(probe, "run_owned", side_effect=run):
            report = probe.collect()
        expected = []
        for tool, args in zip(probe.TOOLS, probe.VERSION_ARGS):
            expected.append(["/fixed/" + tool, *args])
            if tool == "tar":
                expected.append(["/fixed/tar", "-xOf", "-", probe.MEMBER])
            elif tool == "gzip":
                expected.append(["/fixed/gzip", "-dc"])
            elif tool == "python3":
                expected.append(["/fixed/python3", "-I", "-S", "-c", probe.PYTHON_STREAM])
        self.assertEqual([argv for argv, _data, _env in seen], expected)
        self.assertEqual(len(seen), 10)
        for argv, data, env in seen:
            self.assertEqual(set(env), {"PATH", "LANG", "LC_ALL", "TZ"})
            if argv[0] in ("/fixed/zip", "/fixed/unzip", "/fixed/rz", "/fixed/sz"):
                self.assertEqual(data, b"")
        self.assertTrue(all(row["stream_check"] == "not_tested"
                            for row in report["tools"] if row["tool"] in ("zip", "unzip", "rz", "sz")))

    def test_environment_allowlist_excludes_options_loader_hooks_and_relative_paths(self):
        poisoned = {"PATH": ".:/absolute/bin:relative::/safe/归档", "TAR_OPTIONS": "--to-command=bad",
                    "GZIP": "-f", "ZIPOPT": "bad", "UNZIP": "bad", "UNZIPOPT": "bad",
                    "ZIPINFO": "bad", "ZIPINFOOPT": "bad", "LD_PRELOAD": "bad",
                    "DYLD_INSERT_LIBRARIES": "bad", "PYTHONPATH": "bad", "HOME": "/private"}
        with mock.patch.dict(os.environ, poisoned, clear=True):
            env = probe.clean_environment()
        self.assertEqual(env, {"PATH": "/absolute/bin:/safe/归档", "LANG": "C", "LC_ALL": "C", "TZ": "UTC"})

    def test_unsafe_and_missing_resolutions_do_not_dispatch_any_process(self):
        with mock.patch.object(probe, "resolve", return_value=None), \
                mock.patch.object(probe, "run_owned", side_effect=AssertionError("dispatch")):
            report = probe.collect()
        self.assertEqual(len(report["tools"]), 7)
        for row in report["tools"]:
            self.assertEqual((row["path"], row["version"], row["version_status"]),
                             (None, None, "unavailable"))
        with mock.patch.dict(os.environ, {"PATH": "/bin/\x1b:.:relative:/safe/\u202e"}, clear=True):
            env = probe.clean_environment()
            self.assertEqual(env["PATH"], "")
            self.assertIsNone(probe.resolve("tar", env))

    def test_version_text_is_safe_bounded_utf8_without_authenticity_parsing(self):
        self.assertEqual(probe.version_line(b"\nobserved arbitrary banner 1\r\n", b""), "observed arbitrary banner 1")
        self.assertEqual(probe.version_line(b"", b"stderr version 2\n"), "stderr version 2")
        self.assertEqual(probe.version_line(b"v" * 256, b""), "v" * 256)
        for out in (b"", b"\xff", b"v" * 257, b"x\n\x1b[31mhidden", b"x\rforged"):
            self.assertIsNone(probe.version_line(out, b""), out)
        for char in ("\0", "\t", "\u0085", "\u061c", "\u200b", "\u202e", "\u2066", "\ufeff", "\ufffd"):
            self.assertIsNone(probe.version_line(("version" + char).encode(), b""))
            self.assertFalse(probe.safe_text("/bin/" + char + "tar", 4096))

    def test_stream_payload_stderr_and_version_failures_are_independent(self):
        def run(argv, *args):
            if argv[1:] == ["--version"]:
                return "failed", 2, b"untrusted error", b""
            return "passed", 0, probe.PAYLOAD, b""
        with mock.patch.object(probe, "resolve", side_effect=lambda tool, env: "/fixed/tar" if tool == "tar" else None), \
                mock.patch.object(probe, "run_owned", side_effect=run):
            row = probe.collect()["tools"][0]
        self.assertEqual((row["version_status"], row["version"], row["stream_check"]), ("failed", None, "passed"))
        for out, err in [(b"wrong", b""), (probe.PAYLOAD, b"warning")]:
            with mock.patch.object(probe, "resolve", side_effect=lambda tool, env: "/fixed/tar" if tool == "tar" else None), \
                    mock.patch.object(probe, "run_owned", return_value=("passed", 0, out, err)):
                self.assertEqual(probe.collect()["tools"][0]["stream_check"], "failed")

    def test_helper_rejects_user_arguments_without_running_any_checks(self):
        with mock.patch.object(probe.sys, "argv", ["-c", "unexpected"]), \
                mock.patch.object(probe, "collect", side_effect=AssertionError("dispatch")):
            self.assertEqual(probe.main(), 65)

    def test_no_adoptable_report_after_signal_or_unconfirmed_cleanup(self):
        probe.interrupted(signal.SIGTERM, None)
        with self.assertRaises(probe.CleanupUnknown):
            run_python("print('never')")
        probe.STOPPED = False
        sink = types.SimpleNamespace(buffer=io.BytesIO())
        with mock.patch.object(probe.sys, "argv", ["-c"]), \
                mock.patch.object(probe.sys, "stdout", sink), \
                mock.patch.object(probe.signal, "signal"), \
                mock.patch.object(probe, "collect", side_effect=probe.CleanupUnknown):
            self.assertEqual(probe.main(), 65)
        self.assertEqual(sink.buffer.getvalue(), b"")


@unittest.skipUnless(os.name == "posix", "remote helper process ownership requires POSIX")
class OwnedProcessTests(unittest.TestCase):
    def setUp(self):
        probe.STOPPED = False

    def test_actual_fixed_input_and_both_output_streams_are_drained_concurrently(self):
        code = ("import os,sys; os.write(2,b'fixed stderr'); "
                "data=sys.stdin.buffer.read(); os.write(1,data)")
        data = b"fixed input" * 100
        state, status, out, err = run_python(code, data, seconds=2)
        self.assertEqual((state, status, out, err), ("passed", 0, data, b"fixed stderr"))

    def test_combined_per_command_stdout_stderr_flood_is_capped_and_reaped(self):
        budget = [probe.TOTAL_OUTPUT]
        state, code, out, err = run_python(
            "import os,time; os.write(1,b'o'*2400); os.write(2,b'e'*2400); time.sleep(9)",
            budget=budget)
        self.assertEqual(state, "output_limit")
        self.assertIsNotNone(code)
        self.assertLessEqual(len(out) + len(err), probe.COMMAND_OUTPUT)
        self.assertEqual(probe.TOTAL_OUTPUT - budget[0], probe.COMMAND_OUTPUT + 1)

    def test_shared_output_budget_counts_both_streams_across_commands(self):
        budget = [100]
        result = run_python("import os; os.write(1,b'x'*40); os.write(2,b'y'*40)", budget=budget, seconds=2)
        self.assertEqual(result[0], "passed")
        self.assertEqual(budget, [20])
        result = run_python("import os,time; os.write(1,b'x'*40); time.sleep(9)", budget=budget)
        self.assertEqual(result[0], "output_limit")
        self.assertEqual(budget, [0])
        with mock.patch.object(probe.subprocess, "Popen", side_effect=AssertionError("dispatch")):
            self.assertEqual(run_python("pass", budget=budget)[0], "output_limit")

    def test_timeout_kills_and_actually_waits_the_owned_child(self):
        children = []
        actual_popen = subprocess.Popen
        def spawn(*args, **kwargs):
            self.assertIs(kwargs["shell"], False)
            self.assertIs(kwargs["start_new_session"], True)
            child = actual_popen(*args, **kwargs)
            children.append(child)
            return child
        started = time.monotonic()
        with mock.patch.object(probe.subprocess, "Popen", side_effect=spawn):
            result = run_python("import time; time.sleep(9)", seconds=0.1)
        self.assertEqual(result[:2], ("timed_out", -signal.SIGKILL))
        self.assertLess(time.monotonic() - started, 1.5)
        self.assertEqual(children[0].returncode, -signal.SIGKILL)
        with self.assertRaises(ChildProcessError):
            os.waitpid(children[0].pid, os.WNOHANG)

    def test_eof_before_exit_cannot_bypass_the_same_deadline(self):
        result = run_python("import os,time; os.close(1); os.close(2); time.sleep(9)", seconds=0.1)
        self.assertEqual(result[:2], ("timed_out", -signal.SIGKILL))

    def test_exited_leader_with_descendant_holding_pipes_is_timed_out_and_group_killed(self):
        seen_groups = []
        actual_killpg = os.killpg
        def kill_group(pid, number):
            seen_groups.append((pid, number))
            return actual_killpg(pid, number)
        code = ("import os,time; pid=os.fork(); "
                "os.write(1,str(os.getpid()).encode()+b'\\n'); "
                "time.sleep(9) if pid==0 else os._exit(0)")
        started = time.monotonic()
        with mock.patch.object(probe.os, "killpg", side_effect=kill_group):
            state, status, out, err = run_python(code, seconds=0.15)
        self.assertEqual((state, status, err), ("timed_out", 0, b""))
        self.assertEqual([number for _pid, number in seen_groups if number != 0], [signal.SIGKILL])
        self.assertGreaterEqual(len(out.splitlines()), 1)
        self.assertLess(time.monotonic() - started, 1.5)

    def test_success_kills_same_group_descendant_with_closed_stdio_before_reaping_leader(self):
        children = []
        kill_ownership = []
        actual_popen = subprocess.Popen
        actual_killpg = os.killpg
        def spawn(*args, **kwargs):
            child = actual_popen(*args, **kwargs)
            children.append(child)
            return child
        def signal_group(pid, number):
            if number == signal.SIGKILL:
                kill_ownership.append((pid, children[0].returncode))
            return actual_killpg(pid, number)
        code = """import os,time
pid = os.fork()
if pid == 0:
    for fd in (0, 1, 2):
        os.close(fd)
    time.sleep(9)
    os._exit(0)
os.write(1, ("%d %d\\n" % (os.getpgrp(), pid)).encode())
os._exit(0)
"""
        descendant = group = None
        try:
            with mock.patch.object(probe.subprocess, "Popen", side_effect=spawn), \
                    mock.patch.object(probe.os, "killpg", side_effect=signal_group):
                state, status, out, err = run_python(code, seconds=1)
            group, descendant = map(int, out.split())
            self.assertEqual((state, status, err), ("passed", 0, b""))
            self.assertEqual(kill_ownership, [(children[0].pid, None)])
            with self.assertRaises(ProcessLookupError):
                os.kill(descendant, 0)
            with self.assertRaises(ChildProcessError):
                os.waitpid(children[0].pid, os.WNOHANG)
        finally:
            if descendant is not None:
                try:
                    if os.getpgid(descendant) == group:
                        actual_killpg(group, signal.SIGKILL)
                except ProcessLookupError:
                    pass

    def test_child_that_never_reads_fixed_stdin_cannot_block_output_or_timeout(self):
        # Larger fixture input stresses the runner; production input never exceeds 10 KiB.
        state, code, _out, _err = run_python(
            "import os,time; os.write(1,b'started'); time.sleep(9)", b"x" * 1048576, seconds=0.1)
        self.assertEqual((state, code), ("timed_out", -signal.SIGKILL))

    def test_early_stdin_close_cannot_pass_a_stream_check(self):
        result = run_python("import os; os.close(0); os.write(1,b'pretend')",
                            b"x" * 1048576, seconds=2)
        self.assertEqual(result[0], "failed")

    def test_shared_deadline_prevents_later_process_dispatch(self):
        with mock.patch.object(probe.subprocess, "Popen", side_effect=AssertionError("dispatch")):
            result = probe.run_owned([sys.executable], b"", probe.clean_environment(),
                                     time.monotonic() - 0.1, [100])
        self.assertEqual(result, ("timed_out", None, b"", b""))

    def test_actual_shared_deadline_bounds_a_stage_and_prevents_following_dispatch(self):
        deadline = time.monotonic() + 0.1
        started = time.monotonic()
        with mock.patch.object(probe, "COMMAND_SECONDS", 2):
            result = probe.run_owned(
                [sys.executable, "-I", "-S", "-c", "import time; time.sleep(9)"],
                b"", probe.clean_environment(), deadline, [100])
        self.assertEqual(result[:2], ("timed_out", -signal.SIGKILL))
        self.assertLess(time.monotonic() - started, 1.5)
        with mock.patch.object(probe.subprocess, "Popen", side_effect=AssertionError("late dispatch")):
            self.assertEqual(probe.run_owned(
                [sys.executable], b"", probe.clean_environment(), deadline, [100])[0], "timed_out")

    def test_denied_group_kill_fails_closed_and_fixture_still_reaps_its_child(self):
        children = []
        actual_popen = subprocess.Popen
        def spawn(*args, **kwargs):
            child = actual_popen(*args, **kwargs)
            children.append(child)
            return child
        try:
            with mock.patch.object(probe.subprocess, "Popen", side_effect=spawn), \
                    mock.patch.object(probe.os, "killpg", side_effect=PermissionError(errno.EPERM, "owned fixture")):
                with self.assertRaises(probe.CleanupUnknown):
                    run_python("import time; time.sleep(9)", seconds=0.1)
        finally:
            for child in children:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=1)
        self.assertTrue(children)
        self.assertIsNotNone(children[0].returncode)

    def test_unconfirmed_child_wait_fails_closed_instead_of_returning_a_status(self):
        child = types.SimpleNamespace(pid=1234)
        child.wait = mock.Mock(side_effect=subprocess.TimeoutExpired("fixed fixture", 0.2))
        with mock.patch.object(probe.os, "killpg"):
            with self.assertRaises(probe.CleanupUnknown):
                probe.terminate_and_wait(child)
        child.wait.assert_called_once_with(timeout=probe.CLEANUP_SECONDS)

    def test_process_spawn_failures_are_unavailable_and_never_reflect_exception_text(self):
        for error in (FileNotFoundError("private path"), PermissionError("private path")):
            with mock.patch.object(probe.subprocess, "Popen", side_effect=error):
                self.assertEqual(run_python("pass"), ("unavailable", None, b"", b""))


if __name__ == "__main__":
    unittest.main(verbosity=2)
