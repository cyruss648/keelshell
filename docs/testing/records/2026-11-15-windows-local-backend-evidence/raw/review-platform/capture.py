#!/usr/bin/env python3
"""Capture one bounded review command; write only inside this review directory."""

import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
label, *command = sys.argv[1:]
if not label or not command or "/" in label:
    raise SystemExit("usage: capture.py LABEL COMMAND [ARG ...]")
receipt_path = HERE / f"{label}.json"
output_path = HERE / f"{label}.txt"
if receipt_path.exists() or output_path.exists():
    raise SystemExit("refusing to overwrite an existing receipt")

environment = os.environ.copy()
environment["CARGO_TARGET_DIR"] = str(HERE / "target")
environment["CARGO_INCREMENTAL"] = "0"
environment["CARGO_TERM_COLOR"] = "never"
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
clock = time.monotonic()
timed_out = False
with output_path.open("wb") as output:
    child = subprocess.Popen(
        command,
        cwd=ROOT,
        env=environment,
        stdout=output,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        returncode = child.wait(timeout=180)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(child.pid, signal.SIGTERM)
        try:
            returncode = child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            returncode = child.wait(timeout=10)
receipt = {
    "label": label,
    "argv": command,
    "cwd": str(ROOT),
    "cargo_target_dir": environment["CARGO_TARGET_DIR"],
    "cargo_incremental": environment["CARGO_INCREMENTAL"],
    "started_utc": started,
    "finished_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "elapsed_seconds": round(time.monotonic() - clock, 6),
    "pid": child.pid,
    "exit_code": returncode,
    "timed_out": timed_out,
    "waited_and_reaped": True,
    "output_path": str(output_path),
    "output_sha256": hashlib.sha256(output_path.read_bytes()).hexdigest(),
}
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt, indent=2))
print(output_path.read_text(errors="replace"))
raise SystemExit(returncode if returncode >= 0 else 1)
