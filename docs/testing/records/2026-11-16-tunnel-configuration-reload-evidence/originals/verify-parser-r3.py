"""Verify sealed reload evidence and its exact source without rerunning tests."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def original(name):
    return (BASE / "originals" / name).read_bytes()


def document(name):
    return json.loads(original(name))


def successful(name):
    receipt = document(name + ".json")
    raw = original(name + ".log")
    assert receipt["observed_exit"] == receipt["controller_exit"] == 0, name
    assert not receipt["timed_out"] and receipt["terminal"]["direct_reaped"], name
    assert receipt["terminal"]["active_processes"] == 0, name
    assert receipt["terminal"]["pipes_drained"], name
    assert len(raw) == receipt["log_bytes"], name
    assert digest(raw) == receipt["log_sha256"], name
    return raw.decode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", help="Read a full committed Git SHA instead of the checkout")
    args = parser.parse_args()
    if args.source:
        assert re.fullmatch(r"[0-9a-f]{40}", args.source)
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    for entry in manifest["entries"]:
        data = (BASE / entry["path"]).read_bytes()
        assert len(data) == entry["bytes"] and digest(data) == entry["sha256"], entry["path"]
    freeze = document("inputs-freeze-r2.json")
    expected = {entry["path"]: entry["sha256"] for entry in freeze["inputs"]}
    assert len(expected) == freeze["count"] == 721 and list(expected) == sorted(expected)
    assert digest("".join(f"{name}\0{value}\n" for name, value in expected.items()).encode()) == freeze["aggregate_sha256"]
    command = (
        ["git", "ls-tree", "-rz", "--name-only", args.source] if args.source else
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]
    )
    names = sorted({
        name.decode() for name in subprocess.check_output(command, cwd=ROOT, timeout=60).split(b"\0")
        if name and not name.startswith((b"docs/", b".hallmark/"))
    })
    assert names == list(expected), "source input set differs"
    for name in names:
        data = (
            subprocess.check_output(["git", "show", f"{args.source}:{name}"], cwd=ROOT, timeout=30)
            if args.source else (ROOT / name).read_bytes()
        )
        assert digest(data) == expected[name], name

    inventory = re.findall(r"^(\S+): test$", successful("app-inventory-r3"), re.M)
    binary = document("app-binary-r3.json")
    assert len(inventory) == len(set(inventory)) == 1364
    assert inventory == binary["inventory_names"]
    assert len(binary["new_names"]) == binary["new_count"] == 21
    full = successful("full-gate-r1")
    workspace = full.split("Running: cargo test --workspace", 1)[1].split(
        "Running: cargo test -p keelshell-ai", 1
    )[0]
    headers = list(re.finditer(
        r"^\s*((?:Running (?:unittests |tests/)|Doc-tests ).+)$", workspace, re.M
    ))
    suites = []
    controllers = []
    for index, header in enumerate(headers):
        section = workspace[header.end():headers[index + 1].start() if index + 1 < len(headers) else None]
        summaries = re.findall(
            r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;", section
        )
        rustdoc = header[1].startswith("Doc-tests ")
        if not summaries:
            # These two harness-free binaries report their own bounded scenario
            # groups. They are not additional ordinary libtest cases.
            markers = {
                "local_agent_directory_process.rs":
                    "directory owned process controller: 20 cases passed",
                "local_agent_process.rs":
                    "local agent process integration: all assertions/scenario groups passed; no supplier CLI or model calls",
            }
            matched = [
                name for name, marker in markers.items()
                if f"Running tests/{name} " in header[1] and marker in section
            ]
            assert len(matched) == 1, header[1]
            controllers.extend(matched)
            continue
        # Rustdoc can use multiple libtest subruns for one crate, including
        # compile-fail checks. Preserve each real summary under its doc header.
        assert rustdoc or len(summaries) == 1, header[1]
        for status, passed, failed, ignored in summaries:
            assert status == "ok" and int(failed) == 0, header[1]
            suites.append({
                "label": header[1], "passed": int(passed), "failed": int(failed),
                "ignored": int(ignored), "rustdoc": rustdoc,
            })
        if "keelshell_app-" in header[1]:
            executed = re.findall(r"^test (\S+) \.\.\. (ok|ignored)(?:,.*)?$", section, re.M)
            assert len(executed) == len(set(name for name, _ in executed)) == len(inventory)
            assert sorted(name for name, _ in executed) == sorted(inventory)
            outcomes = dict(executed)
            assert all(outcomes[name] == "ok" for name in binary["new_names"])
            assert (int(passed), int(ignored)) == (1362, 2)
    assert any("keelshell_app-" in suite["label"] for suite in suites)
    assert sorted(controllers) == ["local_agent_directory_process.rs", "local_agent_process.rs"]
    assert re.findall(r"Ran (\d+) tests? in", full) == ["293"]
    assert "OK (skipped=2)" in full
    assert document("dependencies-report-r1.json")["status"] == "failed"
    fresh = document("dependencies-report-r2.json")
    assert fresh["status"] == "passed" and len(fresh["rust"]) == 113
    interop = document("openssh-russh-r1/result.json")
    assert interop["outcome"] == "passed" and interop["passed_tests"] == 17
    assert interop["cleanup"]["owned_processes_stopped"]
    assert interop["cleanup"]["temporary_directory_removed"]
    functional = successful("function-review-tunnel-r1")
    assert "63 passed; 0 failed; 0 ignored;" in functional
    normal = [suite for suite in suites if not suite["rustdoc"]]
    docs = [suite for suite in suites if suite["rustdoc"]]
    print(json.dumps({
        "source": args.source or "current", "source_inputs": len(names), "mismatches": 0,
        "sealed_members": len(manifest["entries"]), "ordinary_suites": len(normal),
        "workspace_harness_free_controllers": controllers,
        "ordinary_passed": sum(suite["passed"] for suite in normal),
        "ordinary_ignored": sum(suite["ignored"] for suite in normal),
        "rustdoc_passed": sum(suite["passed"] for suite in docs),
        "app_inventory": len(inventory), "app_passed": 1362, "app_ignored": 2,
        "new_names_passed": 21, "python_run": 293, "python_skipped": 2,
        "independent_prebuilt_tunnel_passed": 63, "real_openssh_passed": 17,
        "tests_replayed": 0, "native_desktop_accepted": False,
    }))


if __name__ == "__main__":
    main()
