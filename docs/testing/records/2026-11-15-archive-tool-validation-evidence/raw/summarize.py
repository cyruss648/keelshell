"""Derive counts from the completed gate without combining repeated controller checks."""
import json
from pathlib import Path
import re

work = Path(__file__).resolve().parent
receipt = json.loads((work / "full-gate-r2.json").read_text())
assert receipt["exit_code"] == 0 and receipt["reaped"]
lines = (work / "full-gate-r2.log").read_text().splitlines()
phase = None
kind = "ordinary"
target = None
summaries = []
passed_names = set()
python_runs = []
for line in lines:
    if line.startswith("Running: "):
        phase = line[len("Running: "):]
    if phase is None:
        continue
    if match := re.fullmatch(r"Ran (\d+) tests? in (.+)", line):
        python_runs.append({"phase": phase, "run": int(match[1]), "duration": match[2]})
    if not phase.startswith("cargo test --workspace"):
        continue
    if line.strip().startswith("Doc-tests "):
        kind = "rustdoc"
        target = line.strip()
    elif line.strip().startswith("Running "):
        kind = "ordinary"
        target = line.strip()
    if match := re.fullmatch(
        r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;.*", line
    ):
        summaries.append({
            "kind": kind, "target": target,
            "passed": int(match[1]), "failed": int(match[2]), "ignored": int(match[3]),
        })
    if match := re.fullmatch(r"test (\S+) \.\.\. ok", line):
        passed_names.add(match[1])
author = json.loads((work / "worker-originals/handoff-current.json").read_text())
expected = []
for item in author["files"]:
    if item["path"].endswith("archive_validation/tests.rs"):
        expected += ["archive_validation::tests::" + name for name in item["test_names"]]
    elif item["path"].endswith("tests/archive_validation.rs"):
        expected += item["test_names"]
expected += json.loads((work / "app-binary-binding-r2.json").read_text())["new_names"]
assert len(expected) == 29, expected
assert not (set(expected) - passed_names), sorted(set(expected) - passed_names)
totals = {}
for category in ["ordinary", "rustdoc"]:
    totals[category] = {
        key: sum(row[key] for row in summaries if row["kind"] == category)
        for key in ["passed", "failed", "ignored"]
    }
assert all(row["failed"] == 0 for row in summaries)
record = {
    "gate": receipt, "totals": totals, "suites": summaries, "python_runs": python_runs,
    "new_default_rust": {"count": 29, "all_passed": True, "names": sorted(expected)},
    "repeat_counts_excluded": [
        "readiness/observer/small-stack controller reruns",
        "targeted and independent reviewer Rust reruns",
        "Python suites repeated by the Rust bridge",
        "opt-in real OpenSSH tests",
    ],
}
(work / "gate-summary-r2.json").write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps({"totals": totals, "python_runs": python_runs, "new_default_rust": 29}))
