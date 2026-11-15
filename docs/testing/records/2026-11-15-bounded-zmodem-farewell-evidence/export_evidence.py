"""Preserve this batch's original evidence before deleting temporary artifacts."""

from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import re

batch = Path(__file__).resolve().parent
root = batch.parents[1]
destination = (
    root / "docs/testing/records/2026-11-15-bounded-zmodem-farewell-evidence"
)
omitted_directories = {
    "build-original": "Rebuildable third-party compiler output; provenance retained.",
    "external-source": "Unmodified upstream extraction; archive identity retained.",
    "peer": "Temporary peer executables; provenance retained.",
    "stable-tests": "Temporary executable copies; hashes and inventories retained.",
    "tmp": "Isolated scratch data, including interrupted gate fixtures.",
}
omitted_files = {
    "lrzsz-0.13.1.tar.gz": "Downloaded third-party archive; SHA256 retained.",
    "observe_flush.dylib": "Temporary diagnostic executable; source retained.",
}


def sha_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


if destination.exists():
    raise SystemExit("Refusing to overwrite an evidence archive")
gate = json.loads((batch / "logs/full-gate-r5.json").read_text())
if gate["exit"] != 0 or gate["timed_out"]:
    raise SystemExit("Final gate is not successful")

sources = []
omitted = []
for path in sorted(batch.rglob("*")):
    if path.is_symlink():
        raise SystemExit(f"Unexpected symlink in batch evidence: {path}")
    if not path.is_file():
        continue
    relative = path.relative_to(batch)
    reason = omitted_directories.get(relative.parts[0])
    if len(relative.parts) == 1:
        reason = omitted_files.get(relative.name, reason)
    if any(part.startswith("tmp-") for part in relative.parts[:-1]):
        reason = "Reviewer-owned temporary runtime data; execution receipts retained."
    if "__pycache__" in relative.parts or path.suffix == ".pyc":
        reason = "Rebuildable Python bytecode."
    if reason:
        omitted.append(
            {
                "path": relative.as_posix(),
                "bytes": path.stat().st_size,
                "sha256": sha_file(path),
                "reason": reason,
            }
        )
        continue
    data = path.read_bytes()
    if re.search(rb"-----BEGIN (?:[A-Z0-9]+ )?PRIVATE KEY-----", data):
        raise SystemExit(f"Private-key marker in candidate evidence: {relative}")
    sources.append((path, relative, data))

destination.mkdir()
files = {}
attributes = [
    "# Original evidence preserves exact bytes, including upstream line endings.",
    "*.gz binary",
]
for path, relative, data in sources:
    compressed = len(data) > 65536
    artifact = relative.as_posix() + (".gz" if compressed else "")
    target = destination / artifact
    target.parent.mkdir(parents=True, exist_ok=True)
    encoded = gzip.compress(data, mtime=0) if compressed else data
    target.write_bytes(encoded)
    decoded = gzip.decompress(target.read_bytes()) if compressed else target.read_bytes()
    if decoded != data or path.read_bytes() != data:
        raise SystemExit(f"Evidence changed during export: {relative}")
    files[relative.as_posix()] = {
        "source_bytes": len(data),
        "source_sha256": hashlib.sha256(data).hexdigest(),
        "artifact": artifact,
        "artifact_bytes": len(encoded),
        "artifact_sha256": hashlib.sha256(encoded).hexdigest(),
        "verified_lossless": True,
    }
    if not compressed and (
        b"\r" in data
        or b"\0" in data
        or any(line.rstrip(b" \t") != line for line in data.splitlines())
    ):
        attributes.append(f'"{artifact}" binary')

(destination / ".gitattributes").write_text("\n".join(attributes) + "\n")
(destination / "archive-manifest.json").write_text(
    json.dumps(
        {
            "recorded_utc": datetime.now(timezone.utc).isoformat(),
            "source": str(batch.relative_to(root)),
            "count": len(files),
            "files": files,
        },
        indent=2,
    )
    + "\n"
)
(destination / "omitted-artifacts.json").write_text(
    json.dumps(
        {
            "scope": "Only rebuildable or temporary artifacts; all original receipts preserved.",
            "count": len(omitted),
            "bytes": sum(entry["bytes"] for entry in omitted),
            "files": omitted,
        },
        indent=2,
    )
    + "\n"
)
print(
    json.dumps(
        {
            "destination": str(destination.relative_to(root)),
            "preserved_files": len(files),
            "omitted_temporary_files": len(omitted),
            "omitted_bytes": sum(entry["bytes"] for entry in omitted),
            "all_original_bytes_verified": True,
        }
    )
)
