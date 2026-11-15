"""Bind a parent-built test executable to its actual inventory and source freeze."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

batch = Path(__file__).resolve().parent
root = batch.parents[1]
label, log_name, *freeze = sys.argv[1:]
paths = re.findall(r"\s+Running [^\n]*\(([^)\n]+)\)", (batch / "logs" / log_name).read_text())
if len(paths) != 1:
    raise SystemExit("expected one executed test binary")
source = root / paths[0]
destination = batch / "stable-tests" / label
destination.parent.mkdir(exist_ok=True)
with source.open("rb") as incoming, destination.open("xb") as output:
    shutil.copyfileobj(incoming, output)
destination.chmod(0o700)
inventory = subprocess.run(
    [str(destination), "--list"],
    check=True,
    capture_output=True,
    timeout=30,
).stdout
inventory_path = batch / (label + "-inventory.txt")
inventory_path.write_bytes(inventory)
sha = lambda data: hashlib.sha256(data).hexdigest()
record = {
    "source": str(source),
    "binary": str(destination),
    "sha256": sha(destination.read_bytes()),
    "inventory": inventory_path.name,
    "inventory_sha256": sha(inventory),
    "inventory_count": sum(line.endswith(b": test") for line in inventory.splitlines()),
    "freeze": freeze[0] if freeze else "frozen-inputs-r1.json",
    "built_by": log_name,
    "parent_built": True,
}
with (batch / (label + "-binary.json")).open("x") as stream:
    json.dump(record, stream, indent=2)
    stream.write("\n")
print(json.dumps(record))
