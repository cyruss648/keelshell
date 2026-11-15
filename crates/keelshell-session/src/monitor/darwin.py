"""Read-only Darwin schema-1 resource collector, supplied through stdin."""

import ctypes as C
import math
import os
import re
import selectors
import signal
import subprocess
import sys
import time

MAX_OUTPUT = 2 * 1024 * 1024
MAX_CHILD_OUTPUT = 1024 * 1024
MAX_INTERFACES = 256
ALARM_SECONDS = 10
CHILD_SECONDS = 3.0
REAP_SECONDS = 1.0
U32_MAX = (1 << 32) - 1
U64_MAX = (1 << 64) - 1
CHILD_ENV = {
    "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
    "LC_ALL": "C",
    "LANG": "C",
}
NETSTAT_HEADER = (
    "Name Mtu Network Address Ipkts Ierrs Ibytes Opkts Oerrs Obytes Coll"
).split()


class CollectionFailure(Exception):
    """Invalid or unavailable core counters; contains no remote diagnostics."""


class DeadlineExceeded(BaseException):
    """The fixed overall deadline cannot be swallowed by an optional failure."""


class Timeval(C.Structure):
    # Xcode SDK sys/_types/_timeval.h and arm|i386/_types.h: LP64 long/int32.
    _fields_ = [("tv_sec", C.c_long), ("tv_usec", C.c_int32)]


class Timebase(C.Structure):
    # mach/mach_time.h: two uint32_t words, including on LP64.
    _fields_ = [("numer", C.c_uint32), ("denom", C.c_uint32)]


def require(condition):
    if not condition:
        raise CollectionFailure()


def unsigned(text, maximum=U64_MAX):
    require(bool(text) and text.isascii() and text.isdecimal())
    # Bound decimal conversion even on Python versions without an int digit cap.
    require(len(text) <= 20)
    value = int(text)
    require(value <= maximum)
    return value


def interface_name(name):
    return bool(re.fullmatch(r"[A-Za-z0-9_.-]{1,15}", name))


def deadline(_signum, _frame):
    raise DeadlineExceeded()


def run_bounded(argv, seconds=CHILD_SECONDS, limit=MAX_CHILD_OUTPUT):
    """Bound output, time and reaping of these owned, fixed-command children."""
    try:
        child = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            env=CHILD_ENV,
            cwd="/",
            close_fds=True,
            bufsize=0,
        )
    except FileNotFoundError:
        return "missing", b""
    except PermissionError:
        return "permission_denied", b""
    except OSError:
        return "failed", b""
    selector = None
    try:
        require(child.stdout is not None)
        selector = selectors.DefaultSelector()
        os.set_blocking(child.stdout.fileno(), False)
        selector.register(child.stdout, selectors.EVENT_READ)
        expires = time.monotonic() + seconds
        output = bytearray()
        while selector.get_map():
            remaining = expires - time.monotonic()
            if remaining <= 0:
                return "timeout", b""
            for key, _events in selector.select(min(remaining, 0.1)):
                try:
                    chunk = os.read(key.fd, min(65536, limit + 1 - len(output)))
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                output.extend(chunk)
                if len(output) > limit:
                    return "invalid_output", b""
        remaining = expires - time.monotonic()
        if remaining <= 0:
            return "timeout", b""
        try:
            code = child.wait(timeout=remaining)
        except subprocess.TimeoutExpired:
            return "timeout", b""
        return ("ok", bytes(output)) if code == 0 else ("failed", b"")
    finally:
        # This is exclusively the Popen child owned by this invocation.
        # Nothing enumerates, signals or otherwise acts on monitored processes.
        try:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=REAP_SECONDS)
        finally:
            if selector is not None:
                selector.close()
            if child.stdout is not None:
                child.stdout.close()


def parse_netstat(raw):
    """Accept only link-layer counters, never address-family duplicate rows."""
    require(len(raw) <= MAX_CHILD_OUTPUT)
    text = raw.decode("ascii")
    lines = text.splitlines()
    require(bool(lines) and lines[0].split() == NETSTAT_HEADER)
    result = []
    indices = set()
    for line in lines[1:]:
        fields = line.split()
        if len(fields) < 3:
            require(not fields)
            continue
        if not fields[2].startswith("<Link"):
            # IPv4/IPv6 rows do not represent independent interface totals.
            continue
        require(len(fields) in (10, 11))
        name = fields[0][:-1] if fields[0].endswith("*") else fields[0]
        require(interface_name(name))
        unsigned(fields[1], U32_MAX)
        link = re.fullmatch(r"<Link#([0-9]+)>", fields[2])
        require(link is not None)
        index = unsigned(link.group(1), U32_MAX)
        require(index > 0 and index not in indices)
        if len(fields) == 11:
            # A link address may be absent, as for loopback and tunnels.
            require(bool(re.fullmatch(r"[0-9A-Fa-f]{1,2}(?::[0-9A-Fa-f]{1,2})+", fields[3])))
        counters = [unsigned(value) for value in fields[-7:]]
        result.append((name, index, counters[2], counters[5]))
        require(len(result) <= MAX_INTERFACES)
        indices.add(index)
    require(bool(result))
    return result


def library():
    # Fixed system image; no library search, user configuration or site import.
    libc = C.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
    require(
        C.sizeof(C.c_void_p) == 8
        and C.sizeof(C.c_long) == 8
        and C.sizeof(C.c_uint32) == 4
        and C.sizeof(Timeval) == 16
        and Timeval.tv_usec.offset == 8
        and C.sizeof(Timebase) == 8
    )
    libc.sysctlbyname.argtypes = [
        C.c_char_p, C.c_void_p, C.POINTER(C.c_size_t), C.c_void_p, C.c_size_t
    ]
    libc.sysctlbyname.restype = C.c_int
    # host_t, natural_t and mach_msg_type_number_t are 32-bit on LP64;
    # host_info_t is integer_t*. vm_size_t alone is pointer-sized.
    libc.mach_host_self.argtypes = []
    libc.mach_host_self.restype = C.c_uint32
    libc.host_statistics.argtypes = [
        C.c_uint32, C.c_int, C.POINTER(C.c_int32), C.POINTER(C.c_uint32)
    ]
    libc.host_statistics.restype = C.c_int
    libc.host_page_size.argtypes = [C.c_uint32, C.POINTER(C.c_size_t)]
    libc.host_page_size.restype = C.c_int
    libc.mach_timebase_info.argtypes = [C.POINTER(Timebase)]
    libc.mach_timebase_info.restype = C.c_int
    libc.mach_continuous_time.argtypes = []
    libc.mach_continuous_time.restype = C.c_uint64
    # net/if.h: char *if_indextoname(unsigned int, char [IF_NAMESIZE=16]).
    libc.if_indextoname.argtypes = [C.c_uint32, C.POINTER(C.c_char)]
    libc.if_indextoname.restype = C.c_void_p
    return libc


def sysctl_value(libc, key, value):
    size = C.c_size_t(C.sizeof(value))
    require(libc.sysctlbyname(key, C.byref(value), C.byref(size), None, 0) == 0)
    require(size.value == C.sizeof(value))
    return value


def host_words(libc, host, flavor, count):
    data = (C.c_uint32 * count)()
    size = C.c_uint32(count)
    require(
        libc.host_statistics(
            host, flavor, C.cast(data, C.POINTER(C.c_int32)), C.byref(size)
        ) == 0
    )
    require(size.value == count)
    return data


def collect_networks(libc):
    status, raw = run_bounded(["/usr/sbin/netstat", "-ibn"])
    if status == "missing":
        # Required command absence shares the parent's typed missing-command path.
        raise FileNotFoundError()
    require(status == "ok")
    result = []
    names = set()
    for display_name, index, received, transmitted in parse_netstat(raw):
        name_buffer = C.create_string_buffer(16)
        require(bool(libc.if_indextoname(index, name_buffer)))
        require(b"\0" in name_buffer.raw)
        name = name_buffer.value.decode("ascii")
        # netstat can truncate its display column. Resolve the kernel name by
        # the Link index, then require that its displayed prefix agrees.
        require(interface_name(name) and name.startswith(display_name) and name not in names)
        names.add(name)
        result.append((name, received, transmitted))
    return result


def collect_core(libc):
    host = libc.mach_host_self()
    require(host != 0)
    total = sysctl_value(libc, b"hw.memsize", C.c_uint64()).value
    boot = sysctl_value(libc, b"kern.boottime", Timeval())
    require(boot.tv_sec > 0 and 0 <= boot.tv_usec < 1_000_000)
    page = C.c_size_t()
    require(libc.host_page_size(host, C.byref(page)) == 0)
    sysconf_page = os.sysconf("SC_PAGE_SIZE")
    require(
        4096 <= page.value <= 65536
        and page.value & (page.value - 1) == 0
        and page.value == sysconf_page
        and total > 0
        and total % page.value == 0
    )
    # host_info.h: HOST_VM_INFO=2, REV2_COUNT=15. vm_statistics.h:
    # fifteen natural_t words; free_count is word 0 (speculative already included).
    vm = host_words(libc, host, 2, 15)
    require(vm[0] != U32_MAX)
    free = vm[0] * page.value
    require(free <= total)
    # machine.h: USER=0, SYSTEM=1, IDLE=2, NICE=3; COUNT=4, flavor=3.
    cpu = host_words(libc, host, 3, 4)
    loads = os.getloadavg()
    require(len(loads) == 3 and all(math.isfinite(x) and x >= 0 for x in loads))
    networks = collect_networks(libc)
    base = Timebase()
    require(libc.mach_timebase_info(C.byref(base)) == 0)
    ticks = libc.mach_continuous_time()
    require(ticks > 0 and base.numer > 0 and base.denom > 0)
    ns = ticks * base.numer // base.denom
    require(0 < ns <= U64_MAX)
    fields = [
        ("schema", "1"),
        ("platform", "Darwin"),
        ("units", "bytes seconds ticks pages nanoseconds"),
        ("boot_time", boot.tv_sec),
        ("boot_time_usec", boot.tv_usec),
        ("mach_ticks", ticks),
        ("timebase_numer", base.numer),
        ("timebase_denom", base.denom),
        ("uptime_ns", ns),
        ("page_size", page.value),
        ("sysconf_page_size", sysconf_page),
        ("memory_total_bytes", total),
        ("memory_free_pages", vm[0]),
        ("memory_free_bytes", free),
        ("cpu_user", cpu[0]),
        ("cpu_nice", cpu[3]),
        ("cpu_system", cpu[1]),
        ("cpu_idle", cpu[2]),
        ("load_1", format(loads[0], ".17f")),
        ("load_5", format(loads[1], ".17f")),
        ("load_15", format(loads[2], ".17f")),
        ("network_count", len(networks)),
    ]
    fields.extend(("network", "%s\t%d\t%d" % row) for row in networks)
    return fields


def collect_capacity(skip_capacity):
    if skip_capacity:
        return "skipped", b""
    status, raw = run_bounded(["/bin/df", "-Pk"])
    if status == "ok":
        try:
            raw.decode("utf-8")
        except UnicodeError:
            return "invalid_output", b""
        if b"\0" in raw or b"\r" in raw:
            return "invalid_output", b""
    return status, raw


def render(fields, df_status, df):
    fields = list(fields) + [("df_status", df_status), ("df_bytes", len(df))]
    header = "@@KS:darwin@@\n" + "\n".join("%s\t%s" % row for row in fields)
    output = header.encode("ascii") + b"\n@@KS:df@@\n" + df + b"\n@@KS:end@@\n"
    require(len(output) <= MAX_OUTPUT)
    return output


def main():
    signal.signal(signal.SIGALRM, deadline)
    signal.alarm(ALARM_SECONDS)
    try:
        if sys.platform != "darwin":
            return 64
        require(len(sys.argv) == 2 and sys.argv[1] in ("0", "1"))
        core = collect_core(library())
        status, df = collect_capacity(sys.argv[1] == "1")
        output = render(core, status, df)
        sys.stdout.buffer.write(output)
        sys.stdout.buffer.flush()
        return 0
    except FileNotFoundError:
        return 66
    except (Exception, DeadlineExceeded):
        # No traceback, errno text, child diagnostic, address or argv is emitted.
        return 65
    finally:
        signal.alarm(0)


if __name__ == "__main__":
    sys.exit(main())
