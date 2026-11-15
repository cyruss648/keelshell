"""Run the existing owned SSH fixture with explicitly labelled external peers."""
import os
from pathlib import Path
import sys

batch = Path(__file__).resolve().parent
root = batch.parents[1]
label, sender = sys.argv[1:]
if not label.replace("-", "").isalnum():
    raise SystemExit("invalid evidence label")
os.environ["KEELSHELL_ZMODEM_RZ"] = str(batch / "peer/lrz")
os.environ["KEELSHELL_ZMODEM_SZ"] = str(batch / "peer" / sender)
os.execv(
    sys.executable,
    [
        sys.executable,
        "-B",
        str(root / "scripts/openssh_interop.py"),
        "--zmodem",
        "--output",
        str(batch / label),
        "--timeout",
        "280",
    ],
)
