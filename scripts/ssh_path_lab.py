#!/usr/bin/env python3
"""Bounded, localhost-only OpenSSH path experiment using a prebuilt Rust test.

Uses an independent target and forwarding-only jump sshd, with pinned identities.
The two HTTP CONNECT gates can delay/throttle encrypted streams independently.
This is explicitly NOT IP packet loss, ICMP RTT, or a native desktop acceptance.
No Cargo command, package installation, or user SSH configuration is involved.
"""

import argparse
import asyncio
from contextlib import ExitStack
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

import openssh_interop as interop

ROOT = Path(__file__).resolve().parents[1]
SAMPLE_TEST = "openssh_path_comparison_collects_eighteen_pinned_samples"
EXTRA_TESTS = [
    "openssh_path_comparison_precancel_preserves_routes",
    "openssh_path_comparison_rejects_unpinned_target_and_jump",
]
SCENARIOS = ("baseline", "direct-impaired", "relay-impaired")
CHUNK_BYTES = 16 * 1024
CLEANUP_RESERVE = 20.0
SCENARIO_SECONDS = 72.0


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def conditions(scenario, delay_ms, rate):
    """Describe the actual stream operation, including what it cannot model."""
    impaired = {"delay_ms_per_write": delay_ms, "bytes_per_second": rate}
    clear = {"delay_ms_per_write": 0, "bytes_per_second": 0}
    return {
        "model": "user-space TCP stream delay/throttle, NOT IP packet loss",
        "write_chunk_max_bytes": CHUNK_BYTES,
        "direction": "both; independent pacing for each direction of each tunnel",
        "pacing": "sleep delay_ms/1000 + chunk_bytes/rate before each forwarding write; rate=0 disables rate sleep",
        "A": dict(impaired if scenario == "direct-impaired" else clear),
        "B": dict(impaired if scenario == "relay-impaired" else clear),
        "application": "A gate -> target; B gate -> jump -> target direct-tcpip",
        "ip_packet_loss": "NOT_TESTED",
    }


async def forward_stream(reader, writer, delay_ms, rate):
    """Bounded buffering with backpressure; EOF preserves the reverse stream."""
    while chunk := await reader.read(CHUNK_BYTES):
        pause = delay_ms / 1000 + (len(chunk) / rate if rate else 0)
        if pause:
            await asyncio.sleep(pause)
        writer.write(chunk)
        await writer.drain()
    if writer.can_write_eof():
        writer.write_eof()
        await writer.drain()


async def connect_gate(reader, writer, target_port, delay_ms=0, rate=0):
    """Allow exactly one loopback authority; never act as an open proxy."""
    upstream = None
    try:
        header = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 3)
        # Header bytes are never logged, even for malformed/unauthorized input.
        request = header.split(b"\r\n", 1)[0]
        expected = f"CONNECT 127.0.0.1:{target_port} HTTP/1.".encode("ascii")
        if len(header) > 4096 or request not in (expected + b"0", expected + b"1"):
            writer.write(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            await writer.drain()
            return
        source, upstream = await asyncio.wait_for(
            asyncio.open_connection("127.0.0.1", target_port), 3,
        )
        writer.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        await writer.drain()
        # A tunnel has a separate 80s cap, below the overall 300s run budget.
        async with asyncio.timeout(80):
            async with asyncio.TaskGroup() as tasks:
                tasks.create_task(forward_stream(reader, upstream, delay_ms, rate))
                tasks.create_task(forward_stream(source, writer, delay_ms, rate))
    except (OSError, EOFError, ValueError, TimeoutError, ExceptionGroup,
            asyncio.IncompleteReadError, asyncio.LimitOverrunError):
        # Protocol failure closes the stream; it does not drop IP packets.
        pass
    finally:
        for stream in (upstream, writer):
            if stream is not None:
                stream.close()
                try:
                    await asyncio.wait_for(stream.wait_closed(), 0.5)
                except (OSError, TimeoutError):
                    pass


async def serve_gate(config):
    active = set()

    async def accept(reader, writer):
        task = asyncio.current_task()
        active.add(task)
        try:
            await connect_gate(reader, writer, config["target_port"],
                               config["delay_ms_per_write"], config["bytes_per_second"])
        finally:
            active.discard(task)

    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, stop.set)
    server = await asyncio.start_server(accept, "127.0.0.1", config["port"], limit=4096)
    print("gate ready: localhost restricted CONNECT; stream pacing only", flush=True)
    try:
        await stop.wait()
    finally:
        # wait_closed can wait for accepted streams on newer Python releases;
        # close those owned streams before awaiting the listening server.
        server.close()
        await asyncio.sleep(0)
        tasks = list(active)
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        await server.wait_closed()


class LabRunner(interop.Runner):
    """Reuse the existing kernel birth identity and child-reaping implementation."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.daemons = set()

    def capture_owned(self):
        # Registration happens before the first inventory read in start().
        # A failing inventory must not lose the daemon role.
        self.daemons.update(
            process for process in self.processes
            if self.names[process].endswith(("-sshd", "-gate"))
        )
        super().capture_owned()
        for process in self.daemons:
            if process.poll() is not None:
                self.ancestry_unverified.add(process.pid)

    def daemon(self, name, command):
        process = self.start(name, command)
        self.daemons.add(process)
        return process

    def wait_bounded(self, name, process, seconds):
        until = min(self.deadline, time.monotonic() + seconds)
        while True:
            self.capture_owned()
            self.check_interrupt()
            if process.poll() is not None:
                self.receipt["steps"].append({
                    "name": name, "returncode": process.returncode, "reaped": True,
                })
                return process.returncode
            if any(daemon.poll() is not None for daemon in self.daemons):
                raise interop.InteropFailure("An owned daemon exited during " + name)
            if time.monotonic() >= until:
                raise interop.InteropFailure("Bounded deadline reached during " + name)
            time.sleep(0.05)


def server_config(directory, username, port, target_port=None):
    """Independent key/config/auth files for each role, never a user's ssh dir."""
    forwarding = target_port is not None
    lines = [
        f"Port {port}", "AddressFamily inet", "ListenAddress 127.0.0.1",
        f"HostKey {interop.config_value(directory / 'host')}",
        f"AuthorizedKeysFile {interop.config_value(directory / 'authorized_keys')}",
        f"PidFile {interop.config_value(directory / 'sshd.pid')}",
        f"AllowUsers {username}", "AuthenticationMethods publickey",
        "PubkeyAuthentication yes", "PasswordAuthentication no",
        "KbdInteractiveAuthentication no", "PermitEmptyPasswords no",
        "UsePAM no", "StrictModes yes", "UseDNS no",
        "PermitRootLogin prohibit-password", "PermitUserEnvironment no",
        "AllowAgentForwarding no", "AllowStreamLocalForwarding no",
        "PermitTunnel no", "X11Forwarding no", "PermitTTY no",
        "LoginGraceTime 10", "MaxAuthTries 2", "LogLevel ERROR",
        "AllowTcpForwarding local" if forwarding else "AllowTcpForwarding no",
        f"PermitOpen 127.0.0.1:{target_port}" if forwarding else "PermitOpen none",
        # Zero disables shell/exec/subsystem sessions but allows forwarding.
        "MaxSessions 0" if forwarding else "MaxSessions 8",
    ]
    return "\n".join([*lines, ""])


def wait_gate(runner, process, port):
    until = min(runner.deadline, time.monotonic() + 5)
    while time.monotonic() < until:
        runner.capture_owned()
        runner.check_interrupt()
        if process.poll() is not None:
            raise interop.InteropFailure("CONNECT gate exited before readiness")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                return
        except OSError:
            time.sleep(0.05)
    raise interop.InteropFailure("CONNECT gate readiness deadline reached")


def validate_report(report):
    """Reject optimistic summaries, missing rows, partials and falsified medians."""
    if report.get("status") != "Complete" or report.get("cleanup_unconfirmed") is not False:
        raise interop.InteropFailure("Comparison incomplete/cancelled or cleanup unconfirmed")
    samples = report.get("samples", [])
    if len(samples) != 18:
        raise interop.InteropFailure("Expected exactly 18 raw samples")
    groups = {(side, kind): [] for side in ("A", "B")
              for kind in ("Exec", "Upload", "Download")}
    for sample in samples:
        key = (sample.get("path"), sample.get("kind"))
        elapsed = sample.get("elapsed_ns")
        if key not in groups or type(elapsed) is not int or not 0 < elapsed <= 8_000_000_000:
            raise interop.InteropFailure("Invalid sample path/kind/duration")
        if sample.get("outcome") != "Passed" or sample.get("failure") is not None:
            raise interop.InteropFailure("Failed samples cannot count as successful measurements")
        if key[1] != "Exec" and sample.get("bytes") != 256 * 1024:
            raise interop.InteropFailure("Upload/download did not confirm exactly 256 KiB")
        groups[key].append(sample)
    rounds = None
    for (side, kind), rows in groups.items():
        found = {row.get("round") for row in rows}
        if len(rows) != 3 or len(found) != 3 or any(type(n) is not int for n in found):
            raise interop.InteropFailure("Missing or duplicate probe rounds")
        if rounds is not None and found != rounds:
            raise interop.InteropFailure("Probe kinds disagree on round identities")
        rounds = found
        median = sorted(row["elapsed_ns"] for row in rows)[1]
        if report.get("median_ns", {}).get(side, {}).get(kind) != median:
            raise interop.InteropFailure("Reported median disagrees with original samples")
    # Three rounds alternate first path. Kinds may be grouped within each side
    # or paired by kind; require that each round starts on the intended path.
    order = sorted(rounds)
    if order not in ([0, 1, 2], [1, 2, 3]):
        raise interop.InteropFailure("Rounds must be three consecutive fixed rounds")
    expected_first = ["A", "B", "A"]
    for n, first in zip(order, expected_first):
        if next(row["path"] for row in samples if row["round"] == n) != first:
            raise interop.InteropFailure("Alternating first-path order was not preserved")


def summarize(report):
    """An observed difference is signed; there is deliberately no speedup label."""
    validate_report(report)
    medians = report["median_ns"]
    return {
        kind: {
            "A_ns": medians["A"][kind], "B_ns": medians["B"][kind],
            "B_minus_A_ns": medians["B"][kind] - medians["A"][kind],
            "interpretation": "local observation only; includes SSH exec startup",
        } for kind in ("Exec", "Upload", "Download")
    }


def shutdown(runner, deadline):
    """Attempt every owned cleanup even if an earlier inventory fails."""
    rows = []
    for process in reversed(runner.processes):
        daemon = process in runner.daemons
        if daemon and process.poll() is not None:
            runner.ancestry_unverified.add(process.pid)
        row = {"name": runner.names[process], "pid": process.pid}
        try:
            def unexpected():
                runner.ancestry_unverified.add(process.pid)
            stopped = interop.stop_owned(
                process, deadline, runner.owned[process],
                unexpected if daemon else None,
            )
            row["observed_stopped"] = stopped
        except (interop.InteropFailure, OSError, subprocess.SubprocessError) as error:
            row.update(observed_stopped=False, error=str(error))
            for pid, identity in runner.owned[process].items():
                try:
                    interop.signal_owned(pid, identity, signal.SIGKILL)
                except (interop.InteropFailure, OSError):
                    pass
        # Popen.wait is an actual direct-child reap, independent of inventory.
        try:
            process.wait(timeout=max(0.001, min(0.2, deadline - time.monotonic())))
            row.update(returncode=process.returncode, reaped=True)
        except subprocess.SubprocessError:
            row["reaped"] = False
        row["identities"] = [
            {"pid": pid, "birth": identity[2]} for pid, identity in runner.owned[process].items()
        ]
        rows.append(row)
    return rows


def run_scenario(runner, name, scratch, base_environment, ports, args):
    destination = runner.output / name
    destination.mkdir(mode=0o700)
    condition = conditions(name, args.delay_ms, args.rate)
    write_json(destination / "conditions.json", condition)
    # Gates remain owned until final cleanup. Distinct ports avoid any scenario
    # changing a live tunnel's parameters or sharing rate/queue state.
    gate_ports = {}
    for side, target in (("A", ports["target"]), ("B", ports["jump"])):
        port = interop.reserve_port()
        config = {"port": port, "target_port": target, **condition[side]}
        config_path = scratch / f"{name}-{side}-gate.json"
        write_json(config_path, config)
        gate = runner.daemon(f"{name}-{side}-gate", [
            sys.executable, str(Path(__file__).resolve()), "--gate", str(config_path),
        ])
        wait_gate(runner, gate, port)
        gate_ports[side] = port
    environment = dict(base_environment, **{
        "KEELSHELL_PATH_DIRECT_PROXY_PORT": str(gate_ports["A"]),
        "KEELSHELL_PATH_RELAY_PROXY_PORT": str(gate_ports["B"]),
        "KEELSHELL_PATH_REPORT_DIR": str(destination),
        "KEELSHELL_PATH_SCENARIO": name,
    })
    row = {"scenario": name, "conditions": condition, "tests": [], "outcome": "failed"}
    row["gate_endpoints"] = {
        side: f"127.0.0.1:{port}" for side, port in gate_ports.items()
    }
    runner.receipt["scenarios"].append(row)
    tests = [SAMPLE_TEST, *(EXTRA_TESTS if name == "baseline" else [])]
    for index, test in enumerate(tests):
        log_name = f"{name}-test-{index}"
        command = [str(args.test_binary), "--ignored", "--exact", test, "--nocapture", "--test-threads=1"]
        process = runner.start(log_name, command, environment)
        code = runner.wait_bounded(log_name, process, SCENARIO_SECONDS if index == 0 else 18)
        log = (runner.output / f"{log_name}.log").read_text(encoding="utf-8", errors="replace")
        entry = {"name": test, "returncode": code, "reaped": True}
        row["tests"].append(entry)
        # A test filter matching zero tests is not successful interoperability.
        if code != 0 or not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", log):
            row["error"] = f"{test} failed or did not execute exactly one test"
            return False
    report_path = destination / "report.json"
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
        row["observed_difference"] = summarize(report)
    except (OSError, ValueError, TypeError, interop.InteropFailure) as error:
        row["error"] = "Raw report rejected: " + str(error)
        return False
    row["outcome"] = "passed"
    return True


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, help="New receipt directory, never reused")
    parser.add_argument("--test-binary", required=True, type=Path,
                        help="Parent-prebuilt ssh_path_interop test executable; no compilation")
    parser.add_argument("--scenario", choices=("all", *SCENARIOS), default="all")
    parser.add_argument("--delay-ms", type=float, default=20,
                        help="Impaired stream delay before each <=16KiB write, both directions")
    parser.add_argument("--rate", type=int, default=524288,
                        help="Impaired bytes/sec/direction/tunnel; 0 disables rate pacing")
    parser.add_argument("--timeout", type=float, default=300,
                        help="Total wall time including cleanup; 30..300 seconds")
    args = parser.parse_args(argv)
    if platform.system() not in ("Darwin", "Linux"):
        parser.error("The local OpenSSH lab requires macOS or Linux")
    if not math.isfinite(args.timeout) or not 30 <= args.timeout <= 300:
        parser.error("--timeout must be finite and between 30 and 300")
    if not math.isfinite(args.delay_ms) or not 0 <= args.delay_ms <= 1000 or args.rate < 0:
        parser.error("Use a finite 0..1000 delay and a nonnegative rate")
    if args.scenario != "baseline" and args.delay_ms == 0 and args.rate == 0:
        parser.error("An impaired scenario requires a nonzero delay or rate limit")
    if not args.test_binary.is_file() or not os.access(args.test_binary, os.X_OK):
        parser.error("--test-binary must be an existing executable")
    args.test_binary = args.test_binary.absolute()

    import pwd

    started = time.monotonic()
    deadline = started + args.timeout
    previous_mask = os.umask(0o077)
    try:
        output = interop.create_output(args.output)
    finally:
        os.umask(previous_mask)
    receipt = {
        "schema": 1, "started_utc": interop.utc_now(), "outcome": "failed",
        "timeout_seconds": args.timeout, "processes": [], "steps": [],
        "scenarios": [], "cleanup": {},
        "test_binary": {"path": str(args.test_binary)},
        "netem": {
            "status": "HOLD", "ip_packet_loss": "NOT_TESTED",
            "reason": "This runner uses host OpenSSH and user-space gates; a separate Linux network namespace with tc/netem is required for IP loss acceptance.",
        },
        "boundary": "Real pinned localhost OpenSSH target/jump; synthetic memory payload; no native desktop or Internet path acceptance",
    }
    print(f"SSH path lab receipt: {output}", flush=True)
    requested_signal = None
    old_handlers = {}
    scratch = None
    runner = None
    result = 1

    def interrupt(signum, _):
        nonlocal requested_signal
        requested_signal = signum

    def check_interrupt():
        if requested_signal is not None:
            raise interop.Interrupted(f"Interrupted by signal {requested_signal}")

    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            old_handlers[sig] = signal.signal(sig, interrupt)
        with ExitStack() as stack:
            runner = LabRunner(output, deadline - CLEANUP_RESERVE, stack, receipt, check_interrupt)
            try:
                receipt["test_binary"]["sha256"] = digest(args.test_binary)
                receipt["source_sha256"] = {name: digest(ROOT / name) for name in (
                    "scripts/ssh_path_lab.py", "scripts/openssh_interop.py",
                    "crates/keelshell-session/tests/ssh_path_interop.rs",
                )}
                sshd = interop.executable("sshd", ("/usr/sbin/sshd", "/usr/local/sbin/sshd"))
                keygen = interop.executable("ssh-keygen")
                # Independently prove that the supplied binary has this inventory.
                inventory = runner.start("inventory", [str(args.test_binary), "--list", "--ignored"])
                if runner.wait_bounded("inventory", inventory, 5) != 0:
                    raise interop.InteropFailure("Test inventory command failed")
                names = set((output / "inventory.log").read_text(encoding="utf-8").splitlines())
                for test in [SAMPLE_TEST, *EXTRA_TESTS]:
                    if test + ": test" not in names:
                        raise interop.InteropFailure("Prebuilt test inventory is missing " + test)
                username = pwd.getpwuid(os.getuid()).pw_name
                if not re.fullmatch(r"[A-Za-z0-9_.-]+\$?", username):
                    raise interop.InteropFailure("Account name cannot form an exact AllowUsers rule")
                # Keep keys outside the evidence directory and avoid /tmp's
                # world-writable ancestor, which StrictModes rejects on Linux.
                private_parent = ROOT / "work"
                private_parent.mkdir(parents=True, exist_ok=True)
                scratch = Path(tempfile.mkdtemp(prefix=".ssh-path-private-", dir=private_parent)).resolve()
                scratch.chmod(0o700)
                (scratch / "tmp").mkdir(mode=0o700)
                receipt["temporary_directory"] = str(scratch)
                ports = {}
                environment = {
                    # No ambient SSH agent, proxy credentials, Rust or test options.
                    "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
                    "HOME": str(scratch), "TMPDIR": str(scratch / "tmp"),
                    "LANG": "C", "KEELSHELL_PATH_USER": username,
                    "KEELSHELL_PATH_OPT_IN": "localhost-owned-fixture",
                }
                runner.run("openssh-version", [sshd, "-V"])
                for role in ("target", "jump"):
                    directory = scratch / role
                    directory.mkdir(mode=0o700)
                    for key in ("host", "client"):
                        runner.run(f"{role}-{key}-key", [
                            keygen, "-q", "-t", "ed25519", "-N", "", "-C", "",
                            "-f", str(directory / key),
                        ])
                    public = (directory / "client.pub").read_text(encoding="utf-8").strip()
                    # target can exec but cannot forward; jump can only forward
                    # to this exact target and cannot open shell/exec/PTY channels.
                    restriction = (
                        f'restrict,port-forwarding,permitopen="127.0.0.1:{ports["target"]}"'
                        if role == "jump" else "restrict"
                    )
                    (directory / "authorized_keys").write_text(
                        restriction + " " + public + "\n", encoding="utf-8",
                    )
                    (directory / "authorized_keys").chmod(0o600)
                    ports[role] = interop.reserve_port()
                    config = directory / "sshd_config"
                    config.write_text(server_config(directory, username, ports[role],
                                      ports["target"] if role == "jump" else None), encoding="utf-8")
                    # Only path references and rules are retained, never key
                    # contents. Fingerprints are public identity evidence.
                    (output / f"{role}-sshd_config.txt").write_text(
                        config.read_text(encoding="utf-8"), encoding="utf-8",
                    )
                    runner.run(f"{role}-host-fingerprint", [keygen, "-lf", str(directory / "host.pub")])
                    runner.run(f"{role}-config-check", [sshd, "-t", "-f", str(config)])
                    daemon = runner.daemon(f"{role}-sshd", [sshd, "-D", "-e", "-f", str(config)])
                    interop.wait_for_server(daemon, ports[role], runner.deadline,
                                            check_interrupt, runner.capture_owned)
                    prefix = "KEELSHELL_PATH_" + role.upper()
                    environment.update({
                        prefix + "_PORT": str(ports[role]),
                        prefix + "_HOST_KEY": str(directory / "host.pub"),
                        prefix + "_CLIENT_KEY": str(directory / "client"),
                    })
                receipt["endpoints"] = {role: f"127.0.0.1:{port}" for role, port in ports.items()}
                selected = SCENARIOS if args.scenario == "all" else (args.scenario,)
                passed = True
                for scenario in selected:
                    check_interrupt()
                    print(f"Running {scenario}: stream delay/throttle only", flush=True)
                    passed = run_scenario(runner, scenario, scratch, environment, ports, args) and passed
                if passed:
                    receipt["outcome"] = "passed"
                    result = 0
            finally:
                for sig in old_handlers:
                    signal.signal(sig, signal.SIG_IGN)
                cleanup = receipt["cleanup"]
                cleanup["processes"] = shutdown(runner, deadline)
                cleanup["ancestry_unverified"] = sorted(runner.ancestry_unverified)
                cleanup["owned_processes_stopped"] = (
                    all(row.get("observed_stopped") and row.get("reaped")
                        for row in cleanup["processes"]) and not runner.ancestry_unverified
                )
                if scratch is not None:
                    try:
                        shutil.rmtree(scratch)
                    except OSError as error:
                        cleanup["private_removal_error"] = str(error)
                cleanup["temporary_directory_removed"] = scratch is None or not scratch.exists()
    except interop.Interrupted as error:
        receipt.update(outcome="interrupted", error=str(error))
        result = 130
    except (interop.InteropFailure, OSError, ValueError, subprocess.SubprocessError) as error:
        receipt.update(outcome="failed", error=str(error))
        result = 1
    finally:
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
        if not all(receipt["cleanup"].get(field, False) for field in (
            "owned_processes_stopped", "temporary_directory_removed",
        )):
            receipt["outcome"] = "failed"
            receipt.setdefault("error", "Owned resource cleanup could not be confirmed")
            result = 1
        receipt.update(finished_utc=interop.utc_now(),
                       elapsed_seconds=round(time.monotonic() - started, 3),
                       returncode=result)
        receipt["logs"] = {p.name: {"bytes": p.stat().st_size, "sha256": digest(p)}
                           for p in sorted(output.glob("*.log"))}
        write_json(output / "result.json", receipt)
        print(f"SSH path lab: {receipt['outcome']}; exit {result}; {output}", flush=True)
    return result


if __name__ == "__main__":
    try:
        if len(sys.argv) == 3 and sys.argv[1] == "--gate":
            asyncio.run(serve_gate(json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))))
        else:
            sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"Cannot start SSH path lab: {error}", file=sys.stderr)
        sys.exit(1)
