"""Preserve original receipts and provenance, excluding disposable vendor payloads."""

from datetime import datetime, timezone
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile

batch = Path(__file__).resolve().parent
root = batch.parents[1]
destination = root / "docs/testing/records/2026-11-15-windows-local-backend-evidence"


def sha_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def check_credential_markers(data, relative):
    pattern = re.compile(rb"-----BEGIN ([A-Z0-9 ]*PRIVATE KEY)-----")
    if not pattern.search(data):
        return
    # Full source archives contain the existing redactor's test sentinels.
    # Only that exact unchanged source, inside cfg(test), is exempted.
    assert relative.name == "source.tar", relative
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        for entry in archive:
            if not entry.isfile():
                continue
            source = archive.extractfile(entry).read()
            markers = list(pattern.finditer(source))
            if not markers:
                continue
            assert entry.name == "crates/keelshell-ai/src/redact.rs", entry.name
            assert source == (root / entry.name).read_bytes()
            tests = source.index(b"#[cfg(test)]")
            for marker in markers:
                assert marker.start() > tests
                end = source.find(b"-----END " + marker[1] + b"-----", marker.end())
                assert end < 0 or end - marker.end() < 64


def omission(relative):
    parts = relative.parts
    if "__pycache__" in parts or relative.suffix == ".pyc":
        return "Rebuildable Python bytecode."
    if any(part in {"tmp", "target", "home"} or part.startswith("tmp-") for part in parts[:-1]):
        return "Owned compiler/runtime scratch; primary receipts retained."
    if relative.name == "registry.tar":
        return "Public offline registry payload; every lock checksum and index manifest retained."
    if parts[0] == "review-session-binaries-r2" and relative.name in {"app", "session"}:
        return "Parent-built review executable copy; full named inventory and SHA retained."
    if parts[0] == "windows-sdk":
        if len(parts) > 1 and parts[1] == "sysroot":
            return "Temporary SDK/CRT extraction; complete inventory and payload provenance retained."
        if len(parts) > 1 and parts[1] == "cache":
            if relative.as_posix() == "windows-sdk/cache/xwin/dl/manifest_17.json":
                return None
            return "Downloaded/extracted vendor payload; exact SDK payload digests retained."
        if relative.name.endswith(".tar.gz") or relative.name == "xwin":
            return "Downloaded build tool; release metadata and archive digest retained."
        if relative.suffix in {".o", ".obj", ".a", ".lib", ".exe", ".dll", ".pdb"}:
            return "Temporary compiler probe binary; source, output and hashes retained."
    return None


gate = json.loads((batch / "logs/full-gate-r2.json").read_text())
assert gate["exit"] == 0 and gate["reaped"] and not gate["timed_out"]
assert sha_file(batch / "logs/full-gate-r2.log") == gate["log_sha256"]
assert not (destination / "archive-manifest.json").exists()

files = {}
omitted = []
for path in sorted(batch.rglob("*")):
    relative = path.relative_to(batch)
    reason = omission(relative)
    if path.is_symlink():
        assert reason, relative
        data = path.readlink().as_posix().encode()
        omitted.append({
            "path": relative.as_posix(), "symlink": True,
            "bytes": len(data), "sha256_of_link_text": hashlib.sha256(data).hexdigest(),
            "reason": reason,
        })
        continue
    if not path.is_file():
        continue
    if reason:
        omitted.append({
            "path": relative.as_posix(), "bytes": path.stat().st_size,
            "sha256": sha_file(path), "reason": reason,
        })
        continue
    data = path.read_bytes()
    check_credential_markers(data, relative)
    readable_freeze = relative.name.startswith(("combined-inputs-", "session-combined-inputs-"))
    compressed = len(data) > 65536 and not readable_freeze
    artifact = "raw/" + relative.as_posix() + (".gz" if compressed else "")
    target = destination / artifact
    target.parent.mkdir(parents=True, exist_ok=True)
    encoded = gzip.compress(data, mtime=0) if compressed else data
    with target.open("xb") as stream:
        stream.write(encoded)
    decoded = gzip.decompress(target.read_bytes()) if compressed else target.read_bytes()
    assert decoded == data == path.read_bytes(), relative
    files[relative.as_posix()] = {
        "source_bytes": len(data), "source_sha256": hashlib.sha256(data).hexdigest(),
        "artifact": artifact, "artifact_bytes": len(encoded),
        "artifact_sha256": hashlib.sha256(encoded).hexdigest(), "verified_lossless": True,
    }

manifest = {
    "recorded_utc": datetime.now(timezone.utc).isoformat(),
    "source": batch.relative_to(root).as_posix(), "count": len(files), "files": files,
}
with (destination / "archive-manifest.json").open("x") as stream:
    json.dump(manifest, stream, indent=2)
    stream.write("\n")
with (destination / "omitted-artifacts.json").open("x") as stream:
    json.dump({
        "scope": "Only disposable vendor/build/runtime artifacts; original receipts preserved.",
        "count": len(omitted), "bytes": sum(item["bytes"] for item in omitted), "files": omitted,
    }, stream, indent=2)
    stream.write("\n")
print(json.dumps({
    "preserved_files": len(files), "omitted_files": len(omitted),
    "omitted_bytes": sum(item["bytes"] for item in omitted),
    "all_original_bytes_verified": True,
}))
