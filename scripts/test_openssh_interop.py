"""Deadline and cleanup regressions; never start or signal a real process."""

import contextlib
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


class ProcessInventoryTests(unittest.TestCase):
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
