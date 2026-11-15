#!/usr/bin/env python3
"""Run one sidecar command with a deadline and a persisted wait/reap receipt."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--label", required=True)
    parser.add_argument("--timeout", type=float, required=True)
    parser.add_argument("argv", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    argv = args.argv[1:] if args.argv[:1] == ["--"] else args.argv
    if not argv:
        parser.error("missing command")
    base = Path(__file__).resolve().parent
    log = base / "logs" / f"{args.label}.log"
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    before = time.monotonic()
    env = os.environ.copy()
    env.update(
        TMPDIR=str(base / "tmp"),
        XDG_CACHE_HOME=str(base / "cache"),
        XDG_DATA_HOME=str(base / "cache"),
    )
    timed_out = False
    with log.open("wb") as output:
        process = subprocess.Popen(
            argv,
            cwd=base,
            env=env,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            code = process.wait(timeout=args.timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                code = process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                code = process.wait(timeout=5)
    record = {
        "argv": argv,
        "cwd": str(base),
        "pid": process.pid,
        "started_utc": started,
        "finished_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "seconds": round(time.monotonic() - before, 3),
        "timeout_seconds": args.timeout,
        "exit_code": code,
        "timed_out": timed_out,
        "reaped": process.poll() is not None,
        "log": str(log),
        "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest(),
    }
    (base / "logs" / f"{args.label}.json").write_text(
        json.dumps(record, indent=2) + "\n"
    )
    print(json.dumps(record), flush=True)
    print(log.read_text(errors="replace")[-12000:], flush=True)
    return 124 if timed_out else (code if code >= 0 else 128 - code)


if __name__ == "__main__":
    raise SystemExit(main())
