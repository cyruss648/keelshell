#!/usr/bin/env python3
"""Small Linux rtnetlink netem adapter for the owned lab containers.

The wire layouts are the public Linux UAPI in linux/rtnetlink.h,
linux/pkt_sched.h and linux/gen_stats.h. No third party Python package or tc
installation is needed. Every response retains its original netlink bytes.
"""

import errno
import os
from pathlib import Path
import socket
import struct
import time

NLMSG = struct.Struct("=IHHII")
TCMSG = struct.Struct("=BBHiIII")
ROOT = 0xFFFFFFFF
HANDLE = 0x10000


def attribute(kind, payload):
    length = 4 + len(payload)
    return struct.pack("=HH", length, kind) + payload + b"\0" * (-length % 4)


def attributes(data):
    """Decode aligned rtattrs, rejecting truncation rather than inventing zeros."""
    result = {}
    while data:
        if len(data) < 4:
            raise ValueError("Truncated netlink attribute header")
        length, kind = struct.unpack_from("=HH", data)
        if length < 4 or length > len(data) or kind & 0x3FFF in result:
            raise ValueError("Invalid or duplicate netlink attribute")
        result[kind & 0x3FFF] = data[4:length]
        data = data[(length + 3) & ~3:]
    return result


def options(delay_ns, loss_u32, limit=1000):
    # LATENCY64 is nanoseconds; legacy latency uses architecture dependent
    # psched ticks. Leave the legacy value zero and require LATENCY64 readback.
    return (
        struct.pack("=6I", 0, limit, loss_u32, 0, 0, 0)
        + attribute(10, struct.pack("=q", delay_ns))
        + attribute(11, struct.pack("=q", 0))
    )


def decode_qdisc(message):
    if len(message) < NLMSG.size + TCMSG.size:
        raise ValueError("Truncated qdisc message")
    _, kind, _, _, _ = NLMSG.unpack_from(message)
    if kind != 36:  # RTM_NEWQDISC, including replies to RTM_GETQDISC.
        raise ValueError("Unexpected qdisc response type")
    _, _, _, index, handle, parent, info = TCMSG.unpack_from(message, NLMSG.size)
    attrs = attributes(message[NLMSG.size + TCMSG.size:])
    row = {
        "ifindex": index, "handle": handle, "parent": parent, "info": info,
        "kind": attrs.get(1, b"").rstrip(b"\0").decode("ascii"),
        "raw_netlink_hex": message.hex(),
    }
    if row["kind"] == "netem":
        raw = attrs.get(2, b"")
        if len(raw) < 24:
            raise ValueError("Missing netem options")
        latency, limit, loss, gap, duplicate, jitter = struct.unpack_from("=6I", raw)
        extended = attributes(raw[24:])
        row["options"] = {
            "legacy_latency_ticks": latency, "limit_packets": limit,
            "loss_u32": loss, "gap": gap, "duplicate_u32": duplicate,
            "legacy_jitter_ticks": jitter,
            "delay_ns": struct.unpack("=q", extended[10])[0] if 10 in extended else None,
            "jitter_ns": struct.unpack("=q", extended[11])[0] if 11 in extended else None,
            "kernel_prng_seed": struct.unpack("=Q", extended[14])[0] if 14 in extended else None,
        }
    if 7 in attrs:  # TCA_STATS2
        stats = attributes(attrs[7])
        counters = {}
        if 1 in stats and len(stats[1]) >= 12:
            counters["bytes"], counters["packets"] = struct.unpack_from("=QI", stats[1])
        if 8 in stats and len(stats[8]) == 8:  # TCA_STATS_PKT64
            counters["packets"] = struct.unpack("=Q", stats[8])[0]
        if 3 in stats and len(stats[3]) >= 20:
            counters.update(zip(
                ("qlen", "backlog", "drops", "requeues", "overlimits"),
                struct.unpack_from("=5I", stats[3]),
            ))
        row["counters"] = counters
    return row


def exchange(kind, flags, payload):
    """Use a bounded kernel socket; require kernel-origin, matching-sequence replies."""
    sequence = time.monotonic_ns() & 0xFFFFFFFF
    request = NLMSG.pack(NLMSG.size + len(payload), kind, flags, sequence, 0) + payload
    replies = []
    with socket.socket(socket.AF_NETLINK, socket.SOCK_RAW, socket.NETLINK_ROUTE) as netlink:
        netlink.settimeout(2)
        netlink.bind((0, 0))
        netlink.sendto(request, (0, 0))
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            packet, _, packet_flags, sender = netlink.recvmsg(1024 * 1024)
            if sender[0] != 0 or packet_flags & socket.MSG_TRUNC:
                raise ValueError("Non-kernel or truncated netlink response")
            while packet:
                if len(packet) < NLMSG.size:
                    raise ValueError("Truncated netlink header")
                length, reply_kind, reply_flags, reply_sequence, _ = NLMSG.unpack_from(packet)
                if length < NLMSG.size or length > len(packet) or reply_sequence != sequence:
                    raise ValueError("Malformed or unrelated netlink response")
                message = packet[:length]
                packet = packet[(length + 3) & ~3:]
                if reply_flags & 0x10:  # NLM_F_DUMP_INTR
                    raise ValueError("Kernel qdisc dump interrupted")
                if reply_kind in (2, 3):  # NLMSG_ERROR / NLMSG_DONE
                    if len(message) < NLMSG.size + 4:
                        raise ValueError("Truncated netlink completion")
                    error = struct.unpack_from("=i", message, NLMSG.size)[0]
                    if error:
                        raise OSError(-error, os.strerror(-error))
                    return replies
                replies.append(message)
    raise TimeoutError("Kernel netlink response deadline")


def guarded_namespace(owner):
    """Refuse an accidental invocation outside the runner's owned container."""
    if not Path("/run/.containerenv").is_file():
        raise PermissionError("An owned Podman container is required")
    marker = Path("/run/netem/owner")
    if not owner or marker.read_text(encoding="ascii").strip() != owner:
        raise PermissionError("Container owner marker mismatch")
    return os.readlink("/proc/self/ns/net")


def snapshot(owner):
    namespace = guarded_namespace(owner)
    index = socket.if_nametoindex("eth0")
    request = TCMSG.pack(socket.AF_UNSPEC, 0, 0, index, 0, ROOT, 0)
    messages = exchange(38, 1 | 0x300, request)  # RTM_GETQDISC / REQUEST / DUMP
    return {
        "schema": 1, "owner": owner, "network_namespace": namespace,
        "interface": "eth0", "ifindex": index,
        "qdiscs": [decode_qdisc(m) for m in messages if TCMSG.unpack_from(m, 16)[3] == index],
        "counter_unit": "Linux qdisc packets/skbs; GSO may aggregate wire packets",
    }


def replace(owner, delay_ns, loss_u32):
    guarded_namespace(owner)
    if type(delay_ns) is not int or not 0 <= delay_ns <= 1_000_000_000:
        raise ValueError("Invalid netem delay")
    if type(loss_u32) is not int or not 0 <= loss_u32 <= 0xFFFFFFFF:
        raise ValueError("Invalid random loss probability")
    index = socket.if_nametoindex("eth0")
    request = (
        TCMSG.pack(socket.AF_UNSPEC, 0, 0, index, HANDLE, ROOT, 0)
        + attribute(1, b"netem\0") + attribute(2, options(delay_ns, loss_u32))
    )
    # REQUEST | ACK | CREATE | REPLACE. This only affects this container's eth0.
    exchange(36, 1 | 4 | 0x400 | 0x100, request)
    return snapshot(owner)


def error_value(error):
    """Keep errno and a typed failure; unsupported kernels cannot become a pass."""
    return {
        "type": "KernelNetemUnavailable" if isinstance(error, OSError) and error.errno in (
            errno.ENOENT, errno.EOPNOTSUPP, errno.EPERM,
        ) else type(error).__name__,
        "message": str(error), "errno": getattr(error, "errno", None),
    }
