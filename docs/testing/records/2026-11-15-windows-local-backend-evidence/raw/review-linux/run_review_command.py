#!/usr/bin/env python3
"""Capture bounded reviewer commands; all generated files remain in this folder."""

import argparse
import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("label")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    review = Path(__file__).resolve().parent
    root = review.parents[2]
    temporary = review / "tmp"
    temporary.mkdir(exist_ok=True)
    environment = dict(os.environ)
    environment.update({
        "TMPDIR": str(temporary),
        "PYTHONDONTWRITEBYTECODE": "1",
        "GIT_OPTIONAL_LOCKS": "0",
    })
    receipt_path = review / (args.label + ".command.json")
    if receipt_path.exists():
        parser.error("This command label already has a receipt")
    start = time.monotonic()
    receipt = {
        "argv": args.command, "cwd": str(root),
        "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "timeout_seconds": 45, "tmpdir": str(temporary),
    }
    timed_out = False
    with (review / (args.label + ".stdout.log")).open("wb") as stdout:
        with (review / (args.label + ".stderr.log")).open("wb") as stderr:
            process = subprocess.Popen(
                args.command, cwd=root, env=environment,
                stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                start_new_session=True,
            )
            receipt["pid"] = process.pid
            try:
                process.wait(timeout=45)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
    receipt.update({
        "returncode": process.returncode, "timed_out": timed_out,
        "waited_and_reaped": process.returncode is not None,
        "elapsed_seconds": round(time.monotonic() - start, 3),
    })
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))
    for suffix in ("stdout.log", "stderr.log"):
        content = (review / (args.label + "." + suffix)).read_text(errors="replace")
        if content:
            print(suffix + ":\n" + content[-10000:])
    return 124 if timed_out else process.returncode


if __name__ == "__main__":
    raise SystemExit(main())
