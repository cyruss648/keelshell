"""Bound and record parent-owned validation without sharing Cargo with agents."""

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

batch = Path(__file__).resolve().parent
root = batch.parents[1]
label, seconds, *command = sys.argv[1:]
assert label.replace("-", "").isalnum() and command
(batch / "logs").mkdir(exist_ok=True)
(batch / "tmp").mkdir(exist_ok=True)
log = batch / "logs" / (label + ".log")
receipt = log.with_suffix(".json")
assert not log.exists() and not receipt.exists()
env = dict(
    os.environ,
    CARGO_TARGET_DIR=str(root / "work/recovery-batch-integration-20261115/target"),
    CARGO_BUILD_JOBS="4",
    CARGO_NET_OFFLINE="true",
    TMPDIR=str(batch / "tmp") + "/",
    PYTHONDONTWRITEBYTECODE="1",
)
start = time.monotonic()
started = datetime.now(timezone.utc).isoformat()
timed_out = False
with log.open("xb") as stream:
    proc = subprocess.Popen(
        command,
        cwd=root,
        env=env,
        stdout=stream,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    print("START", label, proc.pid, flush=True)
    try:
        code = proc.wait(timeout=float(seconds))
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(proc.pid, signal.SIGTERM)
        try:
            code = proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            code = proc.wait()
record = dict(
    command=command,
    cwd=str(root),
    label=label,
    pid=proc.pid,
    started_utc=started,
    finished_utc=datetime.now(timezone.utc).isoformat(),
    seconds=round(time.monotonic() - start, 3),
    exit=code,
    timed_out=timed_out,
    reaped=True,
    log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),
    target=env["CARGO_TARGET_DIR"],
)
with receipt.open("x") as stream:
    json.dump(record, stream, indent=2)
    stream.write("\n")
print(json.dumps(record), flush=True)
print("\n".join(log.read_text(errors="replace").splitlines()[-18:]), flush=True)
raise SystemExit(code if not timed_out else 124)
