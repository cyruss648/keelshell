#!/usr/bin/env python3
"""Run one bounded batch command and retain its original output and exit receipt."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = Path(__file__).resolve().parent


def snapshot(label):
    candidates = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    candidates += list((ROOT / "crates").rglob("*.rs"))
    candidates += list((ROOT / "crates").rglob("*.py"))
    candidates += list((ROOT / "crates").rglob("Cargo.toml"))
    candidates += list((ROOT / "scripts").glob("*.py"))
    config = ROOT / ".cargo"
    if config.exists():
        candidates += list(config.rglob("*.toml"))
    files = {
        str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(set(candidates))
    }
    encoded = json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
    record = {"files": files, "count": len(files), "sha256": hashlib.sha256(encoded).hexdigest()}
    (OUTPUT / f"{label}.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({"snapshot": label, "count": len(files), "sha256": record["sha256"]}))
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("label")
    parser.add_argument("--seconds", type=float, default=1200)
    parser.add_argument("--snapshot", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    if options.snapshot:
        snapshot(options.label)
        return
    command = options.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        parser.error("command is required")
    receipt_path = OUTPUT / f"{options.label}.json"
    log_path = OUTPUT / f"{options.label}.log"
    if receipt_path.exists() or log_path.exists():
        parser.error("receipt already exists; use a new label")
    environment = dict(os.environ)
    environment["CARGO_TARGET_DIR"] = str(ROOT / "work/recovery-batch-integration-20261115/target")
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    started = time.monotonic()
    utc = datetime.now(timezone.utc).isoformat()
    expired = False
    with log_path.open("xb") as stream:
        process = subprocess.Popen(
            command, cwd=ROOT, env=environment, stdout=stream,
            stderr=subprocess.STDOUT, start_new_session=True,
        )
        try:
            process.wait(timeout=options.seconds)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            expired = True
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
    record = {
        "command": command, "started_utc": utc,
        "finished_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "exit_code": process.returncode, "timeout_or_interrupt": expired, "reaped": True,
        "cargo_target": environment["CARGO_TARGET_DIR"],
        "log_sha256": hashlib.sha256(log_path.read_bytes()).hexdigest(),
    }
    receipt_path.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({key: value for key, value in record.items() if key != "command"}))
    raise SystemExit(0 if process.returncode == 0 and not expired else 1)


if __name__ == "__main__":
    main()
