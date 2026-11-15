#!/usr/bin/env python3
"""Fetch primary Win32 contracts with bounded I/O into the exclusive r2 folder."""

from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import urllib.request

HERE = Path(__file__).resolve().parent
OUTPUT = HERE / "sources"
OUTPUT.mkdir(exist_ok=True)
SOURCES = {
    "reopenfile": "https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/winbase/nf-winbase-reopenfile.md",
    "createfilew": "https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/fileapi/nf-fileapi-createfilew.md",
    "rust-openoptions-access-mode": "https://raw.githubusercontent.com/rust-lang/rust/0ef1bc386059596dada8fb65b50425aab2ea637a/library/std/src/sys/fs/windows.rs",
}


def fetch(item):
    name, url = item
    with urllib.request.urlopen(url, timeout=25) as response:
        data = response.read()
        status = response.status
    destination = OUTPUT / f"{name}.md"
    if destination.exists():
        raise RuntimeError(f"refusing to overwrite: {destination}")
    destination.write_bytes(data)
    return {
        "name": name,
        "url": url,
        "http_status": status,
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


with ThreadPoolExecutor(max_workers=3) as pool:
    results = list(pool.map(fetch, SOURCES.items()))
(OUTPUT / "fetch-reopen.json").write_text(json.dumps(results, indent=2) + "\n")
print(json.dumps(results, indent=2))
