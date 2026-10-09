"""Remote supervisor: terminate and actually wait even if libc resolution blocks.

Python signal callbacks can be delayed by a C resolver. The worker's socket and
signal deadlines remain useful, but this separate process enforces the operation
budget. No daemon thread, persistent file or shell-spawned interpreter is used.
"""
import base64
import json
import os
import signal
import subprocess
import sys
import time

FRAME = "KEELSHELL_DIAGNOSTIC_V1\n"
started = time.monotonic()
child = None
output = None
status = "unsupported_environment"
stop_attempted = False
interrupted = False


def interrupt(_signum, _frame):
    global interrupted
    # A signal must not interrupt Popen between child creation and assignment,
    # or the actual reap. communicate retains the fixed eight-second deadline.
    interrupted = True


def terminal(category):
    return (FRAME + json.dumps({"version": 1, "status": category,
        "addresses": [], "peer": None, "tls": None, "http": None,
        "limited": False, "timing": {"resolve_ms": None, "connect_ms": None,
        "tls_ms": None, "headers_ms": None,
        "total_ms": min(10000, round((time.monotonic() - started) * 1000))}},
        separators=(",", ":")) + "\n").encode("ascii")


def stop_worker():
    global stop_attempted
    stop_attempted = True
    # The unreaped child retains its PID. Its new process group belongs solely
    # to this invocation, so no other user's processes are targeted.
    if child.poll() is None:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except OSError:
            # Denied or failed signalling cannot become a capability failure.
            # The bounded actual wait below decides confirmed vs unknown exit.
            pass
    try:
        child.communicate(timeout=1)
        return child.returncode is not None
    except (subprocess.TimeoutExpired, OSError):
        return False


try:
    if sys.version_info < (3, 8) or os.name != "posix":
        raise NotImplementedError()
    for number in (signal.SIGHUP, signal.SIGTERM, signal.SIGINT):
        signal.signal(number, interrupt)
    worker = base64.b64decode(sys.argv[2], validate=True).decode("utf-8")
    child = subprocess.Popen([sys.executable, "-I", "-S", "-B", "-c", worker,
                              sys.argv[1]], stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                             start_new_session=True)
    try:
        output, _ = child.communicate(timeout=max(0.001, 8 - (time.monotonic() - started)))
        if interrupted:
            status = "interrupted"
            output = None
        elif child.returncode != 0 or len(output) > 32768:
            output = None
        elif output.startswith(FRAME.encode("ascii")):
            payload = json.loads(output[len(FRAME):])
            payload["timing"]["total_ms"] = min(10000, round((time.monotonic() - started) * 1000))
            output = (FRAME + json.dumps(payload, separators=(",", ":")) + "\n").encode("ascii")
    except subprocess.TimeoutExpired:
        # communicate(timeout) does not kill or reap. Both actions are explicit.
        status = ("interrupted" if interrupted else "timeout") if stop_worker() else "cleanup_unknown"
        output = None
except Exception:
    output = None
finally:
    if child is not None and child.poll() is None and not stop_attempted:
        if not stop_worker():
            status = "cleanup_unknown"
    if output is None:
        output = terminal(status)
    sys.stdout.buffer.write(output)
    sys.stdout.buffer.flush()
