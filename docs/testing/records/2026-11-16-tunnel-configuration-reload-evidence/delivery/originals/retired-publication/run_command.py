"""Run an owned batch command, streaming output and keeping a bounded receipt."""
import hashlib
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import windows_native_crash as owned

label, seconds, *command = sys.argv[1:]
base = Path(__file__).resolve().parent
log = base / f"{label}.log"
receipt_path = base / f"{label}.json"
assert command and not log.exists() and not receipt_path.exists()
environment = dict(
    os.environ,
    CARGO_TARGET_DIR=str(ROOT / "work/recovery-batch-integration-20261115/target"),
    CARGO_BUILD_JOBS="2",
    MACOSX_DEPLOYMENT_TARGET="15.0",
    CARGO_TERM_COLOR="never",
)
wrapper = (
    "import subprocess,sys\n"
    "with open(sys.argv[1], 'xb', buffering=0) as output:\n"
    "    code = subprocess.call(sys.argv[2:], stdout=output, stderr=subprocess.STDOUT)\n"
    "sys.exit(code)\n"
)
started = time.monotonic()
result, receipt = owned.run_owned(
    [sys.executable, "-c", wrapper, str(log), *command],
    ROOT,
    environment,
    owned.Deadline(time.time() + float(seconds)),
)
receipt.update(
    command=command,
    seconds=time.monotonic() - started,
    cwd=str(ROOT),
    cargo_target=environment["CARGO_TARGET_DIR"],
    cargo_owner_checkout_unchanged=True,
    wrapper_stdout=result.stdout,
    wrapper_stderr=result.stderr,
    log_bytes=log.stat().st_size,
    log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),
)
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt))
sys.exit(result.returncode)
