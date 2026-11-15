"""Read-only review command runner; all writes stay next to this file."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SCOPED = [
    "crates/keelshell-session/src/archive_validation.rs",
    "crates/keelshell-session/src/archive_validation.py",
    "crates/keelshell-session/src/archive_validation/tests.rs",
    "crates/keelshell-session/src/archive_validation/test_archive_validation.py",
    "crates/keelshell-session/tests/archive_validation.rs",
    "crates/keelshell-session/tests/openssh_interop.rs",
    "crates/keelshell-session/src/lib.rs",
]


def input_hashes():
    return {
        name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
        for name in SCOPED
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True)
    parser.add_argument("--seconds", type=float, default=45)
    parser.add_argument("--quiet-log", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or Path(args.name).name != args.name:
        raise SystemExit("command and one-component evidence name required")
    home = HERE / "fixture-home"
    home.mkdir(exist_ok=True)
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(home),
        "TMPDIR": str(HERE),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "PYTHONDONTWRITEBYTECODE": "1",
    }
    base = HERE / args.name
    if base.with_suffix(".json").exists() or base.with_suffix(".log").exists():
        raise SystemExit("refusing to overwrite existing evidence")
    started = time.time()
    monotonic = time.monotonic()
    before = input_hashes()
    timed_out = False
    child = None
    with base.with_suffix(".log").open("xb") as output:
        child = subprocess.Popen(
            command,
            cwd=HERE,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
            close_fds=True,
        )
        try:
            child.wait(timeout=args.seconds)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=3)
    receipt = {
        "argv": command,
        "cwd": str(HERE),
        "environment": env,
        "started_unix": started,
        "elapsed_seconds": time.monotonic() - monotonic,
        "timeout_seconds": args.seconds,
        "timed_out": timed_out,
        "pid": child.pid,
        "exit_code": child.returncode,
        "direct_child_waited": True,
        "log_sha256": hashlib.sha256(base.with_suffix(".log").read_bytes()).hexdigest(),
        "independent_rust_rebuild": False,
        "scoped_sha256_before": before,
        "scoped_sha256_after": input_hashes(),
    }
    base.with_suffix(".json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))
    if not args.quiet_log:
        print(base.with_suffix(".log").read_text(errors="replace"))
    return 1 if timed_out or child.returncode else 0


if __name__ == "__main__":
    sys.exit(main())
