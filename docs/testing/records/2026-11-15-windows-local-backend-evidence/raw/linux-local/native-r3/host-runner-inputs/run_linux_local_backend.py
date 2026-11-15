#!/usr/bin/env python3
"""Run selected Rust filesystem tests in an owned, offline Linux container.

The source is a fresh ``git archive`` of an exact commit, never a copy of a
mutable checkout. No host Cargo, target directory, credential store, Cargo
configuration, or registry authentication file is accessed. Only public
crates.io sparse-index entries and lock-checksummed crate archives are copied
from an explicitly supplied registry cache. A cache populated by a public
mirror is acceptable: each archive must match the official-registry lock entry.

Example (the output directory must not exist)::

    python3 scripts/run_linux_local_backend.py --revision <40-character-SHA> \
        --registry-cache ~/.cargo/registry/cache/rsproxy.cn-e3de039b2554c837 \
        --registry-index ~/.cargo/registry/index/rsproxy.cn-e3de039b2554c837 \
        --output work/linux-local/run-01

The cached image must already exist. There is no image pull, network access,
package installation, dependency download, or MSRV bypass. Compilation and
tests run as UID 1000 with a fresh container-only Linux target and Cargo home.
The existing fixture's kernel-birth-checked process owner handles host children.
Container cleanup requires its full immutable ID, unique name and both labels.
Images, volumes, the Podman machine, and other containers are never removed.

Receipts retain commands, budgets, stdout/stderr, source and cache digests,
compiler artifacts, named test inventories/results, and cleanup readback.
Successful local backend tests do not certify a desktop, an independent
protocol peer, another architecture, or the Windows backend.
"""

import argparse
import hashlib
import io
import json
import math
from pathlib import Path, PurePosixPath
import re
import signal
import subprocess
import sys
import tarfile
import time
import tomllib
import uuid

sys.dont_write_bytecode = True
import run_linux_file_interop as fixture

ROOT = Path(__file__).resolve().parents[1]
LABEL = "io.keelshell.fixture"
LABEL_VALUE = "linux-local-backend"
OWNER_LABEL = "io.keelshell.owner"
PREFIX = "keelshell-linux-local-"
OFFICIAL_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
INDEX_DIRECTORY = "index.crates.io-1949cf8c6b5b557f"
SOURCE_PATHS = ("Cargo.toml", "Cargo.lock", "crates")
SPECS = (
    ("keelshell-session", "keelshell_session", "zmodem::", (
        "zmodem::local::tests::zmodem_local_publication_verifies_binary_content_identity_private_acl_and_new_target",
        "zmodem::local::tests::zmodem_local_no_replace_primitive_preserves_existing_files_directories_and_links",
    )),
    ("keelshell-session", "keelshell_session", "sftp::archive_transfer::local::tests::", (
        "sftp::archive_transfer::local::tests::local_archive_publication_primitive_never_replaces_files_or_empty_directories",
    )),
    ("keelshell-session", "zmodem", "", (
        "zmodem_controller_new_local_file_is_acknowledged_only_after_complete_no_replace_readback",
    )),
    ("keelshell-session", "archive_transfer", "", (
        "reviewed_archive_binary_multiselect_roundtrip_uses_private_exclusive_staging_and_standard_rename",
    )),
    ("keelshell-core", "zmodem", "", (
        "receiver_actual_oo_preserves_earlier_ordinary_bytes_and_suffix_order",
    )),
    ("keelshell-core", "archive", "", (
        "both_formats_roundtrip_binary_unicode_and_explicit_empty_directories",
    )),
    ("keelshell-core", "archive_pax", "", (
        "pax_rejects_full_width_base256_sizes_in_every_admitted_header_kind",
    )),
)


class BackendFailure(fixture.FixtureFailure):
    """A reproducibility, budget, build, test, or owned-resource check failed."""


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    fixture.write_json(path, value)


def revision(value):
    """Require an exact Git object ID, not a moving branch or option."""
    if not re.fullmatch(r"[0-9a-f]{40}", value):
        raise argparse.ArgumentTypeError("revision must be an exact lowercase 40-character SHA")
    return value


def index_key(name):
    """Return Cargo's public sparse-index path without permitting traversal."""
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name):
        raise BackendFailure("Invalid registry package name")
    name = name.lower()
    if len(name) <= 2:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def cache_file(root, relative):
    """Only inspect ordinary registry files beneath the declared cache root."""
    path = root / relative
    if path.is_symlink() or not path.is_file():
        return None
    if not path.resolve().is_relative_to(root.resolve()):
        raise BackendFailure("Registry cache path escaped its declared root")
    return path


def snapshot_manifest(archive_path):
    """Hash every archived file and reject unsafe/ambiguous archive members."""
    files = {}
    with tarfile.open(archive_path) as archive:
        for entry in archive:
            name = entry.name.rstrip("/")
            path = PurePosixPath(name)
            if (path.is_absolute() or ".." in path.parts or str(path) != name
                    or any(c in name for c in "\r\n\0\\")):
                raise BackendFailure("Unsafe source archive member")
            if entry.isdir():
                continue
            if not entry.isfile() or name in files:
                raise BackendFailure("Source snapshot must contain unique ordinary files")
            files[name] = sha256(archive.extractfile(entry).read())
    if not {"Cargo.toml", "Cargo.lock"}.issubset(files):
        raise BackendFailure("Source snapshot is missing workspace inputs")
    encoded = json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
    return {"files": files, "file_count": len(files), "manifest_sha256": sha256(encoded)}


def registry_bundle(lock, cache_root, index_root, destination):
    """Build a credential-free offline Cargo home using verified public crates."""
    result = {"packages": [], "index_entries": [], "missing_archives": [], "missing_index": []}
    copied_index = set()
    with tarfile.open(destination, "w") as archive:
        for package in lock.get("package", []):
            if "source" not in package:
                continue
            if package["source"] != OFFICIAL_SOURCE:
                raise BackendFailure("Only crates.io registry packages are supported")
            key = index_key(package["name"])
            if not re.fullmatch(r"[0-9A-Za-z.+_-]+", package["version"]):
                raise BackendFailure("Invalid registry package version")
            checksum = package.get("checksum", "")
            if not re.fullmatch(r"[0-9a-f]{64}", checksum):
                raise BackendFailure("Missing official lock checksum")
            name = package["name"] + "-" + package["version"] + ".crate"
            path = cache_file(cache_root, name)
            item = {"name": package["name"], "version": package["version"], "lock_sha256": checksum}
            if path is None:
                result["missing_archives"].append(name)
                item["cached"] = False
            else:
                data = path.read_bytes()
                observed = sha256(data)
                if observed != checksum:
                    raise BackendFailure("Cached crate checksum does not match Cargo.lock: " + name)
                # Do not copy Cargo home/configuration or registry credentials.
                entry = tarfile.TarInfo(f"registry/cache/{INDEX_DIRECTORY}/{name}")
                entry.size = len(data)
                entry.mode = 0o444
                archive.addfile(entry, io.BytesIO(data))
                item.update({"cached": True, "sha256": observed, "bytes": len(data)})
            result["packages"].append(item)
            if key in copied_index:
                continue
            copied_index.add(key)
            index = cache_file(index_root, ".cache/" + key)
            if index is None:
                result["missing_index"].append(key)
            else:
                data = index.read_bytes()
                entry = tarfile.TarInfo(f"registry/index/{INDEX_DIRECTORY}/.cache/{key}")
                entry.size = len(data)
                entry.mode = 0o444
                archive.addfile(entry, io.BytesIO(data))
                result["index_entries"].append({"path": key, "sha256": sha256(data)})
        # Cargo's source identity stays crates.io, regardless of cache origin.
        data = b'{"dl":"https://static.crates.io/crates","api":"https://crates.io"}\n'
        entry = tarfile.TarInfo(f"registry/index/{INDEX_DIRECTORY}/config.json")
        entry.size = len(data)
        archive.addfile(entry, io.BytesIO(data))
    result["bundle_sha256"] = sha256(destination.read_bytes())
    return result


def container_matches(info, container_id, owner):
    """Pin cleanup to a complete ID, a non-reusable owner token and both labels."""
    return (
        isinstance(info, dict)
        and re.fullmatch(r"[0-9a-f]{64}", container_id or "") is not None
        and re.fullmatch(r"[0-9a-f]{32}", owner or "") is not None
        and info.get("Id") == container_id
        and isinstance(info.get("Name"), str)
        and info.get("Name", "").lstrip("/") == PREFIX + owner[:12]
        and isinstance(info.get("Labels"), dict)
        and info["Labels"].get(LABEL) == LABEL_VALUE
        and info["Labels"].get(OWNER_LABEL) == owner
    )


def parse_artifacts(text, package):
    """Select executable test artifacts emitted by the actual Cargo build."""
    result = {}
    for line in text.splitlines():
        if not line.startswith("{"):
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("reason") != "compiler-artifact" or not value.get("profile", {}).get("test"):
            continue
        identity = value.get("package_id", "")
        if not re.search(r"(?:#|/)" + re.escape(package) + r"(?:@|#)[0-9][0-9A-Za-z.+_-]*$", identity):
            continue
        executable = value.get("executable")
        if executable:
            if not re.fullmatch(r"/linux-target/debug/deps/[A-Za-z0-9_-]+", executable):
                raise BackendFailure("Cargo test executable escaped the owned Linux target")
            name = value["target"]["name"]
            if name in result:
                raise BackendFailure("Ambiguous Cargo test target")
            result[name] = executable
    return result


def named_inventory(text):
    names = [line[:-6] for line in text.splitlines() if line.endswith(": test")]
    if not names or len(names) != len(set(names)):
        raise BackendFailure("Empty or duplicate named inventory")
    return names


def named_results(text, expected):
    """A green exit cannot substitute for exact named test execution."""
    results = {}
    for line in text.splitlines():
        match = re.fullmatch(r"test (.+) \.\.\. (ok|FAILED|ignored(?:, .*)?)", line)
        if match:
            if match[1] in results:
                raise BackendFailure("Duplicate named test result")
            results[match[1]] = match[2]
    if set(results) != set(expected):
        raise BackendFailure("Executed named tests differ from selected inventory")
    return {
        "passed": sum(value == "ok" for value in results.values()),
        "failed": sum(value == "FAILED" for value in results.values()),
        "ignored": sum(value.startswith("ignored") for value in results.values()),
        "results": results,
    }


class Commands(fixture.Commands):
    """Reuse fixture process ownership and add one non-renewable run budget."""

    def __init__(self, output, receipt, deadline, interrupted):
        super().__init__(output, receipt, interrupted)
        self.deadline = deadline
        self.cleaning = False

    def run(self, name, command, data=None, timeout=45, **kwargs):
        if not self.cleaning:
            timeout = min(timeout, self.deadline - time.monotonic())
        if timeout <= 0:
            raise BackendFailure("Overall Linux backend budget exhausted")
        start = len(self.receipt["steps"])
        try:
            return super().run(name, command, data=data, timeout=timeout, **kwargs)
        finally:
            if len(self.receipt["steps"]) > start:
                self.receipt["steps"][-1]["timeout_seconds"] = round(timeout, 3)
                write_json(self.output / "progress.json", self.receipt)


def inspect_container(commands, selector):
    _, text = commands.run("container-identity", [
        "podman", "inspect", "--type=container", "--format",
        '{"Id":{{json .ID}},"Name":{{json .Name}},"Labels":{{json .Config.Labels}}}',
        selector,
    ], timeout=10)
    return json.loads(text)


def identity(commands, container_id, owner):
    value = inspect_container(commands, container_id)
    if not container_matches(value, container_id, owner):
        raise BackendFailure("Container identity changed; refusing mutation/cleanup")
    return value


def remove_owned_container(commands, container_id, candidate_name, owner):
    """Recover an interrupted create only by rechecking its complete identity."""
    if container_id is None:
        if candidate_name is None:
            return {"container_removed": None, "container_created": False}
        rc, _ = commands.run("ambiguous-create-exists", [
            "podman", "container", "exists", candidate_name,
        ], timeout=10, check=False)
        if rc == 1:
            return {"container_removed": True, "container_created": False}
        if rc != 0:
            raise BackendFailure("Could not resolve ambiguous container creation")
        info = inspect_container(commands, candidate_name)
        container_id = info.get("Id") if isinstance(info, dict) else None
        if not container_matches(info, container_id, owner):
            raise BackendFailure("Ambiguous create does not match this owner; refusing cleanup")
    # Recheck by immutable ID even if creation returned normally.
    identity(commands, container_id, owner)
    commands.run("remove-owned-container", ["podman", "rm", "-f", container_id], timeout=20)
    rc, _ = commands.run("owned-container-absent", [
        "podman", "container", "exists", container_id,
    ], timeout=10, check=False)
    if rc != 1:
        raise BackendFailure("Removed container absence was not verified")
    return {"container_removed": True, "container_created": True, "container_id": container_id}


def linux_exec(container, arguments, seconds, user=False):
    command = ["podman", "exec"]
    if user:
        command.extend([
            "--user=1000:1000", "--workdir=/snapshot",
            "--env=CARGO_HOME=/linux-cargo", "--env=CARGO_TARGET_DIR=/linux-target",
            "--env=CARGO_NET_OFFLINE=true", "--env=CARGO_BUILD_JOBS=4",
            "--env=CARGO_INCREMENTAL=0", "--env=HOME=/scratch", "--env=TMPDIR=/scratch",
        ])
    return command + [container, "timeout", "--kill-after=3", str(seconds), *arguments]


def validate_tools(text, vm_arch):
    """Require a native Linux compiler/VM architecture; do not accept emulation."""
    host = re.search(r"^host: (\S+)$", text, re.MULTILINE)
    kernel = re.search(r"^Linux (aarch64|x86_64)$", text, re.MULTILINE)
    versions = re.search(r"^rustc (\d+\.\d+\.\d+)", text, re.MULTILINE)
    arch = {"aarch64": "arm64", "x86_64": "amd64"}
    if not host or not kernel or not versions or "-unknown-linux-" not in host[1]:
        raise BackendFailure("Toolchain evidence is not a Linux Rust compiler")
    if not host[1].startswith(kernel[1] + "-") or arch[kernel[1]] != vm_arch.strip():
        raise BackendFailure("Container compiler, kernel and VM architectures differ")
    return {"rustc": versions[1], "host": host[1], "kernel_arch": kernel[1], "vm_arch": vm_arch.strip()}


def execute(args):
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    owner = uuid.uuid4().hex
    start = time.monotonic()
    receipt = {
        "schema": 1, "scope": "native Linux Rust local filesystem and codecs; no desktop/peer acceptance",
        "revision": args.revision, "owner": owner, "image": args.image,
        "started_unix": time.time(), "budget_seconds": args.budget_seconds, "steps": [], "suites": [],
        "status": "RUNNING", "host_cargo_used": False, "host_target_accessed": False,
        "network": "none", "msrv_bypass": False,
    }
    interrupted = [False]

    def interrupt(_signal=None, _frame=None):
        interrupted[0] = True

    def check_interrupt():
        if interrupted[0] and not commands.cleaning:
            raise BackendFailure("Interrupted; retaining receipts and cleaning owned resources")

    commands = Commands(output, receipt, start + args.budget_seconds, check_interrupt)
    previous = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGINT, signal.SIGTERM)}
    container = None
    candidate_name = None
    cleanup = {"container_removed": False, "owned_processes_stopped": False}
    try:
        host_inputs = {}
        host_freeze = output / "host-runner-inputs"
        host_freeze.mkdir()
        for name in ("run_linux_local_backend.py", "run_linux_file_interop.py", "openssh_interop.py"):
            data = (ROOT / "scripts" / name).read_bytes()
            (host_freeze / name).write_bytes(data)
            host_inputs["scripts/" + name] = sha256(data)
        write_json(output / "host-runner-manifest.json", host_inputs)
        receipt["host_runner_inputs"] = host_inputs
        _, object_id = commands.run("resolve-revision", [
            "git", "rev-parse", "--verify", args.revision + "^{commit}",
        ])
        if object_id.decode().strip() != args.revision:
            raise BackendFailure("Source revision did not resolve exactly")
        source_tar = output / "source.tar"
        commands.run("source-git-archive", [
            "git", "archive", "--format=tar", "--output=" + str(source_tar),
            args.revision, *SOURCE_PATHS,
        ], timeout=30)
        manifest = snapshot_manifest(source_tar)
        manifest.update({"revision": args.revision, "archive_sha256": sha256(source_tar.read_bytes())})
        write_json(output / "source-manifest.json", manifest)
        receipt["source"] = {k: v for k, v in manifest.items() if k != "files"}
        with tarfile.open(source_tar) as archive:
            lock = tomllib.loads(archive.extractfile("Cargo.lock").read().decode())
        registry_tar = output / "registry.tar"
        cache = registry_bundle(lock, args.registry_cache, args.registry_index, registry_tar)
        write_json(output / "registry-manifest.json", cache)
        receipt["registry"] = {k: v for k, v in cache.items() if k not in ("packages", "index_entries")}
        _, vm = commands.run("podman-vm-architecture", ["podman", "info", "--format", "{{.Host.Arch}}"])
        name = PREFIX + owner[:12]
        candidate_name = name
        _, data = commands.run("create-container", [
            "podman", "create", "--pull=never", "--http-proxy=false", "--network=none",
            "--label", LABEL + "=" + LABEL_VALUE, "--label", OWNER_LABEL + "=" + owner,
            "--name", name, args.image,
            "timeout", "--kill-after=3", str(args.budget_seconds + 60), "sleep",
            str(args.budget_seconds + 60),
        ])
        container = data.decode().strip()
        receipt["container_id"] = container
        identity(commands, container, owner)
        commands.run("start-container", ["podman", "start", container])
        _, tools = commands.run("linux-toolchain", linux_exec(container, [
            "sh", "-ec", "uname -sm\nrustc -vV\ncargo -V\ncc --version\n"
            "command -v timeout\ncommand -v readelf\nid -u\n",
        ], 15), timeout=20)
        receipt["toolchain"] = validate_tools(tools.decode(), vm.decode())
        _, image_id = commands.run("container-image-id", [
            "podman", "inspect", "--type=container", "--format", "{{.Image}}", container,
        ])
        receipt["image_id"] = image_id.decode().strip()
        commands.run("prepare-private-directories", linux_exec(container, [
            "mkdir", "-p", "/snapshot", "/linux-target", "/linux-cargo", "/scratch",
        ], 10))
        for path in (source_tar, registry_tar):
            commands.run("copy-" + path.stem, ["podman", "cp", str(path), container + ":/tmp/" + path.name], timeout=60)
        commands.run("extract-private-inputs", linux_exec(container, [
            "sh", "-ec", "tar -xf /tmp/source.tar -C /snapshot\n"
            "tar -xf /tmp/registry.tar -C /linux-cargo\n"
            "chmod -R a-w /snapshot\n"
            "chown -R 1000:1000 /linux-target /linux-cargo /scratch\n",
        ], 60), timeout=65)
        _, user = commands.run("test-user", linux_exec(container, ["id"], 10, user=True))
        if not user.startswith(b"uid=1000"):
            raise BackendFailure("Tests must execute as the non-root fixture user")
        artifacts = {}
        build_args = (
            ("keelshell-session", ["--lib", "--test=zmodem", "--test=archive_transfer"]),
            ("keelshell-core", ["--test=zmodem", "--test=archive", "--test=archive_pax"]),
        )
        for package, targets in build_args:
            identity(commands, container, owner)
            seconds = max(1, min(600, math.floor(commands.deadline - time.monotonic()) - 5))
            _, text = commands.run("compile-" + package, linux_exec(container, [
                "cargo", "test", "--locked", "--offline", "-p", package, *targets,
                "--no-run", "--message-format=json",
            ], seconds, user=True), timeout=seconds + 4)
            artifacts[package] = parse_artifacts(text.decode(), package)
        write_json(output / "artifacts.json", artifacts)
        inventories = {}
        for package, target, prefix, required in SPECS:
            executable = artifacts.get(package, {}).get(target)
            if executable is None:
                raise BackendFailure("Missing compiled test executable: " + package + "/" + target)
            key = package + "/" + target
            label = key.replace("/", "-") + ("-archive-local" if prefix.startswith("sftp") else "")
            if key not in inventories:
                _, text = commands.run("inventory-" + label, linux_exec(container, [
                    executable, "--list", "--format=terse",
                ], 15, user=True), timeout=20)
                inventories[key] = named_inventory(text.decode())
                _, integrity = commands.run("binary-sha-" + label, linux_exec(container, [
                    "sha256sum", executable,
                ], 10))
                _, elf = commands.run("binary-elf-" + label, linux_exec(container, [
                    "readelf", "-h", executable,
                ], 10))
                write_json(output / (label + "-binary.json"), {
                    "executable": executable, "sha256sum": integrity.decode(),
                    "elf_header": elf.decode(), "revision": args.revision,
                    "inventory": inventories[key],
                })
            selected = [test for test in inventories[key] if not prefix or test.startswith(prefix)]
            if not selected or not set(required).issubset(selected):
                raise BackendFailure("Required Linux backend test is absent from " + key)
            if any("unverified" in test or "darwin" in test for test in selected):
                raise BackendFailure("Platform-specific inventory contradicts Linux selection")
            _, text = commands.run("test-" + label, linux_exec(container, [
                executable, *([prefix] if prefix else []),
                "--format=pretty", "--color=never", "--test-threads=2",
            ], 120, user=True), timeout=125, check=False)
            step = receipt["steps"][-1]
            result = named_results(text.decode(), selected)
            result.update({"package": package, "target": target, "filter": prefix,
                           "selected": selected, "step": step["name"]})
            receipt["suites"].append(result)
            write_json(output / "test-results.json", receipt["suites"])
            if step["returncode"] or step["timed_out"] or result["failed"]:
                raise BackendFailure("Linux test suite failed: " + label)
        _, hashes = commands.run("source-after-run", linux_exec(container, [
            "sha256sum", "--", *manifest["files"],
        ], 15, user=True), timeout=20)
        # sha256sum paths are controlled archive members, with no newline names.
        observed = {}
        for line in hashes.decode().splitlines():
            checksum, name = line.split("  ", 1)
            observed[name] = checksum
        if observed != manifest["files"]:
            raise BackendFailure("Snapshot input drift after execution")
        receipt["source_after_run_matches"] = True
        if any(sha256((ROOT / path).read_bytes()) != checksum for path, checksum in host_inputs.items()):
            raise BackendFailure("Host runner inputs changed during verification")
        receipt["host_runner_after_run_matches"] = True
        receipt["status"] = "PASS"
    except (fixture.FixtureFailure, fixture.ownership.InteropFailure, OSError,
            ValueError, tarfile.TarError, subprocess.SubprocessError) as error:
        receipt.update({"status": "FAIL", "error": type(error).__name__ + ": " + str(error)})
    finally:
        commands.cleaning = True
        try:
            cleanup.update(remove_owned_container(commands, container, candidate_name, owner))
        except (fixture.FixtureFailure, fixture.ownership.InteropFailure, OSError,
                ValueError, subprocess.SubprocessError) as error:
            cleanup["error"] = type(error).__name__ + ": " + str(error)
        cleanup["owned_processes_stopped"] = commands.finish()
        cleanup["container_filesystem_and_target_removed"] = cleanup["container_removed"]
        receipt["cleanup"] = cleanup
        receipt["elapsed_seconds"] = round(time.monotonic() - start, 3)
        receipt["finished_unix"] = time.time()
        if not cleanup["owned_processes_stopped"] or cleanup["container_removed"] is False:
            receipt["status"] = "FAIL"
        receipt["totals"] = {
            outcome: sum(suite[outcome] for suite in receipt["suites"])
            for outcome in ("passed", "failed", "ignored")
        }
        write_json(output / "cleanup.json", cleanup)
        write_json(output / "receipt.json", receipt)
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    print(json.dumps({key: receipt.get(key) for key in (
        "status", "revision", "error", "elapsed_seconds", "toolchain", "totals", "cleanup",
    )}, indent=2))
    return 0 if receipt["status"] == "PASS" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--revision", type=revision, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--registry-cache", type=Path, required=True)
    parser.add_argument("--registry-index", type=Path, required=True)
    parser.add_argument("--image", default="docker.io/library/rust:1.98.1-slim-bookworm")
    parser.add_argument("--budget-seconds", type=int, default=900)
    args = parser.parse_args()
    if not 60 <= args.budget_seconds <= 1800:
        parser.error("budget-seconds must be in 60..1800")
    return execute(args)


if __name__ == "__main__":
    raise SystemExit(main())
