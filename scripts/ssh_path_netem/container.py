#!/usr/bin/env python3
"""Services for a disposable, isolated SSH path netem laboratory."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time

try:
    from . import qdisc
except ImportError:
    import qdisc

LAB = Path("/run/netem")


def identity():
    stat = Path("/proc/1/stat").read_text()
    status = dict(line.split(":", 1) for line in Path("/proc/1/status").read_text().splitlines()
                  if ":" in line)
    return {
        "network_namespace": os.readlink("/proc/self/ns/net"),
        "pid1_birth_ticks": int(stat[stat.rfind(")") + 2:].split()[19]),
        "boot_id": Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
        "effective_caps_hex": status["CapEff"].strip(),
        "bounding_caps_hex": status["CapBnd"].strip(),
    }


def config(role):
    forwarding = role == "jump"
    return "\n".join([
        f"Port {2223 if forwarding else 2222}", "AddressFamily inet",
        "ListenAddress 0.0.0.0", f"HostKey {LAB / role / 'host'}",
        f"AuthorizedKeysFile {LAB / role / 'authorized_keys'}",
        f"PidFile {LAB / role / 'sshd.pid'}", "AllowUsers netem",
        "AuthenticationMethods publickey", "PubkeyAuthentication yes",
        "PasswordAuthentication no", "KbdInteractiveAuthentication no",
        "PermitEmptyPasswords no", "UsePAM no", "StrictModes yes", "UseDNS no",
        "PermitRootLogin no", "PermitUserEnvironment no",
        "AllowAgentForwarding no", "AllowStreamLocalForwarding no",
        "PermitTunnel no", "X11Forwarding no", "PermitTTY no",
        "LoginGraceTime 10", "MaxAuthTries 2", "LogLevel ERROR",
        "AllowTcpForwarding local" if forwarding else "AllowTcpForwarding no",
        "PermitOpen 127.0.0.1:2222" if forwarding else "PermitOpen none",
        "MaxSessions 0" if forwarding else "MaxSessions 8", "",
    ])


async def servers(stop):
    processes = []
    try:
        LAB.chmod(0o755)
        Path("/run/sshd").mkdir(mode=0o755, exist_ok=True)
        for role in ("target", "jump"):
            directory = LAB / role
            directory.mkdir(mode=0o755)
            for key in ("host", "client"):
                subprocess.run([
                    "ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "",
                    "-f", str(directory / key),
                ], check=True, timeout=4, stdin=subprocess.DEVNULL)
            restriction = (
                'restrict,port-forwarding,permitopen="127.0.0.1:2222"'
                if role == "jump" else "restrict"
            )
            (directory / "authorized_keys").write_text(
                restriction + " " + (directory / "client.pub").read_text(),
            )
            (directory / "authorized_keys").chmod(0o644)
            path = directory / "sshd_config"
            path.write_text(config(role))
            subprocess.run(["/usr/sbin/sshd", "-t", "-f", str(path)],
                           check=True, timeout=4, stdin=subprocess.DEVNULL)
            processes.append(await asyncio.create_subprocess_exec(
                "/usr/sbin/sshd", "-D", "-e", "-f", str(path),
            ))
        # Readiness is confirmed by actual TCP listeners, not merely Popen.
        for port in (2222, 2223):
            until = time.monotonic() + 4
            while True:
                try:
                    reader, writer = await asyncio.wait_for(
                        asyncio.open_connection("127.0.0.1", port), 0.3,
                    )
                    del reader
                    writer.close()
                    await writer.wait_closed()
                    break
                except OSError:
                    if time.monotonic() >= until:
                        raise TimeoutError("Owned sshd did not listen")
                    await asyncio.sleep(0.05)
        (LAB / "ready.json").write_text(json.dumps(identity()))
        while not stop.is_set():
            if any(process.returncode is not None for process in processes):
                raise RuntimeError("Owned sshd exited")
            try:
                await asyncio.wait_for(stop.wait(), 0.2)
            except TimeoutError:
                pass
    finally:
        for process in processes:
            if process.returncode is None:
                process.terminate()
        for process in processes:
            try:
                await asyncio.wait_for(process.wait(), 2)
            except TimeoutError:
                process.kill()
                await process.wait()


async def forward(reader, writer):
    # No pacing, byte dropping, or simulated impairment. Only the Linux
    # interface qdisc may introduce the experiment's delay/random loss.
    while chunk := await reader.read(16384):
        writer.write(chunk)
        await writer.drain()
    if writer.can_write_eof():
        writer.write_eof()
        await writer.drain()


async def tunnel(reader, writer, upstream_host, target_port):
    upstream = None
    try:
        header = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 3)
        request = header.split(b"\r\n", 1)[0]
        prefix = f"CONNECT 127.0.0.1:{target_port} HTTP/1.".encode("ascii")
        if len(header) > 4096 or request not in (prefix + b"0", prefix + b"1"):
            writer.write(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            await writer.drain()
            return
        source, upstream = await asyncio.wait_for(
            asyncio.open_connection(upstream_host, target_port), 4,
        )
        writer.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        await writer.drain()
        async with asyncio.timeout(80):
            async with asyncio.TaskGroup() as tasks:
                tasks.create_task(forward(reader, upstream))
                tasks.create_task(forward(source, writer))
    except (OSError, EOFError, ValueError, TimeoutError, ExceptionGroup,
            asyncio.IncompleteReadError, asyncio.LimitOverrunError):
        pass
    finally:
        for stream in (upstream, writer):
            if stream is not None:
                stream.close()
                try:
                    await asyncio.wait_for(stream.wait_closed(), 0.5)
                except (OSError, TimeoutError):
                    pass


async def gate(stop, upstream_host, target_port):
    tasks = set()

    def accept(reader, writer):
        task = asyncio.create_task(tunnel(reader, writer, upstream_host, target_port))
        tasks.add(task)
        task.add_done_callback(tasks.discard)

    server = await asyncio.start_server(accept, "0.0.0.0", 8080, limit=4096)
    (LAB / "ready.json").write_text(json.dumps(identity()))
    try:
        await stop.wait()
    finally:
        server.close()
        for task in list(tasks):
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        await server.wait_closed()


async def serve(args):
    LAB.mkdir(mode=0o755, exist_ok=True)
    (LAB / "owner").write_text(args.owner, encoding="ascii")
    stop = asyncio.Event()
    for sig in (signal.SIGINT, signal.SIGTERM):
        asyncio.get_running_loop().add_signal_handler(sig, stop.set)
    if args.role == "servers":
        await servers(stop)
    else:
        await gate(stop, args.upstream, args.target_port)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("role", choices=("servers", "gate", "status", "qdisc"))
    parser.add_argument("--owner", required=True)
    parser.add_argument("--upstream")
    parser.add_argument("--target-port", type=int, choices=(2222, 2223))
    parser.add_argument("--delay-ns", type=int)
    parser.add_argument("--loss-u32", type=int)
    args = parser.parse_args()
    try:
        if args.role == "status":
            qdisc.guarded_namespace(args.owner)
            print((LAB / "ready.json").read_text())
        elif args.role == "qdisc":
            if args.delay_ns is None and args.loss_u32 is None:
                result = qdisc.snapshot(args.owner)
            elif args.delay_ns is not None and args.loss_u32 is not None:
                result = qdisc.replace(args.owner, args.delay_ns, args.loss_u32)
            else:
                raise ValueError("Both delay and loss are required")
            print(json.dumps(result))
        else:
            if args.role == "gate":
                socket.inet_pton(socket.AF_INET, args.upstream)
                if args.target_port is None:
                    raise ValueError("A gate needs an exact upstream port")
            asyncio.run(serve(args))
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(json.dumps({"failure": qdisc.error_value(error)}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
