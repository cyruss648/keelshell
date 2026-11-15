"""Diagnostic: unmodified external sz, empty file, controlled PTY drain timing."""
import binascii
import errno
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import sys
import tempfile
import time
import tty

batch = Path(__file__).resolve().parent
output = batch / sys.argv[1]
output.mkdir()


def header(kind, position=0):
    raw = bytes([kind]) + position.to_bytes(4, "little")
    crc = binascii.crc_hqx(raw, 0).to_bytes(2, "big")
    return b"**\x18B" + (raw + crc).hex().encode() + b"\r\n" + (
        b"" if kind in (3, 8) else b"\x11"
    )


def read_until(fd, marker):
    output = bytearray()
    deadline = time.monotonic() + 5
    while marker not in output:
        if time.monotonic() >= deadline:
            raise RuntimeError("independent empty-file probe deadline: " + output.hex())
        if select.select([fd], [], [], 0.05)[0]:
            output.extend(os.read(fd, 65536))
    return bytes(output)


results = []
for index in range(4):
    with tempfile.TemporaryDirectory(prefix="keelshell-peer-tail-") as temporary:
        root = Path(temporary).resolve()
        (root / "empty.bin").write_bytes(b"")
        master, slave = pty.openpty()
        tty.setraw(slave)
        with (output / f"tail-peer-{index}.log").open("xb") as log:
            environment = dict(
                os.environ,
                DYLD_INSERT_LIBRARIES=str(batch / "observe_flush.dylib"),
                KEELSHELL_DIAGNOSTIC_FLUSH_LOG=str(output / "tail-flush.jsonl"),
            )
            peer = subprocess.Popen(
                [str(batch / "peer/lsz"), "-b", "-vv", "--", "empty.bin"],
                cwd=root,
                stdin=slave,
                stdout=slave,
                stderr=log,
                env=environment,
            )
            os.close(slave)
            wire = bytearray()
            try:
                wire.extend(read_until(master, b"**\x18B00"))
                os.write(master, header(1, (0x60 << 24) | 1024))
                wire.extend(read_until(master, b"empty.bin"))
                os.write(master, header(9))
                wire.extend(read_until(master, b"*\x18C\x18K"))
                os.write(master, header(1, (0x60 << 24) | 1024))
                wire.extend(read_until(master, header(8)[:-2]))
                os.write(master, header(8))
                # Deliberately let the same peer exit before the PTY master is
                # scheduled again. This changes scheduling, never peer code.
                status = peer.wait(timeout=3)
                tail = bytearray()
                while select.select([master], [], [], 0)[0]:
                    try:
                        block = os.read(master, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            break
                        raise
                    if not block:
                        break
                    tail.extend(block)
                results.append(
                    {
                        "case": index,
                        "peer_exit": status,
                        "tail_hex": tail.hex(),
                        "farewell_received": b"OO" in tail,
                        "pre_farewell_wire_sha256": hashlib.sha256(wire).hexdigest(),
                        "peer_code_modified": False,
                        "master_read_held_until_peer_exit": True,
                    }
                )
            finally:
                if peer.poll() is None:
                    peer.kill()
                peer.wait()
                os.close(master)

record = {
    "boundary": "controlled local PTY empty-file wire diagnostic; no application acceptance",
    "peer_sha256": hashlib.sha256((batch / "peer/lsz").read_bytes()).hexdigest(),
    "cases": results,
    "all_peer_processes_reaped": True,
    "all_temporary_roots_removed": True,
}
(output / "result.json").write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps(record))
