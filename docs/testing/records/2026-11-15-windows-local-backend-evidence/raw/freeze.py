"""Freeze engineering bytes and create a branchless validation-only Git snapshot."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

batch = Path(__file__).resolve().parent
root = batch.parents[1]
label = sys.argv[1]
assert label in {"r2", "r3", "r4"}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(name, data):
    path = batch / name
    with path.open("x") as stream:
        json.dump(data, stream, indent=2)
        stream.write("\n")
    return path


source = [
    root / "Cargo.toml",
    root / "Cargo.lock",
    *sorted(
        path for path in (root / "crates").rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    ),
]
scripts = sorted((root / "scripts").glob("*.py"))
baseline = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
freeze = save(f"combined-inputs-{label}.json", {
    "baseline": baseline,
    "files": {str(path.relative_to(root)): sha(path) for path in source},
    "scripts": {str(path.relative_to(root)): sha(path) for path in scripts},
    "scope": "exact combined engineering inputs; documentation excluded; no Windows native claim",
})
prefixes = (
    "crates/keelshell-platform/",
    "crates/keelshell-session/src/zmodem",
    "crates/keelshell-session/src/sftp/archive_transfer",
    "crates/keelshell-session/src/sftp/queue",
    "crates/keelshell-app/src/files/archive",
    "crates/keelshell-app/src/terminal_zmodem",
)
extra = {
    "crates/keelshell-session/Cargo.toml",
    "crates/keelshell-session/tests/zmodem.rs",
    "crates/keelshell-session/tests/archive_transfer.rs",
}
paths = [p for p in source if str(p.relative_to(root)).startswith(prefixes)
         or str(p.relative_to(root)) in extra]
review = save(f"session-combined-inputs-{label}.json", {
    "baseline": baseline,
    "files": {str(path.relative_to(root)): sha(path) for path in paths},
    "windows_strict_clippy": "logs/windows-combined-clippy-r3.json",
    "handoff_green": "logs/archive-handoff-green-r1.json",
    "unicode_green": "logs/unicode-reservation-green-r2.json",
})
index = batch / f"snapshot-{label}.index"
assert not index.exists()
env = dict(os.environ, GIT_INDEX_FILE=str(index))
try:
    subprocess.run(["git", "read-tree", "HEAD"], cwd=root, env=env, check=True)
    subprocess.run(["git", "add", "--", "Cargo.toml", "Cargo.lock", "crates", "scripts"],
                   cwd=root, env=env, check=True)
    tree = subprocess.check_output(["git", "write-tree"], cwd=root, env=env, text=True).strip()
    commit = subprocess.check_output([
        "git", "-c", "commit.gpgsign=false", "commit-tree", tree, "-p", baseline,
    ], cwd=root, text=True,
        input="Validation-only immutable snapshot for Windows local backend batch.\n").strip()
finally:
    index.unlink(missing_ok=True)
save(f"validation-snapshot-{label}.json", {
    "revision": commit,
    "tree": tree,
    "base": baseline,
    "engineering_freeze": freeze.name,
    "freeze_sha256": sha(freeze),
    "branch_moved": False,
    "main_index_changed": False,
    "purpose": "immutable git archive validation only; formal commit follows gates and review",
})
print(json.dumps({
    "freeze": freeze.name, "source_files": len(source), "scripts": len(scripts),
    "sha256": sha(freeze), "session_files": len(paths), "session_sha256": sha(review),
    "validation_revision": commit,
}))
