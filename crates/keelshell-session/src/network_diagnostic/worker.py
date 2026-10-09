"""Fixed supervised worker. Stdlib only; KEELSHELL_DIAGNOSTIC_WORKER_V1."""
import base64
import json
import os
import signal
import sys
import time

FRAME = "KEELSHELL_DIAGNOSTIC_V1\n"
started = time.monotonic()
result = {"version": 1, "status": "unsupported_environment", "addresses": [],
          "peer": None, "tls": None, "http": None, "limited": False,
          "timing": {"resolve_ms": None, "connect_ms": None, "tls_ms": None,
                     "headers_ms": None, "total_ms": 0}}
sock = None


def elapsed(at):
    return min(10000, max(0, round((time.monotonic() - at) * 1000)))


def remaining():
    return max(0.001, 8 - (time.monotonic() - started))


def expire(_signum, _frame):
    raise TimeoutError()


def text(value, limit=512):
    # Untrusted certificate names are literal escaped display data, not markup.
    value = str(value).encode("unicode_escape").decode("ascii")
    if len(value) > limit:
        result["limited"] = True
    return value[:limit]


try:
    import hashlib
    import http.client
    import socket
    import ssl

    if sys.version_info < (3, 8) or os.name != "posix" or not hasattr(signal, "setitimer"):
        raise NotImplementedError()
    request = json.loads(base64.b64decode(sys.argv[1], validate=True))
    signal.signal(signal.SIGALRM, expire)
    signal.setitimer(signal.ITIMER_REAL, remaining())
    status = "dns_failure"
    at = time.monotonic()
    addresses = socket.getaddrinfo(request["host"], request["port"],
                                   socket.AF_UNSPEC, socket.SOCK_STREAM)
    result["timing"]["resolve_ms"] = elapsed(at)
    unique = []
    for address in addresses:
        if address[0] not in (socket.AF_INET, socket.AF_INET6):
            continue
        if address[4][0] not in [item[4][0] for item in unique]:
            unique.append(address)
    if not unique:
        raise OSError()
    result["limited"] = len(unique) > 16
    unique = unique[:16]
    result["addresses"] = [{"family": "IPv4" if item[0] == socket.AF_INET else "IPv6",
                            "address": item[4][0]} for item in unique]
    if request["kind"] != "dns":
        status = "connection_failure"
        at = time.monotonic()
        for family, socktype, protocol, _name, address in unique:
            sock = socket.socket(family, socktype, protocol)
            sock.settimeout(remaining())
            try:
                sock.connect(address)
                break
            except (OSError, TimeoutError):
                sock.close()
                sock = None
                if time.monotonic() - started >= 8:
                    raise TimeoutError()
        if sock is None:
            raise OSError()
        result["timing"]["connect_ms"] = elapsed(at)
        result["peer"] = sock.getpeername()[0]
        if request["kind"] == "tls" or request["https"]:
            status = "tls_failure"
            at = time.monotonic()
            sock.settimeout(remaining())
            context = ssl.create_default_context()
            sock = context.wrap_socket(sock, server_hostname=request["host"])
            result["timing"]["tls_ms"] = elapsed(at)
            cert = sock.getpeercert()
            def name(field):
                return text(", ".join(key + "=" + value for row in cert.get(field, [])
                                      for key, value in row), 1024)
            names = [text(value, 256) for kind, value in cert.get("subjectAltName", [])
                     if kind in ("DNS", "IP Address")]
            if len(names) > 16:
                result["limited"] = True
            result["tls"] = {"protocol": text(sock.version(), 64),
                "cipher": text(sock.cipher()[0], 128), "subject": name("subject"),
                "issuer": name("issuer"), "not_before": text(cert.get("notBefore", ""), 64),
                "not_after": text(cert.get("notAfter", ""), 64), "names": names[:16],
                "sha256": hashlib.sha256(sock.getpeercert(binary_form=True)).hexdigest()}
        if request["kind"] == "http":
            status = "http_failure"
            # No redirects, proxy environment, cookies, auth or body are used.
            # Bound response header work as well as the SSH response envelope.
            http.client._MAXLINE = 8192
            http.client._MAXHEADERS = 32
            connection = http.client.HTTPConnection(request["host"], request["port"],
                                                    timeout=remaining())
            connection.sock = sock
            sock.settimeout(remaining())
            at = time.monotonic()
            connection.request("HEAD", request["path"], headers={"Connection": "close",
                               "User-Agent": "KeelShell diagnostic"})
            response = connection.getresponse()
            result["timing"]["headers_ms"] = elapsed(at)
            result["http"] = {"status": response.status,
                              "version": "HTTP/1.1" if response.version == 11 else "HTTP/1.0"}
            response.close()
            connection.close()
            sock = None
    result["status"] = "success"
except (ImportError, NotImplementedError):
    result["status"] = "unsupported_environment"
except TimeoutError:
    result["status"] = "timeout"
except ssl.SSLCertVerificationError:
    result["status"] = "certificate_rejected"
except Exception:
    # Never reflect exception text, response headers, URLs or credentials.
    result["status"] = locals().get("status", "unsupported_environment")
finally:
    if sock is not None:
        sock.close()
    if hasattr(signal, "setitimer"):
        signal.setitimer(signal.ITIMER_REAL, 0)
    result["timing"]["total_ms"] = elapsed(started)
    sys.stdout.write(FRAME + json.dumps(result, separators=(",", ":")) + "\n")
