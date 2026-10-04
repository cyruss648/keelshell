#!/usr/bin/env python3
"""Run opt-in transport tests against a disposable, pinned OpenSSH loopback server.

Requires existing sshd, ssh-keygen and Cargo installations on macOS or Linux.
Nothing is installed and the user's SSH configuration is never changed. Receipts
contain logs and process metadata; temporary host/client keys live separately and
are removed even after a failed or interrupted test run.
"""

import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import ctypes
import errno
import json
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
import uuid

ROOT = Path(__file__).resolve().parents[1]
TEST_COMMAND = [
    "cargo", "test", "-p", "keelshell-session", "--test", "openssh_interop",
    "--locked", "--", "--ignored", "--test-threads=1",
]
CLEANUP_RESERVE = 12.0
PROCESS_TABLE_TIMEOUT = 3.0


class InteropFailure(Exception):
    """An actionable precondition, process or deadline failure."""


class Interrupted(Exception):
    """A handled signal that must unwind through owned-resource cleanup."""


def utc_now():
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def executable(name, fallbacks=()):
    found = shutil.which(name)
    if found:
        # Cargo is often a rustup multi-call symlink: resolving that symlink
        # would change argv[0] and accidentally invoke the rustup CLI instead.
        return str(Path(found).absolute())
    for candidate in fallbacks:
        if Path(candidate).is_file() and os.access(candidate, os.X_OK):
            return str(Path(candidate).absolute())
    raise InteropFailure(f"Required executable {name!r} is missing; install it outside this script")


def config_value(value):
    """Quote a single sshd_config argument without allowing added directives."""
    value = str(value)
    if any(character in value for character in "\r\n\0"):
        raise InteropFailure("A configuration path contains a control character")
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def reserve_port():
    # sshd cannot inherit this socket. If another process wins the small bind
    # race, readiness or the tests' independent host-key pin fails closed.
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


class DarwinProcessInfo(ctypes.Structure):
    """Public proc_bsdinfo ABI from the macOS SDK's sys/proc_info.h."""
    _fields_ = [
        (name, ctypes.c_uint32) for name in (
            "flags", "status", "exit_status", "pid", "parent", "uid", "gid",
            "real_uid", "real_gid", "saved_uid", "saved_gid", "reserved",
        )
    ] + [
        ("command", ctypes.c_char * 16), ("name", ctypes.c_char * 32),
        ("files", ctypes.c_uint32), ("group", ctypes.c_uint32),
        ("job_count", ctypes.c_uint32), ("device", ctypes.c_uint32),
        ("terminal_group", ctypes.c_uint32), ("nice", ctypes.c_int32),
        ("birth_seconds", ctypes.c_uint64), ("birth_microseconds", ctypes.c_uint64),
    ]


LIBPROC = None


def process_info(pid):
    """Read ancestry and a kernel birth identity together, without ps rounding."""
    if platform.system() == "Linux":
        try:
            # comm may itself contain spaces or parentheses. Fields after its
            # final ')' begin with stat field 3; field 22 is boot-relative birth.
            content = Path(f"/proc/{pid}/stat").read_text(encoding="utf-8", errors="replace")
        except (FileNotFoundError, ProcessLookupError):
            return None
        fields = content[content.rfind(")") + 2:].split()
        return (int(fields[1]), int(fields[2]), ("linux", int(fields[19])), fields[0])
    if platform.system() != "Darwin":
        raise InteropFailure("Kernel process identities require macOS or Linux")
    global LIBPROC
    if LIBPROC is None:
        LIBPROC = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        LIBPROC.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
        LIBPROC.proc_pidinfo.restype = ctypes.c_int
    info = DarwinProcessInfo()
    ctypes.set_errno(0)
    count = LIBPROC.proc_pidinfo(pid, 3, 0, ctypes.byref(info), ctypes.sizeof(info))
    error = ctypes.get_errno()
    if count == 0 and error in (0, errno.ESRCH, errno.ENOENT):
        return None
    if count == 0 and error in (errno.EACCES, errno.EPERM):
        raise PermissionError(error, os.strerror(error))
    if count != ctypes.sizeof(info):
        raise InteropFailure(f"Cannot verify process identity for PID {pid}")
    return (info.parent, info.group, ("darwin", info.birth_seconds, info.birth_microseconds), "Z" if info.status == 5 else "R")


def process_table(deadline):
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise InteropFailure("Deadline reached while inspecting owned processes")
    # Loaded CI hosts can take more than 0.5 seconds to enumerate processes.
    # Keep a bounded probe inside the existing overall deadline; a timeout must
    # fail closed rather than masquerade as an empty process table.
    try:
        result = subprocess.run(
            ["ps", "-eo", "pid="], capture_output=True, text=True,
            check=True, timeout=min(PROCESS_TABLE_TIMEOUT, remaining),
        )
    except subprocess.TimeoutExpired as error:
        raise InteropFailure("Process inventory exceeded its bounded deadline") from error
    table = {}
    for value in result.stdout.split():
        if time.monotonic() >= deadline:
            raise InteropFailure("Deadline reached while reading process identities")
        if value.isdigit():
            pid = int(value)
            try:
                info = process_info(pid)
            except PermissionError:
                # Other users' processes need not be inspected or signalled.
                continue
            if info is not None:
                table[pid] = info
    return table


def same_process(current, recorded):
    # setsid/reparenting changes group/parent, not kernel birth identity.
    return current is not None and current[2] == recorded[2] and not current[3].startswith("Z")


def remember_owned(process, identities, table):
    """Extend cumulative ownership from the original process and known children."""
    # An unreaped, live Popen PID cannot have been reused. Never adopt a PID
    # merely because it matches an already-reaped process or old process group.
    if process.poll() is None and process.pid not in identities and process.pid in table:
        identities[process.pid] = table[process.pid]
    parents = {pid for pid, info in identities.items() if same_process(table.get(pid), info)}
    while parents:
        children = {
            pid for pid, info in table.items()
            if info[0] in parents and (pid not in identities or not same_process(info, identities[pid]))
            and same_process(process_info(info[0]), identities[info[0]])
        }
        for pid in children:
            identities[pid] = table[pid]
        parents = children


def signal_owned(pid, identity, sig):
    """Signal only the recorded process; return whether a signal was delivered."""
    if hasattr(os, "pidfd_open") and hasattr(signal, "pidfd_send_signal"):
        try:
            descriptor = os.pidfd_open(pid)
        except ProcessLookupError:
            return False
        try:
            if same_process(process_info(pid), identity):
                signal.pidfd_send_signal(descriptor, sig)
                return True
        except ProcessLookupError:
            pass
        finally:
            os.close(descriptor)
        return False
    if same_process(process_info(pid), identity):
        try:
            os.kill(pid, sig)
            return True
        except ProcessLookupError:
            pass
    return False


def stop_owned(process, deadline, identities, unexpected_exit=None):
    """Drain cumulative owned identities even after the original parent exits.

    Polling during normal execution preserves ancestry before reparenting. Each
    signal rechecks a kernel birth identity; Linux additionally binds the signal
    to a pidfd. Zombies have exited and only their new parent can reap them.
    """
    if process is None:
        return True
    if process.poll() is not None and unexpected_exit is not None:
        unexpected_exit()
    for sig in (signal.SIGTERM, signal.SIGKILL):
        table = process_table(deadline)
        remember_owned(process, identities, table)
        # Stop the known fork source before draining its observed children.
        if process.pid in identities:
            delivered = signal_owned(process.pid, identities[process.pid], sig)
            if sig == signal.SIGTERM and not delivered and unexpected_exit is not None:
                unexpected_exit()
        for pid, identity in list(identities.items()):
            if pid != process.pid:
                signal_owned(pid, identity, sig)
        until = min(deadline, time.monotonic() + 1.0)
        while time.monotonic() < until:
            process.poll()  # Reap our direct child even if every descendant exited.
            table = process_table(deadline)
            remember_owned(process, identities, table)
            live = {pid: info for pid, info in identities.items() if same_process(table.get(pid), info)}
            if process.returncode is not None and not live:
                return True
            # Include a late observed fork without restarting the cleanup budget.
            for pid, identity in live.items():
                signal_owned(pid, identity, sig)
            time.sleep(min(0.05, max(0, until - time.monotonic())))
    return False


class Runner:
    def __init__(self, output, deadline, stack, receipt, check_interrupt):
        self.output = output
        self.deadline = deadline
        self.stack = stack
        self.receipt = receipt
        self.processes = []
        self.owned = {}
        self.names = {}
        self.ancestry_unverified = set()
        self.check_interrupt = check_interrupt

    def start(self, name, command, environment=None):
        self.check_interrupt()
        if time.monotonic() >= self.deadline:
            raise InteropFailure("Overall deadline reached before " + name)
        log = self.stack.enter_context((self.output / f"{name}.log").open("wb"))
        process = subprocess.Popen(
            command, cwd=ROOT, env=environment, stdin=subprocess.DEVNULL,
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
        )
        self.processes.append(process)
        self.owned[process] = {}
        self.names[process] = name
        info = process_info(process.pid)
        if info is not None:
            self.owned[process][process.pid] = info
        self.capture_owned()
        self.receipt["processes"].append({"name": name, "pid": process.pid})
        return process

    def capture_owned(self):
        table = process_table(self.deadline)
        for process in self.processes:
            remember_owned(process, self.owned[process], table)
            if self.names[process] == "sshd" and process.poll() is not None:
                # A child could have detached between observations before this
                # unexpected exit. Still clean recorded children, but do not
                # certify that the unobserved ancestry is absent.
                self.ancestry_unverified.add(process.pid)

    def wait(self, name, process, server=None):
        while True:
            self.capture_owned()
            if process.poll() is not None:
                break
            self.check_interrupt()
            if server is not None and server.poll() is not None:
                raise InteropFailure(f"OpenSSH exited during {name}; inspect sshd.log")
            if time.monotonic() >= self.deadline:
                raise InteropFailure(f"Overall deadline reached during {name}")
            time.sleep(0.05)
        self.check_interrupt()
        self.receipt["steps"].append({"name": name, "returncode": process.returncode})
        if process.returncode != 0:
            raise InteropFailure(f"{name} exited with {process.returncode}; inspect {name}.log")

    def run(self, name, command, environment=None):
        process = self.start(name, command, environment)
        self.wait(name, process)


def wait_for_server(process, port, deadline, check_interrupt, capture_owned):
    until = min(deadline, time.monotonic() + 12)
    while time.monotonic() < until:
        capture_owned()
        check_interrupt()
        if process.poll() is not None:
            raise InteropFailure(f"OpenSSH exited during startup ({process.returncode}); inspect sshd.log")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.15) as peer:
                banner = peer.recv(256)
                if banner.startswith(b"SSH-2.0-OpenSSH_") and process.poll() is None:
                    return
        except (OSError, TimeoutError):
            pass
        time.sleep(0.05)
    raise InteropFailure("OpenSSH was not ready within its startup deadline; inspect sshd.log")


def create_output(requested):
    if requested:
        output = Path(requested).absolute()
        output.parent.mkdir(parents=True, exist_ok=True)
        # Never reuse or overwrite a prior run's evidence.
        output.mkdir(mode=0o700)
    else:
        parent = ROOT / "work" / "openssh-interop"
        parent.mkdir(parents=True, exist_ok=True)
        name = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:10]
        output = parent / name
        output.mkdir(mode=0o700)
    return output.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", help="New receipt directory; it must not already exist")
    parser.add_argument("--timeout", type=float, default=300, help="Total seconds including cleanup (15–300, default 300)")
    arguments = parser.parse_args()
    if platform.system() not in ("Darwin", "Linux"):
        parser.error("OpenSSH interoperability is supported only on macOS and Linux; Windows is not supported")
    if not 15 <= arguments.timeout <= 300:
        parser.error("--timeout must be between 15 and 300 seconds, including cleanup")

    # Imported after the platform check so unsupported Windows invocation still
    # gives the deliberate diagnostic, rather than a missing pwd-module error.
    import pwd

    start = time.monotonic()
    overall_deadline = start + arguments.timeout
    os.umask(0o077)
    output = create_output(arguments.output)
    receipt = {
        "schema": 1, "started_utc": utc_now(), "platform": platform.system(),
        "timeout_seconds": arguments.timeout, "outcome": "failed", "steps": [],
        "processes": [], "command": TEST_COMMAND, "cleanup": {},
        "boundary": "Disposable localhost OpenSSH SFTP and exec interoperability; not production or target-native desktop acceptance",
    }
    print(f"OpenSSH interoperability receipt: {output}", flush=True)
    scratch = None
    runner = None
    old_handlers = {}
    result = 1
    requested_signal = None

    def interrupt(signum, _):
        # Do not raise between Popen returning and registration: an async
        # exception in that gap could orphan an untracked daemon. The bounded
        # polling loops observe the request after ownership is established.
        nonlocal requested_signal
        requested_signal = signum

    def check_interrupt():
        if requested_signal is not None:
            raise Interrupted(f"Interrupted by signal {requested_signal}")

    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            old_handlers[sig] = signal.signal(sig, interrupt)
        with ExitStack() as stack:
            runner = Runner(output, overall_deadline - CLEANUP_RESERVE, stack, receipt, check_interrupt)
            try:
                sshd = executable("sshd", ("/usr/sbin/sshd", "/usr/local/sbin/sshd"))
                keygen = executable("ssh-keygen", ("/usr/bin/ssh-keygen",))
                cargo = executable("cargo")
                username = pwd.getpwuid(os.getuid()).pw_name
                if not re.fullmatch(r"[A-Za-z0-9_.-]+\$?", username):
                    raise InteropFailure("Current account name cannot be represented as an exact AllowUsers rule")
                private_parent = ROOT / "work" / ".openssh-private"
                private_parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                scratch = Path(tempfile.mkdtemp(prefix="run-", dir=private_parent)).resolve()
                scratch.chmod(0o700)
                (scratch / "tmp").mkdir(mode=0o700)
                receipt["temporary_directory"] = str(scratch)
                receipt["sshd"] = sshd
                runner.run("openssh-version", [sshd, "-V"])
                for name in ("host", "client"):
                    runner.run(f"generate-{name}-key", [keygen, "-q", "-t", "ed25519", "-N", "", "-C", "", "-f", str(scratch / name)])
                port = reserve_port()
                receipt["listen"] = f"127.0.0.1:{port}"
                config = scratch / "sshd_config"
                config.write_text("\n".join([
                    f"Port {port}", "AddressFamily inet", "ListenAddress 127.0.0.1",
                    f"HostKey {config_value(scratch / 'host')}",
                    f"AuthorizedKeysFile {config_value(scratch / 'client.pub')}",
                    f"PidFile {config_value(scratch / 'sshd.pid')}",
                    f"AllowUsers {username}", "PubkeyAuthentication yes",
                    "AuthenticationMethods publickey", "PasswordAuthentication no",
                    "KbdInteractiveAuthentication no", "PermitEmptyPasswords no", "UsePAM no",
                    "StrictModes yes", "UseDNS no", "PermitRootLogin prohibit-password",
                    "PermitUserEnvironment no", "AllowAgentForwarding no",
                    "AllowTcpForwarding no", "AllowStreamLocalForwarding no", "PermitTunnel no",
                    "X11Forwarding no", "PermitTTY no", "LoginGraceTime 10", "MaxAuthTries 2",
                    "MaxSessions 8", "LogLevel ERROR", "Subsystem sftp internal-sftp", "",
                ]), encoding="utf-8")
                runner.run("validate-sshd-config", [sshd, "-t", "-f", str(config)])
                server = runner.start("sshd", [sshd, "-D", "-e", "-f", str(config)])
                wait_for_server(server, port, runner.deadline, check_interrupt, runner.capture_owned)
                environment = dict(os.environ, **{
                    "TMPDIR": str((scratch / "tmp").resolve()),
                    "KEELSHELL_OPENSSH_USER": username,
                    "KEELSHELL_OPENSSH_PORT": str(port),
                    "KEELSHELL_OPENSSH_HOST_KEY": str(scratch / "host.pub"),
                    "KEELSHELL_OPENSSH_CLIENT_KEY": str(scratch / "client"),
                    "KEELSHELL_OPENSSH_ROOT": str(scratch),
                })
                tests = runner.start("tests", [cargo, *TEST_COMMAND[1:]], environment)
                runner.wait("tests", tests, server)
                summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", (output / "tests.log").read_text(encoding="utf-8", errors="replace"))
                if summary is None or int(summary.group(1)) < 6:
                    raise InteropFailure("Cargo did not confirm the required OpenSSH interoperability tests; inspect tests.log")
                receipt["passed_tests"] = int(summary.group(1))
                if server.poll() is not None:
                    runner.ancestry_unverified.add(server.pid)
                    raise InteropFailure("OpenSSH exited before test completion was recorded")
                check_interrupt()
                receipt["outcome"] = "passed"
                result = 0
            finally:
                # A second Ctrl-C must not leave credentials or a test daemon.
                for sig in old_handlers:
                    signal.signal(sig, signal.SIG_IGN)
                cleanup = []
                for process in runner.processes:
                    if runner.names[process] == "sshd" and process.poll() is not None:
                        runner.ancestry_unverified.add(process.pid)
                for process in reversed(runner.processes):
                    try:
                        on_exit = None
                        if runner.names[process] == "sshd":
                            on_exit = lambda pid=process.pid: runner.ancestry_unverified.add(pid)
                            if process.poll() is not None:
                                on_exit()
                        cleanup.append(stop_owned(process, overall_deadline, runner.owned[process], on_exit))
                    except (InteropFailure, OSError, subprocess.SubprocessError) as error:
                        # Never fall back to an unverified historical PGID.
                        # Try individually recorded identities, then report any
                        # process-table failure as unverified cleanup.
                        for pid, identity in runner.owned[process].items():
                            try:
                                signal_owned(pid, identity, signal.SIGKILL)
                            except (InteropFailure, OSError):
                                pass
                        try:
                            process.wait(timeout=max(0.01, min(0.25, overall_deadline-time.monotonic())))
                        except subprocess.SubprocessError:
                            pass
                        cleanup.append(False)
                        receipt["cleanup"].setdefault("errors", []).append(str(error))
                receipt["cleanup"]["observed_processes_stopped"] = all(cleanup)
                receipt["cleanup"]["ancestry_unverified"] = sorted(runner.ancestry_unverified)
                receipt["cleanup"]["owned_processes_stopped"] = all(cleanup) and not runner.ancestry_unverified
                receipt["cleanup"]["ownership_boundary"] = "Cumulative ancestry observations with kernel birth identity checks; an unexpected daemon exit cannot certify descendants that detached entirely between observations"
                receipt["cleanup"]["tracked_process_identities"] = sum(len(identities) for identities in runner.owned.values())
                if scratch is not None:
                    try:
                        shutil.rmtree(scratch)
                    except OSError as error:
                        receipt["cleanup"].setdefault("errors", []).append(str(error))
                    receipt["cleanup"]["temporary_directory_removed"] = not scratch.exists()
                else:
                    receipt["cleanup"]["temporary_directory_removed"] = True
    except Interrupted as error:
        receipt["outcome"] = "interrupted"
        receipt["error"] = str(error)
        result = 130
    except (InteropFailure, OSError, subprocess.SubprocessError) as error:
        receipt["outcome"] = "failed"
        receipt["error"] = str(error)
        result = 1
    finally:
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
        if not all(receipt["cleanup"].get(key, False) for key in (
            "owned_processes_stopped", "temporary_directory_removed",
        )):
            receipt["outcome"] = "failed"
            receipt.setdefault("error", "Owned process or temporary credential cleanup did not complete")
            result = 1
        receipt["finished_utc"] = utc_now()
        receipt["elapsed_seconds"] = round(time.monotonic() - start, 3)
        receipt["logs"] = {path.name: path.stat().st_size for path in sorted(output.glob("*.log"))}
        (output / "result.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(f"OpenSSH interoperability: {receipt['outcome']} ({receipt['elapsed_seconds']}s). Logs: {output}", flush=True)
        if "error" in receipt:
            print(receipt["error"], file=sys.stderr)
    return result


if __name__ == "__main__":
    try:
        sys.exit(main())
    except OSError as error:
        print(f"Cannot create a new interoperability receipt: {error}", file=sys.stderr)
        sys.exit(1)
