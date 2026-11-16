"""Fixed POSIX Python 3.8+ archive checks; no files, user arguments or network."""
import sys

sys.dont_write_bytecode = True

import errno
import gzip
import io
import json
import os
import selectors
import signal
import stat
import subprocess
import tarfile
import time

PROTOCOL = "keelshell.archive-validation.v1"
TOOLS = ("tar", "gzip", "zip", "unzip", "python3", "rz", "sz")
VERSION_ARGS = (("--version",), ("--version",), ("-v",), ("-v",),
                ("-I", "-S", "--version"), ("--version",), ("--version",))
PAYLOAD = b"KeelShell fixed archive stream probe.\n"
MEMBER = "keelshell-probe.txt"
MAX_PATH = 4096
MAX_VERSION = 256
MAX_RESPONSE = 32768
COMMAND_OUTPUT = 4096
TOTAL_OUTPUT = 16384
COMMAND_SECONDS = 1.0
TOTAL_SECONDS = 6.0
# One cleanup envelope covers leader reap and asynchronous group disappearance.
# collect's shared six-second deadline includes earlier cleanup; the final
# envelope therefore leaves room inside the transport's ten-second deadline.
CLEANUP_SECONDS = 1.0
STOPPED = False

# The child source is fully visible in the review command. Both formats stay in BytesIO.
PYTHON_STREAM = """import sys
sys.dont_write_bytecode = True
import io, tarfile, zipfile
payload = b"KeelShell fixed archive stream probe.\\n"
member = "keelshell-probe.txt"
z = io.BytesIO()
with zipfile.ZipFile(z, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    archive.writestr(member, payload)
with zipfile.ZipFile(io.BytesIO(z.getvalue()), "r") as archive:
    if (archive.namelist() != [member] or archive.read(member) != payload
            or archive.getinfo(member).compress_type != zipfile.ZIP_DEFLATED):
        sys.exit(65)
t = io.BytesIO()
with tarfile.open(fileobj=t, mode="w:gz", format=tarfile.USTAR_FORMAT) as archive:
    entry = tarfile.TarInfo(member)
    entry.size = len(payload)
    entry.mtime = 0
    archive.addfile(entry, io.BytesIO(payload))
with tarfile.open(fileobj=io.BytesIO(t.getvalue()), mode="r:gz") as archive:
    entries = archive.getmembers()
    if len(entries) != 1 or entries[0].name != member or not entries[0].isfile():
        sys.exit(65)
    stream = archive.extractfile(entries[0])
    if stream is None or stream.read(len(payload) + 1) != payload:
        sys.exit(65)
sys.stdout.buffer.write(payload)
"""

# Keep the process-group leader alive after the tool exits. A separate pipe reports the
# actual tool status; it is not inherited by the tool. The parent can then kill the owned
# group before reaping its leader, including descendants that closed all standard streams.
SUPERVISOR = """import sys
sys.dont_write_bytecode = True
import os, signal, subprocess
status_fd = int(sys.argv[1])
def stop_group(_number, _frame):
    os.killpg(os.getpgrp(), signal.SIGKILL)
signal.signal(signal.SIGALRM, stop_group)
signal.alarm(3)
try:
    code = subprocess.Popen(sys.argv[2:], shell=False, close_fds=True).wait()
    status = ("exited:%d\\n" % code).encode("ascii")
except (FileNotFoundError, PermissionError):
    status = b"unavailable\\n"
except Exception:
    status = b"failed\\n"
try:
    os.write(status_fd, status)
except OSError:
    stop_group(None, None)
os.close(status_fd)
for fd in (0, 1, 2):
    try:
        os.close(fd)
    except OSError:
        pass
while True:
    signal.pause()
"""


class CleanupUnknown(Exception):
    """Do not emit an adoptable report when the owned child's exit is unconfirmed."""

    def __init__(self, phase="cleanup_unconfirmed", number=None):
        # Fixed local phase and numeric errno only; no child output or paths.
        self.phase = phase
        self.errno = number
        super().__init__(phase, number)


def safe_text(value, limit):
    try:
        if len(value.encode("utf-8", "strict")) > limit:
            return False
    except UnicodeError:
        return False
    return not any(
        ord(c) < 32 or 127 <= ord(c) <= 159 or ord(c) == 0x061C
        or 0x200B <= ord(c) <= 0x200F or 0x2028 <= ord(c) <= 0x202E
        or 0x2060 <= ord(c) <= 0x206F or ord(c) in (0xFEFF, 0xFFFD)
        for c in value)


def clean_environment():
    # An allowlist also excludes loader hooks, Python startup/config and tool-specific options
    # such as TAR_OPTIONS/GZIP/ZIPOPT/UNZIP/UNZIPOPT/ZIPINFO/ZIPINFOOPT.
    value = os.environ.get("PATH", os.defpath)
    if len(value) > 16384:
        value = ""
    entries = [entry for entry in value.split(os.pathsep)[:64]
               if entry.startswith("/") and safe_text(entry, MAX_PATH)]
    return {"PATH": os.pathsep.join(entries), "LANG": "C", "LC_ALL": "C", "TZ": "UTC"}


def resolve(tool, env):
    for directory in env["PATH"].split(os.pathsep):
        if not directory:
            continue
        path = os.path.join(directory, tool)
        if not safe_text(path, MAX_PATH):
            continue
        try:
            if stat.S_ISREG(os.stat(path).st_mode) and os.access(path, os.X_OK):
                return path
        except (OSError, ValueError):
            pass
    return None


def interrupted(_number, _frame):
    # Only set a flag: raising in Popen before assignment can lose ownership of a child.
    global STOPPED
    STOPPED = True


def terminate_and_wait(child):
    until = time.monotonic() + CLEANUP_SECONDS
    # Never poll/reap the leader while inherited output pipes remain open. Its unreaped PID
    # prevents process-group reuse before this kill, including when the leader exited first.
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except OSError as error:
        raise CleanupUnknown("group_kill_failed", error.errno) from None
    try:
        child.wait(timeout=max(0, until - time.monotonic()))
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CleanupUnknown("leader_wait_unconfirmed", getattr(error, "errno", None)) from None
    # Observation after reap is non-destructive: a reused PGID can only produce an
    # unconfirmed result. Never send a destructive signal after leader ownership ends.
    last_error = None
    while True:
        try:
            os.killpg(child.pid, 0)
            last_error = None
        except ProcessLookupError:
            return
        except OSError as error:
            if error.errno != errno.EPERM:
                raise CleanupUnknown("group_query_failed", error.errno) from None
            # A reaped leader can leave a group awaiting system reaping. EPERM
            # is inconclusive, so only retry this non-destructive observation.
            # A new group reusing the number also cannot authorize another kill.
            last_error = error.errno
        remaining = until - time.monotonic()
        if remaining <= 0:
            raise CleanupUnknown("group_absence_unconfirmed", last_error)
        time.sleep(min(0.005, remaining))


def run_owned(argv, data, env, deadline, remaining_output):
    """Fixed bytes in; bounded concurrent stdout/stderr out. Never use communicate().

    A live supervisor keeps ownership of the session/process group until group cleanup.
    This includes ordinary descendants that close their standard streams. Escaped
    process groups and hostile executable behavior cannot be certified by this diagnostic.
    Popen uses no preexec hook; OS process creation itself is not a hard real-time primitive.
    """
    if STOPPED:
        raise CleanupUnknown()
    if time.monotonic() >= deadline:
        return "timed_out", None, b"", b""
    if remaining_output[0] <= 0:
        return "output_limit", None, b"", b""
    until = min(deadline, time.monotonic() + COMMAND_SECONDS)
    child = None
    selector = selectors.DefaultSelector()
    streams = [bytearray(), bytearray()]
    state = "passed"
    count = 0
    offset = 0
    control = bytearray()
    control_read = None
    control_write = None
    tool_code = None
    try:
        control_read, control_write = os.pipe()
        child = subprocess.Popen(
                                 [sys.executable, "-I", "-S", "-c", SUPERVISOR,
                                  str(control_write), *argv],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, shell=False, start_new_session=True,
                                 close_fds=True, pass_fds=(control_write,), env=env, bufsize=0)
        os.close(control_write)
        control_write = None
        os.set_blocking(control_read, False)
        selector.register(control_read, selectors.EVENT_READ, 3)
        for index, pipe in enumerate((child.stdout, child.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, index)
        if data:
            os.set_blocking(child.stdin.fileno(), False)
            selector.register(child.stdin, selectors.EVENT_WRITE, 2)
        else:
            child.stdin.close()
        while selector.get_map():
            if STOPPED:
                raise CleanupUnknown()
            left = until - time.monotonic()
            if left <= 0:
                state = "timed_out"
                break
            for key, _events in selector.select(min(0.02, left)):
                if time.monotonic() >= until:
                    state = "timed_out"
                    break
                if key.data == 2:
                    try:
                        offset += os.write(key.fd, data[offset:offset + 4096])
                    except BrokenPipeError:
                        selector.unregister(key.fileobj)
                        key.fileobj.close()
                        if offset != len(data):
                            state = "failed"
                        continue
                    except BlockingIOError:
                        continue
                    if offset == len(data):
                        selector.unregister(key.fileobj)
                        key.fileobj.close()
                    continue
                if key.data == 3:
                    try:
                        chunk = os.read(key.fd, 33 - len(control))
                    except BlockingIOError:
                        continue
                    if not chunk:
                        selector.unregister(control_read)
                    else:
                        control.extend(chunk)
                        if len(control) > 32:
                            state = "failed"
                            break
                    continue
                allowance = min(COMMAND_OUTPUT - count, remaining_output[0])
                try:
                    chunk = os.read(key.fd, min(4096, allowance + 1))
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                remaining_output[0] = max(0, remaining_output[0] - len(chunk))
                count += len(chunk)
                if count > COMMAND_OUTPUT or len(chunk) > allowance:
                    state = "output_limit"
                    break
                streams[key.data].extend(chunk)
            if state != "passed":
                break
        if STOPPED:
            raise CleanupUnknown()
        if time.monotonic() >= until:
            state = "timed_out"
        if state == "passed":
            if control == b"unavailable\n":
                state = "unavailable"
            elif control.startswith(b"exited:") and control.endswith(b"\n"):
                try:
                    tool_code = int(control[7:-1].decode("ascii"))
                    if not -255 <= tool_code <= 255:
                        state = "failed"
                except (ValueError, UnicodeError):
                    state = "failed"
                if tool_code != 0:
                    state = "failed"
            else:
                state = "failed"
            if offset != len(data):
                state = "failed"
        elif control.startswith(b"exited:") and control.endswith(b"\n"):
            try:
                tool_code = int(control[7:-1].decode("ascii"))
            except (ValueError, UnicodeError):
                pass
    except (FileNotFoundError, PermissionError):
        state = "unavailable"
    except (OSError, ValueError):
        state = "failed"
    finally:
        selector.close()
        for fd in (control_read, control_write):
            if fd is not None:
                os.close(fd)
        if child is not None:
            try:
                # Never poll or wait above: even on success the unreaped leader
                # still owns this PGID when its ordinary descendants are killed.
                terminate_and_wait(child)
            finally:
                for pipe in (child.stdin, child.stdout, child.stderr):
                    pipe.close()
    code = tool_code if tool_code is not None else (None if child is None else child.returncode)
    return state, code, bytes(streams[0]), bytes(streams[1])


def version_line(stdout, stderr):
    try:
        # Validate every captured line; do not silently strip escape/control characters.
        lines = (stdout + b"\n" + stderr).decode("utf-8", "strict").replace("\r\n", "\n").split("\n")
        if any(not safe_text(line, COMMAND_OUTPUT) for line in lines):
            return None
        first = next((line.strip() for line in lines if line.strip()), None)
        return first if first is not None and safe_text(first, MAX_VERSION) else None
    except UnicodeError:
        return None


def tar_input():
    memory = io.BytesIO()
    with tarfile.open(fileobj=memory, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        entry = tarfile.TarInfo(MEMBER)
        entry.size = len(PAYLOAD)
        entry.mtime = 0
        archive.addfile(entry, io.BytesIO(PAYLOAD))
    return memory.getvalue()


def collect():
    deadline = time.monotonic() + TOTAL_SECONDS
    budget = [TOTAL_OUTPUT]
    env = clean_environment()
    checks = []
    for tool, args in zip(TOOLS, VERSION_ARGS):
        path = resolve(tool, env)
        row = {"tool": tool, "path": path, "version": None,
               "version_status": "unavailable",
               "stream_check": "unavailable" if tool in ("tar", "gzip", "python3") else "not_tested"}
        if path is not None:
            state, _code, out, err = run_owned([path, *args], b"", env, deadline, budget)
            if state == "passed":
                row["version"] = version_line(out, err)
                if row["version"] is None:
                    state = "failed"
            row["version_status"] = state
            sample = None
            if tool == "tar":
                sample = ([path, "-xOf", "-", MEMBER], tar_input())
            elif tool == "gzip":
                sample = ([path, "-dc"], gzip.compress(PAYLOAD, mtime=0))
            elif tool == "python3":
                sample = ([path, "-I", "-S", "-c", PYTHON_STREAM], b"")
            if sample is not None:
                state, _code, out, err = run_owned(*sample, env, deadline, budget)
                if state == "passed" and (out != PAYLOAD or err):
                    state = "failed"
                row["stream_check"] = state
        checks.append(row)
    if STOPPED:
        raise CleanupUnknown()
    return {"protocol": PROTOCOL, "tools": checks}


def main():
    if sys.version_info < (3, 8) or os.name != "posix" or len(sys.argv) != 1:
        return 65
    for number in (signal.SIGHUP, signal.SIGTERM, signal.SIGINT):
        signal.signal(number, interrupted)
    try:
        packet = (json.dumps(collect(), ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
        if len(packet) > MAX_RESPONSE:
            return 65
        sys.stdout.buffer.write(packet)
        sys.stdout.buffer.flush()
        return 0
    except Exception:
        # Never reflect paths, exception text, diagnostics or an unconfirmed cleanup as success.
        return 65


if __name__ == "__main__":
    sys.exit(main())
