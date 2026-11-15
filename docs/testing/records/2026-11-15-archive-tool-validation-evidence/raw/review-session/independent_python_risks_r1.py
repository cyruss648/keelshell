"""Bounded independent checks for the fixed helper's real I/O and protocol boundaries."""
import ast
import gzip
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tarfile
import time
import types
import unittest
from unittest import mock

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / "crates/keelshell-session/src/archive_validation.py"
probe = types.ModuleType("independent_archive_risks")
exec(compile(SOURCE.read_text(), str(SOURCE), "exec"), probe.__dict__)


class IndependentRisks(unittest.TestCase):
    def setUp(self):
        probe.STOPPED = False
        self.children = []
        self.actual_popen = subprocess.Popen

    def spawn(self, *args, **kwargs):
        child = self.actual_popen(*args, **kwargs)
        self.children.append(child)
        return child

    def run_child(self, code, data=b"", budget=None, seconds=0.5):
        with mock.patch.object(probe.subprocess, "Popen", side_effect=self.spawn):
            with mock.patch.object(probe, "COMMAND_SECONDS", seconds):
                return probe.run_owned(
                    [sys.executable, "-I", "-S", "-B", "-c", code],
                    data,
                    probe.clean_environment(),
                    time.monotonic() + 2,
                    [probe.TOTAL_OUTPUT] if budget is None else budget,
                )

    def tearDown(self):
        for child in self.children:
            if child.returncode is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=1)
            with self.assertRaises(ChildProcessError):
                os.waitpid(child.pid, os.WNOHANG)

    def test_exact_4096_combined_bytes_are_accepted_without_truncation(self):
        result = self.run_child("import os;os.write(1,b'a'*2048);os.write(2,b'b'*2048)")
        self.assertEqual(result, ("passed", 0, b"a" * 2048, b"b" * 2048))

    def test_one_byte_combined_overrun_fails_and_reaps(self):
        state, code, out, err = self.run_child(
            "import os,time;os.write(1,b'a'*2048);os.write(2,b'b'*2049);time.sleep(3)"
        )
        self.assertEqual(state, "output_limit")
        self.assertIsNotNone(code)
        self.assertLessEqual(len(out) + len(err), 4096)

    def test_exact_shared_budget_is_accepted_then_future_dispatch_is_denied(self):
        budget = [80]
        result = self.run_child("import os;os.write(1,b'a'*40);os.write(2,b'b'*40)", budget=budget)
        self.assertEqual(result, ("passed", 0, b"a" * 40, b"b" * 40))
        self.assertEqual(budget, [0])
        with mock.patch.object(probe.subprocess, "Popen", side_effect=AssertionError("dispatch")):
            self.assertEqual(
                probe.run_owned(["unused"], b"", {}, time.monotonic() + 1, budget),
                ("output_limit", None, b"", b""),
            )

    def test_exit_nonzero_preserves_failure_even_with_expected_sample(self):
        result = self.run_child("import os;os.write(1,b'pretend successful bytes');os._exit(65)")
        self.assertEqual(result, ("failed", 65, b"pretend successful bytes", b""))

    def test_signal_mid_read_reaps_direct_child_and_refuses_a_result(self):
        started = time.monotonic()
        code = "import os,signal,time;os.kill(os.getppid(),signal.SIGTERM);time.sleep(3)"
        previous = signal.signal(signal.SIGTERM, probe.interrupted)
        try:
            with self.assertRaises(probe.CleanupUnknown):
                self.run_child(code)
        finally:
            signal.signal(signal.SIGTERM, previous)
        self.assertTrue(probe.STOPPED)
        self.assertLess(time.monotonic() - started, 1)

    def test_child_never_reads_input_and_both_streams_flood_without_deadlock(self):
        result = self.run_child(
            "import os,time;os.write(2,b'e'*4097);time.sleep(3)",
            data=b"fixed" * 20000,
        )
        self.assertEqual(result[0], "output_limit")
        self.assertIsNotNone(result[1])

    def test_real_tar_and_gzip_only_receive_fixed_memory_streams(self):
        data = probe.tar_input()
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
            entries = archive.getmembers()
            self.assertEqual(len(entries), 1)
            self.assertEqual((entries[0].name, entries[0].size, entries[0].mtime),
                             (probe.MEMBER, len(probe.PAYLOAD), 0))
            self.assertTrue(entries[0].isfile())
        for argv, sample in [
            (["/usr/bin/tar", "-xOf", "-", probe.MEMBER], data),
            (["/usr/bin/gzip", "-dc"], gzip.compress(probe.PAYLOAD, mtime=0)),
        ]:
            with self.subTest(argv=argv):
                with mock.patch.object(probe.subprocess, "Popen", side_effect=self.spawn):
                    result = probe.run_owned(argv, sample, probe.clean_environment(),
                                             time.monotonic() + 2, [4096])
                self.assertEqual(result, ("passed", 0, probe.PAYLOAD, b""))

    def test_version_and_stream_classifications_do_not_infer_capabilities(self):
        def run(argv, data, *_):
            if argv[1:] == ["--version"]:
                return "passed", 0, b"opaque banner\n", b""
            return "passed", 0, b"wrong bytes", b""
        with mock.patch.object(probe, "resolve", side_effect=lambda tool, _: "/fixture/" + tool):
            with mock.patch.object(probe, "run_owned", side_effect=run):
                rows = probe.collect()["tools"]
        self.assertEqual(rows[0]["version"], "opaque banner")
        self.assertEqual(rows[0]["version_status"], "passed")
        self.assertEqual(rows[0]["stream_check"], "failed")
        self.assertTrue(all(row["stream_check"] == "not_tested"
                            for row in rows if row["tool"] in ("zip", "unzip", "rz", "sz")))

    def test_unsafe_later_version_line_invalidates_safe_first_line(self):
        for byte in (b"\x00", b"\x1b", b"\x7f", "\u202e".encode(), b"\xff"):
            with self.subTest(byte=byte):
                self.assertIsNone(probe.version_line(b"safe version\nlater " + byte, b""))
                self.assertIsNone(probe.version_line(b"safe version", b"later " + byte))

    def test_in_memory_python_source_has_no_path_archive_extract_calls(self):
        nodes = list(ast.walk(ast.parse(probe.PYTHON_STREAM)))
        self.assertFalse(any(isinstance(n, ast.Call)
                             and isinstance(n.func, ast.Name)
                             and n.func.id == "open" for n in nodes))
        self.assertFalse(any(isinstance(n, ast.Call)
                             and isinstance(n.func, ast.Attribute)
                             and n.func.attr in ("extract", "extractall", "system", "Popen")
                             for n in nodes))
        result = self.run_child(probe.PYTHON_STREAM, seconds=1)
        self.assertEqual(result, ("passed", 0, probe.PAYLOAD, b""))

    def test_missing_all_tools_produces_complete_separate_unavailable_rows(self):
        with mock.patch.dict(probe.os.environ, {"PATH": ""}, clear=True):
            result = probe.collect()
        self.assertEqual(result["protocol"], "keelshell.archive-validation.v1")
        self.assertEqual([row["tool"] for row in result["tools"]], list(probe.TOOLS))
        for row in result["tools"]:
            self.assertEqual((row["path"], row["version"], row["version_status"]),
                             (None, None, "unavailable"))
            self.assertEqual(row["stream_check"],
                             "unavailable" if row["tool"] in ("tar", "gzip", "python3")
                             else "not_tested")
        self.assertLess(len(json.dumps(result).encode()), probe.MAX_RESPONSE)


if __name__ == "__main__":
    unittest.main(verbosity=2)
