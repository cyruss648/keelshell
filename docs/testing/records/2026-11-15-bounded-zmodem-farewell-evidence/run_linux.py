"""Run the existing Linux fixture with explicit packaged peer paths."""
import os
from pathlib import Path
import sys

batch = Path(__file__).resolve().parent
root = batch.parents[1]
label, binary = sys.argv[1:]
if not label.replace("-", "").isalnum():
    raise SystemExit("invalid evidence label")
os.environ["KEELSHELL_ZMODEM_RZ"] = "/usr/bin/rz"
os.environ["KEELSHELL_ZMODEM_SZ"] = "/usr/bin/sz"
os.execv(sys.executable, [
    sys.executable, "-B", str(root / "scripts/run_linux_file_interop.py"),
    "--output", str(batch / label),
    "--skip-archive", "--host-package-bridge",
    "--command-timeout", "220", "--",
    str(batch / "stable-tests" / binary),
    "--ignored", "--test-threads=1", "--nocapture",
])
