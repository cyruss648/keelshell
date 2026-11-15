#!/usr/bin/env python3
"""Independently bind retained Linux evidence to Git blobs without rebuilding."""

import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import time
import tomllib

REVIEW = Path(__file__).resolve().parent
ROOT = REVIEW.parents[2]
DELIVERY = ROOT / "work/windows-local-backend-20261115/linux-local"
REVISION = "a092dd5712c0a92ee4af18970fb0500fb230b760"
OFFICIAL = "registry+https://github.com/rust-lang/crates.io-index"
CHECKS = []
GIT_COMMANDS = []


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(relative):
    return json.loads((DELIVERY / relative).read_text())


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def checked(name, detail):
    CHECKS.append({"name": name, "status": "PASS", "detail": detail})


def git(arguments, data=None):
    argv = ["git", "--no-optional-locks", *arguments]
    started = time.monotonic()
    result = subprocess.run(
        argv, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        cwd=ROOT, env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
        timeout=20, check=True,
    )
    GIT_COMMANDS.append({
        "argv": argv, "returncode": result.returncode,
        "stdout_bytes": len(result.stdout), "stdout_sha256": digest(result.stdout),
        "stderr_bytes": len(result.stderr),
        "elapsed_seconds": round(time.monotonic() - started, 3),
    })
    return result.stdout


def step_text(run, step):
    return (DELIVERY / run / step["stdout_log"]).read_text()


def named_inventory(text):
    names = [line[:-6] for line in text.splitlines() if line.endswith(": test")]
    require(names and len(names) == len(set(names)), "Invalid raw test inventory")
    return names


def main():
    handoff = load("handoff.json")
    initial = json.loads((REVIEW / "input-binding-start.json").read_text())
    for item in initial["inputs"]:
        require(digest((ROOT / item["path"]).read_bytes()) == item["sha256"],
                "Reviewed input changed: " + item["path"])
    for path, checksum in handoff["host_inputs"].items():
        require(digest((ROOT / path).read_bytes()) == checksum, "Handoff script mismatch")
        if path.endswith("test_linux_local_backend.py"):
            frozen = DELIVERY / "python-r2" / Path(path).name
        else:
            frozen = DELIVERY / "native-r3/host-runner-inputs" / Path(path).name
        require(digest(frozen.read_bytes()) == checksum, "Frozen script mismatch")
    checked("host-inputs", {"sha256": handoff["host_inputs"], "drift": False})

    evidence = load("evidence-manifest.json")
    evidence_bytes = 0
    for path, recorded in evidence["files"].items():
        member = DELIVERY / path
        require(member.is_file() and not member.is_symlink(), "Missing evidence: " + path)
        data = member.read_bytes()
        require(digest(data) == recorded["sha256"] and len(data) == recorded["bytes"],
                "Evidence bytes mismatch: " + path)
        evidence_bytes += len(data)
    require(len(evidence["files"]) == evidence["file_count"], "Evidence count mismatch")
    require(evidence_bytes == evidence["total_bytes"], "Evidence byte count mismatch")
    checked("evidence-manifest", {
        "files": evidence["file_count"], "bytes": evidence_bytes,
        "manifest_sha256": digest((DELIVERY / "evidence-manifest.json").read_bytes()),
    })

    raw_gzip = (DELIVERY / handoff["compressed_source"]).read_bytes()
    require(digest(raw_gzip) == handoff["compressed_source_sha256"], "Gzip SHA mismatch")
    source = gzip.decompress(raw_gzip)
    require(digest(source) == handoff["source"]["archive_sha256"], "Source tar SHA mismatch")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(source)) as archive:
        require(archive.pax_headers.get("comment") == REVISION, "Archive commit mismatch")
        for member in archive:
            if member.isdir():
                continue
            require(member.isfile() and member.name not in files, "Nonordinary/duplicate source")
            files[member.name] = archive.extractfile(member).read()
    manifest = load("native-r3/source-manifest.json")
    source_hashes = {path: digest(data) for path, data in files.items()}
    require(source_hashes == manifest["files"], "Archived source manifest mismatch")
    encoded = json.dumps(source_hashes, sort_keys=True, separators=(",", ":")).encode()
    require(digest(encoded) == manifest["manifest_sha256"], "Manifest digest mismatch")
    require(len(files) == handoff["source"]["file_count"] == 573, "Source count mismatch")

    head = git(["rev-parse", "HEAD", REVISION + "^{commit}"]).decode().splitlines()
    require(head[1] == REVISION, "Baseline commit changed")
    raw_tree = git(["ls-tree", "-rz", "--full-tree", REVISION, "--",
                    "Cargo.toml", "Cargo.lock", "crates"])
    tree = {}
    for item in raw_tree.split(b"\0"):
        if not item:
            continue
        metadata, path = item.split(b"\t", 1)
        mode, kind, object_id = metadata.decode().split()
        require(kind == "blob" and mode in ("100644", "100755"), "Unexpected Git member")
        tree[path.decode()] = object_id
    require(set(tree) == set(files), "Exact Git tree path set differs from archive")
    for path, object_id in tree.items():
        data = files[path]
        header = b"blob " + str(len(data)).encode() + b"\0"
        require(hashlib.sha1(header + data).hexdigest() == object_id, "Git blob mismatch: " + path)
    batch_input = "".join(object_id + "\n" for object_id in tree.values()).encode()
    batch = io.BytesIO(git(["cat-file", "--batch"], batch_input))
    for path, object_id in tree.items():
        observed_id, kind, size = batch.readline().decode().split()
        data = batch.read(int(size))
        require(batch.read(1) == b"\n", "Invalid cat-file response boundary")
        require(observed_id == object_id and kind == "blob" and data == files[path],
                "Git object content mismatch: " + path)
    require(not batch.read(), "Unexpected extra Git objects")
    binding = load("git-source-binding.json")
    require(not binding["mismatches"], "Author recorded source binding mismatch")
    for item in binding["files"]:
        require(tree[item["path"]] == item["git_blob"] and
                source_hashes[item["path"]] == item["sha256"] and item["matches"],
                "Author binding entry differs from independent readback")
    checked("git-archive", {
        "baseline": REVISION, "current_head_observed": head[0], "files": len(files),
        "archive_sha256": digest(source), "manifest_sha256": digest(encoded),
        "compressed_sha256": digest(raw_gzip), "exact_blob_matches": len(tree),
        "worktree_source_read_or_copied": False,
    })

    lock = tomllib.loads(files["Cargo.lock"].decode())
    locked = {(p["name"], p["version"]): p for p in lock["package"] if "source" in p}
    require(all(p["source"] == OFFICIAL for p in locked.values()), "Nonpublic lock source")
    registry = load("native-r3/registry-manifest.json")
    packages = {(p["name"], p["version"]): p for p in registry["packages"]}
    require(set(packages) == set(locked), "Registry lock package set mismatch")
    cached = 0
    index_paths = set()
    missing = set()
    for key, package in locked.items():
        record = packages[key]
        require(record["lock_sha256"] == package["checksum"], "Registry lock checksum mismatch")
        if record["cached"]:
            cached += 1
            require(record["sha256"] == package["checksum"] and record["bytes"] > 0,
                    "Cached crate checksum mismatch")
        else:
            missing.add(package["name"] + "-" + package["version"] + ".crate")
        name = package["name"].lower()
        if len(name) <= 2:
            index_paths.add(f"{len(name)}/{name}")
        elif len(name) == 3:
            index_paths.add(f"3/{name[0]}/{name}")
        else:
            index_paths.add(f"{name[:2]}/{name[2:4]}/{name}")
    require(missing == set(registry["missing_archives"]), "Missing crate set mismatch")
    require(not registry["missing_index"], "Unexpected missing sparse index")
    require(index_paths == {p["path"] for p in registry["index_entries"]},
            "Sparse entry whitelist mismatch")
    checked("registry-receipts", {
        "official_locked": len(locked), "cached_lock_checksums": cached,
        "missing_archives": len(missing), "unique_index_paths": len(index_paths),
        "bundle_sha256": registry["bundle_sha256"],
        "transport_tar_retained": False,
        "cached_payloads_reopened_by_reviewer": False,
    })

    receipt = load("native-r3/receipt.json")
    steps = receipt["steps"]
    require(len(steps) == 46, "Unexpected native step count")
    for step in steps:
        require(step["owned_processes_stopped"] and not step["timed_out"] and
                not step["interrupted"] and step["returncode"] is not None,
                "Unsuccessful wait/reap in native receipt")
        require(step["returncode"] == (1 if step["name"] == "owned-container-absent" else 0),
                "Unexpected native exit code")
        require(step["timeout_seconds"] > 0 and
                step["elapsed_seconds"] < step["timeout_seconds"] + 8,
                "Missing/exceeded bounded native command")
        require((DELIVERY / "native-r3" / step["stdout_log"]).is_file() and
                (DELIVERY / "native-r3" / step["stderr_log"]).is_file(), "Missing raw native log")
    by_name = {s["name"]: s for s in steps}
    owner, container = receipt["owner"], receipt["container_id"]
    for step in (s for s in steps if s["name"] == "container-identity"):
        value = json.loads(step_text("native-r3", step))
        require(value["Id"] == container and
                value["Name"].lstrip("/") == "keelshell-linux-local-" + owner[:12] and
                value["Labels"]["io.keelshell.fixture"] == "linux-local-backend" and
                value["Labels"]["io.keelshell.owner"] == owner, "Container identity mismatch")
    create = by_name["create-container"]["argv"]
    require(all(flag in create for flag in (
        "--pull=never", "--http-proxy=false", "--network=none"
    )), "Container isolation flags absent")
    require(not any(flag in create for flag in ("-v", "--volume", "--mount")),
            "Unexpected host mount")
    require(by_name["remove-owned-container"]["argv"] == ["podman", "rm", "-f", container],
            "Cleanup target identity mismatch")
    require(steps[-1]["name"] == "owned-container-absent", "Missing cleanup readback")
    tools = step_text("native-r3", by_name["linux-toolchain"])
    vm = step_text("native-r3", by_name["podman-vm-architecture"]).strip()
    require("Linux aarch64\n" in tools and "host: aarch64-unknown-linux-gnu\n" in tools and
            re.search(r"^rustc 1\.98\.1 ", tools, re.MULTILINE) and
            re.search(r"^cargo 1\.98\.1 ", tools, re.MULTILINE) and vm == "arm64",
            "Raw native toolchain mismatch")
    require(step_text("native-r3", by_name["test-user"]).startswith("uid=1000"),
            "Native tests did not use fixture UID")
    require(step_text("native-r3", by_name["container-image-id"]).strip() == handoff["image_id"],
            "Image ID mismatch")
    after = {}
    for line in step_text("native-r3", by_name["source-after-run"]).splitlines():
        checksum, path = line.split("  ", 1)
        after[path] = checksum
    require(after == source_hashes, "Raw after-run source hash mismatch")
    checked("native-runtime-and-cleanup", {
        "steps": len(steps), "all_waited_reaped": True, "toolchain": receipt["toolchain"],
        "container_id": container, "image_id": receipt["image_id"],
        "container_identity_readbacks": sum(s["name"] == "container-identity" for s in steps),
        "source_after_matches": len(after), "cleanup": receipt["cleanup"],
    })

    artifacts = {}
    for package in ("keelshell-core", "keelshell-session"):
        compile_step = by_name["compile-" + package]
        argv = compile_step["argv"]
        require("--locked" in argv and "--offline" in argv and "--no-run" in argv and
                "--user=1000:1000" in argv and "--env=CARGO_TARGET_DIR=/linux-target" in argv and
                "--ignore-rust-version" not in argv, "Unbound/nonoffline compile command")
        for line in step_text("native-r3", compile_step).splitlines():
            if not line.startswith("{"):
                continue
            value = json.loads(line)
            if value.get("reason") != "compiler-artifact" or not value.get("executable"):
                continue
            if value.get("profile", {}).get("test") and (
                value["package_id"].startswith("path+file:///snapshot/crates/" + package + "#")
            ):
                artifacts.setdefault(package, {})[value["target"]["name"]] = value["executable"]
    require(artifacts == load("native-r3/artifacts.json"), "Raw compiler artifact mismatch")
    binaries = {}
    for item in handoff["binaries"]:
        raw = (DELIVERY / item["path"]).read_bytes()
        value = json.loads(raw)
        require(value["sha256sum"].split()[0] == item["sha256"],
                "Handoff executable SHA mismatch")
        label = Path(item["path"]).name.removesuffix("-binary.json")
        require(value["revision"] == REVISION, "Binary receipt revision mismatch")
        require(value["inventory"] == named_inventory(
            step_text("native-r3", by_name["inventory-" + label])
        ) and len(value["inventory"]) == item["inventory_count"], "Raw named inventory mismatch")
        require(value["sha256sum"] == step_text(
            "native-r3", by_name["binary-sha-" + label]
        ), "Binary checksum log mismatch")
        require(value["elf_header"] == step_text(
            "native-r3", by_name["binary-elf-" + label]
        ) and "Machine:                           AArch64" in value["elf_header"],
                "Binary ELF log/machine mismatch")
        require(value["sha256sum"].strip().split()[1] == value["executable"],
                "Binary checksum path mismatch")
        require(value["executable"] in by_name["inventory-" + label]["argv"],
                "Inventory command executable mismatch")
        binaries[label] = value
    suites = load("native-r3/test-results.json")
    require(suites == receipt["suites"], "Derived/native suite mismatch")
    total, identities, counts = 0, set(), []
    for suite in suites:
        label = suite["package"] + "-" + suite["target"]
        binary = binaries[label]
        require(artifacts[suite["package"]][suite["target"]] == binary["executable"],
                "Built/inventoried binary mismatch")
        selected = [test for test in binary["inventory"] if test.startswith(suite["filter"])]
        require(selected == suite["selected"], "Selected inventory mismatch")
        step = by_name[suite["step"]]
        require(binary["executable"] in step["argv"] and
                "--user=1000:1000" in step["argv"], "Executed binary/UID mismatch")
        if suite["filter"]:
            require(suite["filter"] in step["argv"], "Test filter command mismatch")
        results = {}
        for line in step_text("native-r3", step).splitlines():
            match = re.fullmatch(r"test (.+) \.\.\. (ok|FAILED|ignored(?:, .*)?)", line)
            if match:
                require(match[1] not in results, "Duplicate raw test result")
                results[match[1]] = match[2]
        require(set(results) == set(selected) and results == suite["results"] and
                all(value == "ok" for value in results.values()), "Named execution mismatch")
        require(suite["passed"] == len(results) and not suite["failed"] and not suite["ignored"],
                "Native suite totals mismatch")
        for name in results:
            key = (suite["package"], suite["target"], name)
            require(key not in identities, "Duplicate native coverage")
            identities.add(key)
        total += len(results)
        counts.append({"package": suite["package"], "target": suite["target"],
                       "filter": suite["filter"], "passed": len(results)})
    require(total == 251 and receipt["totals"] == handoff["rust_tests"] and
            receipt["status"] == "PASS", "Final native result mismatch")
    checked("native-named-execution", {
        "unique_tests": len(identities), "binaries": len(binaries),
        "suites": counts, "elapsed_seconds": receipt["elapsed_seconds"],
        "live_binary_bytes_retained": False,
    })

    python_receipt = load("python-r2/receipt.json")
    final = python_receipt["steps"][-1]
    output = (DELIVERY / "python-r2" / final["stderr_log"]).read_text()
    require(final["returncode"] == 0 and final["owned_processes_stopped"] and
            "Ran 32 tests" in output and "\nOK\n" in output, "Author Python run mismatch")
    python_names = load("python-r2/inventory.json")
    named = load("python-r2/named-results.json")
    require(len(python_names) == 32 and set(python_names) == set(named["results"]) and
            all(value == "ok" for value in named["results"].values()), "Author Python names mismatch")
    red = python_receipt["steps"][0]
    require(red["returncode"] == 1 and red["owned_processes_stopped"] and
            "FAILED (errors=2)" in (DELIVERY / "python-r2" / red["stderr_log"]).read_text(),
            "Historical red error receipt regression missing")
    preflight = load("preflight-r1/receipt.json")
    failures = [s for s in preflight["steps"] if s["returncode"] == 125]
    require(len(failures) == 2 and all(
        "<.Id>" in (DELIVERY / "preflight-r1" / s["stderr_log"]).read_text()
        for s in failures
    ), "Historical inspect-template failures missing")
    cleanup = load("cleanup-evidence-final.json")
    final_audit = load("final-audit-r2/receipt.json")
    absent_steps = {s["name"]: s for s in final_audit["steps"] if s["name"].startswith("owned-absent-")}
    require(len(absent_steps) == len(cleanup["containers"]) == 4, "Cleanup identity count mismatch")
    for record in cleanup["containers"]:
        step = absent_steps["owned-absent-" + record["run"]]
        require(step["argv"] == ["podman", "container", "exists", record["container_id"]] and
                step["returncode"] == 1 and step["owned_processes_stopped"] and not step["timed_out"],
                "Historical absence readback mismatch")
    for removed in cleanup["removed"]:
        require(not (DELIVERY / removed["path"]).exists(), "Removed duplicate still present")
    checked("history-and-cleanup-receipts", {
        "author_python_passed": len(python_names), "author_red_errors_retained": 2,
        "preflight_inspect_failures_retained": len(failures),
        "owned_container_absence_receipts": len(absent_steps),
        "reclaimed_bytes": cleanup["reclaimed_bytes"],
        "podman_commands_executed_by_reviewer": 0,
    })
    return {"status": "PASS", "checks": CHECKS, "git_commands": GIT_COMMANDS}


if __name__ == "__main__":
    try:
        result = main()
    except Exception as error:
        result = {"status": "FAIL", "error": type(error).__name__ + ": " + str(error),
                  "checks": CHECKS, "git_commands": GIT_COMMANDS}
        (REVIEW / "delivery-audit.json").write_text(json.dumps(result, indent=2) + "\n")
        raise
    (REVIEW / "delivery-audit.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
