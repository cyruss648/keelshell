"""Fixed ICMP adapter, bundled before probe.py; never installed on the target.

Only Linux iputils 20240117's exact version/config and quiet summary grammar
are admitted. The supervisor owns each process directly, including resolution.
No untrusted output is returned as display text. See ADR 0105 for provenance.
"""
import ipaddress
import json
import os
import re
import selectors
import signal
import stat
import subprocess
import sys
import time
from decimal import Decimal

ICMP_LIMIT = 32768
ICMP_COUNT = 4
ICMP_IMPLEMENTATION = "iputils_20240117"
ICMP_ENV = {"LC_ALL": "C", "LANG": "C", "LANGUAGE": "C",
            "PATH": "/usr/bin:/bin"}
ICMP_VERSION = re.compile(
    rb"ping from iputils 20240117\n"
    rb"libcap: (?:yes|no), IDN: (?:yes|no), NLS: (?:yes|no), "
    rb"error\.h: (?:yes|no), getrandom\(\): (?:yes|no), "
    rb"__fpending\(\): (?:yes|no)\n")
ICMP_COUNTS = re.compile(
    r"([1-4]) packets transmitted, ([0-4]) received"
    r"(?:, \+([1-9][0-9]{0,3}) duplicates)?"
    r"(?:, \+([1-9][0-9]{0,3}) corrupted)?"
    r"(?:, \+([1-9][0-9]{0,3}) errors)?"
    r", ([0-9]{1,3}(?:\.[0-9]{1,6})?)% packet loss, time ([0-9]{1,5})ms")
ICMP_RTT = re.compile(
    r"rtt min/avg/max/mdev = ([0-9]{1,4}\.[0-9]{3})/"
    r"([0-9]{1,4}\.[0-9]{3})/([0-9]{1,4}\.[0-9]{3})/"
    r"([0-9]{1,4}\.[0-9]{3}) ms(?:, pipe [2-4])?")


def icmp_packet(reason, implementation=None, address=None, counts=None, rtt=None):
    return {"reason": reason, "implementation": implementation,
            "address": address, "counts": counts, "rtt": rtt}


def icmp_parse_summary(data, address):
    """Accept the complete fixed quiet output, not a substring in arbitrary text."""
    text = data.decode("ascii")
    if (not text.endswith("\n") or
            any(ord(c) < 32 and c != "\n" for c in text) or "\x7f" in text):
        raise ValueError()
    lines = text.splitlines()
    if len(lines) != 5 or lines[1] != "":
        raise ValueError()
    # Ping receives a numeric literal and an explicit family. No host-supplied
    # name, reverse lookup, zone or other executable argument enters this grammar.
    header = (f"PING {address} ({address}) 56(84) bytes of data."
              if ipaddress.ip_address(address).version == 4
              else f"PING {address} ({address}) 56 data bytes")
    if lines[0] != header or lines[2] != f"--- {address} ping statistics ---":
        raise ValueError()
    match = ICMP_COUNTS.fullmatch(lines[3])
    if match is None:
        raise ValueError()
    sent, received = int(match[1]), int(match[2])
    duplicates, corrupted, errors = (int(match[n] or 0) for n in (3, 4, 5))
    loss = Decimal(match[6])
    if (received > sent or (duplicates and not received) or int(match[7]) > 8000 or
            abs(loss - Decimal((sent - received) * 100) / sent) > Decimal(".001")):
        raise ValueError()
    counts = {"transmitted": sent, "received": received, "duplicates": duplicates,
              "corrupted": corrupted, "errors": errors}
    # In the admitted upstream, hard_local_error also advances ntransmitted.
    # Quiet mode can hide local send failures without increasing nerrors.
    # These are tool-counted attempts/replies, not proven sends/network loss.
    rtt = None
    if received:
        match = ICMP_RTT.fullmatch(lines[4]) if len(lines) == 5 else None
        if match is None:
            raise ValueError()
        values = [int(Decimal(match[n]) * 1000) for n in range(1, 5)]
        if not (0 <= values[0] <= values[1] <= values[2] <= 8000000
                and 0 <= values[3] <= values[2]):
            raise ValueError()
        rtt = dict(zip(("min_us", "avg_us", "max_us", "mdev_us"), values))
    elif lines[4] and re.fullmatch(r"pipe [2-4]", lines[4]) is None:
        raise ValueError()
    return counts, rtt


def icmp_permission(stderr, executable):
    # Only exact C-locale errno diagnostics from known operations are mapped.
    # Unknown stderr is discarded, including tokens adjacent to these phrases.
    prefixes = (os.path.basename(executable), executable)
    operations = ("socket", "cap_get_proc", "cap_set_proc", "prctl",
                  "setuid", "seteuid")
    allowed = {f"{prefix}: {operation}: {error}"
               for prefix in prefixes for operation in operations
               for error in ("Operation not permitted", "Permission denied")}
    lines = stderr.decode("ascii", errors="replace").splitlines()
    return any(line in allowed for line in lines)


def icmp_run_owned(argv, until, remaining_output, stopped, env=None):
    """Bound stdout+stderr while running and always actually wait for our child.

    Each stage has its own process group and is a direct supervisor child.
    The DNS worker cannot launch tools; admitted iputils does not fork. Group
    termination also contains a fixture holding inherited pipes open.
    """
    child = None
    selector = selectors.DefaultSelector()
    streams = [bytearray(), bytearray()]
    state = "exited"
    try:
        if stopped():
            return "interrupted", None, b"", b""
        if time.monotonic() >= until:
            return "deadline", None, b"", b""
        child = subprocess.Popen(argv, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 shell=False, start_new_session=True, env=env)
        for index, pipe in enumerate((child.stdout, child.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, index)
        while selector.get_map():
            if stopped():
                state = "interrupted"
                break
            left = until - time.monotonic()
            if left <= 0:
                state = "deadline"
                break
            for key, _ in selector.select(min(.02, left)):
                chunk = os.read(key.fileobj.fileno(), min(4096, remaining_output[0] + 1))
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                remaining_output[0] -= len(chunk)
                if remaining_output[0] < 0:
                    state = "output_limit"
                    break
                streams[key.data].extend(chunk)
            if state != "exited":
                break
        if state == "exited":
            # A process can close both output streams before exiting.
            while child.poll() is None:
                if stopped() or time.monotonic() >= until:
                    state = "interrupted" if stopped() else "deadline"
                    break
                try:
                    child.wait(timeout=min(.02, max(.001, until - time.monotonic())))
                except subprocess.TimeoutExpired:
                    pass
    except FileNotFoundError:
        state = "utility_missing"
    except PermissionError:
        state = "permission_denied"
    except OSError:
        state = "process_failed"
    finally:
        selector.close()
        if child is not None:
            if child.returncode is None:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                except OSError:
                    # A failed kill is not a successful cleanup. Only the actual
                    # bounded wait below can establish that the owned child exited.
                    pass
            try:
                child.wait(timeout=.4)
            except (subprocess.TimeoutExpired, OSError):
                state = "cleanup_unknown"
            for pipe in (child.stdout, child.stderr):
                pipe.close()
    return state, None if child is None else child.returncode, bytes(streams[0]), bytes(streams[1])


def icmp_diagnostic(request, worker, started, stopped):
    """Return fixed metadata; never probe locally or fall back to TCP/another tool."""
    report = {"version": 1, "status": "unsupported_environment", "addresses": [],
              "peer": None, "tls": None, "http": None, "limited": False,
              "timing": {"resolve_ms": None, "connect_ms": None, "tls_ms": None,
                         "headers_ms": None, "total_ms": 0},
              "icmp": icmp_packet("unsupported_platform")}
    until = started + 7.5  # Reserve bounded actual reap inside the eight seconds.
    remaining_output = [ICMP_LIMIT]
    implementation = None
    address = None

    def finish(status, reason, counts=None, rtt=None):
        report["status"] = status
        report["icmp"] = icmp_packet(reason, implementation, address, counts, rtt)
        report["timing"]["total_ms"] = min(10000, round((time.monotonic() - started) * 1000))
        return report

    def process_failure(state):
        return finish({"deadline": "timeout", "interrupted": "interrupted",
                       "cleanup_unknown": "cleanup_unknown",
                       "utility_missing": "utility_unavailable",
                       "permission_denied": "permission_denied"}.get(state, "icmp_failure"),
                      state)

    if sys.platform != "linux" or os.name != "posix":
        return report
    executable = None
    for candidate in ("/usr/bin/ping", "/bin/ping"):
        try:
            info = os.stat(candidate)
            if stat.S_ISREG(info.st_mode):
                executable = candidate
                break
        except FileNotFoundError:
            continue
        except PermissionError:
            return finish("permission_denied", "permission_denied")
    if executable is None:
        return finish("utility_unavailable", "utility_missing")
    state, code, out, err = icmp_run_owned(
        [executable, "-V"], min(until, time.monotonic() + 1),
        remaining_output, stopped, ICMP_ENV)
    if state != "exited":
        return process_failure(state)
    if code != 0 or err or ICMP_VERSION.fullmatch(out) is None:
        if icmp_permission(err, executable):
            return finish("permission_denied", "permission_denied")
        return finish("unsupported_environment", "unsupported_implementation")
    implementation = ICMP_IMPLEMENTATION
    # Resolve in a separately supervised stdlib worker so a blocked libc resolver
    # cannot hold the supervisor. Only one selected numeric address is pinged.
    import base64
    dns = dict(request, kind="dns")
    encoded = base64.b64encode(json.dumps(dns).encode("ascii")).decode("ascii")
    state, code, out, err = icmp_run_owned(
        [sys.executable, "-I", "-S", "-B", "-c", worker, encoded],
        until, remaining_output, stopped)
    if state != "exited":
        return process_failure(state)
    try:
        if code != 0 or err or not out.startswith(b"KEELSHELL_DIAGNOSTIC_V1\n"):
            raise ValueError()
        dns = json.loads(out.split(b"\n", 1)[1])
        if dns["status"] != "success":
            if dns["status"] in ("dns_failure", "timeout", "unsupported_environment"):
                return finish(dns["status"], {"dns_failure": "resolution_failed",
                                             "timeout": "deadline",
                                             "unsupported_environment": "unsupported_platform"}[dns["status"]])
            raise ValueError()
        addresses = dns["addresses"]
        if not addresses or len(addresses) > 16:
            raise ValueError()
        for item in addresses:
            ip = ipaddress.ip_address(item["address"])
            if "%" in item["address"] or item["family"] != ("IPv4" if ip.version == 4 else "IPv6"):
                raise ValueError()
        report["addresses"] = addresses
        report["timing"]["resolve_ms"] = dns["timing"]["resolve_ms"]
        report["limited"] = dns["limited"]
        address = str(ipaddress.ip_address(addresses[0]["address"]))
    except (ValueError, KeyError, TypeError):
        return finish("icmp_failure", "invalid_output")
    argv = [executable, "-4" if ipaddress.ip_address(address).version == 4 else "-6",
            "-n", "-q", "-c", str(ICMP_COUNT), "-i", "1", "-s", "56", "-W", "1", "--", address]
    state, code, out, err = icmp_run_owned(argv, until, remaining_output, stopped, ICMP_ENV)
    if state not in ("exited", "deadline"):
        return process_failure(state)
    if state == "exited" and code != 0 and icmp_permission(err, executable):
        return finish("permission_denied", "permission_denied")
    counts = rtt = None
    try:
        counts, rtt = icmp_parse_summary(out, address)
    except (ValueError, UnicodeError):
        pass
    if state == "deadline":
        # SIGKILL does not promise a summary. Never invent zero loss/counters.
        return finish("timeout", "deadline", counts, rtt)
    if code not in (0, 1):
        return finish("icmp_failure", "process_failed")
    if err or counts is None or counts["transmitted"] != ICMP_COUNT:
        return finish("icmp_failure", "invalid_output")
    # There is deliberately no -w: the admitted source caps transmitted packets
    # at -c only without a deadline. Exit alone never establishes received counts.
    if (code == 1) != (counts["received"] == 0):
        return finish("icmp_failure", "invalid_output")
    if counts["errors"] or counts["corrupted"]:
        return finish("icmp_failure", "reported_errors", counts, rtt)
    if counts["received"] == 0:
        return finish("icmp_no_reply", "no_echo_reply", counts)
    if counts["received"] < ICMP_COUNT:
        # Retain the wire category only: packet_loss does not identify the cause
        # of unanswered attempts or establish that every attempt reached the wire.
        return finish("icmp_partial", "packet_loss", counts, rtt)
    return finish("success", "completed", counts, rtt)
