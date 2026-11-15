#!/usr/bin/env python3
"""Record the authorized read-only review commands and byte integrity."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

SOURCE = Path("/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell")
BASE = Path("/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115")
OUTPUT = BASE / "fix/review"
TARGET = BASE / "review/target"
MANIFEST = BASE / "fix/frozen-inputs.json"
EXPECTED_MANIFEST = "f8d1acaa5dffdbf27fa0207dbceb03774cc154ffa84adde7f55b9befc98df724"
EXPECTED_HEAD = "5cdbc9617164d6cfbd5e7a9a022539934221ee75"
FILES = [
    "crates/keelshell-app/src/files.rs",
    "crates/keelshell-app/src/files/parallel.rs",
    "crates/keelshell-app/src/files/parallel_tests.rs",
]
OVERRIDES = {
    "CARGO_TARGET_DIR": str(TARGET),
    "CARGO_BUILD_JOBS": "2",
    "CARGO_NET_OFFLINE": "true",
    "TMPDIR": str(OUTPUT / "tmp") + "/",
    "TMP": str(OUTPUT / "tmp"),
    "TEMP": str(OUTPUT / "tmp"),
    "GIT_OPTIONAL_LOCKS": "0",
    "PYTHONDONTWRITEBYTECODE": "1",
}
ENV = dict(os.environ, **OVERRIDES)


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(name, data):
    with (OUTPUT / name).open("x") as stream:
        json.dump(data, stream, ensure_ascii=False, indent=2)
        stream.write("\n")


def command(name, argv):
    receipt_path = OUTPUT / (name + ".json")
    log_path = OUTPUT / (name + ".log")
    if receipt_path.exists() or log_path.exists():
        raise RuntimeError(f"Refusing to overwrite evidence: {name}")
    started = utc()
    then = time.monotonic()
    print(f"START {name}: {argv}", flush=True)
    with log_path.open("xb") as stream:
        result = subprocess.run(argv, cwd=SOURCE, env=ENV, stdout=stream, stderr=subprocess.STDOUT)
    receipt = {
        "argv": argv,
        "cwd": str(SOURCE),
        "environment_overrides": OVERRIDES,
        "started_at_utc": started,
        "finished_at_utc": utc(),
        "elapsed_seconds": round(time.monotonic() - then, 3),
        "exit_code": result.returncode,
        "log": str(log_path),
        "log_sha256": sha(log_path),
    }
    save(name + ".json", receipt)
    print(f"END {name}: exit={result.returncode} seconds={receipt['elapsed_seconds']}", flush=True)
    return receipt


def git(*args):
    return subprocess.check_output(["git", *args], cwd=SOURCE, env=ENV)


def snapshot(phase):
    frozen = json.loads(MANIFEST.read_text())
    actual = {p: sha(SOURCE / p) for p in frozen["inputs"]}
    mismatches = {p: {"expected": h, "actual": actual[p]} for p, h in frozen["inputs"].items() if actual[p] != h}
    tracked = [p.decode() for p in git("ls-files", "-z").split(b"\0") if p]
    tracked_bytes = {p: sha(SOURCE / p) if (SOURCE / p).is_file() else None for p in tracked}
    index = Path(git("rev-parse", "--git-path", "index").decode().strip())
    if not index.is_absolute():
        index = SOURCE / index
    scripts = {}
    for absolute, expected in frozen["scripts"].items():
        rel = Path(absolute).relative_to(BASE.parent.parent)
        scripts[str(rel)] = {"expected": expected, "actual": sha(SOURCE / rel)}
    data = {
        "recorded_utc": utc(),
        "source": str(SOURCE),
        "head": git("rev-parse", "HEAD").decode().strip(),
        "status_porcelain_v1": git("status", "--porcelain=v1", "--untracked-files=all").decode(),
        "changed_paths_vs_head": git("diff", "--name-only", "HEAD").decode().splitlines(),
        "index_path": str(index),
        "index_sha256": sha(index),
        "manifest_sha256": sha(MANIFEST),
        "matched_frozen_inputs": sum(actual[p] == h for p, h in frozen["inputs"].items()),
        "frozen_input_count": len(actual),
        "frozen_input_mismatches": mismatches,
        "candidate_file_sha256": {p: actual[p] for p in FILES},
        "local_script_hashes": scripts,
        "tracked_file_count": len(tracked),
        "tracked_file_sha256": tracked_bytes,
    }
    if phase == "end":
        before = json.loads((OUTPUT / "source-start.json").read_text())
        data["changed_tracked_bytes"] = [
            p for p in sorted(set(before["tracked_file_sha256"]) | set(tracked_bytes))
            if before["tracked_file_sha256"].get(p) != tracked_bytes.get(p)
        ]
        data["index_unchanged"] = data["index_sha256"] == before["index_sha256"]
        data["status_unchanged"] = data["status_porcelain_v1"] == before["status_porcelain_v1"]
        data["head_unchanged"] = data["head"] == before["head"]
    save("source-" + phase + ".json", data)
    valid = (
        data["head"] == EXPECTED_HEAD
        and data["manifest_sha256"] == EXPECTED_MANIFEST
        and not mismatches
        and set(data["changed_paths_vs_head"]) == set(FILES)
        and all(x["expected"] == x["actual"] for x in scripts.values())
    )
    if phase == "end":
        valid = valid and not data["changed_tracked_bytes"] and data["index_unchanged"] and data["status_unchanged"] and data["head_unchanged"]
    print(json.dumps({k: v for k, v in data.items() if k not in ["tracked_file_sha256", "local_script_hashes"]}, ensure_ascii=False, indent=2), flush=True)
    if not valid:
        raise RuntimeError("Frozen-source integrity check failed; preserve evidence and stop")


def gates():
    results = []
    result = command("app-inventory", ["cargo", "test", "-p", "keelshell-app", "--locked", "--offline", "--", "--list"])
    results.append(result)
    text = (OUTPUT / "app-inventory.log").read_text(errors="replace")
    names = re.findall(r"^(\S+): test$", text, re.MULTILINE)
    recovery = [name for name in names if name.split("::")[-1].startswith("persisted_recovery_batch_")]
    parallel = [name for name in names if "::parallel_tests::" in name]
    inventory = {
        "tests": len(names),
        "unique_tests": len(set(names)),
        "parallel_tests": len(parallel),
        "persisted_recovery_batch_count": len(recovery),
        "persisted_recovery_batch_names": recovery,
        "parallel_test_names": parallel,
        "cargo_exit_code": result["exit_code"],
    }
    save("inventory-validation.json", inventory)
    if result["exit_code"] != 0 or len(names) != 1186 or len(set(names)) != 1186 or len(recovery) != 10 or len(parallel) != 29:
        raise RuntimeError(f"Rebuilt inventory does not match the frozen candidate: {inventory}")
    print(json.dumps(inventory, indent=2), flush=True)
    commands = [
        ("app-parallel-tests", ["cargo", "test", "-p", "keelshell-app", "--locked", "--offline", "--", "parallel_tests", "--test-threads=1"]),
        ("app-clippy", ["cargo", "clippy", "-p", "keelshell-app", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]),
        ("fmt-check", ["cargo", "fmt", "--all", "--", "--check"]),
        ("git-diff-check", ["git", "diff", "--check", "HEAD"]),
    ]
    for name, argv in commands:
        results.append(command(name, argv))
    save("gate-summary.json", {"commands": results, "all_passed": all(r["exit_code"] == 0 for r in results)})
    if any(r["exit_code"] != 0 for r in results):
        raise RuntimeError("An authorized gate failed; retain original logs")


def main():
    (OUTPUT / "tmp").mkdir(exist_ok=True)
    phase = sys.argv[1]
    if phase == "start":
        snapshot("start")
        command("candidate-diff", ["git", "diff", "--no-ext-diff", "--no-textconv", "HEAD", "--", *FILES])
        command("rustc-version", ["rustc", "-vV"])
        command("cargo-version", ["cargo", "--version"])
        command("clippy-version", ["cargo", "clippy", "--version"])
    elif phase == "gates":
        gates()
    elif phase == "end":
        snapshot("end")
    elif phase == "record":
        result = command(sys.argv[2], sys.argv[3:])
        if result["exit_code"]:
            sys.exit(result["exit_code"])
    else:
        raise ValueError(phase)


if __name__ == "__main__":
    main()
