"""Verify retained originals and full source bindings without extracting or running tests.

Default/--current reads current bytes; --index reads staged bytes. --source and
--evidence independently select complete commit SHAs. Final choices come from
the manifest, or all four --final-* arguments may be supplied to check it.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
PREFIX = BASE.relative_to(ROOT).as_posix() + "/"
BATCH = "work/windows-directory-rename-integration-20261116"
PREVIOUS_BATCH = "work/windows-directory-cursor-integration-20261116"
PREVIOUS_EVIDENCE = "docs/testing/records/2026-11-16-windows-directory-cursor-evidence"
KIT = "work/gpui-kit-010-candidate-20261116"
BASE_COMMIT = "4d72e1e592bdd6194f6eadca3a6a760192b5fe82"
SCHEMA = "keelshell.windows-directory-rename.evidence.v1"
KNOWN_FREEZES = {
    1: "16037a06d3d88f25505b9f53154a7730f0defbab0798f061323f243d69442d47",
    2: "d0707bf3c8971f4d1116fe9b9fc39b1d153d3471639764db0cebe0feb645f4b2",
    3: "83e4927b62b7e86592c485f9cb65e46d2d96f0f39a52adb9c2278efe018b4593",
    4: "6ac314882cc707b7622a4b95082e3264b5e3e9359109d48cfcaf4b12c9f87755",
    5: "f2374487a954c6bc2be418461d8199e3c77ae1cf6d54f13b49103cf53ece4af4",
    6: "f0f64a6629b2441d63eef1e801189eaef69dd848aea0039eef4d09ff143b5164",
}
NATIVE = "crates/keelshell-platform/src/windows/native.rs"
TESTS = "crates/keelshell-platform/src/windows/tests.rs"
LOCALE_CASE = "i18n::tests::app_languages_keep_plain_input_navigation_ltr_after_rtl_locale"
AI_CASE = "ai_settings::request_options::draft_tests::duplicate_draft_edits_prune_models_cancel_and_discard_late_callback"
FILES_CASE = "files::browser_keyboard_tests::browser_keyboard_delete_repeats_only_review_and_cancel_keeps_remote_bytes"
TUNNEL_CASE = "workspace::tunnel_rule_tests::review_regressions::conflict_blocks_new_workspace_panels_until_authoritative_reload"
TERMINAL = {"direct_reaped": True, "active_processes": 0, "pipes_drained": True, "errors": []}


class EvidenceError(Exception):
    """An incomplete or inconsistent evidence binding."""


def require(condition, message):
    if not condition:
        raise EvidenceError(str(message))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def record(data, label):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f"{label}: duplicate JSON key {key}")
            result[key] = value
        return result
    try:
        return json.loads(data, object_pairs_hook=unique)
    except (UnicodeError, ValueError) as error:
        raise EvidenceError(f"{label}: invalid JSON: {error}") from error


def safe_name(name):
    require(isinstance(name, str) and name, "empty or non-string path")
    path = PurePosixPath(name)
    require(bool(path.parts) and not path.is_absolute() and name == path.as_posix()
            and not any(part in (".", "..") for part in path.parts)
            and "\\" not in name and "\0" not in name, f"unsafe path: {name!r}")
    return name


def read_regular(root, name):
    """Reject links and concurrent changes; leave the filesystem untouched."""
    safe_name(name)
    path = root / name
    for parent in (path, *path.parents):
        if parent == root:
            break
        require(not parent.is_symlink(), f"symbolic link: {name}")
    require(path.is_file(), f"missing regular file: {name}")
    with path.open("rb") as stream:
        before = os.fstat(stream.fileno())
        data = stream.read()
        after = os.fstat(stream.fileno())
    current = path.stat()
    fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(all(getattr(before, field) == getattr(after, field) == getattr(current, field)
                for field in fields) and len(data) == before.st_size,
            f"file changed during read: {name}")
    return data


def git(*arguments, input_data=None):
    result = subprocess.run(
        ["git", *arguments], cwd=ROOT,
        env=dict(os.environ, GIT_OPTIONAL_LOCKS="0", GIT_NO_LAZY_FETCH="1"),
        input=input_data, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        timeout=120, check=False,
    )
    require(result.returncode == 0,
            "git " + " ".join(arguments) + ": " + result.stderr.decode(errors="replace"))
    return result.stdout


def commit(value):
    require(re.fullmatch(r"[0-9a-f]{40}", value or ""), "expected complete 40-character commit SHA")
    require(git("rev-parse", "--verify", value + "^{commit}").decode().strip() == value,
            f"not an exact commit object: {value}")


def blobs(objects):
    identities = sorted(set(objects.values()))
    stream = io.BytesIO(git("cat-file", "--batch",
                           input_data="".join(oid + "\n" for oid in identities).encode()))
    cache = {}
    for oid in identities:
        header = stream.readline().decode().strip().split()
        require(len(header) == 3 and header[:2] == [oid, "blob"], f"missing Git blob: {oid}")
        data = stream.read(int(header[2]))
        require(len(data) == int(header[2]) and stream.read(1) == b"\n", f"short Git blob: {oid}")
        cache[oid] = data
    require(stream.read() == b"", "unexpected Git batch output")
    return {name: cache[oid] for name, oid in objects.items()}


def source_files(revision=None, index=False):
    """Read every non-document Git-visible input, using the freeze.py path policy."""
    objects = {}
    if revision:
        commit(revision)
        for entry in filter(None, git("ls-tree", "-rz", "--full-tree", revision).split(b"\0")):
            metadata, raw = entry.split(b"\t", 1)
            mode, kind, oid = metadata.decode().split()
            name = safe_name(raw.decode())
            if name.startswith("docs/"):
                continue
            require(mode in ("100644", "100755") and kind == "blob", f"non-file source: {name}")
            objects[name] = oid
        return blobs(objects)
    if index:
        for entry in filter(None, git("ls-files", "--stage", "-z").split(b"\0")):
            metadata, raw = entry.split(b"\t", 1)
            mode, oid, stage = metadata.decode().split()
            name = safe_name(raw.decode())
            if name.startswith("docs/"):
                continue
            require(stage == "0" and mode in ("100644", "100755") and name not in objects,
                    f"unmerged/non-file index: {name}")
            objects[name] = oid
        return blobs(objects)
    names = git("ls-files", "--cached", "--others", "--exclude-standard", "-z").split(b"\0")
    return {safe_name(raw.decode()): read_regular(ROOT, raw.decode())
            for raw in sorted(set(names))
            if raw and not raw.startswith(b"docs/") and (ROOT / raw.decode()).exists()}


def file_row(name, data):
    return {"path": safe_name(name), "bytes": len(data), "sha256": digest(data)}


def check_rows(rows):
    require(isinstance(rows, list), "file rows must be a list")
    names = set()
    for row in rows:
        name = safe_name(row["path"])
        require(name not in names, f"duplicate manifest path: {name}")
        names.add(name)
        require(type(row["bytes"]) is int and row["bytes"] >= 0, f"invalid length: {name}")
        require(re.fullmatch(r"[0-9a-f]{64}", row["sha256"]), f"invalid SHA-256: {name}")
    return names


def unpack(packed, manifest):
    require(manifest["archive"] == "originals.tar.gz", "unexpected archive filename")
    require(len(packed) == manifest["archive_bytes"]
            and digest(packed) == manifest["archive_sha256"], "archive length/SHA mismatch")
    rows = manifest["files"]
    names = check_rows(rows)
    require(len(rows) == manifest["count"]
            and sum(row["bytes"] for row in rows) == manifest["original_bytes"], "archive totals mismatch")
    result = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        require(len(members) == len(rows) and {item.name for item in members} == names,
                "archive member set or duplicate mismatch")
        require(all(item.isfile() and not item.issparse() for item in members), "non-regular archive member")
        by_name = {item.name: item for item in members}
        for row in rows:
            member = by_name[row["path"]]
            require(member.size == row["bytes"], "member length mismatch: " + member.name)
            stream = archive.extractfile(member)
            require(stream is not None, "missing member bytes: " + member.name)
            data = stream.read()
            require(file_row(row["path"], data) == {key: row[key] for key in ("path", "bytes", "sha256")},
                    "member length/SHA mismatch: " + row["path"])
            result[row["path"]] = data
    return result


def freeze_record(data, name):
    freeze = record(data, name)
    rows = freeze["inputs"]
    names = [safe_name(row["path"]) for row in rows]
    require(freeze["count"] == len(rows) == len(set(names)) == 719, f"{name}: expected 719 unique inputs")
    require(names == sorted(names) and all(not path.startswith("docs/") for path in names),
            f"{name}: unsorted or document inputs")
    require(all(re.fullmatch(r"[0-9a-f]{64}", row["sha256"]) for row in rows), f"{name}: invalid input SHA")
    aggregate = "".join(f"{row['path']}\0{row['sha256']}\n" for row in rows).encode()
    require(digest(aggregate) == freeze["aggregate_sha256"], f"{name}: aggregate mismatch")
    return freeze


def match_inputs(actual, freeze, label):
    expected = {row["path"]: row["sha256"] for row in freeze["inputs"]}
    require(set(actual) == set(expected),
            f"{label}: path mismatch: missing={sorted(set(expected)-set(actual))}, "
            f"extra={sorted(set(actual)-set(expected))}")
    require(not (bad := [name for name, data in actual.items() if digest(data) != expected[name]]),
            f"{label}: SHA mismatch: {bad}")


def reconstruct_history(files, selection, base_files):
    freezes = {}
    for name in sorted(files):
        match = re.fullmatch(r"batch/(?:inputs-freeze|engineering-inputs)-r(\d+)\.json", name)
        if match:
            number = int(match[1])
            value = freeze_record(files[name], name)
            require(number not in freezes or freezes[number][1] == value, f"conflicting r{number} freezes")
            freezes[number] = (name, value)
    require(set(range(1, 7)) <= set(freezes), "historical r1-r6 freezes missing")
    require(selection["freeze"] in {name for name, _ in freezes.values()},
            "selected final freeze must be a retained inputs-freeze-rN or engineering-inputs-rN")
    final = freeze_record(files[selection["freeze"]], selection["freeze"])
    history = []
    for number, (name, value) in sorted(freezes.items()):
        if number in KNOWN_FREEZES:
            require(value["aggregate_sha256"] == KNOWN_FREEZES[number], f"historical r{number} changed")
        if number <= 2:
            replacements = {
                NATIVE: "batch/source-r3/" + NATIVE,
                TESTS: ("batch/failed-r1-tests-reconstructed.rs" if number == 1 else "batch/source-r3/" + TESTS),
            }
        else:
            prefixes = [selection["snapshot"]] if name == selection["freeze"] else [
                prefix for prefix in (f"batch/source-r{number}", f"batch/snapshot-r{number}")
                if any(item.startswith(prefix + "/") for item in files)
            ]
            require(len(prefixes) == 1, f"r{number}: missing/ambiguous snapshot")
            prefix = prefixes[0] + "/"
            replacements = {item[len(prefix):]: item for item in files if item.startswith(prefix)}
            require(bool(replacements), f"r{number}: empty snapshot")
            if number in (3, 4, 5, 6):
                require(len(replacements) == number + 2, f"r{number}: snapshot count changed")
            rows = [file_row(path, files[item]) for path, item in sorted(replacements.items())]
            parent_manifest = f"batch/source-r{number}-manifest.json"
            if parent_manifest in files:
                original = record(files[parent_manifest], parent_manifest)
                check_rows(original["files"])
                require(sorted(original["files"], key=lambda row: row["path"]) == rows
                        and original.get("base", BASE_COMMIT) == BASE_COMMIT, f"r{number}: parent manifest mismatch")
        expected = {row["path"] for row in value["inputs"]}
        require(set(replacements) <= expected, f"r{number}: snapshot includes non-input")
        reconstructed = {}
        for path in expected:
            require(path in replacements or path in base_files, f"r{number}: missing input: {path}")
            reconstructed[path] = files[replacements[path]] if path in replacements else base_files[path]
        match_inputs(reconstructed, value, f"historical r{number}")
        history.append({"revision": f"r{number}", "freeze": name, "count": value["count"],
                        "aggregate_sha256": value["aggregate_sha256"], "replacements": replacements,
                        "other_inputs": "exact base Git blobs"})
    reconstruction = record(files["batch/failed-r1-reconstruction.json"], "r1 reconstruction")
    author = record(files["batch/test-author/FROZEN.json"], "test author freeze")
    failed = files["batch/failed-r1-tests-reconstructed.rs"]
    require(len(failed) == reconstruction["bytes"] == author["engineering_bytes"]
            and digest(failed) == reconstruction["sha256"] == author["engineering_sha256"],
            "failed r1 reconstructed test binding mismatch")
    return final, history


def receipt(files, path, expected_exit=None, timed_out=False):
    value = record(files[path], path)
    log = path[:-5] + ".log"
    require(log in files, "missing command log: " + log)
    data = files[log]
    require(len(data) == value["log_bytes"] and digest(data) == value["log_sha256"],
            "command log length/SHA mismatch: " + path)
    if expected_exit is not None:
        require(value["controller_exit"] == expected_exit
                and value["observed_exit"] == (None if timed_out else expected_exit)
                and value["timed_out"] is timed_out and value["interrupted"] is False
                and value["terminal"] == TERMINAL and value.get("errors", []) == [],
                "command exit/termination mismatch: " + path)
    return value, data.decode(errors="replace")


def totals(text):
    rows = [tuple(map(int, row)) for row in re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", text, re.M)]
    require(bool(rows) and "test result: FAILED." not in text, "missing/failed test summary")
    return dict(zip(("passed", "failed", "ignored"),
                    (sum(row[index] for row in rows) for index in range(3))))


def package_output(ordinary, package, target):
    starts = list(re.finditer(r"^     Running ([^\n]+) \(([^\n]+)\)\n", ordinary, re.M))
    for index, match in enumerate(starts):
        if match[1] == target and ("/" + package + "/" in match[2]
                                  or "/" + package.replace("-", "_") + "-" in match[2]):
            end = starts[index + 1].start() if index + 1 < len(starts) else len(ordinary)
            return ordinary[match.end():end]
    raise EvidenceError(f"missing compiled package output: {package}/{target}")


def gate_sections(text):
    marker = "Running: cargo test --workspace --locked -- --test-threads=4\n"
    require(text.count(marker) == 1, "workspace gate marker missing/ambiguous")
    workspace = text.split(marker, 1)[1].split("Running: cargo test -p keelshell-ai", 1)[0]
    require("   Doc-tests " in workspace, "final gate did not reach rustdoc")
    return workspace.split("   Doc-tests ", 1)


def test_names(text):
    return re.findall(r"^test ([^\n]+?) \.\.\. (?:ok|ignored[^\n]*)$", text, re.M)


def check_final_gate(files, selection, final, previous):
    value, text = receipt(files, selection["gate"], 0)
    require(value["command"] == ["python3", "scripts/check.py"], "selected command is not the full gate")
    summary = record(files[selection["summary"]], selection["summary"])
    require(summary["gate"] == value and summary["engineering_inputs"] == final["count"]
            and summary["engineering_sha256"] == final["aggregate_sha256"]
            and summary["complete_input_path_and_byte_set_matches"] is True, "final summary input binding mismatch")
    ordinary, rustdoc = gate_sections(text)
    require(summary["rust"] == {"ordinary": totals(ordinary), "rustdoc": totals(rustdoc)}, "Rust totals mismatch")
    app = package_output(ordinary, "keelshell-app", "unittests src/main.rs")
    executed = test_names(app)
    require(len(executed) == len(set(executed)) == summary["app_inventory_count"]
            and summary["app"] == totals(app)
            and summary["app_executed_names_match_prior_compiled_inventory"] is True, "app results mismatch")
    inventory_name = safe_name(summary["app_inventory"])
    require("/" not in inventory_name and inventory_name.endswith(".log"), "invalid selected inventory")
    _, inventory = receipt(files, "batch/" + inventory_name[:-4] + ".json", 0)
    names = re.findall(r"^([^\n]+): test$", inventory, re.M)
    require(len(names) == len(set(names)) == len(executed) and set(names) == set(executed),
            "selected compiled/executed name sets differ")
    required = summary["required_current_app_names_passed"]
    require(isinstance(required, list) and len(required) == len(set(required))
            and {LOCALE_CASE, AI_CASE, FILES_CASE} <= set(required),
            "final summary omits required regression names")
    require(all(f"test {name} ... ok" in app for name in required)
            and f"test {TUNNEL_CASE} ... ok" in app, "required final regression did not pass")
    archive = package_output(ordinary, "keelshell-session", "tests/server_archive.rs")
    previous_ordinary, _ = gate_sections(previous["batch/full-gate-r2.log"].decode())
    previous_archive = package_output(previous_ordinary, "keelshell-session", "tests/server_archive.rs")
    require(len(test_names(archive)) == len(set(test_names(archive))) == 40
            and set(test_names(archive)) == set(test_names(previous_archive))
            and summary["server_archive"] == {**totals(archive), "exact_40_names_match": True},
            "server archive exact 40 results mismatch")
    platform = package_output(ordinary, "keelshell-platform", "unittests src/lib.rs")
    require(all(summary["platform"][key] == count for key, count in totals(platform).items()), "platform totals mismatch")
    for name in summary["platform"]["retained_host_diagnostic_names_passed"]:
        require(f"test {name} ... ok" in platform, "host diagnostic result missing")
    python = re.search(r"^Ran (\d+) tests in [^\n]+\n\nOK \(skipped=(\d+)\)", text, re.M)
    require(python is not None, "Python gate completion missing")
    require(summary["python"] == {"run": int(python[1]), "passed": int(python[1]) - int(python[2]),
                                  "skipped": int(python[2]), "failed": 0}, "Python totals mismatch")
    controllers = re.findall(r"^Running: (cargo test -p keelshell-ai[^\n]+)$", text, re.M)
    require(len(controllers) == 3 and summary["controllers"] == controllers
            and summary["format_strict_clippy_xy_policy_and_three_controllers_passed"] is True,
            "final gate/controller completion mismatch")
    require(summary["windows_native_execution"] == 0 and summary["new_native_gui_acceptance"] is False,
            "new native acceptance needs a separate evidence policy")
    return {"receipt": selection["gate"], "exit": 0, "seconds": value["seconds"],
            "compiled_inventory": inventory_name, "app": summary["app"]}


def review_binding(files, path, freeze, final=False):
    value = record(files[path], path)
    bindings = [value.get(key) for key in ("input", "final_input", "input_binding")] + [value]
    require(any(isinstance(binding, dict)
                and binding.get("count", binding.get("engineering_inputs")) == freeze["count"]
                and binding.get("aggregate_sha256", binding.get("engineering_sha256")) == freeze["aggregate_sha256"]
                for binding in bindings), f"{path}: complete input binding missing")
    directory = path.rsplit("/", 1)[0] + "/"
    require(any(name.startswith(directory) and name.endswith(".md") for name in files), f"{path}: report missing")
    if final:
        decision = value.get("decision", "")
        require(decision.startswith("APPROVE") and not any(
            word in decision.upper() for word in ("UNSATISFIED", "REJECT", "FAILED")),
            f"{path}: final composition approval not ready")
        require(value.get("blocking_findings", value.get("blocking_code_findings_in_reviewed_scope")) == 0
                and all(value.get("findings_by_severity", {}).get(key) == 0 for key in ("P0", "P1", "P2", "P3")),
                f"{path}: blocking or unreported findings")
        require(not value.get("delivery_blockers"), f"{path}: final review has delivery blockers")
        condition = value.get("approval_condition", value.get("approval_conditions"))
        require(isinstance(condition, (dict, list)) and bool(condition),
                f"{path}: own combination/gate condition must be explicit")
        if isinstance(condition, dict) and condition.get("status") == (
            "SATISFIED_BY_PARENT_SAME_R6_FINAL_COMPLETE_GATE"
        ):
            # The final reviewer records satisfied facts instead of retaining a
            # pending "required" list. Bind that distinct shape to its readback.
            require(value.get("r6_complete_gate_passed") is True
                    and condition.get("engineering_inputs") == freeze["count"]
                    and condition.get("engineering_sha256") == freeze["aggregate_sha256"]
                    and condition.get("terminal_exit") == 0
                    and condition.get("direct_reaped") is True
                    and condition.get("all_1342_app_names_equal_inventory") is True
                    and condition.get("all_four_exact_names_ok") is True
                    and condition.get("server_archive_all_40_names_match") is True,
                    f"{path}: satisfied condition facts do not match the final freeze")
            readback_path = directory + "full-gate-r4-final-readback.json"
            readback = record(files[readback_path], readback_path)
            require(readback["engineering_inputs"] == freeze["count"]
                    and readback["engineering_sha256"] == freeze["aggregate_sha256"]
                    and readback["complete_input_path_and_byte_set_matches"] is True
                    and readback["gate"]["observed_exit"] == 0
                    and readback["gate"]["controller_exit"] == 0
                    and readback["gate"]["terminal"]["direct_reaped"] is True
                    and readback["gate"]["seconds"] == condition["seconds"]
                    and readback["log_sha256"] == digest(files["batch/full-gate-r4.log"])
                    and readback["receipt_sha256"] == digest(files["batch/full-gate-r4.json"])
                    and readback["summary_sha256"] == digest(files["batch/full-gate-r4-summary.json"]),
                    f"{path}: satisfied final review readback differs from the gate originals")
        else:
            conditions = condition if isinstance(condition, list) else [condition]
            require(all(isinstance(item, dict) and bool(item.get("required", item.get("condition")))
                        and not any(word in str(item.get("status", "")).upper()
                                    for word in ("FAILED", "UNSATISFIED", "REJECT"))
                        for item in conditions),
                    f"{path}: final combination condition is failed or unspecified")
    return {"path": path, "sha256": digest(files[path]), "decision": value.get("decision"),
            "aggregate_sha256": freeze["aggregate_sha256"],
            "approval_conditions_preserved": True, "used_for_final_binding": final}


def historical_commands(files, selection, additional_bindings=None):
    for path, data in files.items():
        if path.endswith(".json"):
            value = record(data, path)
            if isinstance(value, dict) and {"observed_exit", "controller_exit", "log_bytes", "log_sha256"} <= value.keys():
                receipt(files, path)
    receipt(files, "batch/scoped-gate-r1.json", 1)
    receipt(files, "batch/scoped-gate-r2.json", 0)
    failures = []
    known = {1: (3, AI_CASE), 2: (4, FILES_CASE), 3: (5, TUNNEL_CASE)}
    require(all(f"batch/full-gate-r{number}.json" in files for number in known), "historical full gates missing")
    extra_bindings = record(files.get("batch/gate-input-bindings.json", b"{}"), "gate input bindings")
    require(isinstance(extra_bindings, dict), "gate input bindings must be an object")
    for label, revision in (additional_bindings or {}).items():
        require(label not in extra_bindings or extra_bindings[label] == revision,
                "conflicting historical gate input: " + label)
        extra_bindings[label] = revision
    for label, revision in extra_bindings.items():
        require(re.fullmatch(r"full-gate-r\d+", label) and re.fullmatch(r"r\d+", revision),
                "invalid historical gate input binding")
        number = int(label.rsplit("r", 1)[1])
        require(number not in known or revision == f"r{known[number][0]}",
                "known historical gate input binding changed")
    for path in sorted(files):
        match = re.fullmatch(r"batch/(full-gate-r(\d+))\.json", path)
        if not match or path == selection["gate"]:
            continue
        value, text = receipt(files, path)
        if value["controller_exit"] != 0:
            number = int(match[2])
            revision = f"r{known[number][0]}" if number in known else extra_bindings.get(match[1])
            require(revision is not None, f"{path}: supply --history-gate {match[1]}=rN when sealing")
            require(any(re.fullmatch(r"batch/(?:inputs-freeze|engineering-inputs)-" + re.escape(revision) + r"\.json", name)
                        for name in files), f"{path}: historical input revision missing")
            if number in known:
                receipt(files, path, 1)
                require(known[number][1] in text and "has overflowed its stack" in text and "SIGABRT" in text,
                        path + ": original failure changed")
            failures.append({"receipt": path, "input_revision": revision,
                             "controller_exit": value["controller_exit"], "seconds": value["seconds"]})
    _, repro = receipt(files, "batch/kit-stack-repro-r1.json", 250)
    require(AI_CASE in repro and "stack overflow" in repro, "stack reproduction missing")
    receipt(files, "batch/kit-stack-lldb-r1.json", 124, timed_out=True)
    for label, counts, case in (
        ("kit-stack-fixed-r1", (1, 0, 0), AI_CASE),
        ("kit-ai-settings-r1", (79, 0, 2), AI_CASE),
        ("kit-files-stack-fixed-r1", (1, 0, 0), FILES_CASE),
        ("kit-tunnels-stack-fixed-r1", (1, 0, 0), TUNNEL_CASE),
    ):
        _, text = receipt(files, "batch/" + label + ".json", 0)
        require(totals(text) == dict(zip(("passed", "failed", "ignored"), counts))
                and f"test {case} ... ok" in text, label + ": repair results mismatch")
        if label != "kit-ai-settings-r1":
            require("Compiling keelshell-app" in text, label + ": real recompile missing")
    for label in ("kit-stack-own-crash-frames-r1", "kit-files-stack-crash-frames-r1"):
        require(bool(record(files["batch/" + label + ".json"], label)), label + ": crash frames missing")
    receipt(files, "batch/kit-lock-update-r1.json", 0)
    for label in ("kit-locale-green-r1", "kit-locale-green-r2"):
        _, text = receipt(files, "batch/" + label + ".json", 0)
        require(totals(text) == {"passed": 1, "failed": 0, "ignored": 0}, label + ": locale result mismatch")
    _, red = receipt(files, "batch/kit-locale-red-r1.json", 101)
    require("test result: FAILED. 0 passed; 1 failed;" in red and 'left: "abX"' in red and 'right: "aXb"' in red,
            "locale real mutation red mismatch")
    locale = files["batch/source-r3/crates/keelshell-app/src/i18n.rs"]
    removed = b"    gpui_kit::base::LayoutDirection::from_locale(language.code()).set_global(cx);\n"
    require(locale.count(removed) == 1 and files["batch/kit-locale-mutant-r1.rs"] == locale.replace(removed, b"", 1),
            "locale mutant is not the exact one-line removal")
    restored = record(files["batch/kit-locale-restored-r3.json"], "locale restore")
    require(restored["restored_r3_complete_719_inputs"] is True and restored["aggregate_sha256"] == KNOWN_FREEZES[3],
            "locale full input restore mismatch")
    receipt(files, "batch/dependencies-live-r3.json", 0)
    return failures


def previous_archive(files, reference, read_evidence):
    manifest_data = read_evidence(PREVIOUS_EVIDENCE + "/archive-manifest.json")
    require(digest(manifest_data) == reference["manifest_sha256"]
            and len(manifest_data) == reference["manifest_bytes"]
            and manifest_data == files["previous-delivery/prior-archive-manifest.json"], "prior manifest mismatch")
    prior_manifest = record(manifest_data, "prior manifest")
    require(reference["directory"] == PREVIOUS_EVIDENCE
            and reference["archive_bytes"] == prior_manifest["archive_bytes"]
            and reference["archive_sha256"] == prior_manifest["archive_sha256"], "prior archive reference mismatch")
    previous = unpack(read_evidence(PREVIOUS_EVIDENCE + "/originals.tar.gz"), prior_manifest)
    omitted = reference["omitted_unchanged"]
    require(len({row["path"] for row in omitted}) == len(omitted) >= 70, "prior omission count mismatch")
    for row in omitted:
        require(row["path"] in previous and file_row(row["path"], previous[row["path"]]) == row,
                "omission not present in prior archive")
        require("previous-delivery/" + row["path"].removeprefix("batch/") not in files, "old duplicate copied")
    readback = record(files["previous-delivery/source-readback-r1.json"], "prior source readback")
    require(readback["local"] == readback["tracking"] == readback["remote"] == BASE_COMMIT
            and readback["equal"] is True and readback["status_porcelain"] == "", "prior source identity mismatch")
    terminal = record(files["previous-delivery/source-ci-terminal-r1.log"], "prior CI terminal")
    require(terminal["headSha"] == BASE_COMMIT and terminal["status"] == "completed"
            and terminal["conclusion"] == "failure" and terminal["databaseId"] == 43822369487,
            "prior terminal CI mismatch")
    expected = record(files["batch/windows-expected-inventory-r1.json"], "Windows expectation")
    require(expected["expected_count"] == len(set(expected["expected_names"])) == 70
            and expected["new_windows_execution"] == "NOT_RUN", "Windows expectation mismatch")
    text = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", files["previous-delivery/source-windows-log-r3.log"].decode())
    results = re.findall(r"\btest ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED)\s*$", text, re.M)
    require(len(results) == len({name for name, _ in results}) == 67
            and sum(status == "ok" for _, status in results) == 63
            and sum(status == "FAILED" for _, status in results) == 4
            and {name for name, _ in results} == set(expected["expected_names"]) - set(expected["new_names"]),
            "prior Windows actual 63/4/0 mismatch")
    return previous


def validate(files, manifest, read_evidence, actual_source):
    require(manifest["schema"] == SCHEMA and manifest["base_commit"] == BASE_COMMIT, "schema/base mismatch")
    selection = manifest["final"]
    require(set(selection) == {"freeze", "snapshot", "gate", "summary", "review"}, "incomplete final selection")
    require(all(safe_name(name).startswith("batch/") for name in selection.values()), "selection outside batch")
    previous = previous_archive(files, manifest["previous_archive"], read_evidence)
    final, history = reconstruct_history(files, selection, source_files(BASE_COMMIT))
    require(manifest["engineering_inputs"] == final["count"]
            and manifest["engineering_sha256"] == final["aggregate_sha256"], "manifest final input mismatch")
    require(record(files["derived/input-bindings.json"], "derived bindings") == history, "derived history mismatch")
    for item in history:
        if item["revision"] in ("r1", "r2"):
            continue
        expected = {"base": BASE_COMMIT, "revision": item["revision"],
                    "files": [file_row(path, files[name]) for path, name in sorted(item["replacements"].items())],
                    "derived_from_retained_snapshot": True}
        name = "derived/source-" + item["revision"] + "-manifest.json"
        require(record(files[name], name) == expected, "derived snapshot mismatch: " + name)
    failures = historical_commands(files, selection, manifest["additional_gate_input_bindings"])
    require(manifest["historical_full_gate_failures"] == failures, "historical failure manifest mismatch")
    review_freezes = {item["aggregate_sha256"]: freeze_record(files[item["freeze"]], item["freeze"])
                      for item in history}
    reviews = []
    for path in sorted(name for name in files if re.fullmatch(r"batch/reviewer[^/]*/summary\.json", name)):
        value = record(files[path], path)
        binding = value.get("input", value.get("final_input", value.get("input_binding", value)))
        sha = binding.get("aggregate_sha256", binding.get("engineering_sha256"))
        require(sha in review_freezes, path + ": review freeze missing")
        reviews.append(review_binding(files, path, review_freezes[sha], final=path == selection["review"]))
    require(selection["review"] in {item["path"] for item in reviews}, "selected final review summary missing")
    require(manifest["reviews"] == reviews, "retained review scope mismatch")
    r4 = record(files["batch/reviewer-stack-r4/summary.json"], "r4 review")
    require(r4["r4_combination_passed"] is False and r4["merge_recommendation"] == "HOLD_R4_COMPOSITION"
            and r4["approval_condition"]["status"] == "UNSATISFIED_R4_FULL_GATE_R2_FAILED",
            "historical r4 composition HOLD changed")
    r5 = record(files["batch/reviewer-files-r5/summary.json"], "r5 review")
    require(r5["r5_combination_passed"] is False and r5["merge_recommendation"] == "HOLD_R5_COMPOSITION"
            and r5["approval_condition"]["status"] == "UNSATISFIED_R5_FULL_GATE_R3_FAILED",
            "historical r5 composition HOLD changed")
    windows_app = record(files["batch/windows-app-archive-expected-r1.json"], "Windows app archive expectation")
    require(windows_app["expected_count"] == len(windows_app["expected_names"])
            == len(set(windows_app["expected_names"])) == 8
            and windows_app["current_windows_execution"] == "NOT_RUN"
            and re.fullmatch(r"[0-9a-f]{40}", windows_app["prior_source"])
            and re.fullmatch(r"[0-9a-f]{64}", windows_app["source_record_sha256"]),
            "old eight Windows app expectations must remain expectations, not current execution")
    gate = check_final_gate(files, selection, final, previous)
    delivery = record(files["kit-author/DELIVERY.json"], "Kit delivery")
    kit_names = check_rows(delivery["files"])
    require({name.removeprefix("kit-author/") for name in files if name.startswith("kit-author/")}
            == kit_names | {"DELIVERY.json"}, "Kit delivery set mismatch")
    for row in delivery["files"]:
        require(file_row(row["path"], files["kit-author/" + row["path"]]) == row, "Kit original mismatch")
    require(not any(set(PurePosixPath(name).parts) & {"downloads", "upstream", "diffs", "__pycache__", "target"}
                    or name.endswith((".pyc", ".html")) for name in files), "excluded third-party/build payload")
    for item in record(files["batch/official-source-receipts-r1.json"], "official reference metadata"):
        require(item["url"].startswith("https://") and re.fullmatch(r"[0-9a-f]{64}", item["sha256"])
                and type(item["bytes"]) is int, "official reference metadata incomplete")
    match_inputs(actual_source, final, "selected final source")
    require(manifest["original_count"] == sum(row["kind"] == "original" for row in manifest["files"])
            and manifest["derived_count"] == sum(row["kind"] == "derived" for row in manifest["files"])
            and manifest["original_count"] + manifest["derived_count"] == manifest["count"],
            "original/derived counts mismatch")
    return {"members": manifest["count"], "originals": manifest["original_count"],
            "derived": manifest["derived_count"], "engineering_inputs": final["count"],
            "engineering_sha256": final["aggregate_sha256"], "history": [row["revision"] for row in history],
            "historical_full_gate_failures": failures, "final": gate,
            "kit_author_files_including_delivery": len(kit_names) + 1,
            "previous_unchanged_omitted": len(manifest["previous_archive"]["omitted_unchanged"]),
            "locale_mutant_exit": 101, "stack_repro_controller_exit": 250,
            "lldb_controller_exit": 124, "lldb_timed_out": True, "lldb_processes_reaped": True,
            "predecessor_windows": "63/4/0", "windows_app_archive_expected_only": 8,
            "new_windows_execution": 0, "mismatches": []}


def add_selection_arguments(parser, required=False):
    parser.add_argument("--final-freeze", required=required, help="batch-relative inputs-freeze-rN.json")
    parser.add_argument("--final-snapshot", required=required, help="batch-relative source-rN/ or snapshot-rN/")
    parser.add_argument("--final-gate", required=required, help="full-gate-rN label, receipt, or summary JSON")
    parser.add_argument("--final-review", required=required, help="reviewer directory or its summary.json")


def selection_from_args(args):
    def relative(value):
        value = str(value).rstrip("/")
        path = Path(value)
        if path.is_absolute():
            try:
                value = path.relative_to(ROOT / BATCH).as_posix()
            except ValueError as error:
                raise EvidenceError("final parameter outside batch") from error
        elif value.startswith(BATCH + "/"):
            value = value[len(BATCH) + 1:]
        elif value.startswith("batch/"):
            value = value[6:]
        return safe_name(value)
    gate = relative(args.final_gate)
    if gate.endswith("-summary.json"):
        gate = gate[:-13]
    elif gate.endswith(".json"):
        gate = gate[:-5]
    review = relative(args.final_review)
    if not review.endswith(".json"):
        review += "/summary.json"
    return {"freeze": "batch/" + relative(args.final_freeze), "snapshot": "batch/" + relative(args.final_snapshot),
            "gate": "batch/" + gate + ".json", "summary": "batch/" + gate + "-summary.json",
            "review": "batch/" + review}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--current", action="store_true", help="current evidence (default)")
    modes.add_argument("--index", action="store_true", help="staged source and evidence")
    modes.add_argument("--evidence", help="complete 40-character evidence commit SHA")
    parser.add_argument("--source", help="independent complete 40-character source commit SHA")
    add_selection_arguments(parser)
    args = parser.parse_args()
    supplied = [getattr(args, name) is not None for name in ("final_freeze", "final_snapshot", "final_gate", "final_review")]
    if any(supplied) and not all(supplied):
        parser.error("supply all four --final-* parameters or read the manifest selection")
    for name in ("source", "evidence"):
        value = getattr(args, name)
        if value and not re.fullmatch(r"[0-9a-f]{40}", value):
            parser.error("--" + name + " requires a complete lowercase 40-character commit SHA")
    if args.evidence:
        commit(args.evidence)
    revision = ":" if args.index else args.evidence + ":" if args.evidence else None

    def read_evidence(name):
        safe_name(name)
        return git("show", revision + name) if revision else read_regular(ROOT, name)

    manifest = record(read_evidence(PREFIX + "archive-manifest.json"), "archive manifest")
    if all(supplied):
        require(selection_from_args(args) == manifest["final"], "explicit final parameters differ from manifest")
    files = unpack(read_evidence(PREFIX + "originals.tar.gz"), manifest)
    verifier = read_evidence(PREFIX + "verify_evidence.py")
    require(verifier == files["tools/verify_evidence.py"] and file_row("verify_evidence.py", verifier) == manifest["verifier"],
            "evidence verifier differs from archived tool")
    require(file_row("README.md", read_evidence(PREFIX + "README.md")) == manifest["readme"], "README mismatch")
    result = validate(files, manifest, read_evidence, source_files(args.source, index=args.index and not args.source))
    result.update({"source": args.source or ("index" if args.index else "current"),
                   "evidence": "index" if args.index else args.evidence or "current"})
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except (EvidenceError, OSError, KeyError, TypeError, ValueError, EOFError,
            tarfile.TarError, subprocess.TimeoutExpired) as error:
        print("Evidence verification failed: " + str(error), file=sys.stderr)
        sys.exit(2)
