"""Seal owned text receipts into a deterministic archive without binaries."""
import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("base", type=Path)
parser.add_argument("destination", type=Path)
options = parser.parse_args()
base = options.base.resolve()
destination = options.destination.resolve()
destination.mkdir(parents=True, exist_ok=True)
assert not (destination / "originals.tar.gz").exists()
assert not (destination / "manifest.json").exists()
payloads = {}
for path in sorted(base.rglob("*")):
    relative = path.relative_to(base)
    if ("__pycache__" in relative.parts or "runtime-tmp" in relative.parts
            or relative.parts[0].startswith("package-")):
        continue
    if not path.is_file():
        continue
    assert not path.is_symlink()
    if relative.as_posix() == "lsof-upstream.8":
        continue
    assert path.suffix in (".py", ".json", ".jsonl", ".log", ".md", ".txt", ".diff"), relative
    payloads[relative.as_posix()] = path.read_bytes()
rows = [{"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        for name, raw in payloads.items()]
buffer = io.BytesIO()
with gzip.GzipFile(fileobj=buffer, mode="wb", filename="", mtime=0) as compressed:
    with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for row in rows:
            member = tarfile.TarInfo(row["path"])
            member.size = row["bytes"]
            member.mode = 0o644
            archive.addfile(member, io.BytesIO(payloads[row["path"]]))
raw = buffer.getvalue()
manifest = {
    "sealed_utc": datetime.now(timezone.utc).isoformat(),
    "count": len(rows), "archive_bytes": len(raw),
    "archive_sha256": hashlib.sha256(raw).hexdigest(), "entries": rows,
    "excluded": ["Application packages and binaries", "Python bytecode",
                 "Downloaded full upstream manual; reference digest retained separately"],
}
assert all((base / name).read_bytes() == data for name, data in payloads.items())
with (destination / "originals.tar.gz").open("xb") as output:
    output.write(raw)
with (destination / "manifest.json").open("x") as output:
    json.dump(manifest, output, indent=2)
    output.write("\n")
print(json.dumps({key: value for key, value in manifest.items() if key != "entries"}))
