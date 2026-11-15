"""Capture or verify the complete engineering input set without overwriting it."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import sys

batch = Path(__file__).resolve().parent
root = batch.parents[1]
sha = lambda data: hashlib.sha256(data).hexdigest()
mode, label = sys.argv[1:]
destination = batch / (label + ".json")
if mode == "capture":
    previous = json.loads((batch / "frozen-inputs-r1.json").read_text())
    paths = {"Cargo.toml", "Cargo.lock"}
    paths.update(
        str(path.relative_to(root))
        for path in (root / "crates").rglob("*")
        if path.is_file() and (
            path.suffix in {".rs", ".py"} or path.name == "Cargo.toml"
        )
    )
    result = {
        "recorded_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "source_root": str(root),
        "base_sha": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, text=True
        ).strip(),
        "inputs": {path: sha((root / path).read_bytes()) for path in sorted(paths)},
        "scripts": {
            path: sha((root / path).read_bytes())
            for path in previous["scripts"]
        },
    }
    with destination.open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
elif mode == "verify":
    result = json.loads(destination.read_text())
    mismatches = [
        path
        for section in ("inputs", "scripts")
        for path, expected in result[section].items()
        if not (root / path).is_file() or sha((root / path).read_bytes()) != expected
    ]
    if mismatches:
        raise SystemExit("INPUT DRIFT: " + repr(mismatches))
else:
    raise SystemExit("invalid mode")
print(json.dumps({
    "freeze": destination.name,
    "sha256": sha(destination.read_bytes()),
    "inputs": len(result["inputs"]),
    "scripts": len(result["scripts"]),
    "mismatches": [],
}))
