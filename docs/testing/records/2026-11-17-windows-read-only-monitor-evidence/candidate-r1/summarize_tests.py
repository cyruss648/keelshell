"""Parse local or CI gates without treating early rustdoc headers as boundaries."""

import argparse
from collections import Counter, deque
import hashlib
import json
from pathlib import Path
import re

ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
STAMP = re.compile(r"^\ufeff?\d{4}-\d{2}-\d{2}T[\d:.]+Z ?")
HEADER = re.compile(r"(?:Running (?:unittests |tests[\\/])|Doc-tests )[^\n]+")
START = re.compile(r"^running (\d+) tests?\s*$")
RESULT = re.compile(r"\btest (.*?) \.\.\. (ok|ignored|FAILED)(?:,.*)?$")
SUMMARY = re.compile(
    r"\btest result: (ok|FAILED)\. (\d+) passed; (\d+) failed; "
    r"(\d+) ignored; (\d+) measured; (\d+) filtered out; finished in ([\d.]+)s"
)


def normalize(raw):
    return "\n".join(
        STAMP.sub("", ANSI.sub("", line)).rstrip("\r")
        for line in raw.decode("utf-8-sig").splitlines()
    )


def summarize(raw):
    text = normalize(raw)
    marker = "##[group]Run python scripts/check.py"
    offset = text.index(marker) if marker in text else 0
    end = text.find("\n##[group]Run ", offset + len(marker))
    gate = text[offset:end if end >= 0 else None]
    workspace = gate.split("Running: cargo test --workspace --locked -- --test-threads=4\n", 1)[1]
    workspace = workspace.split("Running: cargo test -p keelshell-ai ", 1)[0]
    pending = deque()
    suites = []
    custom_harnesses = []
    doc_headers = []
    doc_runs = []
    current = None
    for line_number, line in enumerate(workspace.splitlines(), 1):
        header = HEADER.search(line)
        if header:
            # These two Cargo targets use harness=false and print their own
            # controller receipts instead of a libtest run/summary pair.
            if header.group().startswith("Doc-tests "):
                doc_headers.append(header.group())
            elif re.search(r"tests[/\\]local_agent_(directory_)?process\.rs ", header.group()):
                custom_harnesses.append(header.group())
            else:
                pending.append((header.group(), line_number))
        start = START.match(line)
        if start:
            assert current is None, ("unfinished suite", current)
            if pending:
                suite, header_line = pending.popleft()
                kind = "ordinary"
            else:
                assert doc_headers, ("run without header", line_number)
                suite, header_line = f"Rustdoc subrun {len(doc_runs) + 1}", None
                kind = "rustdoc"
            current = {
                "suite": suite,
                "kind": kind,
                "header_line_in_workspace": header_line,
                "start_line_in_workspace": line_number,
                "declared_tests": int(start[1]),
                "results": [],
            }
            (suites if kind == "ordinary" else doc_runs).append(current)
        result = RESULT.search(line)
        if result:
            assert current is not None, ("result without run", line_number)
            current["results"].append(list(result.groups()))
        summary = SUMMARY.search(line)
        if summary:
            assert current is not None, ("summary without run", line_number)
            state, passed, failed, ignored, measured, filtered, seconds = summary.groups()
            current["summary"] = {
                "state": state,
                "passed": int(passed),
                "failed": int(failed),
                "ignored": int(ignored),
                "measured": int(measured),
                "filtered": int(filtered),
                "seconds": float(seconds),
            }
            counts = Counter(value for _, value in current["results"])
            assert len(current["results"]) == current["declared_tests"], current["suite"]
            assert len({name for name, _ in current["results"]}) == len(current["results"])
            assert [counts[value] for value in ("ok", "FAILED", "ignored")] == [
                int(passed), int(failed), int(ignored)
            ], current["suite"]
            current = None
    assert current is None and not pending, ("incomplete output", current, list(pending))
    return {
        "parser_revision": 4,
        "raw_bytes": len(raw),
        "raw_sha256": hashlib.sha256(raw).hexdigest(),
        "gate_command_present": True,
        "scope": "ordinary and rustdoc runs; rustdoc headers are metadata, not stdout boundaries",
        "suites": suites,
        "custom_harness_headers": custom_harnesses,
        "doctest_headers_unassigned": doc_headers,
        "doctest_runs_unassigned": doc_runs,
        "inventory_result_summary_mismatches": [],
        "tests_replayed": 0,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("output", type=Path)
    options = parser.parse_args()
    report = summarize(options.log.read_bytes())
    report["log"] = options.log.name
    with options.output.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    important = {
        row["suite"]: {
            "declared_tests": row["declared_tests"],
            "result_names": len(row["results"]),
            "summary": row["summary"],
        }
        for row in report["suites"]
        if "tests/archive.rs " in row["suite"]
        or "tests/archive_pax.rs " in row["suite"]
        or ("unittests src/main.rs " in row["suite"] and "keelshell_app" in row["suite"])
        or ("unittests src/lib.rs " in row["suite"] and "keelshell_session" in row["suite"])
    }
    print(json.dumps({
        "log": report["log"],
        "suites": len(report["suites"]),
        "important": important,
        "mismatches": report["inventory_result_summary_mismatches"],
        "tests_replayed": 0,
    }, ensure_ascii=False))


if __name__ == "__main__":
    main()
