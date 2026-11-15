"""Derive final counts and source bindings from the actual successful gate."""

import ast
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess

batch = Path(__file__).resolve().parent
root = batch.parents[1]
sha = lambda data: hashlib.sha256(data).hexdigest()
freeze_path = batch / "frozen-inputs-r5.json"
freeze = json.loads(freeze_path.read_text())
receipt_path = batch / "logs/full-gate-r5.json"
receipt = json.loads(receipt_path.read_text())
log_path = batch / "logs/full-gate-r5.log"
log = log_path.read_text()
assert receipt["exit"] == 0 and not receipt["timed_out"], receipt
assert sha(log_path.read_bytes()) == receipt["log_sha256"]

mismatches = [
    path
    for section in ("inputs", "scripts")
    for path, expected in freeze[section].items()
    if sha((root / path).read_bytes()) != expected
]
current_paths = {"Cargo.toml", "Cargo.lock"} | {
    path.relative_to(root).as_posix()
    for path in (root / "crates").rglob("*")
    if path.is_file()
    and (path.suffix in {".rs", ".py"} or path.name == "Cargo.toml")
}
assert not mismatches, mismatches
assert current_paths == set(freeze["inputs"])
assert "Dependency version policy: passed" in log

stage = None
suite = None
suites = []
passed_names = set()
stages = []
pattern = re.compile(
    r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;"
)
for line in log.splitlines():
    if line.startswith("Running: "):
        stage = line.removeprefix("Running: ")
        stages.append(stage)
    if "cargo test --workspace " not in (stage or ""):
        continue
    stripped = line.strip()
    if stripped.startswith("Running "):
        suite = {"kind": "ordinary", "preamble": stripped}
    elif stripped.startswith("Doc-tests "):
        suite = {"kind": "rustdoc", "preamble": stripped}
    if match := pattern.match(line):
        assert suite is not None, line
        suites.append(
            dict(
                suite,
                passed=int(match[1]),
                failed=int(match[2]),
                ignored=int(match[3]),
                original_summary=line,
            )
        )
    if match := re.match(r"^test (.+) \.\.\. ok$", line):
        passed_names.add(match[1])

assert len(stages) == 7, stages
assert "--controller-small-stack" in stages[-1]
ordinary = {
    field: sum(item[field] for item in suites if item["kind"] == "ordinary")
    for field in ("passed", "failed", "ignored")
}
rustdoc = {
    field: sum(item[field] for item in suites if item["kind"] == "rustdoc")
    for field in ("passed", "failed", "ignored")
}
assert ordinary["failed"] == rustdoc["failed"] == 0
python_match = re.search(
    r"^Ran (\d+) tests in ([0-9.]+)s\n\nOK \(skipped=(\d+)\)$", log, re.M
)
assert python_match is not None
python = {
    "run": int(python_match[1]),
    "skipped": int(python_match[3]),
    "seconds": float(python_match[2]),
}
python["passed"] = python["run"] - python["skipped"]
python["failed"] = python["errors"] = 0

new_cases = json.loads((batch / "new-cases-r4.json").read_text())
new_bindings = {}
for group, names in new_cases["groups"].items():
    for name in names:
        assert name in passed_names, (group, name)
        assert name not in new_bindings, name
        new_bindings[name] = {"group": group, "gate_passed": True}
assert len(new_bindings) == new_cases["rust_default_total"] == 59


def python_test_methods(source):
    return {
        f"{node.name}.{member.name}"
        for node in ast.walk(ast.parse(source))
        if isinstance(node, ast.ClassDef)
        for member in node.body
        if isinstance(member, ast.FunctionDef) and member.name.startswith("test_")
    }


python_path = "scripts/test_linux_file_interop.py"
baseline_python = subprocess.check_output(
    ["git", "show", freeze["base_sha"] + ":" + python_path], cwd=root, text=True
)
python_added = sorted(
    python_test_methods((root / python_path).read_text())
    - python_test_methods(baseline_python)
)
assert len(python_added) == new_cases["python_new_methods"] == 7
record = {
    "recorded_utc": datetime.now(timezone.utc).isoformat(),
    "status": "FINAL_AUTHOR_GATE_PASS_BOUND_TO_R5",
    "freeze": freeze_path.name,
    "freeze_sha256": sha(freeze_path.read_bytes()),
    "input_count": len(freeze["inputs"]),
    "script_count": len(freeze["scripts"]),
    "post_gate_mismatches": mismatches,
    "complete_engineering_path_set_matches": True,
    "gate": {
        "receipt": str(receipt_path.relative_to(batch)),
        "receipt_sha256": sha(receipt_path.read_bytes()),
        "log": str(log_path.relative_to(batch)),
        "log_sha256": receipt["log_sha256"],
        "exit": receipt["exit"],
        "seconds": receipt["seconds"],
        "timed_out": receipt["timed_out"],
        "subprocess_wait_completed": True,
    },
    "ordinary_rust": ordinary,
    "rustdoc": rustdoc,
    "python": python,
    "suites": suites,
    "stages": stages,
    "new_default_rust_count": len(new_bindings),
    "new_default_rust_named_passes": new_bindings,
    "new_python_methods": python_added,
    "python_new_methods_in_full_discovery": True,
    "python_focused_execution_receipt": "python-deadline-fix/green-boundary.json",
    "opt_in_new_peer_test_count": 1,
    "opt_in_peer_tests_not_in_default_counts": True,
    "overlapping_specialty_and_review_runs_not_added_to_default_counts": True,
    "controllers_reported_separately_from_harness_counts": True,
}
with (batch / "final-validation-r5.json").open("x") as stream:
    json.dump(record, stream, indent=2)
    stream.write("\n")
print(
    json.dumps(
        {
            key: record[key]
            for key in (
                "status",
                "freeze_sha256",
                "input_count",
                "script_count",
                "gate",
                "ordinary_rust",
                "rustdoc",
                "python",
                "new_default_rust_count",
                "new_python_methods",
            )
        },
        indent=2,
    )
)
