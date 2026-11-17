#!/usr/bin/env python3
"""Real Windows collector/API lab with separately reported isolated SSH HOLD.

Only the new lab's unittest suite is suitable for offline execution. Running
this CLI on another OS records HOLD; it never substitutes simulated Windows.
"""

import argparse
import base64
import ctypes as C
from dataclasses import dataclass
import hashlib
import ipaddress
import json
import math
import os
from pathlib import Path
import platform
import re
import socket
import struct
import subprocess
import sys
import tempfile
import time
import unicodedata
import uuid

import windows_native_crash as lifecycle


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = Path(__file__).resolve().parent / "windows_monitor"
COLLECTORS = ROOT / "crates/keelshell-session/src/monitor/windows"
MAX_FRAME = 2 * 1024 * 1024
MAX_COMMAND = 8000
HOLD_EXIT = 78
OPERATIONS = ("probe", "resources", "processes", "sockets")
FAILURES = {"command_missing", "permission_denied", "command_failed", "unsupported_options"}
FRAME_KEYS = {"version", "operation", "platform", "runtime", "runtime_version",
              "units", "status", "data"}
POWERSHELL = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand "
ASSEMBLY = {
    "probe": ("$op='probe';$skip=$false;\n", "probe.ps1"),
    "resources": ("$op='resources';$skip=$false;\n", "resources.ps1"),
    "resources-skipped": ("$op='resources';$skip=$true;\n", "resources.ps1"),
    "processes": ("$op='processes';$skip=$false;\n", "processes.ps1"),
    "sockets": ("$op='sockets';$skip=$false;\n", "sockets.ps1"),
}
SUFFIX = "}catch{F (E $_) $null 1}\n"
U32, U64 = C.c_uint32, C.c_uint64


class LabError(Exception):
    """Required evidence is absent, invalid or contradictory."""


class Hold(LabError):
    """An unavailable or unsafe prerequisite cannot sign acceptance."""


class Budget:
    """An absolute monotonic budget includes cleanup; it is never renewed."""

    def __init__(self, seconds, end=None):
        if not 0 < seconds <= 600:
            raise ValueError("lab budget must be in (0, 600]")
        self.end = min(time.monotonic() + seconds, end if end is not None else math.inf)

    def remaining(self, reserve=0):
        value = self.end - time.monotonic() - reserve
        if value <= 0:
            raise TimeoutError("original lab budget exhausted")
        return value


def strict_json(text):
    """Reject duplicate object keys and non-JSON nonfinite numbers."""
    def pairs(items):
        value = {}
        for key, item in items:
            if key in value:
                raise LabError("duplicate JSON key")
            value[key] = item
        return value

    def bad_constant(_):
        raise LabError("nonfinite JSON constant")

    try:
        return json.loads(text, object_pairs_hook=pairs, parse_constant=bad_constant)
    except (ValueError, UnicodeError, RecursionError) as error:
        raise LabError("invalid JSON") from error


def keys(value, expected):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise LabError("object fields differ from contract")


def integer(value, maximum, minimum=0):
    if type(value) is not int or not minimum <= value <= maximum:
        raise LabError("integer outside contract")
    return value


def decimal(value, maximum, minimum=0):
    if not isinstance(value, str) or not re.fullmatch(r"0|[1-9][0-9]{0,19}", value):
        raise LabError("noncanonical decimal")
    return integer(int(value), maximum, minimum)


def finite(value, minimum=0, maximum=None):
    if type(value) not in (int, float) or not math.isfinite(value) or value < minimum:
        raise LabError("invalid finite number")
    if maximum is not None and value > maximum:
        raise LabError("number outside contract")
    return value


def display(value, limit=512):
    if (not isinstance(value, str) or not value or len(value) > limit
            or any(unicodedata.category(char) in {"Cc", "Cf", "Cs"} for char in value)):
        raise LabError("invalid display text")
    return value


def numeric_address(value):
    display(value, 128)
    address, separator, scope = value.partition("%")
    try:
        parsed = ipaddress.ip_address(address)
    except ValueError as error:
        raise LabError("invalid numeric IP address") from error
    if separator and (parsed.version != 6 or not re.fullmatch(r"[1-9][0-9]{0,9}", scope)
                      or int(scope) > 0xffffffff):
        raise LabError("invalid IPv6 scope")
    return str(parsed) + ("%" + scope if separator else "")


def socket_key(row):
    return row["protocol"], numeric_address(row["address"]), int(row["port"]), int(row["pid"])


def validate_data(operation, data):
    if operation == "probe":
        keys(data, {"os"})
        if data["os"] != "Win32NT":
            raise LabError("probe did not identify Windows")
    elif operation == "processes":
        keys(data, {"count", "processes"})
        rows = data["processes"]
        if not isinstance(rows, list) or integer(data["count"], 20000) != len(rows):
            raise LabError("process count mismatch")
        previous = -1
        for row in rows:
            keys(row, {"pid", "name", "working_set_bytes"})
            pid = decimal(row["pid"], 0xffffffff)
            if pid <= previous:
                raise LabError("process PIDs are duplicated or unordered")
            previous = pid
            display(row["name"])
            if row["working_set_bytes"] is not None:
                decimal(row["working_set_bytes"], 0xffffffffffffffff)
    elif operation == "sockets":
        keys(data, {"count", "sockets"})
        rows = data["sockets"]
        if not isinstance(rows, list) or integer(data["count"], 20000) != len(rows):
            raise LabError("socket count mismatch")
        seen = set()
        for row in rows:
            keys(row, {"protocol", "address", "port", "pid"})
            if row["protocol"] not in ("tcp", "udp"):
                raise LabError("unknown socket protocol")
            numeric_address(row["address"])
            decimal(row["port"], 65535, 1)
            decimal(row["pid"], 0xffffffff)
            identity = socket_key(row)
            if identity in seen:
                raise LabError("duplicate socket row")
            seen.add(identity)
    elif operation == "resources":
        keys(data, {"boot_time", "uptime_seconds", "cpu_percent", "memory_total_bytes",
                    "memory_free_bytes", "network_count", "networks", "capacity_status",
                    "filesystem_count", "filesystems"})
        decimal(data["boot_time"], 0xffffffffffffffff)
        finite(data["uptime_seconds"])
        if data["cpu_percent"] is not None:
            finite(data["cpu_percent"], maximum=100)
        total = decimal(data["memory_total_bytes"], 0xffffffffffffffff, 1)
        if decimal(data["memory_free_bytes"], 0xffffffffffffffff) > total:
            raise LabError("free memory exceeds total memory")
        rows = data["networks"]
        if not isinstance(rows, list) or integer(data["network_count"], 256) != len(rows):
            raise LabError("network count mismatch")
        seen = set()
        for row in rows:
            keys(row, {"name", "received_bytes", "transmitted_bytes"})
            name = display(row["name"])
            if name in seen:
                raise LabError("duplicate network")
            seen.add(name)
            decimal(row["received_bytes"], 0xffffffffffffffff)
            decimal(row["transmitted_bytes"], 0xffffffffffffffff)
        status = data["capacity_status"]
        if status not in {"ok", "skipped"} | FAILURES:
            raise LabError("unknown capacity state")
        rows = data["filesystems"]
        if not isinstance(rows, list) or integer(data["filesystem_count"], 512) != len(rows):
            raise LabError("filesystem count mismatch")
        if status != "ok" and rows:
            raise LabError("failed or skipped capacity has rows")
        seen = set()
        for row in rows:
            keys(row, {"source", "mount", "total_bytes", "total_free_bytes", "available_bytes"})
            identity = display(row["source"]), display(row["mount"])
            if identity in seen:
                raise LabError("duplicate filesystem")
            seen.add(identity)
            total = decimal(row["total_bytes"], 0xffffffffffffffff, 1)
            free = decimal(row["total_free_bytes"], 0xffffffffffffffff)
            available = decimal(row["available_bytes"], 0xffffffffffffffff)
            if not available <= free <= total:
                raise LabError("invalid filesystem byte relationships")
    else:
        raise LabError("unknown operation")


def parse_frame(raw, stderr, exit_code, operation):
    """Accept only one exact successful production frame and an actual exit."""
    if len(raw) + len(stderr) > MAX_FRAME or stderr:
        raise LabError("output over budget or stderr present")
    if exit_code is None or type(exit_code) is not int:
        raise LabError("absent process exit")
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError as error:
        raise LabError("non-UTF8 frame") from error
    if not text.startswith("{") or not text.endswith(("}", "}\n", "}\r\n")):
        raise LabError("incomplete frame or unrelated text")
    # json.loads permits leading/trailing whitespace; the wire contract does not.
    body = text[:-2] if text.endswith("\r\n") else text[:-1] if text.endswith("\n") else text
    if not body.endswith("}"):
        raise LabError("trailing frame text")
    frame = strict_json(body)
    keys(frame, FRAME_KEYS)
    if (type(frame["version"]) is not int or frame["version"] != 1
            or frame["operation"] != operation or operation not in OPERATIONS
            or frame["platform"] != "Windows" or frame["runtime"] != "WindowsPowerShell"
            or frame["units"] != "bytes seconds percent"):
        raise LabError("invalid frame identity or units")
    version = frame["runtime_version"]
    if not isinstance(version, str) or not re.fullmatch(r"5\.1\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise LabError("unsupported runtime version")
    for part in version.split("."):
        integer(int(part), 0xffffffff)
    if frame["status"] in FAILURES:
        if frame["data"] is not None or exit_code == 0:
            raise LabError("invalid typed failure envelope")
        raise LabError("collector typed failure: " + frame["status"])
    if frame["status"] != "ok" or exit_code != 0:
        raise LabError("collector did not complete successfully")
    validate_data(operation, frame["data"])
    return frame


def assemble(operation, source=COLLECTORS):
    """Use precisely the literals and byte concatenation in production ASSEMBLY.md."""
    if operation not in ASSEMBLY:
        raise LabError("unknown production operation")
    prefix, body = ASSEMBLY[operation]
    parts = ("common.ps1", body)
    sources, texts = [], []
    for name in parts:
        path = Path(source) / name
        if path.is_symlink() or not path.is_file():
            raise LabError("production script is missing or symlinked")
        raw = path.read_bytes()
        if len(raw) > 64 * 1024:
            raise LabError("production script over budget")
        texts.append(raw.decode("utf-8", errors="strict"))
        sources.append({"file": name, "sha256": hashlib.sha256(raw).hexdigest()})
    script = prefix + "".join(texts) + SUFFIX
    encoded = base64.b64encode(script.encode("utf-16le")).decode("ascii")
    command = POWERSHELL + encoded
    length = len(command.encode("ascii"))
    if length >= MAX_COMMAND:
        raise LabError("production command reaches cmd.exe limit")
    return command, {"operation": operation, "parts": sources, "join": "verbatim",
                     "literal_prefix": prefix, "literal_suffix": SUFFIX,
                     "script_sha256": hashlib.sha256(script.encode("utf-8")).hexdigest(),
                     "wire_command_sha256": hashlib.sha256(command.encode("ascii")).hexdigest(),
                     "wire_command_ascii_bytes": length}


class OwnedChild:
    """Retain raw pipes and reuse the existing original-object terminal barrier."""

    def __init__(self, command, budget, owner=None, required_image=None):
        self.command, self.budget = list(command), budget
        self.owner = owner if owner is not None else lifecycle.owner_for_host()
        self.process = None
        self.pumps, self.capture = [], {}
        self.stdout, self.stderr = bytearray(), bytearray()
        self.errors = set()
        self.observed = None
        self.receipt = None
        self.required_image = required_image
        self.required_processes = {}
        self.image_checked = set()
        self.image_query = None
        if required_image is not None:
            if os.name != "nt" or not isinstance(self.owner, lifecycle.WindowsJob):
                self.owner.close()
                raise Hold("native_windows_job_required_for_shell_process_proof")
            from ctypes import wintypes as W
            self.image_query = self.owner.api.QueryFullProcessImageNameW
            self.image_query.argtypes = [W.HANDLE, U32, W.LPWSTR, C.POINTER(U32)]
            self.image_query.restype = W.BOOL

    def start(self, environment=None):
        try:
            environment = dict(os.environ) if environment is None else dict(environment)
            self.process = self.owner.spawn(self.command, ROOT, environment, self.budget)
            for stream, destination in ((self.process.stdout, self.stdout),
                                        (self.process.stderr, self.stderr)):
                pump = lifecycle.Pump(stream)
                self.pumps.append(pump)

                def retain(data, destination=destination):
                    available = MAX_FRAME - len(self.stdout) - len(self.stderr)
                    destination.extend(data[:max(0, available)])
                    if len(data) > available:
                        self.errors.add("output_budget_exceeded")

                self.capture[pump] = retain
            return self
        except BaseException:
            self.process = self.process if self.process is not None else self.owner.process
            self.close()
            raise

    def drain(self):
        self.budget.remaining(reserve=lifecycle.CLEANUP_RESERVE)
        if self.required_image is not None:
            # Retain during execution, before a fast cmd child can disappear.
            # Missing a child is an evidence failure, never a terminal inference.
            self.owner.retain_current_members(self.budget)
            for pid, handle in self.owner.terminal_handles.items():
                if pid in self.image_checked:
                    continue
                name = C.create_unicode_buffer(32768)
                length = U32(len(name))
                if not self.image_query(handle, 0, name, C.byref(length)):
                    raise LabError("owned shell descendant image query failed")
                self.image_checked.add(pid)
                if os.path.normcase(name.value) == os.path.normcase(self.required_image):
                    if self.owner.api.WaitForSingleObject(handle, 0) != 258:
                        raise LabError("PowerShell original object was not retained while live")
                    self.required_processes[pid] = handle
        for pump in self.pumps:
            pump.drain(self.capture[pump], self.budget)
            if pump.overflow.is_set() or pump.failed.is_set():
                self.errors.add("pipe_failed")
        if self.errors:
            raise LabError("owned pipe capture failed")
        if self.process is not None:
            self.observed = self.process.poll()

    def wait(self):
        while True:
            self.drain()
            if self.observed is not None and all(pump.complete() for pump in self.pumps):
                return self.observed
            time.sleep(min(0.01, self.budget.remaining(reserve=lifecycle.CLEANUP_RESERVE)))

    def ready(self):
        while True:
            self.drain()
            if b"\n" in self.stdout:
                line, remainder = bytes(self.stdout).split(b"\n", 1)
                if remainder.strip() or self.stderr:
                    raise LabError("unexpected fixture output")
                value = strict_json(line.decode("utf-8"))
                validate_fixture(value, self.process.pid)
                if self.observed is not None:
                    raise LabError("fixture exited before readiness")
                return value
            if self.observed is not None:
                raise LabError("fixture exited without readiness")
            time.sleep(min(0.01, self.budget.remaining(reserve=lifecycle.CLEANUP_RESERVE)))

    def close(self):
        with lifecycle.deferred_interrupts():
            return self._finish()

    def _finish(self):
        if self.receipt is not None:
            return self.receipt
        terminal = None
        if self.process is not None:
            try:
                observed = self.process.poll()
                if observed is not None:
                    self.observed = observed
            except (OSError, lifecycle.OwnershipError):
                self.errors.add("exit_query_failed")
            powershell_objects = []
            if self.required_image is not None:
                for pid, handle in self.required_processes.items():
                    status = self.owner.api.WaitForSingleObject(handle, 0)
                    exit_code = lifecycle.wintypes.DWORD()
                    queried = status == 0 and bool(self.owner.api.GetExitCodeProcess(handle, C.byref(exit_code)))
                    powershell_objects.append({"pid": pid, "retained_while_live": True,
                                               "natural_terminal_before_cleanup": status == 0,
                                               "exit_code": exit_code.value if queried else None})
                # A successful shell run must observe every admitted original
                # process object and exactly one production PowerShell process.
                # An unobserved short-lived descendant cannot disappear from
                # this proof merely because current Job membership became empty.
                try:
                    total = self.owner.accounting(self.budget).TotalProcesses
                    census_complete = total == len(self.owner.terminal_handles)
                except (OSError, lifecycle.OwnershipError, TimeoutError):
                    total, census_complete = None, False
                if (len(powershell_objects) != 1 or not census_complete
                        or not all(row["natural_terminal_before_cleanup"] and row["exit_code"] == 0
                                   for row in powershell_objects)):
                    self.errors.add("powershell_natural_terminal_unproven")
            terminal, errors = lifecycle.finish_owned(
                self.owner, self.process, self.pumps, self.budget, self.capture)
            self.errors.update(errors)
        else:
            self.owner.close()
            self.errors.add("process_not_started")
        if terminal is None or terminal["errors"] or not terminal["direct_reaped"] \
                or terminal["active_processes"] != 0 or not terminal["pipes_drained"]:
            self.errors.add("terminal_unproven")
        self.receipt = {"pid": self.process.pid if self.process is not None else None,
                        "observed_exit": self.observed, "terminal": terminal,
                        "errors": sorted(self.errors), "stdout_bytes": len(self.stdout),
                        "stderr_bytes": len(self.stderr),
                        "command_chars": len(subprocess.list2cmdline(self.command)),
                        "command_sha256": hashlib.sha256(
                            subprocess.list2cmdline(self.command).encode("utf-8")).hexdigest()}
        if self.required_image is not None:
            self.receipt["shell_execution"] = {
                "shell": self.command[0], "argv": self.command,
                "serialized_command_line": subprocess.list2cmdline(self.command),
                "production_command_ascii_bytes": len(self.command[-1].encode("ascii")),
                "required_image": self.required_image,
                "job_total_processes": total if self.process is not None else None,
                "complete_original_object_census": census_complete if self.process is not None else False,
                "powershell_original_objects": powershell_objects if self.process is not None else [],
            }
        return self.receipt

    def stop_fixture(self):
        if self.process is not None and self.process.poll() is None:
            self.process.stdin.write(b"stop\n")
            self.process.stdin.flush()
            self.process.stdin.close()
        return self.wait()


def validate_fixture(value, pid):
    keys(value, {"version", "pid", "endpoints", "connected_only"})
    if value["version"] != 1 or integer(value["pid"], 0xffffffff, 1) != pid:
        raise LabError("fixture identity differs from original child")
    if (not isinstance(value["endpoints"], list) or len(value["endpoints"]) != 4
            or not isinstance(value["connected_only"], list) or len(value["connected_only"]) != 2):
        raise LabError("incomplete dual-stack fixture")
    all_rows = value["endpoints"] + value["connected_only"]
    validate_data("sockets", {"count": len(all_rows), "sockets": all_rows})
    if any(int(row["pid"]) != pid or row["address"] not in ("127.0.0.1", "::1") for row in all_rows):
        raise LabError("fixture endpoint escaped owned loopback identity")
    expected = {(protocol, address) for protocol in ("tcp", "udp") for address in ("127.0.0.1", "::1")}
    if {(row["protocol"], row["address"]) for row in value["endpoints"]} != expected:
        raise LabError("fixture is missing an IP family or protocol")
    if {(row["protocol"], row["address"]) for row in value["connected_only"]} != {
            ("tcp", "127.0.0.1"), ("tcp", "::1")}:
        raise LabError("fixture is missing connected TCP negative controls")


def terminal_ok(receipt):
    terminal = receipt.get("terminal")
    return (not receipt.get("errors") and terminal is not None
            and terminal.get("direct_reaped") is True
            and terminal.get("active_processes") == 0
            and terminal.get("pipes_drained") is True and not terminal.get("errors"))


@dataclass
class CommandResult:
    stdout: bytes
    stderr: bytes
    exit_code: int | None
    receipt: dict


class Evidence:
    """Publish only into a new output directory; private SSH keys live elsewhere."""

    def __init__(self, output, seconds=240):
        self.path = Path(output)
        self.path.mkdir(parents=True, exist_ok=False)
        self.budget = Budget(seconds)
        self.children = []

    def write_json(self, name, value):
        self.write_bytes(name, (json.dumps(value, ensure_ascii=False, indent=2,
                                          allow_nan=False) + "\n").encode("utf-8"))

    def write_bytes(self, name, value):
        if not re.fullmatch(r"[a-z0-9_.-]+", name):
            raise LabError("invalid evidence filename")
        with (self.path / name).open("xb") as stream:
            stream.write(value)

    def keep_child(self, label, child):
        receipt = child.close()
        self.write_bytes(label + ".stdout", bytes(child.stdout))
        self.write_bytes(label + ".stderr", bytes(child.stderr))
        self.write_json(label + ".receipt.json", receipt)
        self.children.append({"label": label, **receipt})
        return receipt

    def run(self, label, command, seconds=30, required_image=None):
        child = OwnedChild(command, Budget(seconds, self.budget.end), required_image=required_image)
        try:
            child.start()
            child.wait()
        finally:
            receipt = self.keep_child(label, child)
        if not terminal_ok(receipt):
            raise LabError("child terminal proof failed: " + label)
        return CommandResult(bytes(child.stdout), bytes(child.stderr), receipt["observed_exit"], receipt)


class MemoryStatus(C.Structure):
    _fields_ = [("length", U32), ("load", U32), ("total", U64), ("free", U64),
                ("total_page", U64), ("free_page", U64), ("total_virtual", U64),
                ("free_virtual", U64), ("extended", U64)]


class ProcessMemory(C.Structure):
    _fields_ = [("cb", U32), ("page_faults", U32)] + [
        (name, C.c_size_t) for name in ("peak_working", "working", "peak_paged",
                                      "paged", "peak_nonpaged", "nonpaged",
                                      "pagefile", "peak_pagefile")]


class Tcp4(C.Structure):
    _fields_ = [(name, U32) for name in ("state", "local", "port", "remote", "remote_port", "pid")]


class Tcp6(C.Structure):
    _fields_ = [("local", C.c_ubyte * 16), ("scope", U32), ("port", U32),
                ("remote", C.c_ubyte * 16), ("remote_scope", U32), ("remote_port", U32),
                ("state", U32), ("pid", U32)]


class Udp4(C.Structure):
    _fields_ = [(name, U32) for name in ("local", "port", "pid")]


class Udp6(C.Structure):
    _fields_ = [("local", C.c_ubyte * 16), ("scope", U32), ("port", U32), ("pid", U32)]


class Guid(C.Structure):
    _fields_ = [("a", U32), ("b", C.c_uint16), ("c", C.c_uint16), ("d", C.c_ubyte * 8)]


class AdapterPrefix(C.Structure):
    """IP_ADAPTER_ADDRESSES_LH prefix through Luid; pointers are never followed unchecked."""
    _fields_ = [("alignment", U64)] + [
        (name, C.c_void_p) for name in (
            "next", "adapter_name", "unicast", "anycast", "multicast",
            "dns_server", "dns_suffix", "description", "friendly_name")
    ] + [
        ("physical", C.c_ubyte * 8),
    ] + [(name, U32) for name in (
        "physical_length", "flags", "mtu", "type", "oper_status", "ipv6_index")
    ] + [
        ("zones", U32 * 16), ("prefix", C.c_void_p),
        ("tx_speed", U64), ("rx_speed", U64),
        ("wins_server", C.c_void_p), ("gateway", C.c_void_p),
        ("ipv4_metric", U32), ("ipv6_metric", U32), ("luid", U64),
    ]


class IfRow(C.Structure):
    """MIB_IF_ROW2 ABI, used only on native Windows (WCHAR is two bytes there)."""
    _fields_ = [
        ("luid", U64), ("index", U32), ("guid", Guid),
        ("alias", C.c_wchar * 257), ("description", C.c_wchar * 257),
        ("physical_length", U32), ("physical", C.c_ubyte * 32),
        ("permanent", C.c_ubyte * 32), ("mtu", U32),
    ] + [(name, U32) for name in ("type", "tunnel", "media", "physical_medium", "access", "direction")] + [
        ("flags", C.c_ubyte),
    ] + [(name, U32) for name in ("oper_status", "admin_status", "connect_state")] + [
        ("network_guid", Guid), ("connection_type", U32),
    ] + [(name, U64) for name in (
        "tx_speed", "rx_speed", "in_octets", "in_unicast", "in_nonunicast", "in_discards",
        "in_errors", "in_unknown", "in_ucast_octets", "in_multicast", "in_broadcast",
        "out_octets", "out_unicast", "out_nonunicast", "out_discards", "out_errors",
        "out_ucast_octets", "out_multicast", "out_broadcast", "out_queue")]


class NativeOracle:
    """Independent direct Win32 API readings; no production script reexecution."""

    def __init__(self, budget):
        if os.name != "nt":
            raise Hold("native_windows_required")
        from ctypes import wintypes as W
        self.budget = budget
        self.kernel = C.WinDLL("kernel32", use_last_error=True)
        self.ip = C.WinDLL("iphlpapi", use_last_error=True)
        self.psapi = C.WinDLL("psapi", use_last_error=True)
        signatures = [
            (self.kernel, "GlobalMemoryStatusEx", [C.POINTER(MemoryStatus)], W.BOOL),
            (self.kernel, "GetTickCount64", [], U64),
            (self.kernel, "GetSystemTimes", [C.c_void_p] * 3, W.BOOL),
            (self.kernel, "OpenProcess", [U32, W.BOOL, U32], W.HANDLE),
            (self.kernel, "CloseHandle", [W.HANDLE], W.BOOL),
            (self.kernel, "QueryFullProcessImageNameW", [W.HANDLE, U32, W.LPWSTR, C.POINTER(U32)], W.BOOL),
            (self.kernel, "GetDiskFreeSpaceExW", [W.LPCWSTR] + [C.POINTER(U64)] * 3, W.BOOL),
            (self.kernel, "GetLogicalDriveStringsW", [U32, W.LPWSTR], U32),
            (self.kernel, "GetDriveTypeW", [W.LPCWSTR], U32),
            (self.kernel, "GetVolumeInformationW",
             [W.LPCWSTR, W.LPWSTR, U32] + [C.POINTER(U32)] * 3 + [W.LPWSTR, U32], W.BOOL),
            (self.kernel, "GetVolumeNameForVolumeMountPointW", [W.LPCWSTR, W.LPWSTR, U32], W.BOOL),
            (self.kernel, "GetThreadErrorMode", [], U32),
            (self.kernel, "SetThreadErrorMode", [U32, C.POINTER(U32)], W.BOOL),
            (self.psapi, "GetProcessMemoryInfo", [W.HANDLE, C.POINTER(ProcessMemory), U32], W.BOOL),
            (self.ip, "GetAdaptersAddresses", [U32, U32, C.c_void_p, C.c_void_p, C.POINTER(U32)], U32),
            (self.ip, "GetIfTable2", [C.POINTER(C.c_void_p)], U32),
            (self.ip, "FreeMibTable", [C.c_void_p], None),
        ]
        for name in ("GetExtendedTcpTable", "GetExtendedUdpTable"):
            signatures.append((self.ip, name, [C.c_void_p, C.POINTER(U32), W.BOOL, U32, U32, U32], U32))
        for dll, name, arguments, result in signatures:
            function = getattr(dll, name)
            function.argtypes, function.restype = arguments, result

    def cpu_times(self):
        self.budget.remaining(reserve=10)
        idle, kernel, user = U64(), U64(), U64()
        if not self.kernel.GetSystemTimes(C.byref(idle), C.byref(kernel), C.byref(user)):
            raise LabError("independent GetSystemTimes failed")
        return [idle.value, kernel.value, user.value]

    def resources(self):
        """Enumerate independently on both sides; production never supplies the read set."""
        self.budget.remaining(reserve=10)
        memory = MemoryStatus()
        memory.length = C.sizeof(memory)
        if not self.kernel.GlobalMemoryStatusEx(C.byref(memory)):
            raise LabError("independent GlobalMemoryStatusEx failed")
        uptime = self.kernel.GetTickCount64() / 1000
        disks = self.filesystems()
        return {"source": "GlobalMemoryStatusEx/GetTickCount64/GetLogicalDriveStringsW/"
                         "GetDriveTypeW/GetVolumeInformationW/GetVolumeNameForVolumeMountPointW/"
                         "GetDiskFreeSpaceExW/GetAdaptersAddresses/GetIfTable2",
                "wall_unix": time.time(), "uptime_seconds": uptime, "boot_unix": time.time() - uptime,
                "memory_total_bytes": memory.total, "memory_free_bytes": memory.free,
                "network_scope": "IP-enabled adapters, AF_UNSPEC, flags INCLUDE_WINS|INCLUDE_GATEWAYS",
                "networks": self.networks(), **disks}

    def filesystems(self):
        """Observe ready fixed roots, volume identities and any incomplete capacity evidence."""
        self.budget.remaining(reserve=10)
        previous = U32()
        # A not-ready fixed drive must return an error rather than show a modal
        # critical-error dialog. Restore this thread's original mode even on failure.
        mode = self.kernel.GetThreadErrorMode()
        if not self.kernel.SetThreadErrorMode(mode | 1, C.byref(previous)):
            return {"filesystems": [], "drive_inventory": [],
                    "capacity_issues": [{"reason": "disk_error_mode_unavailable"}]}
        try:
            roots = C.create_unicode_buffer(128)  # 26 drive roots plus MULTI_SZ terminators.
            length = self.kernel.GetLogicalDriveStringsW(len(roots), roots)
            if not 0 < length < len(roots):
                return {"filesystems": [], "drive_inventory": [],
                        "capacity_issues": [{"reason": "drive_enumeration_incomplete"}]}
            text = "".join(roots[:length + 1])
            if not text.endswith("\0\0"):
                raise LabError("independent drive enumeration is not complete MULTI_SZ")
            mounts = text[:-2].split("\0")
            if (len(mounts) > 26 or any(not re.fullmatch(r"[A-Za-z]:\\", root) for root in mounts)
                    or len({root.casefold() for root in mounts}) != len(mounts)):
                raise LabError("independent drive enumeration has invalid or duplicate roots")
            rows, inventory, issues = [], [], []
            for mount in mounts:
                self.budget.remaining(reserve=10)
                kind = self.kernel.GetDriveTypeW(mount)
                observed = {"mount": mount, "drive_type": kind, "ready": None}
                inventory.append(observed)
                if kind in (0, 1):
                    issues.append({"mount": mount, "reason": "drive_type_unconfirmed"})
                    continue
                if kind != 3:  # DRIVE_FIXED; no UNC, removable, optical or RAM-disk reads.
                    continue
                serial = U32()
                if not self.kernel.GetVolumeInformationW(
                        mount, None, 0, C.byref(serial), None, None, None, 0):
                    error = C.get_last_error()
                    observed["error"] = error
                    if error == 21:  # ERROR_NOT_READY is an observation, not an empty success fallback.
                        observed["ready"] = False
                    else:
                        issues.append({"mount": mount, "reason": "readiness_unconfirmed", "error": error})
                    continue
                observed["ready"] = True
                volume = C.create_unicode_buffer(64)
                if not self.kernel.GetVolumeNameForVolumeMountPointW(mount, volume, len(volume)):
                    issues.append({"mount": mount, "reason": "volume_identity_unconfirmed",
                                   "error": C.get_last_error()})
                    continue
                identity = volume.value.casefold()
                if (not identity.startswith("\\\\?\\volume{") or not identity.endswith("}\\")
                        or len(identity) != 49):
                    raise LabError("independent volume GUID path is invalid")
                try:
                    uuid.UUID(identity[11:-2])
                except ValueError as error:
                    raise LabError("independent volume GUID is invalid") from error
                observed.update(volume_guid=identity, volume_serial=serial.value)
                available, total, free = U64(), U64(), U64()
                if not self.kernel.GetDiskFreeSpaceExW(
                        mount, C.byref(available), C.byref(total), C.byref(free)):
                    issues.append({"mount": mount, "reason": "capacity_unconfirmed",
                                   "error": C.get_last_error()})
                    continue
                rows.append({"mount": mount, "volume_guid": identity, "volume_serial": serial.value,
                             "total_bytes": total.value, "total_free_bytes": free.value,
                             "available_bytes": available.value})
            return {"filesystems": rows, "drive_inventory": inventory, "capacity_issues": issues}
        finally:
            if not self.kernel.SetThreadErrorMode(previous.value, None):
                raise LabError("independent disk thread error mode restore failed")

    def adapter_luids(self):
        """Bound the IP-enabled inventory separately from the broader MIB interface table."""
        capacity = 15000
        for _ in range(5):
            self.budget.remaining(reserve=10)
            if not C.sizeof(AdapterPrefix) <= capacity <= MAX_FRAME:
                raise LabError("independent adapter buffer outside budget")
            storage = C.create_string_buffer(capacity)
            length = U32(capacity)
            # Desktop .NET enumerates IP-enabled adapters with these flags; do
            # not add INCLUDE_ALL_INTERFACES and accidentally require non-IP rows.
            status = self.ip.GetAdaptersAddresses(0, 0xc0, None, storage, C.byref(length))
            if status == 111:  # ERROR_BUFFER_OVERFLOW, with a bounded retry on inventory growth.
                capacity = length.value
                continue
            if status == 232:  # ERROR_NO_DATA is the explicit empty observation.
                return set()
            if status != 0 or length.value > capacity:
                raise Hold("independent_ip_adapter_enumeration_incomplete")
            start, end = C.addressof(storage), C.addressof(storage) + length.value
            pointer, visited, identities = start, set(), set()
            while pointer:
                self.budget.remaining(reserve=10)
                if (len(visited) >= 4096 or pointer in visited or pointer % C.alignment(AdapterPrefix)
                        or not start <= pointer <= end - C.sizeof(AdapterPrefix)):
                    raise LabError("independent adapter list is cyclic, truncated or outside buffer")
                visited.add(pointer)
                row = AdapterPrefix.from_address(pointer)
                row_length = row.alignment & 0xffffffff
                if not C.sizeof(AdapterPrefix) <= row_length <= end - pointer:
                    raise LabError("independent adapter prefix version is truncated")
                if not row.luid or row.luid in identities:
                    raise LabError("independent adapter identity is absent or duplicated")
                identities.add(row.luid)
                pointer = row.next
            return identities
        raise Hold("independent_ip_adapter_inventory_kept_growing")

    def networks(self):
        self.budget.remaining(reserve=10)
        adapters = self.adapter_luids()
        table = C.c_void_p()
        if self.ip.GetIfTable2(C.byref(table)) != 0 or not table.value:
            raise LabError("independent GetIfTable2 failed")
        try:
            count = U32.from_address(table.value).value
            if count > 4096:
                raise LabError("independent interface count over budget")
            alignment = C.alignment(IfRow)
            offset = (C.sizeof(U32) + alignment - 1) // alignment * alignment
            rows = []
            for index in range(count):
                self.budget.remaining(reserve=10)
                row = IfRow.from_address(table.value + offset + index * C.sizeof(IfRow))
                if row.luid not in adapters:
                    continue
                rows.append({"name": row.alias, "received_bytes": row.in_octets,
                             "transmitted_bytes": row.out_octets, "interface_index": row.index,
                             "interface_luid": row.luid,
                             "interface_guid": str(uuid.UUID(bytes_le=C.string_at(C.byref(row.guid), 16)))})
            if {row["interface_luid"] for row in rows} != adapters:
                raise Hold("independent_ip_adapter_and_counter_inventory_changed")
            return rows
        finally:
            self.ip.FreeMibTable(table)

    def process(self, pid):
        self.budget.remaining(reserve=10)
        handle = self.kernel.OpenProcess(0x1000 | 0x0400 | 0x0010, False, pid)
        if not handle:
            raise LabError("independent OpenProcess failed for owned fixture")
        try:
            name = C.create_unicode_buffer(32768)
            length = U32(len(name))
            if not self.kernel.QueryFullProcessImageNameW(handle, 0, name, C.byref(length)):
                raise LabError("independent process image query failed")
            memory = ProcessMemory()
            memory.cb = C.sizeof(memory)
            if not self.psapi.GetProcessMemoryInfo(handle, C.byref(memory), C.sizeof(memory)):
                raise LabError("independent working-set query failed")
            return {"pid": pid, "name": Path(name.value).stem, "working_set_bytes": memory.working,
                    "source": "OpenProcess/QueryFullProcessImageNameW/GetProcessMemoryInfo"}
        finally:
            self.kernel.CloseHandle(handle)

    def sockets(self):
        rows = []
        for protocol, family, row_type in (
                ("tcp", socket.AF_INET, Tcp4), ("tcp", socket.AF_INET6, Tcp6),
                ("udp", socket.AF_INET, Udp4), ("udp", socket.AF_INET6, Udp6)):
            function = getattr(self.ip, "GetExtendedTcpTable" if protocol == "tcp" else "GetExtendedUdpTable")
            # OWNER_PID_ALL for TCP deliberately includes established rows. Their
            # native state, not the mere presence of an endpoint, decides LISTEN.
            table_class = 5 if protocol == "tcp" else 1
            capacity = U32()
            status = function(None, C.byref(capacity), False, family, table_class, 0)
            for _ in range(5):
                self.budget.remaining(reserve=10)
                if status not in (0, 122) or not 4 <= capacity.value <= MAX_FRAME:
                    raise LabError("independent IP Helper table sizing failed")
                storage = C.create_string_buffer(capacity.value)
                length = U32(len(storage))
                status = function(storage, C.byref(length), False, family, table_class, 0)
                if status != 122:
                    break
                capacity = length
            if status != 0:
                raise LabError("independent IP Helper table failed or kept growing")
            count = U32.from_buffer_copy(storage.raw[:4]).value
            if count > 20000 or 4 + count * C.sizeof(row_type) > len(storage):
                raise LabError("independent IP Helper table is truncated")
            for index in range(count):
                row = row_type.from_buffer_copy(storage, 4 + index * C.sizeof(row_type))
                address = (socket.inet_ntop(family, struct.pack("=I", row.local))
                           if family == socket.AF_INET else socket.inet_ntop(family, bytes(row.local)))
                if family == socket.AF_INET6 and row.scope:
                    address += "%" + str(row.scope)
                port = socket.ntohs(row.port & 0xffff)
                # Unbound TCP CLOSED rows may legitimately have port zero.
                if not port:
                    continue
                rows.append({"protocol": protocol, "address": address, "port": str(port),
                             "pid": str(row.pid),
                             "state": "LISTEN" if protocol == "tcp" and row.state == 2
                             else "CONNECTED_OR_OTHER" if protocol == "tcp" else "BOUND"})
        return rows


def compare_owned(fixture, processes, sockets, native_process, native_sockets):
    """Require all owned endpoints and reject connected TCP negative controls."""
    expected_pid = fixture["pid"]
    found = [row for row in processes["processes"] if int(row["pid"]) == expected_pid]
    if len(found) != 1 or native_process["pid"] != expected_pid:
        raise LabError("owned process PID missing or ambiguous")
    if found[0]["name"].casefold() != native_process["name"].casefold():
        raise LabError("owned process name differs from independent image")
    working = found[0]["working_set_bytes"]
    # The resident fixture is idle; allow paging and allocator variation between
    # independent samples without accepting null as a working-set observation.
    tolerance = max(16 * 1024 * 1024, native_process["working_set_bytes"] // 2)
    if working is None or abs(int(working) - native_process["working_set_bytes"]) > tolerance:
        raise LabError("owned process working set is missing or inconsistent")
    actual = {socket_key(row) for row in sockets["sockets"]}
    independent = {socket_key(row) for row in native_sockets if row["state"] in ("LISTEN", "BOUND")}
    all_native = {socket_key(row): row["state"] for row in native_sockets}
    for row in fixture["endpoints"]:
        identity = socket_key(row)
        if identity not in actual or identity not in independent:
            raise LabError("owned listener or UDP binding missing from production/independent API")
    for row in fixture["connected_only"]:
        identity = socket_key(row)
        if identity in actual or all_native.get(identity) != "CONNECTED_OR_OTHER":
            raise LabError("connected TCP negative control is missing or classified as LISTEN")
    return {"pid": expected_pid, "owned_endpoints_matched": 4, "connected_tcp_excluded": 2,
            "working_set_difference_bytes": abs(int(working) - native_process["working_set_bytes"]),
            "working_set_tolerance_bytes": tolerance}


def resource_inventory(rows, facility):
    """Index independent identities; an absent identity cannot prove a stable bracket."""
    indexed, labels = {}, set()
    for row in rows:
        fields = (("name", "interface_guid", "interface_luid", "interface_index") if facility == "network"
                  else ("mount", "volume_guid", "volume_serial"))
        if any(field not in row for field in fields):
            raise Hold("independent_" + facility + "_identity_missing")
        label = display(row[fields[0]])
        if facility == "network":
            identity = (display(row["interface_guid"]), integer(row["interface_luid"], 2**64 - 1, 1),
                        integer(row["interface_index"], 2**32 - 1, 1), label)
        else:
            if not re.fullmatch(r"[A-Za-z]:\\", label):
                raise LabError("independent disk mount is not a local drive root")
            label = label.casefold()
            identity = (label, display(row["volume_guid"]).casefold(),
                        integer(row["volume_serial"], 2**32 - 1))
        if identity in indexed or label in labels:
            raise Hold("independent_" + facility + "_identity_ambiguous")
        indexed[identity] = row
        labels.add(label)
    return indexed


def resource_coverage(rows, before, after, facility, incomplete=False):
    """Require every stable identity, even when production reports no rows."""
    earlier = resource_inventory(before, facility)
    later = resource_inventory(after, facility)
    stable = earlier.keys() & later.keys()
    field = "name" if facility == "network" else "mount"
    normalize = (lambda value: value) if facility == "network" else str.casefold
    actual = {normalize(row[field]): row for row in rows}
    if len(actual) != len(rows):
        raise LabError("production " + facility + " labels are ambiguous")
    required = {normalize(earlier[identity][field]): identity for identity in stable}
    missing = sorted(required.keys() - actual.keys())
    if missing:
        raise LabError("production omitted stable " + facility + " resources: " + ", ".join(missing))
    changed = earlier.keys() != later.keys()
    if not changed and not incomplete and actual.keys() != required.keys():
        raise LabError("production " + facility + " resource absent from independent stable inventory")
    evidence = {"required": sorted(required), "production": sorted(actual),
                "stable_identities": [list(identity) for identity in sorted(stable)],
                "before_only": [list(identity) for identity in sorted(earlier.keys() - later.keys())],
                "after_only": [list(identity) for identity in sorted(later.keys() - earlier.keys())]}
    matched = [(actual[label], earlier[identity], later[identity])
               for label, identity in sorted(required.items())]
    return matched, evidence, changed


def compare_resources(data, before, after, cpu_before, cpu_after):
    """Compare different real APIs with explicit, saved sampling tolerances."""
    memory_tolerance = max(1024 * 1024, after["memory_total_bytes"] // 200)
    free_tolerance = max(256 * 1024 * 1024, after["memory_total_bytes"] // 10)
    if abs(int(data["memory_total_bytes"]) - after["memory_total_bytes"]) > memory_tolerance:
        raise LabError("visible memory total differs from independent physical memory")
    free = int(data["memory_free_bytes"])
    if not min(before["memory_free_bytes"], after["memory_free_bytes"]) - free_tolerance <= free <= \
            max(before["memory_free_bytes"], after["memory_free_bytes"]) + free_tolerance:
        raise LabError("free memory differs from independent physical memory")
    if abs(int(data["boot_time"]) - after["boot_unix"]) > 120:
        raise LabError("CIM wall-clock boot estimate differs from independent uptime")
    if not before["uptime_seconds"] - 120 <= data["uptime_seconds"] <= after["uptime_seconds"] + 120:
        raise LabError("CIM wall-clock uptime estimate differs from independent uptime")
    networks, network_inventory, networks_changed = resource_coverage(
        data["networks"], before["networks"], after["networks"], "network")
    for row, earlier, later in networks:
        for field in ("received_bytes", "transmitted_bytes"):
            if later[field] < earlier[field]:
                raise Hold("independent_network_counters_reset_during_bracket")
            if not earlier[field] <= int(row[field]) <= later[field]:
                raise LabError("cumulative interface bytes outside independent API bracket")
    disk_tolerances = []
    capacity_inventory = {"status": "not_compared", "reason": data["capacity_status"]}
    disks_changed, capacity_issues = False, []
    if data["capacity_status"] == "ok":
        capacity_issues = before.get("capacity_issues", []) + after.get("capacity_issues", [])
        disks, capacity_inventory, disks_changed = resource_coverage(
            data["filesystems"], before["filesystems"], after["filesystems"], "capacity",
            incomplete=bool(capacity_issues))
        for row, earlier, later in disks:
            if earlier["total_bytes"] != later["total_bytes"]:
                raise Hold("independent_capacity_total_changed_during_bracket")
            if later["total_bytes"] != int(row["total_bytes"]):
                raise LabError("capacity total differs from independent API")
            tolerance = max(256 * 1024 * 1024, later["total_bytes"] // 100)
            for field in ("total_free_bytes", "available_bytes"):
                if not min(earlier[field], later[field]) - tolerance <= int(row[field]) <= \
                        max(earlier[field], later[field]) + tolerance:
                    raise LabError("capacity free/available differs from independent API")
            disk_tolerances.append({"mount": row["mount"], "bytes": tolerance})
    # Stable omissions are definite mismatches. Changing or incompletely observed
    # inventories cannot establish complete acceptance, even if their intersection matched.
    if networks_changed:
        raise Hold("independent_network_inventory_changed_during_bracket")
    if data["capacity_status"] == "ok" and (disks_changed or capacity_issues):
        raise Hold("independent_capacity_inventory_changed_or_incomplete")
    deltas = [right - left for left, right in zip(cpu_before, cpu_after)]
    total = deltas[1] + deltas[2]
    if any(delta < 0 for delta in deltas) or total <= 0 or deltas[0] > total:
        raise LabError("independent CPU sample has invalid time counters")
    native_cpu = 100 * (total - deltas[0]) / total
    return {"memory_total_tolerance_bytes": memory_tolerance,
            "memory_free_tolerance_bytes": free_tolerance, "uptime_tolerance_seconds": 120,
            "capacity_tolerances": disk_tolerances, "networks_matched": len(data["networks"]),
            "network_inventory": network_inventory, "capacity_inventory": capacity_inventory,
            "independent_cpu_percent": native_cpu, "production_cpu_percent": data["cpu_percent"],
            "cpu_difference_is_informational": True,
            "cpu_reason": "CIM LoadPercentage and GetSystemTimes cover different sampling windows",
            "capacity_status": data["capacity_status"]}


def powershell_path():
    path = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32/WindowsPowerShell/v1.0/powershell.exe"
    if not path.is_file():
        raise Hold("desktop_windows_powershell_missing")
    return str(path)


def cmd_path():
    path = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32/cmd.exe"
    if not path.is_file():
        raise Hold("windows_default_cmd_shell_missing")
    return str(path)


def cmd_argv(command, shell):
    """Preserve the complete fixed production command as cmd's single /c value."""
    if (not command.startswith(POWERSHELL) or not command.isascii()
            or len(command) >= MAX_COMMAND or "\n" in command or "\r" in command):
        raise LabError("invalid fixed command for cmd boundary")
    argv = [shell, "/d", "/s", "/c", command]
    if len(subprocess.list2cmdline(argv)) >= 8191:
        raise LabError("serialized cmd launch reaches Windows shell boundary")
    return argv


def direct_gate(evidence):
    if os.name != "nt":
        raise Hold("native_windows_required")
    commands = {}
    for operation in ASSEMBLY:
        command, binding = assemble(operation)
        evidence.write_json(operation + ".assembly.json", binding)
        evidence.write_bytes(operation + ".wire.txt", command.encode("ascii"))
        commands[operation] = command
    executable = powershell_path()
    shell = cmd_path()

    def collect(operation, label=None):
        label = operation if label is None else label
        result = evidence.run(label, cmd_argv(commands[label], shell),
                              seconds=35, required_image=executable)
        return parse_frame(result.stdout, result.stderr, result.exit_code, operation)

    probe = collect("probe")
    evidence.write_json("runtime.json", {"runtime": probe["runtime"],
                                        "runtime_version": probe["runtime_version"],
                                        "executable": executable,
                                        "sha256": hashlib.sha256(Path(executable).read_bytes()).hexdigest()})
    fixture = OwnedChild([sys.executable, "-u", str(FIXTURES / "owned_fixture.py"), "--seconds", "180"],
                         Budget(210, evidence.budget.end))
    try:
        fixture.start()
        ready = fixture.ready()
        evidence.write_json("fixture.json", ready)
        oracle = NativeOracle(evidence.budget)
        cpu_before = oracle.cpu_times()
        before = oracle.resources()
        evidence.write_json("independent-before.json", {"resources": before, "cpu": cpu_before})
        resources = collect("resources")
        processes = collect("processes")
        sockets = collect("sockets")
        skipped = collect("resources", "resources-skipped")
        if skipped["data"]["capacity_status"] != "skipped" or skipped["data"]["filesystems"]:
            raise LabError("fixed skip-capacity production variant did not preserve skipped state")
        for frame in (resources, processes, sockets, skipped):
            if frame["runtime_version"] != probe["runtime_version"]:
                raise LabError("runtime changed between production operations")
        after = oracle.resources()
        cpu_after = oracle.cpu_times()
        native_process = oracle.process(fixture.process.pid)
        native_sockets = oracle.sockets()
        if fixture.process.poll() is not None:
            raise LabError("original fixture process ended during independent API readings")
        evidence.write_json("independent.json", {"before": before, "after": after,
                                                "cpu_before": cpu_before, "cpu_after": cpu_after,
                                                "process": native_process, "sockets": native_sockets})
        compared = {
            "owned": compare_owned(ready, processes["data"], sockets["data"], native_process, native_sockets),
            "resources": compare_resources(resources["data"], before, after, cpu_before, cpu_after),
            "resources_skipped": compare_resources(skipped["data"], before, after, cpu_before, cpu_after),
        }
        evidence.write_json("comparison.json", compared)
        if fixture.stop_fixture() != 0:
            raise LabError("owned fixture did not exit naturally after stop")
    finally:
        receipt = evidence.keep_child("fixture", fixture)
    if not terminal_ok(receipt):
        raise LabError("fixture original-process terminal not proven")
    if resources["data"]["capacity_status"] != "ok":
        raise Hold("capacity_" + resources["data"]["capacity_status"] + "_core_frames_retained")
    return {"status": "PASS", "scope": "production collectors/direct Win32 APIs/owned loopback fixtures",
            "ssh": "UNRUN", "native_gui": "UNRUN"}


def secure_directory(path):
    """Set a protected inheritable DACL on this lab's new private directory."""
    if os.name != "nt":
        raise Hold("native_windows_required")
    from ctypes import wintypes as W
    kernel, advapi = C.WinDLL("kernel32", use_last_error=True), C.WinDLL("advapi32", use_last_error=True)
    signatures = [
        (kernel, "GetCurrentProcess", [], W.HANDLE),
        (kernel, "CloseHandle", [W.HANDLE], W.BOOL),
        (kernel, "LocalFree", [C.c_void_p], C.c_void_p),
        (advapi, "OpenProcessToken", [W.HANDLE, U32, C.POINTER(W.HANDLE)], W.BOOL),
        (advapi, "GetTokenInformation", [W.HANDLE, U32, C.c_void_p, U32, C.POINTER(U32)], W.BOOL),
        (advapi, "ConvertSidToStringSidW", [C.c_void_p, C.POINTER(W.LPWSTR)], W.BOOL),
        (advapi, "ConvertStringSecurityDescriptorToSecurityDescriptorW",
         [W.LPCWSTR, U32, C.POINTER(C.c_void_p), C.c_void_p], W.BOOL),
        (advapi, "SetFileSecurityW", [W.LPCWSTR, U32, C.c_void_p], W.BOOL),
    ]
    for dll, name, arguments, result in signatures:
        function = getattr(dll, name)
        function.argtypes, function.restype = arguments, result
    token, sid_text, descriptor = W.HANDLE(), W.LPWSTR(), C.c_void_p()
    try:
        if not advapi.OpenProcessToken(kernel.GetCurrentProcess(), 8, C.byref(token)):
            raise Hold("cannot_secure_ephemeral_directory")
        required = U32()
        advapi.GetTokenInformation(token, 1, None, 0, C.byref(required))
        if not 0 < required.value < 65536:
            raise Hold("cannot_size_current_token_identity")
        storage = C.create_string_buffer(required.value)
        if not advapi.GetTokenInformation(token, 1, storage, len(storage), C.byref(required)):
            raise Hold("cannot_read_current_token_identity")
        sid = C.c_void_p.from_buffer(storage)
        if not advapi.ConvertSidToStringSidW(sid, C.byref(sid_text)):
            raise Hold("cannot_bind_current_user_dacl")
        sddl = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;" + sid_text.value + ")"
        if not advapi.ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl, 1, C.byref(descriptor), None):
            raise Hold("cannot_construct_private_dacl")
        if not advapi.SetFileSecurityW(str(path), 0x80000004, descriptor):
            raise Hold("cannot_apply_private_dacl")
    finally:
        if token:
            kernel.CloseHandle(token)
        if sid_text:
            kernel.LocalFree(sid_text)
        if descriptor:
            kernel.LocalFree(descriptor)


def ssh_config(directory, port):
    """Explicit local-only configuration; no default Match/include/key paths."""
    integer(port, 65535, 49152)
    directory = Path(directory)

    def quoted(name):
        value = str(directory / name).replace("\\", "/")
        if any(char in value for char in ('"', "\r", "\n")):
            raise Hold("unsafe_ephemeral_path")
        return '"' + value + '"'

    return "\n".join([
        f"Port {port}", "ListenAddress 127.0.0.1",
        "HostKey " + quoted("host"), "PidFile " + quoted("sshd.pid"),
        "AuthorizedKeysFile " + quoted("authorized_keys"),
        "PubkeyAuthentication yes", "AuthenticationMethods publickey",
        "PasswordAuthentication no", "KbdInteractiveAuthentication no",
        "HostbasedAuthentication no", "PermitEmptyPasswords no", "StrictModes yes",
        "DisableForwarding yes", "AllowTcpForwarding no", "PermitTunnel no", "PermitTTY no",
        "PermitUserEnvironment no", "MaxSessions 1", "MaxStartups 1", "LoginGraceTime 10",
        "LogLevel ERROR",
        # Until a remote identity/admission barrier exists, configuration checks
        # must never accidentally execute an arbitrary authenticated command.
        "ForceCommand cmd.exe /c exit 125", "",
    ])


def ssh_gate(evidence):
    """Non-mutating isolated prerequisites, never a synthetic SSH success."""
    if os.name != "nt":
        raise Hold("native_windows_required")
    directory = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32/OpenSSH"
    tools = {name: directory / (name + ".exe") for name in ("sshd", "ssh", "ssh-keygen")}
    evidence.write_json("openssh-tools.json", {
        name: {"present": path.is_file(),
               "sha256": hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None}
        for name, path in tools.items()})
    if not all(path.is_file() for path in tools.values()):
        raise Hold("openssh_binaries_missing_no_install_or_service_changes")
    temporary = None
    removed = False
    preflight = "UNRUN"
    try:
        with tempfile.TemporaryDirectory(prefix="keelshell-monitor-ssh-") as name:
            temporary = Path(name)
            secure_directory(temporary)
            # Reserve a temporary loopback high port, without changing firewall.
            # No server is started in this prerequisite-only implementation.
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as reservation:
                reservation.bind(("127.0.0.1", 0))
                port = reservation.getsockname()[1]
                if port < 49152:
                    raise Hold("isolated_high_port_unavailable")
                for key in ("host", "client"):
                    result = evidence.run("ssh-keygen-" + key, [
                        str(tools["ssh-keygen"]), "-q", "-t", "ed25519", "-N", "",
                        "-C", "keelshell-owned-monitor-lab", "-f", str(temporary / key)])
                    if result.exit_code != 0 or result.stdout or result.stderr:
                        raise Hold("ephemeral_key_generation_failed")
                public = (temporary / "client.pub").read_bytes()
                if len(public) > 1024 or not public.startswith(b"ssh-ed25519 "):
                    raise Hold("ephemeral_public_key_invalid")
                (temporary / "authorized_keys").write_bytes(public)
                config = ssh_config(temporary, port)
                (temporary / "sshd_config").write_text(config, encoding="utf-8", newline="\n")
                evidence.write_json("isolated-ssh.json", {
                    "port": port, "listen": "127.0.0.1", "temporary_keys": True,
                    "protected_directory_dacl": True,
                    "host_public_sha256": hashlib.sha256((temporary / "host.pub").read_bytes()).hexdigest(),
                    "client_public_sha256": hashlib.sha256(public).hexdigest(),
                    "config_sha256": hashlib.sha256(config.encode("utf-8")).hexdigest(),
                    "server_started": False, "authenticated_commands": 0})
            # Config validation has its own Job and full terminal receipt.
            result = evidence.run("sshd-config", [
                str(tools["sshd"]), "-t", "-f", str(temporary / "sshd_config")])
            preflight = "PASS" if result.exit_code == 0 and not result.stdout and not result.stderr else "HOLD"
    finally:
        removed = temporary is not None and not temporary.exists()
        evidence.write_json("ssh-cleanup.json", {"temporary_directory_removed": removed,
                                                "config_preflight": preflight,
                                                "server_started": False,
                                                "remote_powershell_terminal": "UNPROVEN"})
    if not removed:
        raise LabError("ephemeral SSH material cleanup unproven")
    raise Hold("isolated_ssh_remote_process_admission_and_terminal_unproven")


def run_lab(mode, output, seconds=240):
    evidence = Evidence(output, seconds)
    receipt = {"schema": 1, "mode": mode, "status": "FAIL", "started_unix": time.time(),
               "os": platform.platform(), "system": platform.system(), "release": platform.release(),
               "build": platform.version(), "architecture": platform.machine(),
               "python": sys.version, "python_executable": sys.executable,
               "windows_api": "UNRUN", "ssh": "UNRUN", "native_gui": "UNRUN",
               "children": evidence.children}
    if os.name == "nt":
        version = sys.getwindowsversion()
        receipt["windows_version"] = {"major": version.major, "minor": version.minor,
                                      "build": version.build, "platform": version.platform}
    code = 1
    try:
        if mode == "direct":
            result = direct_gate(evidence)
            receipt["windows_api"] = result["status"]
            receipt["scope"] = result["scope"]
            receipt["status"] = "PASS"
            code = 0
        elif mode == "ssh":
            ssh_gate(evidence)
            # This version intentionally has no SSH success path.
            raise LabError("unexpected SSH success without remote process proof")
        else:
            raise LabError("unknown lab mode")
    except Hold as error:
        receipt["status"], receipt["reason"] = "HOLD", str(error)
        receipt["ssh" if mode == "ssh" else "windows_api"] = "HOLD"
        code = HOLD_EXIT
    except (LabError, OSError, ValueError, TimeoutError, lifecycle.OwnershipError,
            subprocess.SubprocessError) as error:
        # Errors are fixed lab messages or exception class names, not arbitrary
        # command output. Raw command frames have their own bounded artifacts.
        receipt["reason"] = str(error) if isinstance(error, LabError) else type(error).__name__
        receipt["ssh" if mode == "ssh" else "windows_api"] = "FAIL"
    finally:
        if any(not terminal_ok(child) for child in evidence.children):
            receipt["status"] = "FAIL"
            receipt["cleanup_failure"] = True
            code = 1
        receipt["finished_unix"] = time.time()
        evidence.write_json("receipt.json", receipt)
    return code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("direct", "ssh"), required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=240)
    args = parser.parse_args()
    return run_lab(args.mode, args.output, args.seconds)


if __name__ == "__main__":
    raise SystemExit(main())
