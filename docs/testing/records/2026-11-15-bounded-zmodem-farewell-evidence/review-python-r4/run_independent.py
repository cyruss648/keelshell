#!/usr/bin/env python3
"""Run the scoped review checks and retain bounded execution receipts."""

import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
REVIEW = Path(__file__).resolve().parent
BATCH = REVIEW.parent
BASE = "b66ec7e564e97c3cc8261b447288377ac1ffccc2"
PATHS = (
    "scripts/run_linux_file_interop.py",
    "scripts/test_linux_file_interop.py",
    "scripts/openssh_interop.py",
)


def hashes():
    return {
        relative: hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()
        for relative in PATHS
    }


def run(label, command, expected_exit):
    environment = dict(
        os.environ,
        TMPDIR=str(BATCH / "tmp"),
        PYTHONDONTWRITEBYTECODE="1",
        GIT_OPTIONAL_LOCKS="0",
    )
    assert (BATCH / "tmp").is_dir()
    before = hashes()
    timeout_seconds = 45
    began = time.monotonic()
    started_utc = datetime.now(timezone.utc).isoformat()
    expired = False
    cleanup_errors = []
    identities = {}
    # Import the unchanged ownership helper only for emergency cleanup of this
    # launcher's own child. Normal completion uses Popen.wait and no signals.
    sys.path.insert(0, str(ROOT / "scripts"))
    import openssh_interop as ownership

    with (REVIEW / (label + ".log")).open("wb") as log:
        process = subprocess.Popen(
            command, cwd=ROOT, env=environment,
            stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        info = ownership.process_info(process.pid)
        if info is not None:
            identities[process.pid] = info
        deadline = began + timeout_seconds
        try:
            while process.poll() is None:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    expired = True
                    break
                try:
                    table = ownership.process_table(min(deadline, time.monotonic() + 1))
                    ownership.remember_owned(process, identities, table)
                except (ownership.InteropFailure, OSError, subprocess.SubprocessError) as error:
                    cleanup_errors.append(type(error).__name__ + ": " + str(error))
                try:
                    process.wait(timeout=min(0.1, max(0, deadline - time.monotonic())))
                except subprocess.TimeoutExpired:
                    pass
        finally:
            if process.poll() is None:
                expired = True
                try:
                    if not ownership.stop_owned(process, time.monotonic() + 7, identities):
                        cleanup_errors.append("Emergency owned cleanup returned false")
                except (ownership.InteropFailure, OSError, subprocess.SubprocessError) as error:
                    cleanup_errors.append(type(error).__name__ + ": " + str(error))
                if process.poll() is None:
                    # This Popen is still unreaped and therefore cannot be a reused PID.
                    process.kill()
            process.wait(timeout=2)
    elapsed = time.monotonic() - began
    text = (REVIEW / (label + ".log")).read_text()
    count = re.search(r"Ran (\d+) tests? in ([\d.]+)s", text)
    statuses = [
        line for line in text.splitlines()
        if line.startswith(("Ran ", "OK", "FAILED"))
    ]
    after = hashes()
    receipt = {
        "label": label,
        "command": command,
        "cwd": str(ROOT),
        "started_utc": started_utc,
        "finished_utc": datetime.now(timezone.utc).isoformat(),
        "timeout_seconds": timeout_seconds,
        "environment": {
            "TMPDIR": environment["TMPDIR"],
            "PYTHONDONTWRITEBYTECODE": environment["PYTHONDONTWRITEBYTECODE"],
        },
        "python": sys.version,
        "source_sha256_before": before,
        "source_sha256_after": after,
        "source_drift": before != after,
        "returncode": process.returncode,
        "expected_returncode": expected_exit,
        "timed_out": expired,
        "reaped": process.returncode is not None,
        "elapsed_seconds": round(elapsed, 6),
        "tests_run": int(count[1]) if count else None,
        "result_lines": statuses,
        "supervisor_errors": cleanup_errors,
        "supervisor_owned_identities": {
            str(pid): identity for pid, identity in identities.items()
        },
        "log": label + ".log",
        "log_sha256": hashlib.sha256((REVIEW / (label + ".log")).read_bytes()).hexdigest(),
    }
    (REVIEW / (label + ".json")).write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({
        key: receipt[key] for key in (
            "label", "returncode", "timed_out", "reaped", "elapsed_seconds",
            "tests_run", "result_lines", "source_drift", "supervisor_errors",
        )
    }, indent=2), flush=True)
    assert not expired and process.returncode == expected_exit
    assert before == after and not cleanup_errors
    assert count is not None
    return receipt


if __name__ == "__main__":
    # Execute the original implementation from its pinned Git object in memory.
    # The tests and read dependency are the final frozen files in this checkout.
    baseline_code = r"""
import subprocess
import sys
import types
import unittest
from pathlib import Path
sys.dont_write_bytecode = True
root = Path.cwd()
sys.path.insert(0, str(root / "scripts"))
base = "b66ec7e564e97c3cc8261b447288377ac1ffccc2"
source = subprocess.run(
    ["git", "show", base + ":scripts/run_linux_file_interop.py"],
    check=True, capture_output=True, timeout=10,
).stdout
module = types.ModuleType("run_linux_file_interop")
module.__file__ = str(root / "scripts/run_linux_file_interop.py")
sys.modules[module.__name__] = module
exec(compile(source, base + ":scripts/run_linux_file_interop.py", "exec"), module.__dict__)
import test_linux_file_interop
suite = unittest.defaultTestLoader.loadTestsFromTestCase(test_linux_file_interop.CommandDeadlineTests)
result = unittest.TextTestRunner(verbosity=2).run(suite)
sys.exit(not result.wasSuccessful())
"""
    red = run(
        "independent-baseline-red",
        [sys.executable, "-B", "-c", baseline_code],
        expected_exit=1,
    )
    green = run(
        "independent-file-suite",
        [
            sys.executable, "-B", "-m", "unittest", "discover",
            "-s", "scripts", "-p", "test_linux_file_interop.py", "-v",
        ],
        expected_exit=0,
    )
    assert red["tests_run"] == 7
    assert green["tests_run"] == 17
