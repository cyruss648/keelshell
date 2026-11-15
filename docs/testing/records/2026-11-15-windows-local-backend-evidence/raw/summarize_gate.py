"""Bind the completed gate, named new cases and final engineering bytes."""

from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re

batch = Path(__file__).resolve().parent
root = batch.parents[1]
sha = lambda data: hashlib.sha256(data).hexdigest()
freeze_path = batch / "combined-inputs-r2.json"
freeze = json.loads(freeze_path.read_text())
receipt = json.loads((batch / "logs/full-gate-r2.json").read_text())
log_data = (batch / "logs/full-gate-r2.log").read_bytes()
log = log_data.decode()
assert receipt["exit"] == 0 and receipt["reaped"] and not receipt["timed_out"]
assert sha(log_data) == receipt["log_sha256"]
drift = [
    path for section in ("files", "scripts") for path, digest in freeze[section].items()
    if sha((root / path).read_bytes()) != digest
]
paths = {"Cargo.toml", "Cargo.lock"} | {
    path.relative_to(root).as_posix() for path in (root / "crates").rglob("*")
    if path.is_file() and "__pycache__" not in path.parts
}
assert not drift and paths == set(freeze["files"])
assert {p.relative_to(root).as_posix() for p in (root / "scripts").glob("*.py")} == set(freeze["scripts"])

stages = []
suites = []
passed = set()
stage = ""
suite = None
for line in log.splitlines():
    if line.startswith("Running: "):
        stage = line[9:]
        stages.append(stage)
    if "cargo test --workspace " not in stage:
        continue
    stripped = line.strip()
    if stripped.startswith("Running "):
        suite = {"kind": "ordinary", "preamble": stripped}
    elif stripped.startswith("Doc-tests "):
        suite = {"kind": "rustdoc", "preamble": stripped}
    match = re.match(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", line)
    if match:
        assert suite is not None
        suites.append(dict(suite, passed=int(match[1]), failed=int(match[2]), ignored=int(match[3])))
    match = re.match(r"^test (.+) \.\.\. ok$", line)
    if match:
        passed.add(match[1])
assert len(stages) == 7 and "--controller-small-stack" in stages[-1]
assert "Dependency version policy: passed" in log
totals = {
    kind: {field: sum(s[field] for s in suites if s["kind"] == kind)
           for field in ("passed", "failed", "ignored")}
    for kind in ("ordinary", "rustdoc")
}
python = re.search(r"^Ran (\d+) tests in ([0-9.]+)s\n\nOK \(skipped=(\d+)\)$", log, re.M)
assert python
python_totals = {
    "run": int(python[1]), "skipped": int(python[3]),
    "passed": int(python[1]) - int(python[3]), "failed": 0,
}


def source_names(path):
    return re.findall(r"#\[(?:tokio::)?test\][\s\S]*?\bfn\s+(\w+)\s*\(", path.read_text())


names = []
for module in ("path", "directory_records"):
    names += [f"{module}::tests::{name}" for name in source_names(
        root / f"crates/keelshell-platform/src/{module}.rs")]
names += [
    "sftp::queue::tests::local_unicode_case_aliases_cover_missing_leaf_and_ancestor_components",
    "sftp::queue::tests::local_unicode_case_aliases_preserve_real_unknown_reservations_after_staging",
    "zmodem_controller_unicode_alias_is_busy_before_accepting_any_receive_data",
    "files::transfer_tests::archive_tests::archive_unobserved_worker_result_keeps_the_unknown_close_hold",
]
names += [
    "sftp::archive_transfer::mutation_worker::tests::" + name
    for name in source_names(
        root / "crates/keelshell-session/src/sftp/archive_transfer/mutation_worker/tests.rs")
]
assert len(names) == len(set(names)) == 23
assert set(names).issubset(passed), sorted(set(names) - passed)
windows_sources = {
    path: source_names(root / path)
    for path in (
        "crates/keelshell-platform/src/windows/tests.rs",
        "crates/keelshell-platform/src/windows/security/tests.rs",
        "crates/keelshell-session/src/zmodem/local_windows/tests.rs",
        "crates/keelshell-session/src/sftp/archive_transfer/local_windows/tests.rs",
    )
}
record = {
    "recorded_utc": datetime.now(timezone.utc).isoformat(),
    "status": "AUTHOR_GATE_PASS",
    "freeze": freeze_path.name, "freeze_sha256": sha(freeze_path.read_bytes()),
    "engineering_files": len(freeze["files"]), "scripts": len(freeze["scripts"]),
    "source_drift": drift, "full_source_and_script_path_sets_match": True,
    "gate": receipt, "totals": totals, "python": python_totals, "suites": suites,
    "new_default_rust_cases": {name: "passed in workspace gate" for name in names},
    "new_default_rust_count": len(names),
    "new_python_cases": 39,
    "windows_test_definitions": windows_sources,
    "windows_native_executed": 0,
    "windows_native_status": "NOT_RUN",
    "stages": stages,
}
with (batch / "gate-summary-r2.json").open("x") as stream:
    json.dump(record, stream, indent=2)
    stream.write("\n")
print(json.dumps({key: record[key] for key in (
    "status", "engineering_files", "scripts", "totals", "python", "new_default_rust_count",
)}))
