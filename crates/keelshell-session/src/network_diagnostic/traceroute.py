"""Fixed Linux traceroute 2.1.5 adapter; no shell, installation or fallback.

The upstream option parser, print_header/print_probe and parse_icmp_res define
the admitted grammar. Complete hop rows survive deadlines; an incomplete last
row is unknown. The existing ICMP stage runner owns and actually waits children.
"""
TRACE_VERSION = (
    b"Modern traceroute for Linux, version 2.1.5\n"
    b"Copyright (c) 2016  Dmitry Butskoy,   License: GPL v2 or any later\n"
)
TRACE_IMPLEMENTATION = "linux_traceroute_2_1_5"
TRACE_ANNOTATION = re.compile(r"!(?:[NHXPSVC]|F-[0-9]{1,10}|<[0-9]{1,3}(?:-[0-9]{1,3})?>)")
TRACE_RESOLVER = """
import base64,ipaddress,json,socket,sys
try:
    request=json.loads(base64.b64decode(sys.argv[1],validate=True))
    family=socket.AF_INET if request['family']=='ipv4' else socket.AF_INET6
    found=[]
    for item in socket.getaddrinfo(request['host'],None,family,socket.SOCK_DGRAM):
        address=str(ipaddress.ip_address(item[4][0]))
        if '%' in address:
            continue
        if address not in found:
            found.append(address)
        if len(found)==16:
            break
    if not found:
        sys.exit(3)
    print(json.dumps(found,separators=(',',':')))
except Exception:
    sys.exit(3)
"""


def traceroute_options(request):
    """Recheck typed request before even executable admission can run."""
    options = request["traceroute"]
    if (request["kind"] != "traceroute" or options["protocol"] not in ("udp", "icmp")
            or options["family"] not in ("ipv4", "ipv6")
            or any(type(options[key]) is not int for key in ("max_hops", "probes_per_hop", "wait_ms"))
            or not 1 <= options["max_hops"] <= 16
            or not 1 <= options["probes_per_hop"] <= 3
            or not 100 <= options["wait_ms"] <= 1000
            or options["max_hops"] * options["probes_per_hop"] * options["wait_ms"] > 6000):
        raise ValueError()
    host = request["host"]
    if not isinstance(host, str) or not 1 <= len(host) <= 253 or not host.isascii():
        raise ValueError()
    try:
        ip = ipaddress.ip_address(host)
        if "%" in host or (ip.version == 4) != (options["family"] == "ipv4"):
            raise ValueError()
    except ValueError:
        if (":" in host or re.fullmatch(r"[0-9.]+", host)
                or not all(re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?", label)
                           for label in (host[:-1] if host.endswith(".") else host).split("."))):
            raise ValueError()
    return options


def traceroute_parse(data, address, options, partial=False):
    """Parse exact numeric rows; never return tool text as arbitrary display data."""
    text = data.decode("ascii")
    if any(ord(c) < 32 and c != "\n" for c in text) or "\x7f" in text:
        raise ValueError()
    lines = text.split("\n")
    # Fixed total packet size is 60 bytes for both IP families and both methods.
    header = f"traceroute to {address} ({address}), {options['max_hops']} hops max, 60 byte packets"
    if lines[0] != header:
        raise ValueError()
    if not partial and lines[-1] != "":
        raise ValueError()
    rows = lines[1:-1]  # Only newline-terminated hop rows have complete evidence.
    hops = []
    for line in rows:
        tokens = line.split()
        ttl = len(hops) + 1
        if not tokens or tokens.pop(0) != str(ttl) or ttl > options["max_hops"]:
            raise ValueError()
        probes = []
        previous = None
        while tokens:
            if tokens[0] == "*":
                tokens.pop(0)
                probes.append({"address": None, "rtt_us": None, "annotation": None})
                continue
            try:
                ip = ipaddress.ip_address(tokens[0])
            except ValueError:
                ip = None
            if ip is not None:
                if "%" in tokens[0] or ip.version != ipaddress.ip_address(address).version:
                    raise ValueError()
                previous = str(ip)
                tokens.pop(0)
            if previous is None or len(tokens) < 2:
                raise ValueError()
            milliseconds = tokens.pop(0)
            if re.fullmatch(r"[0-9]{1,4}\.[0-9]{3}", milliseconds) is None or tokens.pop(0) != "ms":
                raise ValueError()
            rtt = int(Decimal(milliseconds) * 1000)
            if rtt > options["wait_ms"] * 1000:
                raise ValueError()
            annotation = None
            if tokens and tokens[0].startswith("!"):
                annotation = tokens.pop(0)
                if TRACE_ANNOTATION.fullmatch(annotation) is None:
                    raise ValueError()
            probes.append({"address": previous, "rtt_us": rtt, "annotation": annotation})
            if len(probes) > options["probes_per_hop"]:
                raise ValueError()
        if len(probes) != options["probes_per_hop"]:
            raise ValueError()
        hops.append({"ttl": ttl, "probes": probes})
    return hops


def traceroute_diagnostic(request, started, stopped):
    report = {"version": 1, "status": "unsupported_environment", "addresses": [],
              "peer": None, "tls": None, "http": None, "limited": False,
              "icmp": None, "traceroute": None,
              "timing": {"resolve_ms": None, "connect_ms": None, "tls_ms": None,
                         "headers_ms": None, "total_ms": 0}}
    until = started + 7.5
    remaining_output = [32768]
    implementation = address = None

    def finish(status, reason, hops=None):
        report["status"] = status
        report["traceroute"] = {"reason": reason, "implementation": implementation,
                                "address": address, "hops": hops or []}
        report["timing"]["total_ms"] = min(10000, round((time.monotonic() - started) * 1000))
        return report

    def failure(state):
        return finish({"deadline": "timeout", "interrupted": "interrupted",
                       "cleanup_unknown": "cleanup_unknown",
                       "utility_missing": "utility_unavailable",
                       "permission_denied": "permission_denied"}.get(state, "traceroute_failure"), state)

    try:
        options = traceroute_options(request)
    except (ValueError, KeyError, TypeError):
        return finish("traceroute_failure", "invalid_output")
    if sys.platform != "linux" or os.name != "posix":
        return finish("unsupported_environment", "unsupported_platform")
    executable = None
    for candidate in ("/usr/bin/traceroute", "/bin/traceroute"):
        try:
            if stat.S_ISREG(os.stat(candidate).st_mode):
                executable = candidate
                break
        except FileNotFoundError:
            continue
        except PermissionError:
            return finish("permission_denied", "permission_denied")
    if executable is None:
        return finish("utility_unavailable", "utility_missing")
    state, code, out, err = icmp_run_owned(
        [executable, "--version"], min(until, time.monotonic() + .75),
        remaining_output, stopped, ICMP_ENV)
    if state != "exited":
        return failure(state)
    # Upstream --version writes these exact two lines to stderr, not stdout.
    if code != 0 or out or err != TRACE_VERSION:
        return finish("unsupported_environment", "unsupported_implementation")
    implementation = TRACE_IMPLEMENTATION
    at = time.monotonic()
    import base64
    encoded = base64.b64encode(json.dumps(
        {"host": request["host"], "family": options["family"]}).encode("ascii")).decode("ascii")
    state, code, out, err = icmp_run_owned(
        [sys.executable, "-I", "-S", "-B", "-c", TRACE_RESOLVER, encoded],
        until, remaining_output, stopped, ICMP_ENV)
    if state != "exited":
        return failure(state)
    if code == 3 and not out and not err:
        return finish("dns_failure", "resolution_failed")
    try:
        addresses = json.loads(out)
        if code != 0 or err or not isinstance(addresses, list) or not 1 <= len(addresses) <= 16:
            raise ValueError()
        for item in addresses:
            ip = ipaddress.ip_address(item)
            if "%" in item or (ip.version == 4) != (options["family"] == "ipv4"):
                raise ValueError()
        address = str(ipaddress.ip_address(addresses[0]))
        report["addresses"] = [{"family": "IPv4" if options["family"] == "ipv4" else "IPv6",
                                "address": str(ipaddress.ip_address(item))} for item in addresses]
        report["timing"]["resolve_ms"] = round((time.monotonic() - at) * 1000)
    except (ValueError, TypeError):
        return finish("traceroute_failure", "invalid_output")
    wait = f"{options['wait_ms'] // 1000}.{options['wait_ms'] % 1000:03d}"
    argv = [executable, "-4" if options["family"] == "ipv4" else "-6",
            "-n", "-M", "default" if options["protocol"] == "udp" else "icmp",
            "-f", "1", "-m", str(options["max_hops"]),
            "-q", str(options["probes_per_hop"]), "-N", "1", "-w", wait, "-z", "0",
            "--", address, "60"]
    state, code, out, err = icmp_run_owned(argv, until, remaining_output, stopped, ICMP_ENV)
    if state not in ("exited", "deadline", "interrupted"):
        return failure(state)
    hops = []
    try:
        hops = traceroute_parse(out, address, options, partial=state != "exited")
    except (ValueError, UnicodeError):
        if state == "exited":
            hops = None
    if state != "exited":
        return finish("timeout" if state == "deadline" else "interrupted", state, hops)
    if code != 0:
        # Exact C-locale errno only. Unknown utility/version/option diagnostics
        # remain failure, never a zero-hop successful route or arbitrary text.
        if err.strip() in (
                b"socket: Operation not permitted", b"socket: Permission denied",
                b"You do not have enough privileges to use this traceroute method.\n"
                b"socket: Operation not permitted"):
            return finish("permission_denied", "permission_denied")
        if code == 2:
            return finish("unsupported_environment", "unsupported_options")
        return finish("traceroute_failure", "process_failed")
    if err or not hops:
        return finish("traceroute_failure", "invalid_output")
    if any(p["annotation"] for hop in hops for p in hop["probes"]):
        return finish("traceroute_unreachable", "reported_unreachable", hops)
    if any(p["address"] == address for p in hops[-1]["probes"]):
        return finish("success", "destination_responded", hops)
    if len(hops) == options["max_hops"]:
        return finish("traceroute_hop_limit", "hop_limit", hops)
    return finish("traceroute_failure", "invalid_output")
