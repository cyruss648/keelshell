"""Independent bounded regression for ordinary descendants that close inherited pipes."""
import os
from pathlib import Path
import signal
import sys
import time
import types
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / "crates/keelshell-session/src/archive_validation.py"
probe = types.ModuleType("independent_archive_validation")
exec(compile(SOURCE.read_text(), str(SOURCE), "exec"), probe.__dict__)


class DescendantCleanup(unittest.TestCase):
    def test_same_group_descendant_cannot_survive_a_successful_leader(self):
        code = """import os,time
pid = os.fork()
if pid == 0:
    os.close(0)
    os.close(1)
    os.close(2)
    time.sleep(2.5)
    os._exit(0)
os.write(1, ("%d %d\\n" % (os.getpid(), pid)).encode())
os._exit(0)
"""
        state, status, out, err = probe.run_owned(
            [sys.executable, "-I", "-S", "-B", "-c", code],
            b"",
            probe.clean_environment(),
            time.monotonic() + 3,
            [4096],
        )
        leader, descendant = map(int, out.split())
        try:
            try:
                group = os.getpgid(descendant)
                survivor = True
            except ProcessLookupError:
                group = None
                survivor = False
            print(
                {
                    "helper_state": state,
                    "direct_exit": status,
                    "stderr": err.decode(),
                    "leader": leader,
                    "descendant": descendant,
                    "descendant_group": group,
                    "same_group": group == leader,
                    "descendant_still_exists": survivor,
                },
                flush=True,
            )
            self.assertEqual((state, status, err), ("passed", 0, b""))
            self.assertFalse(
                survivor,
                "successful helper returned while its ordinary same-group descendant was alive",
            )
        finally:
            try:
                if os.getpgid(descendant) == leader:
                    os.killpg(leader, signal.SIGKILL)
            except ProcessLookupError:
                pass
            until = time.monotonic() + 1
            while time.monotonic() < until:
                try:
                    os.kill(descendant, 0)
                except ProcessLookupError:
                    print("reviewer_cleanup: descendant absent", flush=True)
                    break
                time.sleep(0.01)
            else:
                print("reviewer_cleanup: SIGKILL sent; PID not yet absent", flush=True)


if __name__ == "__main__":
    unittest.main(verbosity=2)
