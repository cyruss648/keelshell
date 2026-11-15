#!/usr/bin/env python3
"""Real, bounded Linux netem A/B lab using Podman and prebuilt ssh_path_interop.

An offline image build needs an existing Linux image containing Python 3.11+,
OpenSSH and useradd. The runner never pulls images, installs packages or compiles
Rust. Only owned gate containers receive NET_ADMIN. All publications bind
127.0.0.1; the dedicated bridge is internal and has no DNS or external routing.
"""

import argparse
from contextlib import ExitStack
from decimal import Decimal, InvalidOperation
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import sys
import tempfile
import time
import uuid

import openssh_interop as interop
import ssh_path_lab as stream_lab
from ssh_path_netem import qdisc

ROOT = Path(__file__).resolve().parents[1]
HELPERS = ROOT / "scripts" / "ssh_path_netem"
SCENARIOS = ("baseline", "direct-impaired", "relay-impaired")
KINDS = ("Exec", "Upload", "Download")
OWNER_LABEL = "io.keelshell.netem.owner"
COMMAND_SECONDS = 12
CLEANUP_SECONDS = 60
CONTAINER_SCRIPT = "/opt/keelshell-netem/container.py"


class LabFailure(Exception):
    """A typed failure that cannot be converted to a performance observation."""

    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind

    def value(self):
        return {"type": self.kind, "message": str(self)}


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def full_id(value):
    value = value.strip().removeprefix("sha256:")
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise LabFailure("ResourceIdentityInvalid", "Expected a complete Podman resource ID")
    return value


def loss_threshold(text):
    """Use integer probability, preserving both requested and effective loss."""
    try:
        number = Decimal(text)
        if not number.is_finite() or not 0 < number <= 20:
            raise ValueError("Loss must be finite and greater than 0 through 20 percent")
        return int(number * Decimal(0xFFFFFFFF) / 100)
    except InvalidOperation as error:
        raise ValueError("Invalid random loss probability") from error


def conditions(scenario, delay_ns, loss_u32):
    clear = {"delay_ns": 0, "loss_u32": 0, "limit_packets": 1000}
    impaired = dict(clear, delay_ns=delay_ns, loss_u32=loss_u32)
    return {
        "model": "Linux kernel netem random IP-layer loss and delay via rtnetlink",
        "scenario": scenario, "interface": "eth0", "qdisc_handle": qdisc.HANDLE,
        "A": dict(impaired if scenario == "direct-impaired" else clear),
        "B": dict(impaired if scenario == "relay-impaired" else clear),
        "scope": "egress of each independent gate netns only; both upstream and client-bound packets",
        "topology": {
            "A": "Rust -> localhost CONNECT A -> target:2222",
            "B": "Rust -> localhost CONNECT B -> jump:2223 -> 127.0.0.1:2222",
            "same_logical_target": {"host": "127.0.0.1", "port": 2222, "user": "netem"},
        },
        "gate_behavior": "byte-preserving CONNECT forwarding, no pacing or application byte loss",
        "queue_limit_packets": 1000,
        "loss_effective_percent": loss_u32 / 0xFFFFFFFF * 100,
        "randomness": "independent kernel random loss; readback records the actual kernel PRNG seed when exposed",
        "counter_unit": "kernel qdisc packets/skbs; GSO may aggregate wire packets",
    }


def check_qdisc(snapshot, expected, owner, namespace):
    if (
        snapshot.get("owner") != owner
        or snapshot.get("network_namespace") != namespace
        or snapshot.get("interface") != "eth0"
    ):
        raise LabFailure("QdiscIdentityMismatch", "qdisc snapshot is from another owner/netns/interface")
    rows = snapshot.get("qdiscs")
    if not isinstance(rows, list) or len(rows) != 1:
        raise LabFailure("QdiscConfigurationMismatch", "Expected one root netem qdisc")
    row = rows[0]
    if row.get("kind") != "netem" or row.get("handle") != qdisc.HANDLE or row.get("parent") != qdisc.ROOT:
        raise LabFailure("QdiscConfigurationMismatch", "Root/handle/kind is not the requested netem")
    options = row.get("options", {})
    for name, value in dict(expected, gap=0, duplicate_u32=0, jitter_ns=0).items():
        if options.get(name) != value:
            raise LabFailure("QdiscConfigurationMismatch", f"Kernel netem readback disagrees on {name}")
    counters = row.get("counters", {})
    for name in ("bytes", "packets", "drops", "qlen", "backlog", "requeues", "overlimits"):
        if type(counters.get(name)) is not int or counters[name] < 0:
            raise LabFailure("QdiscCountersUnavailable", "Missing/invalid kernel counter " + name)
    if not row.get("raw_netlink_hex"):
        raise LabFailure("QdiscCountersUnavailable", "Original kernel netlink response is missing")
    return row


def check_counters(before, after, expected):
    """Require actual traffic and actual drops, with a clean unaffected control."""
    deltas = {name: after["counters"][name] - before["counters"][name]
              for name in ("bytes", "packets", "drops", "requeues", "overlimits")}
    if any(value < 0 for value in deltas.values()):
        raise LabFailure("QdiscCounterReset", "Kernel qdisc counters decreased during comparison")
    if deltas["packets"] <= 0 or deltas["bytes"] <= 0:
        raise LabFailure("QdiscNotTraversed", "No measured traffic traversed the declared netem")
    if after["counters"]["qlen"] or after["counters"]["backlog"]:
        raise LabFailure("QdiscQueueNotDrained", "The final qdisc still has queued traffic")
    if expected["loss_u32"]:
        if deltas["drops"] <= 0:
            raise LabFailure("NoObservedIpDrops", "Random loss configured but no kernel drops observed")
        # netem also counts queue overflow as drops; overlimits alone does not
        # distinguish it. With an initially empty queue and fewer total
        # transmitted+dropped skbs than its limit, overflow was impossible.
        if (deltas["overlimits"] or before["counters"]["qlen"]
                or deltas["packets"] + deltas["drops"] >= expected["limit_packets"]):
            raise LabFailure("DropCauseAmbiguous", "Cannot exclude netem queue overflow as a drop cause")
    elif deltas["drops"]:
        raise LabFailure("ControlPathDroppedPackets", "The clear control recorded kernel drops")
    return deltas


def validate_report(report):
    stream_lab.validate_report(report)
    if report.get("schema") != 1 or report.get("limits") != {
        "rounds": 3, "sample_seconds": 8, "comparison_seconds": 60,
        "bytes_per_transfer": 262144, "total_payload_bytes": 3145728,
    }:
        raise LabFailure("ReportInvalid", "Comparison schema or fixed budgets changed")
    expected = []
    for number in (1, 2, 3):
        for kind in KINDS:
            for side in (("B", "A") if number == 2 else ("A", "B")):
                expected.append((number, kind, side))
    actual = [(r.get("round"), r.get("kind"), r.get("path")) for r in report["samples"]]
    if (actual != expected or any(type(r.get("index")) is not int for r in report["samples"])
            or [r.get("index") for r in report["samples"]] != list(range(18))):
        raise LabFailure("ReportInvalid", "Exact 18-sample alternating operation order changed")
    wall = report.get("wall_elapsed_ns")
    if type(wall) is not int or not 0 < wall < 62_000_000_000:
        raise LabFailure("ReportInvalid", "Invalid comparison wall budget")
    if any(r.get("bytes") != 0 for r in report["samples"] if r["kind"] == "Exec"):
        raise LabFailure("ReportInvalid", "Exec is not a payload transfer")
    return stream_lab.summarize(report)


def container_arguments(owner, name, image, network, role, upstream=None):
    args = [
        "create", "--pull=never", "--name", name, "--label", f"{OWNER_LABEL}={owner}",
        "--network", network, "--read-only", "--cap-drop=ALL",
        "--security-opt=no-new-privileges", "--pids-limit=128", "--memory=256m", "--cpus=1",
        "--tmpfs", "/run/netem:rw,noexec,nosuid,nodev,size=16m,mode=755",
        "--tmpfs", "/run/sshd:rw,noexec,nosuid,nodev,size=1m,mode=755",
        "--tmpfs", "/tmp:rw,noexec,nosuid,nodev,size=16m",
    ]
    if role == "servers":
        for capability in ("SETUID", "SETGID", "SYS_CHROOT"):
            args += ["--cap-add", capability]
    else:
        args += ["--cap-add", "NET_ADMIN", "--publish", "127.0.0.1::8080"]
    args += [image, "servers" if role == "servers" else "gate", "--owner", owner]
    if role != "servers":
        args += ["--upstream", upstream, "--target-port", "2222" if role == "A" else "2223"]
    return args


def check_container_isolation(row, network):
    security = row["security"]
    attachments = row["network"].get("Networks", {})
    # Podman's NetworkMode is "bridge" even for an explicitly named custom
    # bridge. The actual attachment IDs establish isolation, not that label.
    if (
        security["privileged"] or security["network_mode"] != "bridge"
        or len(attachments) != 1
        or next(iter(attachments.values())).get("NetworkID") != network["id"]
    ):
        raise LabFailure("ContainerIsolationMismatch", "Container escaped the owned bridge network")
    if not security["readonly"] or "no-new-privileges" not in security["security_opt"]:
        raise LabFailure("ContainerIsolationMismatch", "Container filesystem/privilege guard changed")
    # CapAdd lists only additions to the runtime's default set. Inspect the
    # actual PID 1 kernel masks so default capabilities cannot escape review.
    expected = (1 << 12) if row["role"] != "servers" else (1 << 6) | (1 << 7) | (1 << 18)
    identity = row["identity"]
    if any(int(identity.get(field, "-1"), 16) != expected
           for field in ("effective_caps_hex", "bounding_caps_hex")):
        raise LabFailure("ContainerIsolationMismatch", "Unexpected effective/bounding kernel capabilities")


class PodmanLab:
    """Register names before mutation and bind all later operations to full IDs."""

    def __init__(self, runner, owner, receipt, executable):
        self.runner, self.owner, self.receipt, self.executable = runner, owner, receipt, executable
        self.resources = receipt["resources"]
        self.sequence = 0

    def command(self, label, args, seconds=COMMAND_SECONDS, allowed=(0,), environment=None,
                binary=False, mutation=None):
        self.sequence += 1
        name = f"{self.sequence:03d}-{label}"
        argv = args if binary else [self.executable, *args]
        started = time.monotonic()
        entry = {"name": name, "argv": argv, "timeout_seconds": seconds}
        self.receipt["commands"].append(entry)
        if mutation is not None:
            mutation["creation"] = {"command": name, "status": "PENDING"}
        process = None
        try:
            process = self.runner.start(name, argv, environment)
            entry["pid"] = process.pid
            code = self.runner.wait_bounded(name, process, seconds)
        except Exception as error:
            if mutation is not None:
                # Killing a CLI client cannot establish that a request already
                # accepted by the Podman service has stopped creating resources.
                mutation["creation"]["status"] = "UNKNOWN"
            entry["failure"] = {
                "type": "CommandDeadline" if isinstance(error, interop.InteropFailure)
                else type(error).__name__, "message": str(error),
            }
            if process is not None:
                until = min(self.runner.deadline + 8, time.monotonic() + 3)
                try:
                    entry["observed_stopped"] = interop.stop_owned(
                        process, until, self.runner.owned[process],
                    )
                    process.wait(timeout=max(0.001, until - time.monotonic()))
                except Exception as stop_error:
                    entry["stop_failure"] = {
                        "type": type(stop_error).__name__, "message": str(stop_error),
                    }
                entry["reaped"] = process.poll() is not None
            if isinstance(error, interop.InteropFailure):
                raise LabFailure("CommandDeadline", f"{label} exceeded its bounded deadline") from error
            raise
        finally:
            entry["elapsed_seconds"] = round(time.monotonic() - started, 6)
        entry.update(returncode=code, reaped=True)
        if mutation is not None:
            # A nonzero or signaled client can lose an accepted service-side
            # request just like a deadline. Only its successful reply settles it.
            mutation["creation"].update(
                status="COMPLETED" if code == 0 else "UNKNOWN", returncode=code,
            )
        text = (self.runner.output / f"{name}.log").read_text(encoding="utf-8", errors="replace")
        if code not in allowed:
            raise LabFailure("CommandFailed", f"{label} exited {code}; see {name}.log")
        return code, text

    def json(self, label, args, **kwargs):
        _, text = self.command(label, args, **kwargs)
        try:
            return json.loads(text)
        except ValueError as error:
            raise LabFailure("InvalidCommandOutput", f"{label} did not return JSON") from error

    def inspect(self, resource, template):
        return self.json("inspect-" + resource["kind"], [
            resource["kind"], "inspect", "--format", template, resource.get("id", resource["name"]),
        ])

    def bind(self, resource):
        kind = resource["kind"]
        if kind == "network":
            record = self.json("bind-network", ["network", "inspect", resource["name"]])[0]
            ident, labels = record["id"], record.get("labels", {})
            resource["configuration"] = record
        else:
            record = {
                "Id": self.inspect(resource, '{{json .ID}}'),
                "Labels": self.inspect(resource, '{{json .Config.Labels}}'),
            }
            ident = record["Id"]
            labels = record.get("Labels", record.get("Config", {}).get("Labels", {}))
        if labels.get(OWNER_LABEL) != self.owner:
            raise LabFailure("OwnershipMismatch", f"{kind} does not have this run's owner label")
        resource["id"] = full_id(ident)
        return resource["id"]

    def create_network(self):
        row = {"kind": "network", "name": self.owner + "-internal"}
        self.resources.append(row)
        self.command("create-network", [
            "network", "create", "--internal", "--disable-dns",
            "--label", f"{OWNER_LABEL}={self.owner}", row["name"],
        ], mutation=row)
        self.bind(row)
        config = row["configuration"]
        if not config.get("internal") or config.get("dns_enabled") or config.get("driver") != "bridge":
            raise LabFailure("NetworkIsolationMismatch", "Dedicated network is not an internal DNS-free bridge")
        return row

    def build_image(self, base_image):
        row = {"kind": "image", "name": "localhost/" + self.owner + ":lab"}
        self.resources.append(row)
        base = self.json("base-image", [
            "image", "inspect", "--format",
            '{"id":{{json .ID}},"os":{{json .Os}},"arch":{{json .Architecture}}}',
            base_image,
        ])
        full_id(base["id"])
        self.receipt["base_image"] = base
        if base["os"] != "linux":
            raise LabFailure("CapabilityUnavailable", "The existing base image is not Linux")
        self.command("build-offline-image", [
            "build", "--pull=never", "--network=none", "--no-cache", "--force-rm",
            "--label", f"{OWNER_LABEL}={self.owner}", "--build-arg",
            "BASE_IMAGE=" + base["id"], "--tag", row["name"], str(HELPERS),
        ], seconds=45, mutation=row)
        self.bind(row)
        return row

    def create_container(self, name, image, network, role, upstream=None):
        row = {"kind": "container", "name": self.owner + "-" + name, "role": role}
        self.resources.append(row)
        args = container_arguments(self.owner, row["name"], image["id"], network["id"], role, upstream)
        self.command("create-" + name, args, mutation=row)
        self.bind(row)
        self.command("start-" + name, ["start", row["id"]])
        row["state"] = self.inspect(row, '{{json .State}}')
        row["network"] = self.inspect(row, '{{json .NetworkSettings}}')
        row["security"] = self.inspect(row, (
            '{"network_mode":{{json .HostConfig.NetworkMode}},'
            '"privileged":{{json .HostConfig.Privileged}},'
            '"cap_add":{{json .HostConfig.CapAdd}},'
            '"cap_drop":{{json .HostConfig.CapDrop}},'
            '"readonly":{{json .HostConfig.ReadonlyRootfs}},'
            '"security_opt":{{json .HostConfig.SecurityOpt}}}'
        ))
        # The first status can race key generation. Every retry is bounded and
        # retains its original nonzero output; it does not retry an experiment.
        until = min(self.runner.deadline, time.monotonic() + 10)
        while True:
            code, text = self.command("ready-" + name, [
                "exec", row["id"], "python3", CONTAINER_SCRIPT, "status", "--owner", self.owner,
            ], allowed=(0, 1))
            if code == 0:
                row["identity"] = json.loads(text)
                break
            if time.monotonic() >= until:
                raise LabFailure("ContainerNotReady", f"{role} did not become ready")
        namespace = row["identity"].get("network_namespace")
        if not re.fullmatch(r"net:\[\d+\]", namespace or ""):
            raise LabFailure("NamespaceUnknown", "Container did not attest a kernel network namespace")
        check_container_isolation(row, network)
        others = [r for r in self.resources if r is not row and "identity" in r]
        if any(r["identity"]["network_namespace"] == namespace for r in others):
            raise LabFailure("NamespaceNotIsolated", "Owned containers unexpectedly share a network namespace")
        self.command("processes-" + name, ["top", row["id"], "hpid", "pid", "ppid", "args"])
        return row

    def qdisc(self, row, expected=None):
        args = ["exec", row["id"], "python3", CONTAINER_SCRIPT, "qdisc", "--owner", self.owner]
        if expected is not None:
            args += ["--delay-ns", str(expected["delay_ns"]), "--loss-u32", str(expected["loss_u32"])]
        code, text = self.command("qdisc-" + row["role"], args, allowed=(0, 1))
        try:
            result = json.loads(text)
        except ValueError as error:
            raise LabFailure("QdiscReadbackUnknown", "Kernel helper did not return JSON") from error
        if code:
            failure = result.get("failure", {})
            raise LabFailure(failure.get("type", "QdiscReadbackUnknown"),
                             failure.get("message", "Kernel helper failed"))
        return result

    def cleanup(self):
        """Never select resources by a broad prefix or prune shared images."""
        rows = []
        for resource in reversed(self.resources):
            row = {"kind": resource["kind"], "name": resource["name"], "id": resource.get("id"),
                   "absent": False, "absence_observed": False}
            uncertain = resource.get("creation", {}).get("status") in ("PENDING", "UNKNOWN")
            rows.append(row)
            try:
                kind = resource["kind"]
                code, _ = self.command("cleanup-exists-" + kind, [
                    kind, "exists", resource.get("id", resource["name"]),
                ], seconds=4, allowed=(0, 1))
                if code == 1 and uncertain:
                    # This second read is useful evidence after the local
                    # cancellation barrier, but cannot settle daemon work.
                    code, _ = self.command("recheck-absent-" + kind, [
                        kind, "exists", resource["name"],
                    ], seconds=4, allowed=(0, 1))
                if code == 1:
                    row["absence_observed"] = True
                    row["absent"] = not uncertain
                    continue
                identifier = self.bind(resource)
                row["id"] = identifier
                if kind == "container":
                    self.command("final-processes", ["top", identifier, "hpid", "pid", "ppid", "args"],
                                 seconds=4, allowed=(0, 125))
                    self.command("container-logs", ["logs", identifier], seconds=4, allowed=(0, 125))
                    self.command("remove-container", ["rm", "--force", "--time=2", identifier], seconds=7)
                elif kind == "network":
                    # No force: another consumer must prevent deletion.
                    self.command("remove-network", ["network", "rm", identifier], seconds=5)
                else:
                    consumers = self.json("image-consumers", [
                        "ps", "--all", "--filter", "ancestor=" + identifier, "--format", "json",
                    ], seconds=4)
                    if consumers:
                        raise LabFailure("ImageHasConsumers", "Owned image still has container consumers")
                    self.command("remove-image", ["image", "rm", identifier], seconds=5)
                code, _ = self.command("verify-absent-" + kind, [
                    kind, "exists", identifier,
                ], seconds=4, allowed=(0, 1))
                name_code, _ = self.command("verify-name-absent-" + kind, [
                    kind, "exists", resource["name"],
                ], seconds=4, allowed=(0, 1))
                row["absence_observed"] = code == name_code == 1
                row["absent"] = row["absence_observed"] and not uncertain
            except Exception as error:
                row["failure"] = error.value() if isinstance(error, LabFailure) else {
                    "type": type(error).__name__, "message": str(error),
                }
            finally:
                if uncertain:
                    row["mutation_completion"] = "UNKNOWN"
                    row["absent"] = False
                    row.setdefault("failure", {
                        "type": "MutationCompletionUnknown",
                        "message": "Podman creation completion is unknown; absence reads cannot cancel daemon work",
                    })
        return rows


def gate_endpoint(container):
    bindings = container["network"].get("Ports", {}).get("8080/tcp")
    if not isinstance(bindings, list) or len(bindings) != 1 or bindings[0].get("HostIp") != "127.0.0.1":
        raise LabFailure("PublicationNotLocalhost", "Gate publication is not exactly 127.0.0.1")
    port = int(bindings[0]["HostPort"])
    if not 1 <= port <= 65535:
        raise LabFailure("PublicationInvalid", "Invalid gate port")
    return port


def run_test(api, binary, environment, test, label, seconds):
    code, log = api.command(label, [
        str(binary), "--ignored", "--exact", test, "--nocapture", "--test-threads=1",
    ], seconds=seconds, allowed=(0, 101), environment=environment, binary=True)
    if code or not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", log):
        raise LabFailure("InteropTestFailed", f"{test} failed or did not execute exactly one test")


def run_scenario(api, name, image, network, upstream, environment, args):
    destination = api.runner.output / name
    destination.mkdir(mode=0o700)
    condition = conditions(name, args.delay_ns, args.loss_u32)
    write_json(destination / "conditions.json", condition)
    row = {"scenario": name, "status": "FAILED", "conditions": condition,
           "tests": [], "kernel_counter_deltas": {}, "sample_count": 0}
    api.receipt["scenarios"].append(row)
    gates = {}
    before = {}
    try:
        for side in ("A", "B"):
            gate = api.create_container(name + "-" + side, image, network, side, upstream)
            gates[side] = gate
            snapshot = api.qdisc(gate, condition[side])
            write_json(destination / f"{side}-qdisc-before.json", snapshot)
            before[side] = check_qdisc(snapshot, condition[side], api.owner,
                                       gate["identity"]["network_namespace"])
        environment = dict(environment, **{
            "KEELSHELL_PATH_DIRECT_PROXY_PORT": str(gate_endpoint(gates["A"])),
            "KEELSHELL_PATH_RELAY_PROXY_PORT": str(gate_endpoint(gates["B"])),
            "KEELSHELL_PATH_REPORT_DIR": str(destination), "KEELSHELL_PATH_SCENARIO": name,
        })
        row["gate_endpoints"] = {s: "127.0.0.1:" + environment[
            "KEELSHELL_PATH_DIRECT_PROXY_PORT" if s == "A" else "KEELSHELL_PATH_RELAY_PROXY_PORT"
        ] for s in ("A", "B")}
        test_error = None
        try:
            run_test(api, args.test_binary, environment, stream_lab.SAMPLE_TEST, name + "-samples", 72)
            row["tests"].append({"name": stream_lab.SAMPLE_TEST, "status": "PASSED"})
        except LabFailure as error:
            test_error = error
            row["tests"].append({"name": stream_lab.SAMPLE_TEST, "status": "FAILED",
                                 "failure": error.value()})
        finally:
            # Preserve kernel counts even for a failed or partial Rust test.
            # A failure reading one side must not skip the other side's readback.
            snapshot_errors = []
            for side, gate in gates.items():
                try:
                    snapshot = api.qdisc(gate)
                    write_json(destination / f"{side}-qdisc-after.json", snapshot)
                    after = check_qdisc(snapshot, condition[side], api.owner,
                                        gate["identity"]["network_namespace"])
                    row["kernel_counter_deltas"][side] = {
                        field: after["counters"][field] - before[side]["counters"][field]
                        for field in ("bytes", "packets", "drops", "requeues", "overlimits")
                    }
                    check_counters(before[side], after, condition[side])
                except (LabFailure, interop.InteropFailure) as error:
                    snapshot_errors.append(error)
            if test_error is None and snapshot_errors:
                test_error = snapshot_errors[0]
            row["qdisc_readback_failures"] = [
                e.value() if isinstance(e, LabFailure) else {"type": type(e).__name__, "message": str(e)}
                for e in snapshot_errors
            ]
        report_path = destination / "report.json"
        if report_path.is_file():
            report = json.loads(report_path.read_text(encoding="utf-8"))
            row["sample_count"] = len(report.get("samples", []))
            row["report_status"] = report.get("status")
            row["report_cleanup_unconfirmed"] = report.get("cleanup_unconfirmed")
        else:
            raise test_error or LabFailure("ReportMissing", "The comparison produced no raw report")
        if test_error is not None:
            raise test_error
        if report.get("scenario") != name:
            raise LabFailure("ReportInvalid", "The raw report belongs to another scenario")
        observed = validate_report(report)
        if name == "baseline":
            for index, test in enumerate(stream_lab.EXTRA_TESTS):
                run_test(api, args.test_binary, environment, test, f"{name}-extra-{index}", 18)
                row["tests"].append({"name": test, "status": "PASSED"})
        row.update(status="MEASURED", observed_difference=observed)
    except (LabFailure, interop.InteropFailure, OSError, ValueError, TypeError) as error:
        row["failure"] = error.value() if isinstance(error, LabFailure) else {
            "type": type(error).__name__, "message": str(error),
        }
    write_json(destination / "assessment.json", row)
    return row


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, help="New evidence directory; never reused")
    parser.add_argument("--test-binary", required=True, type=Path, help="Parent-verified ssh_path_interop")
    parser.add_argument("--test-binary-sha256", required=True, help="Expected digest from parent provenance")
    parser.add_argument("--base-image", required=True, help="Existing Linux image with Python/sshd/useradd")
    parser.add_argument("--scenario", choices=("all", *SCENARIOS), default="all")
    parser.add_argument("--delay-ms", type=float, default=20)
    parser.add_argument("--loss-percent", default="1")
    parser.add_argument("--timeout", type=float, default=600, help="Total including cleanup, 120..600 seconds")
    args = parser.parse_args(argv)
    if platform.system() not in ("Darwin", "Linux"):
        parser.error("The Podman lab supports macOS and Linux")
    if not math.isfinite(args.timeout) or not 120 <= args.timeout <= 600:
        parser.error("--timeout must be finite, 120..600 seconds")
    if not math.isfinite(args.delay_ms) or not 0 <= args.delay_ms <= 1000:
        parser.error("--delay-ms must be finite, 0..1000")
    try:
        args.loss_u32 = loss_threshold(args.loss_percent)
    except ValueError as error:
        parser.error(str(error))
    if not args.loss_u32:
        parser.error("--loss-percent is too small for the kernel probability")
    args.delay_ns = round(args.delay_ms * 1_000_000)
    if not re.fullmatch(r"[0-9a-f]{64}", args.test_binary_sha256):
        parser.error("--test-binary-sha256 must be a complete lowercase SHA256")
    if not args.test_binary.is_file() or not os.access(args.test_binary, os.X_OK):
        parser.error("--test-binary must be an existing executable")
    args.test_binary = args.test_binary.resolve()
    if args.base_image.startswith("-") or any(c.isspace() for c in args.base_image):
        parser.error("Invalid base image reference")
    started = time.monotonic()
    deadline = started + args.timeout
    previous_mask = os.umask(0o077)
    try:
        output = interop.create_output(args.output)
    finally:
        os.umask(previous_mask)
    owner = "keelshell-netem-" + uuid.uuid4().hex[:16]
    receipt = {
        "schema": 1, "owner": owner, "status": "FAILED", "started_utc": interop.utc_now(),
        "commands": [], "processes": [], "steps": [], "resources": [], "scenarios": [],
        "cleanup": {}, "external_network_probe_seconds": 0,
        "test_binary": {"path": str(args.test_binary), "expected_sha256": args.test_binary_sha256},
        "boundary": "Owned Linux IP-layer experiment and prebuilt transport test; no native UI, Internet or production benefit acceptance",
        "timeout_seconds": args.timeout,
    }
    old_handlers = {}
    requested_signal = None
    cleaning = False
    scratch = None
    runner = None
    api = None
    result = 1

    def interrupt(signum, _):
        nonlocal requested_signal
        requested_signal = signum

    def check_interrupt():
        if requested_signal is not None and not cleaning:
            raise interop.Interrupted(f"Interrupted by signal {requested_signal}")

    print(f"Real netem receipt: {output}", flush=True)
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            old_handlers[sig] = signal.signal(sig, interrupt)
        with ExitStack() as stack:
            runner = stream_lab.LabRunner(output, deadline - CLEANUP_SECONDS, stack, receipt, check_interrupt)
            try:
                actual_hash = stream_lab.digest(args.test_binary)
                receipt["test_binary"]["sha256"] = actual_hash
                if actual_hash != args.test_binary_sha256:
                    raise LabFailure("PrebuiltDigestMismatch", "The supplied binary disagrees with parent provenance")
                receipt["source_sha256"] = {
                    str(path.relative_to(ROOT)): stream_lab.digest(path)
                    for path in [
                        Path(__file__).resolve(), ROOT / "scripts/ssh_path_lab.py",
                        ROOT / "scripts/openssh_interop.py",
                        ROOT / "crates/keelshell-session/tests/ssh_path_interop.rs",
                        ROOT / "scripts/test_ssh_path_netem_lab.py",
                        *sorted(HELPERS.glob("*.py")), HELPERS / "Containerfile",
                    ]
                }
                podman = interop.executable("podman")
                api = PodmanLab(runner, owner, receipt, podman)
                # Inventory precedes any container/image/network/key creation.
                code, inventory = api.command("test-inventory", [
                    str(args.test_binary), "--list", "--ignored",
                ], seconds=5, binary=True)
                del code
                required = [stream_lab.SAMPLE_TEST, *stream_lab.EXTRA_TESTS]
                if any(inventory.splitlines().count(test + ": test") != 1 for test in required):
                    raise LabFailure("PrebuiltInventoryMismatch", "The binary is missing exact ssh_path_interop test inventory")
                receipt["test_binary"]["inventory"] = required
                api.json("podman-version", ["version", "--format", "json"])
                image = api.build_image(args.base_image)
                network = api.create_network()
                servers = api.create_container("servers", image, network, "servers")
                addresses = servers["network"].get("Networks", {})
                if len(addresses) != 1:
                    raise LabFailure("NetworkIsolationMismatch", "SSH services have unexpected attached networks")
                upstream = next(iter(addresses.values())).get("IPAddress")
                if not upstream:
                    raise LabFailure("ContainerAddressUnknown", "Owned SSH services have no bridge address")
                private_parent = ROOT / "work"
                private_parent.mkdir(exist_ok=True)
                scratch = Path(tempfile.mkdtemp(prefix=".ssh-netem-private-", dir=private_parent)).resolve()
                scratch.chmod(0o700)
                (scratch / "tmp").mkdir(mode=0o700)
                receipt["private_directory"] = str(scratch)
                environment = {
                    "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
                    "HOME": str(scratch), "TMPDIR": str(scratch / "tmp"), "LANG": "C",
                    "KEELSHELL_PATH_OPT_IN": "localhost-owned-fixture", "KEELSHELL_PATH_USER": "netem",
                    "KEELSHELL_PATH_TARGET_PORT": "2222", "KEELSHELL_PATH_JUMP_PORT": "2223",
                }
                identities = {}
                for role in ("target", "jump"):
                    directory = scratch / role
                    directory.mkdir(mode=0o700)
                    for filename in ("host.pub", "client"):
                        path = directory / filename
                        api.command("export-" + role + "-" + filename, [
                            "cp", servers["id"] + ":/run/netem/" + role + "/" + filename, str(path),
                        ])
                        path.chmod(0o600)
                    for suffix, filename in (("HOST_KEY", "host.pub"), ("CLIENT_KEY", "client")):
                        environment[f"KEELSHELL_PATH_{role.upper()}_{suffix}"] = str(directory / filename)
                    # Public fingerprints and configurations, never private key bytes.
                    _, fingerprint = api.command("public-pin-" + role, [
                        "exec", servers["id"], "ssh-keygen", "-lf", "/run/netem/" + role + "/host.pub",
                    ])
                    identities[role] = {"user": "netem", "host": "127.0.0.1",
                                        "port": 2222 if role == "target" else 2223,
                                        "fingerprint": fingerprint.strip()}
                    _, config = api.command("sshd-config-" + role, [
                        "exec", servers["id"], "cat", "/run/netem/" + role + "/sshd_config",
                    ])
                    (output / f"{role}-sshd_config.txt").write_text(config, encoding="utf-8")
                receipt["identities"] = identities
                selected = SCENARIOS if args.scenario == "all" else (args.scenario,)
                for name in selected:
                    check_interrupt()
                    print(f"Running {name}: kernel netem", flush=True)
                    run_scenario(api, name, image, network, upstream, environment, args)
                if all(row["status"] == "MEASURED" for row in receipt["scenarios"]):
                    receipt["status"] = "MEASURED"
                    result = 0
            finally:
                cleaning = True
                for sig in old_handlers:
                    signal.signal(sig, signal.SIG_IGN)
                runner.deadline = deadline - 8
                if api is not None:
                    # Stop all local fork sources before checking resources.
                    # Unknown daemon mutations still remain UNKNOWN afterwards.
                    creators = stream_lab.shutdown(runner, runner.deadline)
                    receipt["cleanup"]["creator_processes"] = creators
                    receipt["cleanup"]["creator_barrier_confirmed"] = (
                        len(creators) == len(runner.processes)
                        and all(p.get("reaped") and p.get("observed_stopped") for p in creators)
                        and not runner.ancestry_unverified
                    )
                    receipt["cleanup"]["resources"] = api.cleanup()
                runner.deadline = deadline
                try:
                    receipt["cleanup"]["processes"] = stream_lab.shutdown(runner, deadline)
                    receipt["cleanup"]["ancestry_unverified"] = sorted(runner.ancestry_unverified)
                finally:
                    if scratch is not None:
                        try:
                            shutil.rmtree(scratch)
                        except OSError as error:
                            receipt["cleanup"]["private_removal_failure"] = str(error)
                    receipt["cleanup"]["private_directory_removed"] = scratch is None or not scratch.exists()
    except Exception as error:
        receipt["failure"] = error.value() if isinstance(error, LabFailure) else {
            "type": type(error).__name__, "message": str(error),
        }
        result = 130 if isinstance(error, interop.Interrupted) else 1
    finally:
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
        cleanup = receipt["cleanup"]
        cleanup["confirmed"] = (
            len(cleanup.get("resources", [])) == len(receipt["resources"])
            and all(r["absent"] for r in cleanup.get("resources", []))
            and bool(cleanup.get("private_directory_removed"))
            and not cleanup.get("ancestry_unverified", [])
            and cleanup.get("creator_barrier_confirmed", True)
            and len(cleanup.get("processes", [])) == len(receipt["processes"])
            and all(p.get("reaped") and p.get("observed_stopped") for p in cleanup.get("processes", []))
        )
        if not cleanup["confirmed"]:
            receipt["failure_cleanup"] = {"type": "CleanupUnconfirmed",
                                          "message": "Owned resources/processes/private data were not all confirmed absent"}
            result = 1
        # Bind successful observations to the bytes actually used at entry.
        # A parent can keep the frozen prebuilt copy stable independently of
        # any source changes in its production checkout.
        try:
            receipt["test_binary"]["sha256_after"] = stream_lab.digest(args.test_binary)
            receipt["source_sha256_after"] = {
                name: stream_lab.digest(ROOT / name) for name in receipt.get("source_sha256", {})
            }
            receipt["input_bytes_unchanged"] = (
                receipt["test_binary"]["sha256_after"] == receipt["test_binary"].get("sha256")
                and receipt["source_sha256_after"] == receipt.get("source_sha256", {})
            )
        except OSError as error:
            receipt["input_readback_failure"] = {"type": type(error).__name__, "message": str(error)}
            receipt["input_bytes_unchanged"] = False
        if not receipt["input_bytes_unchanged"]:
            receipt["failure_input"] = {"type": "InputDrift", "message": "Input byte readback did not match"}
            result = 1
        if result:
            receipt["status"] = "INTERRUPTED" if result == 130 else "FAILED"
        # A scenario's raw samples survive failure. The top-level result only
        # admits differences after successful cleanup of the whole experiment.
        receipt["observations_admissible"] = result == 0
        receipt.update(returncode=result, finished_utc=interop.utc_now(),
                       elapsed_seconds=round(time.monotonic() - started, 3))
        receipt["logs"] = {
            p.name: {"bytes": p.stat().st_size, "sha256": stream_lab.digest(p)}
            for p in sorted(output.glob("*.log"))
        }
        write_json(output / "result.json", receipt)
        print(f"Real netem lab: {receipt['status']}; exit {result}; {output}", flush=True)
    return result


if __name__ == "__main__":
    sys.exit(main())
