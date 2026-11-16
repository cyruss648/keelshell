#!/usr/bin/env python3
"""A short lived real process with owned loopback listeners and UDP bindings.

Its connected TCP client deliberately has a different local port from LISTEN.
The parent verifies readiness against its retained process handle. This program
neither launches children nor opens a non-loopback network connection.
"""

import argparse
from contextlib import ExitStack
import json
import os
import socket
import sys
import threading
import time


def open_endpoints(stack):
    """Keep all four listeners/bindings and one connected client alive."""
    endpoints = []
    connected = []
    for family, address in ((socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")):
        listener = stack.enter_context(socket.socket(family, socket.SOCK_STREAM))
        if family == socket.AF_INET6:
            listener.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
        listener.bind((address, 0))
        listener.listen(1)
        listener.settimeout(2)
        endpoints.append({"protocol": "tcp", "address": address,
                          "port": str(listener.getsockname()[1]), "pid": str(os.getpid())})
        client = stack.enter_context(socket.socket(family, socket.SOCK_STREAM))
        client.settimeout(2)
        client.connect(listener.getsockname())
        accepted, _ = listener.accept()
        stack.enter_context(accepted)
        connected.append({"protocol": "tcp", "address": address,
                          "port": str(client.getsockname()[1]), "pid": str(os.getpid())})
        udp = stack.enter_context(socket.socket(family, socket.SOCK_DGRAM))
        udp.bind((address, 0))
        endpoints.append({"protocol": "udp", "address": address,
                          "port": str(udp.getsockname()[1]), "pid": str(os.getpid())})
    return endpoints, connected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=float, default=90)
    args = parser.parse_args()
    if not 0 < args.seconds <= 180:
        parser.error("fixture lifetime must be in (0, 180] seconds")
    stop = threading.Event()

    def read_stop():
        # EOF also releases the fixture if its controller dies.
        sys.stdin.buffer.readline(16)
        stop.set()

    with ExitStack() as stack:
        endpoints, connected = open_endpoints(stack)
        # Keep real resident pages available to an independent process query.
        resident = bytearray(b"x" * (8 * 1024 * 1024))
        threading.Thread(target=read_stop, daemon=True).start()
        print(json.dumps({"version": 1, "pid": os.getpid(), "endpoints": endpoints,
                          "connected_only": connected}), flush=True)
        end = time.monotonic() + args.seconds
        while not stop.is_set() and time.monotonic() < end:
            stop.wait(min(0.05, max(0, end - time.monotonic())))
        assert resident[0] == ord("x")
    return 0 if stop.is_set() else 4


if __name__ == "__main__":
    raise SystemExit(main())
