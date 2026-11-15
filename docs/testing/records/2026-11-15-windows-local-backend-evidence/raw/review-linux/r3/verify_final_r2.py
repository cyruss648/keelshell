#!/usr/bin/env python3
"""Read retained Linux artifacts; write evidence only in this r3 directory."""

import ast
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import time
import tomllib

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
OUT = HERE / "final-r2"
REVIEW = HERE.parent
BASE = REVIEW.parent
ROOT = BASE.parents[1]
RUN = BASE / "linux-final-r2"
SNAPSHOT_PATH = BASE / "validation-snapshot-r2.json"
SNAPSHOT = json.loads(SNAPSHOT_PATH.read_text())
FREEZE_PATH = BASE / SNAPSHOT["engineering_freeze"]
REVISION = SNAPSHOT["revision"]
EXPECTED_REVISION = "f945c4c780010067aee664bf7ccbf6caace4eea5"
EXPECTED_FREEZE_SHA = "1fb9b4b343d25634fed4bc6cf9ec15d12ac7049562fd83f977960d9d0b78e1c9"
RUNNER_SHA = "b134be0b11b7b79b5c88baddf363a25bad3b5f4583265e1ca26ff8a35a4a55b0"
COMMANDS = []
CHECKS = []


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def load(path):
    return json.loads(path.read_text())


def save(name, value):
    (OUT / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def check(condition, description):
    if not condition:
        raise AssertionError(description)
    CHECKS.append(description)


def observe(argv):
    started = time.monotonic()
    result = subprocess.run(argv, cwd=ROOT, capture_output=True, timeout=10)
    COMMANDS.append({
        "argv": argv,
        "returncode": result.returncode,
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "stdout": result.stdout.decode(),
        "stderr": result.stderr.decode(),
        "waited_and_reaped": True,
    })
    return result


def inventory_manifest(directory, exclude_r3=False, exclude_registry=False):
    result = {}
    for path in sorted(directory.rglob("*")):
        if not path.is_file():
            continue
        if exclude_r3 and path.is_relative_to(HERE):
            continue
        if exclude_registry and path == RUN / "registry.tar":
            continue
        result[str(path.relative_to(directory))] = {
            "sha256": file_digest(path), "bytes": path.stat().st_size,
        }
    return result


def raw(step, stderr=False):
    return (RUN / step["stderr_log" if stderr else "stdout_log"]).read_text()


def index_path(name):
    name = name.lower()
    if len(name) < 3:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def main():
    OUT.mkdir(exist_ok=False)
    historical = inventory_manifest(REVIEW, exclude_r3=True)
    retained_start = inventory_manifest(RUN, exclude_registry=True)
    save("history-manifest-start.json", historical)
    save("native-artifacts-manifest-start.json", retained_start)
    r1_manifest = load(REVIEW / "r2/history-manifest-start.json")
    check(all(historical.get(k) == v for k, v in r1_manifest.items()),
          "All 35 original r1 historical artifacts retain exact bytes")

    receipt = load(RUN / "receipt.json")
    cleanup = load(RUN / "cleanup.json")
    freeze = load(FREEZE_PATH)
    snapshot = load(SNAPSHOT_PATH)
    source = load(RUN / "source-manifest.json")
    runner_freeze = load(BASE / "linux-runner-inputs-r2.json")
    r2_summary = load(REVIEW / "r2/summary-approve-r2.json")
    check(REVISION == EXPECTED_REVISION and file_digest(FREEZE_PATH) == EXPECTED_FREEZE_SHA
          and len(freeze["files"]) == 588 and len(freeze["scripts"]) == 14,
          "The independently pinned final r2 revision and 588+14 freeze match the explicit parent request")
    check(receipt["status"] == receipt["execution_status"] == "PASS"
          and not receipt.get("error") and not receipt.get("persistence_errors"),
          "Final execution and receipt are PASS without persistence or primary error")
    check(receipt["revision"] == source["revision"] == snapshot["revision"] == REVISION,
          "Receipt, source manifest and immutable validation record bind the exact requested commit")
    check(snapshot["freeze_sha256"] == file_digest(FREEZE_PATH)
          and FREEZE_PATH.name == snapshot["engineering_freeze"]
          and snapshot["base"] == freeze["baseline"] == r2_summary["baseline"],
          "Validation snapshot binds the exact combined freeze and original review baseline")

    tree_result = observe([
        "git", "--no-optional-locks", "ls-tree", "-r", "-z", "--full-tree",
        REVISION, "--", "Cargo.toml", "Cargo.lock", "crates", "scripts",
    ])
    check(tree_result.returncode == 0, "Read-only Git tree query completed")
    tree = {}
    for item in tree_result.stdout.split(b"\0"):
        if not item:
            continue
        meta, name = item.split(b"\t", 1)
        mode, kind, oid = meta.decode().split()
        check(kind == "blob" and mode in ("100644", "100755"),
              "Scoped Git input is an ordinary blob: " + name.decode())
        tree[name.decode()] = oid
    commit_result = observe(["git", "--no-optional-locks", "cat-file", "-p", REVISION])
    check(commit_result.returncode == 0
          and commit_result.stdout.splitlines()[0].decode() == "tree " + snapshot["tree"],
          "Git commit object has the recorded immutable tree")
    git_source = {name: oid for name, oid in tree.items() if not name.startswith("scripts/")}
    git_scripts = {name: oid for name, oid in tree.items() if name.startswith("scripts/")}
    check(set(git_source) == set(freeze["files"]) and len(git_source) == len(freeze["files"]),
          "The exact Git source path set equals all combined source inputs")
    check(set(git_scripts) == set(freeze["scripts"]),
          "Git scripts path set equals all combined gate/runner script inputs")

    archived = {}
    with tarfile.open(RUN / "source.tar") as archive:
        archive_comment = archive.pax_headers.get("comment")
        check(archive_comment == REVISION, "Git archive global comment identifies the exact commit")
        for member in archive:
            name = member.name.rstrip("/")
            path = PurePosixPath(name)
            check(not path.is_absolute() and ".." not in path.parts
                  and str(path) == name and not any(c in name for c in "\r\n\0\\"),
                  "Safe canonical source archive member: " + name)
            if member.isdir():
                continue
            check(member.isfile() and name not in archived and name in git_source,
                  "Unique ordinary source archive input: " + name)
            data = archive.extractfile(member).read()
            blob_oid = hashlib.sha1(
                b"blob " + str(len(data)).encode() + b"\0" + data
            ).hexdigest()
            check(blob_oid == git_source[name], "Archived bytes match exact Git blob: " + name)
            archived[name] = digest(data)
        lock = tomllib.loads(archive.extractfile("Cargo.lock").read().decode())
    source_hash = file_digest(RUN / "source.tar")
    canonical = digest(json.dumps(archived, sort_keys=True, separators=(",", ":")).encode())
    check(archived == source["files"] == freeze["files"] and source["file_count"] == len(archived),
          "All archived SHA-256 values equal source and combined manifests")
    check(source_hash == source["archive_sha256"] == receipt["source"]["archive_sha256"]
          and canonical == source["manifest_sha256"] == receipt["source"]["manifest_sha256"],
          "Source archive and canonical source-map SHA-256 match both receipt and manifest")
    check(receipt["source"] == {k: v for k, v in source.items() if k != "files"},
          "Receipt source metadata equals the source manifest")
    current_source_drift_start = []
    for name, expected in freeze["files"].items():
        path = ROOT / name
        observed_hash = file_digest(path) if path.is_file() else None
        if observed_hash != expected:
            current_source_drift_start.append({"path": name, "frozen_sha256": expected,
                                               "observed_sha256": observed_hash})
    for name, expected in freeze["scripts"].items():
        data = (ROOT / name).read_bytes()
        check(digest(data) == expected, "Current scoped script matches combined freeze: " + name)
        oid = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        check(oid == git_scripts[name], "Combined script matches immutable Git blob: " + name)
    host_manifest = load(RUN / "host-runner-manifest.json")
    check(host_manifest == receipt["host_runner_inputs"],
          "Receipt and frozen host-runner manifest agree")
    for name, expected in host_manifest.items():
        check(file_digest(RUN / "host-runner-inputs" / Path(name).name) == expected
              == r2_summary["inputs"][name] == freeze["scripts"][name],
              "Executed frozen host input matches approved r2 and final combined freeze: " + name)
    check(host_manifest["scripts/run_linux_local_backend.py"] == RUNNER_SHA
          and all(freeze["scripts"][k] == v for k, v in runner_freeze["files"].items()),
          "Final run uses the exact r2-approved runner and Python test source hashes")

    # Read the public payload in place; retain only derived checks, never vendor bytes.
    registry = load(RUN / "registry-manifest.json")
    packages = {(p["name"], p["version"]): p for p in registry["packages"]}
    locked = {(p["name"], p["version"]): p for p in lock["package"] if "source" in p}
    check(len(packages) == len(registry["packages"]) == len(locked)
          and packages.keys() == locked.keys(),
          "Registry manifest has exactly every sourced Cargo.lock package without duplicates")
    allowed = {}
    missing = []
    for key, package in locked.items():
        item = packages[key]
        check(package["source"] == "registry+https://github.com/rust-lang/crates.io-index"
              and item["lock_sha256"] == package["checksum"],
              "Registry package binds the official lock checksum: " + "/".join(key))
        name = "-".join(key) + ".crate"
        if item["cached"]:
            check(item["sha256"] == package["checksum"],
                  "Cached package manifest has exact lock checksum: " + name)
            allowed["registry/cache/index.crates.io-1949cf8c6b5b557f/" + name] = (
                package["checksum"], item["bytes"],
            )
        else:
            missing.append(name)
    check(sorted(missing) == sorted(registry["missing_archives"]),
          "The 29 absent public archives are explicitly and accurately recorded")
    expected_index = {index_path(p["name"]) for p in locked.values()}
    indices = {item["path"]: item["sha256"] for item in registry["index_entries"]}
    check(len(indices) == len(registry["index_entries"])
          and set(indices).isdisjoint(registry["missing_index"])
          and set(indices) | set(registry["missing_index"]) == expected_index,
          "Sparse-index manifest is exactly the public lock-name whitelist")
    for name, checksum in indices.items():
        allowed["registry/index/index.crates.io-1949cf8c6b5b557f/.cache/" + name] = (checksum, None)
    config_name = "registry/index/index.crates.io-1949cf8c6b5b557f/config.json"
    config = b'{"dl":"https://static.crates.io/crates","api":"https://crates.io"}\n'
    allowed[config_name] = (digest(config), len(config))
    registry_seen = set()
    with (RUN / "registry.tar").open("rb") as stream:
        registry_hash = hashlib.file_digest(stream, "sha256").hexdigest()
        stream.seek(0)
        with tarfile.open(fileobj=stream, mode="r:") as archive:
            for member in archive:
                check(member.isfile() and member.name in allowed and member.name not in registry_seen,
                      "Unique ordinary registry payload is publicly allowlisted: " + member.name)
                expected_hash, expected_size = allowed[member.name]
                data = archive.extractfile(member).read()
                check(digest(data) == expected_hash
                      and (expected_size is None or len(data) == expected_size),
                      "Retained public payload checksum and size match: " + member.name)
                registry_seen.add(member.name)
    check(registry_seen == set(allowed)
          and registry_hash == registry["bundle_sha256"] == receipt["registry"]["bundle_sha256"],
          "Public payload is complete for its manifest and has the recorded bundle SHA-256")

    steps = receipt["steps"]
    named = {}
    for step in steps:
        named.setdefault(step["name"], []).append(step)
        check((RUN / step["stdout_log"]).is_file() and (RUN / step["stderr_log"]).is_file()
              and step["owned_processes_stopped"] is True
              and not step["timed_out"] and not step["interrupted"]
              and step["timeout_seconds"] > 0,
              "Step has both logs, a budget and stopped owned processes: " + step["name"])
        check(step["returncode"] == (1 if step["name"] == "owned-container-absent" else 0),
              "Step has the expected actual terminal exit code: " + step["name"])
    check(len(steps) == 46 and 0 < receipt["elapsed_seconds"]
          and receipt["elapsed_seconds"] < receipt["budget_seconds"] == 900,
          "All 46 recorded steps finished within the non-renewable 900-second budget")
    one = lambda name: named[name][0]
    check(raw(one("resolve-revision")).strip() == REVISION,
          "Original revision-resolution stdout is the exact immutable commit")
    check(one("source-git-archive")["argv"][-4:] == [REVISION, "Cargo.toml", "Cargo.lock", "crates"],
          "Recorded archive command selects only the exact commit and source paths")
    vm = raw(one("podman-vm-architecture")).strip()
    tools = raw(one("linux-toolchain"))
    check(vm == "arm64" and re.search(r"^Linux aarch64$", tools, re.M)
          and re.search(r"^host: aarch64-unknown-linux-gnu$", tools, re.M)
          and re.search(r"^rustc 1\.98\.1 ", tools, re.M)
          and re.search(r"^cargo 1\.98\.1 ", tools, re.M)
          and receipt["toolchain"] == {
              "rustc": "1.98.1", "host": "aarch64-unknown-linux-gnu",
              "kernel_arch": "aarch64", "vm_arch": "arm64",
          },
          "Raw VM, kernel, compiler and Cargo evidence consistently shows native Linux arm64")
    check(raw(one("test-user")).startswith("uid=1000 gid=1000"),
          "Actual test user is the private non-root UID/GID 1000")

    artifacts = load(RUN / "artifacts.json")
    compiled = {}
    for package in artifacts:
        step = one("compile-" + package)
        command = step["argv"]
        check(all(x in command for x in [
            "--user=1000:1000", "--workdir=/snapshot", "--env=CARGO_HOME=/linux-cargo",
            "--env=CARGO_TARGET_DIR=/linux-target", "--env=CARGO_NET_OFFLINE=true",
            "--locked", "--offline", "--no-run", "--message-format=json",
        ]) and "--ignore-rust-version" not in command,
              "Actual build command uses owned Linux directories, locked offline Cargo and no MSRV bypass")
        result = {}
        for line in raw(step).splitlines():
            if not line.startswith("{"):
                continue
            row = json.loads(line)
            if (row.get("reason") != "compiler-artifact"
                    or not row.get("profile", {}).get("test") or not row.get("executable")):
                continue
            if row["package_id"] != f"path+file:///snapshot/crates/{package}#0.1.0":
                continue
            target = row["target"]["name"]
            check(target not in result, "Compiler emitted one executable per selected target: " + package + "/" + target)
            result[target] = row["executable"]
        check(result == artifacts[package],
              "Compiler JSON exactly matches retained artifact map: " + package)
        compiled[package] = result

    runner_ast = ast.parse((RUN / "host-runner-inputs/run_linux_local_backend.py").read_text())
    specs = next(ast.literal_eval(node.value) for node in runner_ast.body
                 if isinstance(node, ast.Assign)
                 and any(isinstance(x, ast.Name) and x.id == "SPECS" for x in node.targets))
    binary_summary = []
    inventories = {}
    for package, targets in artifacts.items():
        for target, executable in targets.items():
            label = package + "-" + target
            binary = load(RUN / (label + "-binary.json"))
            names = [line[:-6] for line in raw(one("inventory-" + label)).splitlines()
                     if line.endswith(": test")]
            check(names and len(names) == len(set(names)) and names == binary["inventory"],
                  "Raw executable inventory exactly matches the retained named inventory: " + label)
            inventories[package, target] = names
            integrity = raw(one("binary-sha-" + label))
            elf = raw(one("binary-elf-" + label))
            match = re.fullmatch(r"([0-9a-f]{64})  " + re.escape(executable) + r"\n", integrity)
            fields = {}
            for field in ["Class", "Data", "Type", "Machine"]:
                values = re.findall(r"^\s*" + field + r":\s+(.+)$", elf, re.M)
                check(len(values) == 1, "Unique ELF header field " + field + ": " + label)
                fields[field] = values[0].strip()
            check(match is not None and fields["Class"] == "ELF64"
                  and fields["Data"] == "2's complement, little endian"
                  and fields["Machine"] == "AArch64"
                  and fields["Type"].split()[0] in ("EXEC", "DYN"),
                  "Raw executable hash target and ELF architecture agree with native Linux arm64: " + label)
            check(binary["revision"] == REVISION and binary["executable"] == executable
                  and binary["sha256sum"] == integrity and binary["elf_header"] == elf
                  and binary["verified_native_binary"] == {
                      "sha256": match[1], "elf_class": "ELF64", "machine": "AArch64",
                  },
                  "Binary receipt exactly binds revision, artifact path, hash and verified ELF: " + label)
            for prefix in ["inventory-", "binary-sha-", "binary-elf-"]:
                check(executable in one(prefix + label)["argv"],
                      "Inventory/hash/ELF command addresses the actual compiled executable: " + prefix + label)
            binary_summary.append({
                "package": package, "target": target, "executable": executable,
                "sha256": match[1], "machine": "AArch64", "inventory_count": len(names),
            })

    suites = load(RUN / "test-results.json")
    check(suites == receipt["suites"] and len(suites) == len(specs) == 7,
          "All seven retained suite results exactly match the final receipt and approved specifications")
    suite_summary = []
    qualified = set()
    for spec, suite in zip(specs, suites):
        package, target, prefix, required = spec
        selected = [name for name in inventories[package, target] if not prefix or name.startswith(prefix)]
        check((suite["package"], suite["target"], suite["filter"]) == (package, target, prefix)
              and selected == suite["selected"] and set(required).issubset(selected),
              "Suite selection preserves every exact inventory name and required test: " + suite["step"])
        step = one(suite["step"])
        executable = artifacts[package][target]
        command = step["argv"]
        check(executable in command and "--user=1000:1000" in command
              and command[command.index(executable) + 1:] == [
                  *([prefix] if prefix else []),
                  "--format=pretty", "--color=never", "--test-threads=2",
              ],
              "Actual test invocation uses the same compiled executable and exact selector: " + suite["step"])
        results = {}
        output = raw(step)
        for line in output.splitlines():
            match = re.fullmatch(r"test (.+) \.\.\. (ok|FAILED|ignored(?:, .*)?)", line)
            if match:
                check(match[1] not in results, "Named test result occurs only once: " + match[1])
                results[match[1]] = match[2]
        totals = re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", output, re.M)
        check(results == suite["results"] and set(results) == set(selected)
              and all(value == "ok" for value in results.values())
              and len(totals) == 1 and tuple(map(int, totals[0])) == (len(selected), 0, 0)
              and (suite["passed"], suite["failed"], suite["ignored"]) == (len(selected), 0, 0),
              "Every selected test actually reports ok and agrees with the raw summary: " + suite["step"])
        for name in selected:
            key = (package, target, name)
            check(key not in qualified, "No selected test is counted twice: " + "/".join(key))
            qualified.add(key)
        suite_summary.append({
            "package": package, "target": target, "filter": prefix,
            "passed": len(selected), "failed": 0, "ignored": 0, "step": suite["step"],
        })
    check(receipt["totals"] == {"passed": len(qualified), "failed": 0, "ignored": 0},
          "The exact nonduplicated named result set matches its passed total and has no failed/ignored tests")
    observed = {}
    for line in raw(one("source-after-run")).splitlines():
        checksum, name = line.split("  ", 1)
        check(name not in observed, "Post-run source hash is unique: " + name)
        observed[name] = checksum
    check(observed == archived and receipt["source_after_run_matches"] is True
          and receipt["host_runner_after_run_matches"] is True,
          "All source hashes after actual Linux execution still equal the immutable archive")

    container = receipt["container_id"]
    owner = receipt["owner"]
    check(re.fullmatch(r"[0-9a-f]{64}", container) and re.fullmatch(r"[0-9a-f]{32}", owner),
          "Resource identity uses a full container ID and unique owner token")
    for step in named["container-identity"]:
        value = json.loads(raw(step))
        check(value["Id"] == container
              and value["Name"].lstrip("/") == "keelshell-linux-local-" + owner[:12]
              and value["Labels"]["io.keelshell.fixture"] == "linux-local-backend"
              and value["Labels"]["io.keelshell.owner"] == owner,
              "Original identity readback confirms full ID, unique name and both ownership labels")
    create = one("create-container")["argv"]
    check("--network=none" in create and "--pull=never" in create
          and "--http-proxy=false" in create
          and not any(x.startswith(("--volume", "--mount", "--privileged")) for x in create),
          "Recorded container creation has no network, pull, proxy inheritance or host mount")
    check(one("remove-owned-container")["argv"] == ["podman", "rm", "-f", container]
          and one("owned-container-absent")["argv"] == ["podman", "container", "exists", container]
          and not raw(one("owned-container-absent")) and not raw(one("owned-container-absent"), True)
          and steps[-3]["name"] == "container-identity",
          "Cleanup rechecks ownership immediately before deleting the full ID and reads unambiguous absence")
    check(cleanup == receipt["cleanup"] == {
        "container_removed": True, "owned_processes_stopped": True, "container_created": True,
        "container_id": container, "container_filesystem_and_target_removed": True,
    }, "Both terminal receipts agree that the container, owned processes and private target were removed")
    live_absence = observe(["podman", "container", "exists", container])
    check(live_absence.returncode == 1 and not live_absence.stdout and not live_absence.stderr,
          "Independent read-only full-ID query also confirms the container is absent")
    pids = sorted({step["pid"] for step in steps})
    process_result = observe([
        "ps", "-p", ",".join(map(str, pids)), "-o", "pid=,ppid=,state=,command=",
    ])
    check(process_result.returncode == 1 and not process_result.stdout and not process_result.stderr,
          "All recorded 46 command PIDs are absent at independent readback")
    check(receipt["host_cargo_used"] is False and receipt["host_target_accessed"] is False
          and receipt["msrv_bypass"] is False and receipt["network"] == "none",
          "Recorded execution boundary remains container-only, offline and without host Cargo/target access")

    old = load(BASE / "linux-local/native-r3/receipt.json")
    previous_names = {
        (suite["package"], suite["target"], name)
        for suite in old["suites"] for name in suite["selected"]
    }
    new_names = sorted(qualified - previous_names)
    removed_names = sorted(previous_names - qualified)
    check(inventory_manifest(REVIEW, exclude_r3=True) == historical,
          "Every r1/r2 review original retains exact paths and bytes after r3")
    check(inventory_manifest(RUN, exclude_registry=True) == retained_start,
          "All non-payload final-run originals retain exact paths and bytes during r3")
    current_source_drift_end = []
    for name, expected in freeze["files"].items():
        path = ROOT / name
        observed_hash = file_digest(path) if path.is_file() else None
        if observed_hash != expected:
            current_source_drift_end.append({"path": name, "frozen_sha256": expected,
                                             "observed_sha256": observed_hash})
    for name, expected in freeze["scripts"].items():
        check(file_digest(ROOT / name) == expected,
              "Final scoped script readback has no combined-freeze drift: " + name)
    result = {
        "recommendation": "APPROVE",
        "scope": "Read-only verification of retained linux-final-r2 evidence; no test rerun",
        "signed_utc": datetime.now(timezone.utc).isoformat(),
        "open_findings": {"P0": 0, "P1": 0, "P2": 0, "P3": 0},
        "validation_revision": REVISION, "validation_tree": snapshot["tree"],
        "original_baseline": freeze["baseline"],
        "combined_freeze_sha256": file_digest(FREEZE_PATH),
        "receipt_sha256": file_digest(RUN / "receipt.json"),
        "cleanup_sha256": file_digest(RUN / "cleanup.json"),
        "runner_sha256": RUNNER_SHA,
        "python_test_source_sha256": runner_freeze["files"]["scripts/test_linux_local_backend.py"],
        "source": {
            "file_count": len(archived), "script_count": len(git_scripts),
            "archive_sha256": source_hash, "canonical_manifest_sha256": canonical,
            "git_blob_matches": len(archived), "source_after_run_matches": True,
            "current_source_drift_start": current_source_drift_start,
            "current_source_drift_end": current_source_drift_end,
        },
        "registry": {
            "audited_in_place": True, "vendor_payload_copied": False,
            "bundle_sha256": registry_hash, "locked_packages": len(locked),
            "verified_cached_crates": len(locked) - len(missing),
            "verified_sparse_entries": len(indices), "missing_archives": len(missing),
            "missing_index": len(registry["missing_index"]), "allowlisted_payload_files": len(allowed),
            "payload_retention": "Parent may remove registry.tar; retain registry-manifest.json and this audit",
        },
        "suites": suite_summary, "binaries": binary_summary,
        "totals": receipt["totals"], "recorded_steps": len(steps),
        "elapsed_seconds": receipt["elapsed_seconds"], "budget_seconds": receipt["budget_seconds"],
        "toolchain": receipt["toolchain"], "container_id": container, "owner": owner,
        "cleanup": cleanup, "independent_live_container_absent": True,
        "recorded_command_pids_absent": pids,
        "parent_outer_result": {
            "exit": 0, "reaped": True,
            "provenance": "Explicit parent/user final-r2 completion message, 252/0/0 and 59.797s; reviewer did not wait the parent outer process",
        },
        "historical_review_files_preserved": len(historical),
        "original_r1_files_preserved": len(r1_manifest),
        "final_native_nonpayload_files_preserved": len(retained_start),
        "added_selected_names_vs_old_251": new_names, "removed_selected_names_vs_old_251": removed_names,
        "old_251_not_relabelled": True, "prior_r2_47_python_not_repeated_or_added": True,
        "reviewer_tests_executed_this_round": {"Rust": 0, "Python": 0},
        "read_only_subprocesses": COMMANDS,
        "audit_assertions_passed": len(CHECKS),
        "write_scope": str(OUT),
        "limitations": [
            "Retained parent native execution evidence, not an independent rebuild or test execution",
            "Raw binary bytes were removed with the owned container; hash and ELF checked against retained original logs",
            "Only selected native Linux arm64 filesystem/codec tests; no desktop, Windows, x64 or peer acceptance",
            "Immutable validation commit is not represented as a formal feature commit",
            "Current mutable source drift is recorded separately and never relabels the immutable snapshot",
        ],
    }
    save("audit-r3.json", result)
    save("checks-r3.json", {"passed": len(CHECKS), "checks": CHECKS})
    print(json.dumps({
        key: result[key] for key in [
            "recommendation", "validation_revision", "totals", "recorded_steps",
            "audit_assertions_passed", "historical_review_files_preserved",
            "added_selected_names_vs_old_251", "registry",
        ]
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        save("audit-incomplete-r3.json", {
            "status": "INCOMPLETE", "error": type(error).__name__ + ": " + str(error),
            "passed_assertions": len(CHECKS), "read_only_subprocesses": COMMANDS,
        })
        raise
