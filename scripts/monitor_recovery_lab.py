#!/usr/bin/env python3
"""Offline owned Linux monitor degradation/recovery lab, with prebuilt tests.

Example (the parent supplies build provenance and a stable executable copy):
  python3 scripts/monitor_recovery_lab.py --opt-in localhost-owned-fixture \
    --output work/monitor-lab-r1 --source-sha <40 hex> \
    --test-binary <monitor_interop> --test-binary-sha256 <64 hex> \
    --base-image <existing full image ID>

No Cargo, downloads, installation, host mounts or NET_ADMIN. The private client
environment uses KEELSHELL_MONITOR_{OPT_IN,SOURCE_SHA,BINARY_SHA256,OWNER,TOKEN,
PORT,USER,HOST_KEY,HOST_PIN,CLIENT_KEY,CLIENT_PIN,REPORT_DIR}; stdin is DEVNULL.
The host is always 127.0.0.1. TOKEN and key contents are never receipt fields.
"""

import argparse
from contextlib import ExitStack
import copy
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

import openssh_interop as interop
import ssh_path_lab as process_lab

ROOT = Path(__file__).resolve().parents[1]
HELPERS = ROOT / "scripts" / "monitor_recovery"
LABEL = "io.keelshell.monitor-recovery.owner"
REMOTE = "/run/keelshell-monitor"
HELPER = "/opt/keelshell-monitor/fixture.py"
TESTS = (
    "openssh_monitor_linux_capabilities_degrade_and_recover",
    "openssh_monitor_linux_closed_transport_is_not_command_missing",
)
CLEANUP_SECONDS = 70


class LabFailure(Exception):
    """A failed precondition or observation, never a successful empty sample."""

    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind


def failure(error):
    return {"type": error.kind if isinstance(error, LabFailure) else type(error).__name__,
            "message": str(error)}


def write_new(path, value):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def full_id(value):
    value = value.removeprefix("sha256:")
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise LabFailure("ResourceIdentityInvalid", "Expected a full lowercase resource ID")
    return value


def inventory(text):
    """Require the full inventory, not a substring or a zero-test exit code."""
    lines = text.splitlines()
    names = [line.removesuffix(": test") for line in lines if line.endswith(": test")]
    if (sorted(names) != sorted(TESTS) or len(names) != len(set(names))
            or not re.search(r"(?m)^2 tests, 0 benchmarks$", text)):
        raise LabFailure("PrebuiltInventoryMismatch", "Expected exactly the two monitor_interop tests")
    return names


def source_hashes():
    paths = [
        Path(__file__).resolve(), ROOT / "scripts/test_monitor_recovery_lab.py",
        ROOT / "scripts/ssh_path_lab.py", ROOT / "scripts/openssh_interop.py",
        ROOT / "crates/keelshell-session/tests/monitor_interop.rs",
        ROOT / "crates/keelshell-session/src/monitor.rs",
        ROOT / "crates/keelshell-session/src/ssh.rs",
        ROOT / "crates/keelshell-session/src/error.rs",
        ROOT / "Cargo.toml", ROOT / "Cargo.lock",
        ROOT / "crates/keelshell-session/Cargo.toml",
        *sorted((ROOT / "crates/keelshell-session/src/monitor").rglob("*.rs")),
        *sorted(HELPERS.glob("*.py")), HELPERS / "Containerfile",
    ]
    return {str(path.relative_to(ROOT)): process_lab.digest(path) for path in paths}


class ClientRunner(process_lab.LabRunner):
    """Register the child before any fallible ancestry inventory can interrupt."""

    def start(self, name, command, environment=None):
        self.check_interrupt()
        if time.monotonic() >= self.deadline:
            raise interop.InteropFailure("Overall deadline before " + name)
        log = self.stack.enter_context((self.output / (name + ".log")).open("xb"))
        process = subprocess.Popen(
            command, cwd=ROOT, env=environment, stdin=subprocess.DEVNULL,
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
        )
        self.processes.append(process)
        self.owned[process] = {}
        self.names[process] = name
        self.receipt["processes"].append({"name": name, "pid": process.pid})
        try:
            info = interop.process_info(process.pid)
            if info is not None:
                self.owned[process][process.pid] = info
            self.capture_owned()
        except BaseException:
            self.ancestry_unverified.add(process.pid)
            raise
        return process


def stop_clients(runner, deadline):
    rows = process_lab.shutdown(runner, deadline)
    confirmed = (len(rows) == len(runner.processes)
                 and all(row.get("reaped") and row.get("observed_stopped") for row in rows)
                 and not runner.ancestry_unverified)
    return rows, confirmed


def container_arguments(owner, image, network):
    args = [
        "create", "--pull=never", "--name", owner + "-ssh", "--label", f"{LABEL}={owner}",
        "--network", network, "--read-only", "--cap-drop=ALL",
        "--security-opt=no-new-privileges", "--pids-limit=128", "--memory=256m", "--cpus=1",
        "--tmpfs", f"{REMOTE}:rw,noexec,nosuid,nodev,size=16m,mode=755",
        "--tmpfs", "/run/sshd:rw,noexec,nosuid,nodev,size=1m,mode=755",
        "--tmpfs", "/tmp:rw,noexec,nosuid,nodev,size=8m",
        "--publish", "127.0.0.1::2222",
    ]
    for capability in ("CHOWN", "DAC_READ_SEARCH", "SETUID", "SETGID", "SYS_CHROOT"):
        args += ["--cap-add", capability]
    return [*args, image, "serve", "--owner", owner]


def check_isolation(row, network, image, owner):
    security = row["security"]
    attachments = row["network"].get("Networks", {})
    identity = row["fixture"]["identity"]
    expected_caps = (1 << 0) | (1 << 2) | (1 << 6) | (1 << 7) | (1 << 18)
    ports = row["network"].get("Ports", {})
    bindings = ports.get("2222/tcp")
    if (security.get("privileged") or security.get("network_mode") != "bridge"
            or security.get("readonly") is not True
            or "no-new-privileges" not in (security.get("security_opt") or [])
            or full_id(security["image"]) != image["id"]
            or len(attachments) != 1
            or next(iter(attachments.values())).get("NetworkID") != network["id"]
            or any(mount.get("Type") != "tmpfs" for mount in security.get("mounts", []))
            or row["fixture"].get("owner") != owner):
        raise LabFailure("ContainerIsolationMismatch", "Owned read-only container isolation disagrees")
    if (not re.fullmatch(r"net:\[\d+\]", identity.get("network_namespace", ""))
            or type(identity.get("pid1_birth_ticks")) is not int
            or identity["pid1_birth_ticks"] <= 0
            or not re.fullmatch(r"[0-9a-f-]{36}", identity.get("boot_id", ""))
            or any(int(identity.get(field, "-1"), 16) != expected_caps
                   for field in ("effective_caps_hex", "bounding_caps_hex"))):
        raise LabFailure("NamespaceIdentityUnknown", "Kernel netns/birth/capability identity disagrees")
    if (set(ports) != {"2222/tcp"} or not isinstance(bindings, list) or len(bindings) != 1
            or bindings[0].get("HostIp") != "127.0.0.1"):
        raise LabFailure("PublicationNotLocalhost", "SSH must publish only one 127.0.0.1 port")
    port = int(bindings[0]["HostPort"])
    if not 1 <= port <= 65535:
        raise LabFailure("PublicationInvalid", "Invalid published port")
    return port


class PodmanLab:
    """Journal names before mutation; unknown daemon outcomes remain sticky."""

    def __init__(self, runner, owner, receipt, executable):
        self.runner, self.owner, self.receipt, self.executable = runner, owner, receipt, executable
        self.sequence = 0
        self.barrier_confirmed = False

    def journal(self, event):
        with (self.runner.output / "events.jsonl").open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(event, allow_nan=False) + "\n")
            stream.flush()
            os.fsync(stream.fileno())

    def command(self, label, args, seconds=10, allowed=(0,), environment=None,
                binary=False, mutation=None):
        self.sequence += 1
        name = f"{self.sequence:03d}-{label}"
        argv = args if binary else [self.executable, *args]
        entry = {"name": name, "argv": argv, "timeout_seconds": seconds}
        self.receipt["commands"].append(entry)
        operation = None
        if mutation is not None:
            operation = {"command": name, "status": "PENDING"}
            mutation.setdefault("mutations", []).append(operation)
        self.journal({"event": "command-start", "command": entry,
                      "resource": copy.deepcopy(mutation)})
        started = time.monotonic()
        code = None
        try:
            process = self.runner.start(name, argv, environment)
            entry["pid"] = process.pid
            code = self.runner.wait_bounded(name, process, seconds)
            entry.update(returncode=code, reaped=True)
            if operation is not None:
                operation.update(status="COMPLETED" if code == 0 else "UNKNOWN", returncode=code)
            if code not in allowed:
                raise LabFailure("CommandFailed", f"{label} exited {code}; see {name}.log")
            return code, (self.runner.output / (name + ".log")).read_text(
                encoding="utf-8", errors="replace",
            )
        except BaseException as error:
            if operation is not None and code != 0:
                operation["status"] = "UNKNOWN"
            entry["failure"] = failure(error)
            # start() can raise after Popen, before returning its handle. Stop
            # every registered child, including that one, before resource reads.
            self.barrier_confirmed = False
            try:
                entry["client_barrier"], self.barrier_confirmed = stop_clients(
                    self.runner, min(self.runner.deadline + 5, time.monotonic() + 5),
                )
            except Exception as stop_error:
                entry["barrier_failure"] = failure(stop_error)
            raise
        finally:
            entry["elapsed_seconds"] = round(time.monotonic() - started, 6)
            self.journal({"event": "command-end", "command": entry,
                          "resource": copy.deepcopy(mutation)})

    def json(self, label, args, **kwargs):
        _, text = self.command(label, args, **kwargs)
        try:
            return json.loads(text)
        except ValueError as error:
            raise LabFailure("InvalidCommandOutput", label + " did not return JSON") from error

    def register(self, kind, name):
        row = {"kind": kind, "name": name, "mutations": []}
        self.receipt["resources"].append(row)
        self.journal({"event": "resource-intent", "resource": row})
        return row

    def bind(self, resource):
        kind, expected = resource["kind"], resource.get("id")
        identifier = expected or resource["name"]
        if kind == "network":
            value = self.json("bind-network", ["network", "inspect", identifier])[0]
            ident, labels, matches = value["id"], value.get("labels", {}), value.get("name") == resource["name"]
        else:
            template = (
                '{"id":{{json .ID}},"labels":{{json .Config.Labels}},'
                + ('"names":{{json .RepoTags}}}' if kind == "image" else '"name":{{json .Name}}}')
            )
            value = self.json("bind-" + kind, [kind, "inspect", "--format", template, identifier])
            ident, labels = value["id"], value.get("labels") or {}
            matches = (resource["name"] in (value.get("names") or []) if kind == "image"
                       else value.get("name", "").lstrip("/") == resource["name"])
        ident = full_id(ident)
        if labels.get(LABEL) != self.owner or not matches or (expected and expected != ident):
            raise LabFailure("OwnershipMismatch", "Resource owner/full ID/exact name no longer match")
        resource["id"] = ident
        self.journal({"event": "resource-bound", "resource": copy.deepcopy(resource)})
        return ident

    def setup(self, base):
        metadata = self.json("base-image", ["image", "inspect", "--format",
            '{"id":{{json .ID}},"os":{{json .Os}},"arch":{{json .Architecture}}}', base])
        self.receipt["base_image"] = metadata
        if full_id(metadata["id"]) != base or metadata["os"] != "linux":
            raise LabFailure("CapabilityUnavailable", "Existing full image ID must identify Linux")
        image = self.register("image", "localhost/" + self.owner + ":lab")
        self.command("build-offline", [
            "build", "--pull=never", "--network=none", "--no-cache", "--force-rm",
            "--label", f"{LABEL}={self.owner}", "--build-arg", "BASE_IMAGE=" + base,
            "--tag", image["name"], str(HELPERS),
        ], seconds=45, mutation=image)
        self.bind(image)
        network = self.register("network", self.owner + "-internal")
        self.command("create-network", [
            "network", "create", "--internal", "--disable-dns", "--label",
            f"{LABEL}={self.owner}", network["name"],
        ], mutation=network)
        self.bind(network)
        config = self.json("network-isolation", ["network", "inspect", network["id"]])[0]
        network["configuration"] = config
        if not config.get("internal") or config.get("dns_enabled") or config.get("driver") != "bridge":
            raise LabFailure("NetworkIsolationMismatch", "Expected owned internal DNS-free bridge")
        container = self.register("container", self.owner + "-ssh")
        self.command("create-ssh", container_arguments(self.owner, image["id"], network["id"]),
                     mutation=container)
        self.bind(container)
        self.command("start-ssh", ["start", container["id"]], mutation=container)
        until = min(self.runner.deadline, time.monotonic() + 12)
        while True:
            code, text = self.command("fixture-ready", [
                "exec", container["id"], "/usr/bin/python3", HELPER, "status", "--owner", self.owner,
            ], seconds=5, allowed=(0, 1))
            if code == 0:
                container["fixture"] = json.loads(text)
                break
            if time.monotonic() >= until:
                raise LabFailure("CapabilityUnavailable", "Owned Linux fixture did not become ready")
        container["network"] = self.json("container-network", [
            "container", "inspect", "--format", "{{json .NetworkSettings}}", container["id"],
        ])
        container["security"] = self.json("container-isolation", [
            "container", "inspect", "--format",
            '{"network_mode":{{json .HostConfig.NetworkMode}},'
            '"privileged":{{json .HostConfig.Privileged}},'
            '"readonly":{{json .HostConfig.ReadonlyRootfs}},'
            '"security_opt":{{json .HostConfig.SecurityOpt}},'
            '"mounts":{{json .Mounts}},"image":{{json .Image}}}', container["id"],
        ])
        port = check_isolation(container, network, image, self.owner)
        return container, port

    def cleanup(self):
        self.barrier_confirmed = False
        try:
            rows, self.barrier_confirmed = stop_clients(self.runner, self.runner.deadline)
        except Exception as error:
            rows = []
            self.receipt["cleanup"]["client_barrier_failure"] = failure(error)
        self.receipt["cleanup"].update(client_barrier=rows, client_barrier_confirmed=self.barrier_confirmed)
        results = []
        for resource in reversed(self.receipt["resources"]):
            row = {"kind": resource["kind"], "name": resource["name"],
                   "id": resource.get("id"), "absent": False, "absence_observed": False}
            results.append(row)
            try:
                if not self.barrier_confirmed:
                    raise LabFailure("CleanupBarrierUnconfirmed", "Clients must stop/reap before resource reads")
                kind = resource["kind"]
                keys = list(dict.fromkeys([resource.get("id", resource["name"]), resource["name"]]))
                exists = [self.command("cleanup-exists-" + kind, [kind, "exists", key],
                                       seconds=4, allowed=(0, 1))[0] for key in keys]
                if any(code == 0 for code in exists):
                    identifier = self.bind(resource)
                    row["id"] = identifier
                    if kind == "container":
                        # comm intentionally excludes SSH control arguments/tokens.
                        self.command("final-processes", ["top", identifier, "hpid", "pid", "ppid", "comm"],
                                     seconds=4, allowed=(0, 125))
                        self.command("fixture-logs", ["logs", identifier], seconds=4, allowed=(0, 125))
                        args = ["rm", "--force", "--time=2", identifier]
                    elif kind == "network":
                        args = ["network", "rm", identifier]
                    else:
                        consumers = self.json("image-consumers", [
                            "ps", "--all", "--filter", "ancestor=" + identifier, "--format", "json",
                        ], seconds=4)
                        if consumers:
                            raise LabFailure("ImageHasConsumers", "Owned image still has consumers")
                        args = ["image", "rm", identifier]
                    self.command("remove-" + kind, args, seconds=7, mutation=resource)
                    keys = list(dict.fromkeys([identifier, resource["name"]]))
                    exists = [self.command("verify-absent-" + kind, [kind, "exists", key],
                                           seconds=4, allowed=(0, 1))[0] for key in keys]
                row["absence_observed"] = all(code == 1 for code in exists)
                row["absent"] = row["absence_observed"]
            except Exception as error:
                row["failure"] = failure(error)
            finally:
                # Any uncertain mutation, including start/remove, invalidates a
                # later absence read. Never retry the uncertain request itself.
                uncertain = any(op["status"] != "COMPLETED" for op in resource["mutations"])
                if uncertain:
                    row.update(absent=False, mutation_completion="UNKNOWN")
                    row.setdefault("failure", failure(LabFailure(
                        "MutationCompletionUnknown", "Daemon work may outlive its stopped client",
                    )))
        return results


def validate_summary(test, value, owner):
    if (value.get("schema") != 1 or value.get("test") != test or value.get("owner") != owner
            or value.get("status") != "PASSED" or value.get("native_app") != "NOT_RUN"):
        raise LabFailure("ReportInvalid", "Rust summary identity/status/boundary disagrees")
    if test == TESTS[0]:
        expected = [f"{tool}-{mode}" for tool in ("df", "ps", "ss")
                    for mode in ("missing", "unsupported", "permission")]
        if value.get("completed_cases") != expected:
            raise LabFailure("ReportInvalid", "All nine independent recovery cases must finish")
    elif value.get("transport_reason") != "Transport":
        raise LabFailure("ReportInvalid", "Closed SSH was not classified as Transport")


def private_remove(scratch, identity, owner):
    if scratch is None:
        return True
    if (scratch.is_symlink() or (scratch.stat().st_dev, scratch.stat().st_ino) != identity
            or (scratch / "owner").read_text() != owner):
        raise LabFailure("PrivateIdentityMismatch", "Private directory identity changed")
    shutil.rmtree(scratch)
    return not scratch.exists()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--opt-in", required=True, choices=("localhost-owned-fixture",))
    parser.add_argument("--output", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--test-binary", required=True, type=Path)
    parser.add_argument("--test-binary-sha256", required=True)
    parser.add_argument("--base-image", required=True, help="Existing complete Linux image ID")
    parser.add_argument("--timeout", type=float, default=480, help="Total including cleanup, 300..600s")
    args = parser.parse_args(argv)
    if platform.system() not in ("Darwin", "Linux"):
        parser.error("Owned process identities require macOS or Linux")
    if not math.isfinite(args.timeout) or not 300 <= args.timeout <= 600:
        parser.error("--timeout must be finite, 300..600 seconds")
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_sha):
        parser.error("--source-sha must be a full lowercase Git SHA")
    if not re.fullmatch(r"[0-9a-f]{64}", args.test_binary_sha256):
        parser.error("--test-binary-sha256 must be full lowercase SHA256")
    try:
        args.base_image = full_id(args.base_image)
    except LabFailure as error:
        parser.error(str(error))
    if not args.test_binary.is_file() or not os.access(args.test_binary, os.X_OK):
        parser.error("--test-binary must be an existing executable")
    args.test_binary = args.test_binary.resolve()
    started = time.monotonic()
    deadline = started + args.timeout
    mask = os.umask(0o077)
    try:
        output = interop.create_output(args.output)
    finally:
        os.umask(mask)
    owner = "keelshell-monitor-" + uuid.uuid4().hex[:16]
    receipt = {
        "schema": 1, "owner": owner, "status": "FAILED", "started_utc": interop.utc_now(),
        "source_sha": args.source_sha, "commands": [], "processes": [], "steps": [],
        "resources": [], "tests": [], "cleanup": {}, "timeout_seconds": args.timeout,
        "test_binary": {"path": str(args.test_binary), "expected_sha256": args.test_binary_sha256},
        "boundary": "Linux old API only; native app pause/render and macOS collectors NOT_RUN",
        "prebuilt_source_provenance": "parent assertion; runner verifies bytes and exact inventory, not compilation",
    }
    write_new(output / "intent.json", receipt)
    requested_signal = None
    cleaning = False
    previous_handlers = {}
    scratch = None
    scratch_identity = None
    runner = None
    api = None
    result = 1

    def interrupt(signum, _frame):
        nonlocal requested_signal
        requested_signal = signum

    def check_interrupt():
        if requested_signal is not None and not cleaning:
            raise interop.Interrupted("Requested signal " + str(requested_signal))

    print("Monitor recovery evidence: " + str(output), flush=True)
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous_handlers[sig] = signal.signal(sig, interrupt)
        with ExitStack() as stack:
            runner = ClientRunner(output, deadline - CLEANUP_SECONDS, stack, receipt, check_interrupt)
            try:
                receipt["test_binary"]["sha256"] = process_lab.digest(args.test_binary)
                receipt["source_sha256"] = source_hashes()
                if receipt["test_binary"]["sha256"] != args.test_binary_sha256:
                    raise LabFailure("PrebuiltDigestMismatch", "Binary does not match parent digest")
                # Inventory and source SHA precede any Podman resource mutation.
                api = PodmanLab(runner, owner, receipt, interop.executable("podman"))
                _, head = api.command("source-head", ["git", "rev-parse", "HEAD"], seconds=5, binary=True)
                if head.strip() != args.source_sha:
                    raise LabFailure("SourceShaMismatch", "Worktree HEAD disagrees with parent full SHA")
                for flags in (["--list"], ["--list", "--ignored"]):
                    _, text = api.command("inventory", [str(args.test_binary), *flags], seconds=5, binary=True)
                    inventory(text)
                receipt["test_binary"]["inventory"] = list(TESTS)
                container, port = api.setup(args.base_image)
                private_parent = ROOT / "work"
                private_parent.mkdir(exist_ok=True)
                scratch = Path(tempfile.mkdtemp(prefix=".monitor-private-", dir=private_parent)).resolve()
                scratch.chmod(0o700)
                scratch_identity = (scratch.stat().st_dev, scratch.stat().st_ino)
                (scratch / "owner").write_text(owner)
                (scratch / "tmp").mkdir(mode=0o700)
                receipt["private_directory"] = str(scratch)
                for filename, remote in (
                    ("host.pub", REMOTE + "/keys/host.pub"),
                    ("client", REMOTE + "/keys/client"), ("token", REMOTE + "/state/token"),
                ):
                    destination = scratch / filename
                    api.command("export-" + filename, ["cp", container["id"] + ":" + remote, str(destination)])
                    destination.chmod(0o600)
                token = (scratch / "token").read_text()
                if not re.fullmatch(r"[0-9a-f]{64}", token):
                    raise LabFailure("FixtureAuthenticationInvalid", "Invalid ephemeral control token")
                environment = {
                    "PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": str(scratch),
                    "TMPDIR": str(scratch / "tmp"), "LANG": "C",
                    **{"KEELSHELL_MONITOR_" + name: value for name, value in {
                        "OPT_IN": args.opt_in, "SOURCE_SHA": args.source_sha,
                        "BINARY_SHA256": args.test_binary_sha256, "OWNER": owner, "TOKEN": token,
                        "PORT": str(port), "USER": "ksmonitor",
                        "HOST_KEY": str(scratch / "host.pub"), "HOST_PIN": container["fixture"]["host_pin"],
                        "CLIENT_KEY": str(scratch / "client"), "CLIENT_PIN": container["fixture"]["client_pin"],
                    }.items()},
                }
                receipt["endpoint"] = f"ksmonitor@127.0.0.1:{port}"
                for index, test in enumerate(TESTS):
                    report_dir = output / ("rust-" + str(index))
                    report_dir.mkdir(mode=0o700)
                    test_row = {"name": test, "status": "FAILED", "report_directory": report_dir.name}
                    receipt["tests"].append(test_row)
                    try:
                        if process_lab.digest(args.test_binary) != args.test_binary_sha256:
                            raise LabFailure("InputDrift", "Binary changed before execution")
                        code, log = api.command("rust-test-" + str(index), [
                            str(args.test_binary), "--ignored", "--exact", test, "--nocapture", "--test-threads=1",
                        ], seconds=180 if index == 0 else 35, allowed=(0, 101), binary=True,
                            environment=dict(environment, KEELSHELL_MONITOR_REPORT_DIR=str(report_dir)))
                        if code or not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", log):
                            if container["fixture"]["tools"]["ss"]["path"] is None:
                                raise LabFailure(
                                    "CapabilityUnavailable",
                                    "Existing Linux image has no real ss; partial baseline observations are retained",
                                )
                            raise LabFailure("InteropTestFailed", test + " did not pass exactly once")
                        summary = json.loads((report_dir / "summary.json").read_text())
                        validate_summary(test, summary, owner)
                        test_row.update(status="PASSED", summary=summary)
                    except (LabFailure, OSError, ValueError) as error:
                        test_row["failure"] = failure(error)
                        # Preserve the failure and stop. Do not repair/replay a
                        # partially executed fixture or reuse its report names.
                        raise
                check_interrupt()
                result = 0
            finally:
                cleaning = True
                for sig in previous_handlers:
                    signal.signal(sig, signal.SIG_IGN)
                runner.deadline = deadline - 10
                try:
                    if api is not None:
                        receipt["cleanup"]["resources"] = api.cleanup()
                except Exception as error:
                    receipt["cleanup"]["resource_cleanup_failure"] = failure(error)
                finally:
                    runner.deadline = deadline
                    try:
                        rows, stopped = stop_clients(runner, deadline)
                        receipt["cleanup"].update(processes=rows, processes_confirmed=stopped)
                    except Exception as error:
                        receipt["cleanup"]["processes_confirmed"] = False
                        receipt["cleanup"]["process_cleanup_failure"] = failure(error)
                    receipt["cleanup"]["ancestry_unverified"] = sorted(runner.ancestry_unverified)
                    try:
                        if not receipt["cleanup"]["processes_confirmed"] and scratch is not None:
                            raise LabFailure("CleanupBarrierUnconfirmed", "Private keys retained until clients stop")
                        receipt["cleanup"]["private_directory_removed"] = private_remove(
                            scratch, scratch_identity, owner,
                        )
                    except Exception as error:
                        receipt["cleanup"]["private_removal_failure"] = failure(error)
    except (Exception, KeyboardInterrupt) as error:
        receipt["failure"] = failure(error)
        result = 130 if isinstance(error, (interop.Interrupted, KeyboardInterrupt)) else 1
    finally:
        for sig, handler in previous_handlers.items():
            signal.signal(sig, handler)
        cleanup = receipt["cleanup"]
        cleanup["confirmed"] = (
            len(cleanup.get("resources", [])) == len(receipt["resources"])
            and all(row["absent"] for row in cleanup.get("resources", []))
            and cleanup.get("client_barrier_confirmed", not receipt["resources"])
            and cleanup.get("processes_confirmed", not receipt["processes"])
            and cleanup.get("private_directory_removed", scratch is None)
        )
        if not cleanup["confirmed"]:
            receipt["failure_cleanup"] = failure(LabFailure("CleanupUnconfirmed", "Owned cleanup not fully confirmed"))
            result = 1
        try:
            receipt["test_binary"]["sha256_after"] = process_lab.digest(args.test_binary)
            receipt["source_sha256_after"] = source_hashes()
            receipt["input_bytes_unchanged"] = (
                receipt["test_binary"]["sha256_after"] == receipt["test_binary"].get("sha256")
                and receipt["source_sha256_after"] == receipt.get("source_sha256")
            )
        except OSError as error:
            receipt["input_readback_failure"] = failure(error)
            receipt["input_bytes_unchanged"] = False
        if not receipt["input_bytes_unchanged"]:
            receipt["failure_input"] = failure(LabFailure("InputDrift", "Input byte readback disagrees"))
            result = 1
        receipt.update(
            status="PASSED" if result == 0 else "INTERRUPTED" if result == 130 else "FAILED",
            observations_admissible=result == 0, returncode=result,
            elapsed_seconds=round(time.monotonic() - started, 3), finished_utc=interop.utc_now(),
        )
        receipt["logs"] = {path.name: {"bytes": path.stat().st_size, "sha256": process_lab.digest(path)}
                           for path in sorted(output.glob("*.log"))}
        write_new(output / "result.json", receipt)
        print(f"Monitor recovery lab: {receipt['status']}; exit {result}; {output}", flush=True)
    return result


if __name__ == "__main__":
    sys.exit(main())
