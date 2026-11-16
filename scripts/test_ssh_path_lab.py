"""Real loopback forwarding and bounded failure/evidence tests; no Rust or Cargo."""

import asyncio
from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import platform
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import MagicMock, patch

import openssh_interop as interop
import ssh_path_lab as lab


requires_posix_runner = unittest.skipUnless(
    platform.system() in ("Darwin", "Linux"),
    "Lab runners require macOS/Linux kernel birth identities and POSIX signals",
)


def complete_report():
    samples = []
    for round_number in range(1, 4):
        sides = ("B", "A") if round_number == 2 else ("A", "B")
        for side in sides:
            for kind in ("Exec", "Upload", "Download"):
                samples.append({
                    "path": side, "kind": kind, "round": round_number,
                    "elapsed_ns": round_number * 1_000_000,
                    "bytes": 0 if kind == "Exec" else 262144,
                    "outcome": "Passed", "failure": None,
                })
    return {
        "status": "Complete", "cleanup_unconfirmed": False, "samples": samples,
        "median_ns": {side: {kind: 2_000_000 for kind in ("Exec", "Upload", "Download")}
                      for side in ("A", "B")},
    }


class ForwardingTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.writers = []
        self.tasks = set()
        self.connections = 0

        async def echo(reader, writer):
            self.connections += 1
            self.writers.append(writer)
            try:
                while chunk := await reader.read(65536):
                    writer.write(chunk)
                    await writer.drain()
            finally:
                writer.close()

        self.target = await asyncio.start_server(echo, "127.0.0.1", 0)
        self.target_port = self.target.sockets[0].getsockname()[1]

    async def asyncTearDown(self):
        for writer in self.writers:
            writer.close()
        for task in self.tasks:
            task.cancel()
        await asyncio.gather(*self.tasks, return_exceptions=True)
        self.target.close()
        await self.target.wait_closed()

    async def gate(self, delay=0, rate=0):
        def accept(reader, writer):
            task = asyncio.create_task(lab.connect_gate(
                reader, writer, self.target_port, delay, rate,
            ))
            self.tasks.add(task)
            task.add_done_callback(self.tasks.discard)
        server = await asyncio.start_server(accept, "127.0.0.1", 0, limit=4096)
        self.addAsyncCleanup(server.wait_closed)
        self.addCleanup(server.close)
        return server.sockets[0].getsockname()[1]

    async def request(self, gate_port, authority=None, suffix=b""):
        reader, writer = await asyncio.open_connection("127.0.0.1", gate_port)
        self.writers.append(writer)
        authority = authority or f"127.0.0.1:{self.target_port}"
        writer.write(f"CONNECT {authority} HTTP/1.1\r\nHost: ignored\r\n\r\n".encode() + suffix)
        await writer.drain()
        header = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 2)
        return header, reader, writer

    async def test_connect_preserves_binary_payload_already_buffered_after_headers(self):
        port = await self.gate()
        payload = bytes(range(256)) * 512
        header, reader, writer = await self.request(port, suffix=payload)
        self.assertIn(b"200", header)
        received = await asyncio.wait_for(reader.readexactly(len(payload)), 3)
        self.assertEqual(received, payload)
        writer.write_eof()
        self.assertEqual(await asyncio.wait_for(reader.read(), 2), b"")

    async def test_wrong_authority_is_rejected_without_opening_any_upstream(self):
        port = await self.gate()
        header, reader, _ = await self.request(port, authority="example.invalid:22")
        self.assertIn(b"403", header)
        self.assertEqual(await asyncio.wait_for(reader.read(), 2), b"")
        self.assertEqual(self.connections, 0)

    async def test_loopback_other_port_is_also_rejected(self):
        port = await self.gate()
        header, _, _ = await self.request(port, authority=f"127.0.0.1:{self.target_port + 1}")
        self.assertIn(b"403", header)
        self.assertEqual(self.connections, 0)

    async def test_oversized_header_closes_without_an_upstream(self):
        port = await self.gate()
        reader, writer = await asyncio.open_connection("127.0.0.1", port)
        self.writers.append(writer)
        writer.write(b"CONNECT " + b"x" * 5000 + b"\r\n\r\n")
        await writer.drain()
        self.assertEqual(await asyncio.wait_for(reader.read(), 2), b"")
        self.assertEqual(self.connections, 0)

    async def test_throttle_has_measurable_delay_and_preserves_every_byte(self):
        port = await self.gate(delay=10, rate=262144)
        header, reader, writer = await self.request(port)
        self.assertIn(b"200", header)
        payload = b"x" * 32768
        started = time.monotonic()
        writer.write(payload)
        await writer.drain()
        received = await asyncio.wait_for(reader.readexactly(len(payload)), 3)
        # The independent directions pipeline chunks; their full transfer
        # durations therefore overlap rather than adding end to end.
        self.assertGreaterEqual(time.monotonic() - started, 0.18)
        self.assertEqual(received, payload)


class EvidenceTests(unittest.TestCase):
    def test_complete_report_uses_signed_observed_difference(self):
        report = complete_report()
        self.assertEqual(lab.summarize(report)["Upload"]["B_minus_A_ns"], 0)
        for row in report["samples"]:
            if row["path"] == "B":
                row["elapsed_ns"] *= 2
        for kind in report["median_ns"]["B"]:
            report["median_ns"]["B"][kind] *= 2
        self.assertGreater(lab.summarize(report)["Exec"]["B_minus_A_ns"], 0)

    def test_missing_failed_duplicate_and_wrong_payload_cannot_be_signed_complete(self):
        mutations = {
            "missing": lambda r: r["samples"].pop(),
            "failed": lambda r: r["samples"][0].update(outcome="Failed", failure="TimedOut"),
            "unconfirmed": lambda r: r.update(cleanup_unconfirmed=True),
            "cancelled": lambda r: r.update(status="Cancelled"),
            "duplicate_round": lambda r: r["samples"][6].update(round=1),
            "wrong_payload": lambda r: r["samples"][1].update(bytes=262143),
            "fake_median": lambda r: r["median_ns"]["A"].update(Exec=0),
            "wrong_order": lambda r: r["samples"].reverse(),
        }
        for name, mutate in mutations.items():
            with self.subTest(name=name):
                report = complete_report()
                mutate(report)
                with self.assertRaises(interop.InteropFailure):
                    lab.summarize(report)

    def test_nonfinite_and_negative_elapsed_cannot_enter_a_summary(self):
        for elapsed in (float("nan"), float("inf"), -1, 0, True, 9_000_000_000):
            with self.subTest(elapsed=elapsed):
                report = complete_report()
                report["samples"][0]["elapsed_ns"] = elapsed
                with self.assertRaises(interop.InteropFailure):
                    lab.validate_report(report)

    def test_each_scenario_impairs_only_its_declared_gate(self):
        for name in lab.SCENARIOS:
            with self.subTest(scenario=name):
                result = lab.conditions(name, 20, 524288)
                affected = [side for side in ("A", "B") if result[side]["delay_ms_per_write"]]
                self.assertEqual(affected, {
                    "baseline": [], "direct-impaired": ["A"], "relay-impaired": ["B"],
                }[name])
                self.assertEqual(result["ip_packet_loss"], "NOT_TESTED")

    def test_server_roles_have_separate_identity_and_minimal_forwarding(self):
        target = lab.server_config(Path("/private/fixture/target"), "test-user", 2222)
        jump = lab.server_config(Path("/private/fixture/jump"), "test-user", 2223, 2222)
        self.assertIn("AllowTcpForwarding no\n", target)
        self.assertIn("PermitOpen none\n", target)
        self.assertIn("MaxSessions 0\n", jump)
        self.assertIn("AllowTcpForwarding local\n", jump)
        self.assertIn("PermitOpen 127.0.0.1:2222\n", jump)
        for config in (target, jump):
            self.assertIn("ListenAddress 127.0.0.1\n", config)
            self.assertIn("PermitTTY no\n", config)
            self.assertNotIn(".ssh/", config)

    def test_output_cannot_reuse_an_old_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            existing = Path(directory) / "run"
            existing.mkdir()
            marker = existing / "result.json"
            marker.write_text("old failed evidence")
            with self.assertRaises(FileExistsError):
                interop.create_output(str(existing))
            self.assertEqual(marker.read_text(), "old failed evidence")

    def test_bad_numeric_cli_values_never_launch_a_process(self):
        for flag, value in (("--timeout", "nan"), ("--timeout", "301"),
                            ("--delay-ms", "inf"), ("--rate", "-1")):
            with self.subTest(flag=flag, value=value), \
                    redirect_stderr(io.StringIO()), \
                    patch.object(lab.platform, "system", return_value="Linux"), \
                    patch.object(lab.subprocess, "Popen") as launch:
                with self.assertRaises(SystemExit) as error:
                    lab.main(["--output", "/unused", "--test-binary", sys.executable, flag, value])
                self.assertEqual(error.exception.code, 2)
                launch.assert_not_called()

    def test_noop_impairment_is_rejected_before_launch(self):
        with redirect_stderr(io.StringIO()), \
                patch.object(lab.platform, "system", return_value="Linux"), \
                patch.object(lab.subprocess, "Popen") as launch:
            with self.assertRaises(SystemExit) as error:
                lab.main(["--output", "/unused", "--test-binary", sys.executable,
                          "--delay-ms", "0", "--rate", "0"])
        self.assertEqual(error.exception.code, 2)
        launch.assert_not_called()

    def test_unsupported_host_is_rejected_before_output_or_process_creation(self):
        for system in ("Windows", "FreeBSD"):
            with self.subTest(system=system), redirect_stderr(io.StringIO()) as errors, \
                    patch.object(lab.platform, "system", return_value=system), \
                    patch.object(interop, "create_output") as output, \
                    patch.object(lab.subprocess, "Popen") as launch:
                with self.assertRaises(SystemExit) as error:
                    lab.main(["--output", "/unused", "--test-binary", sys.executable])
                self.assertEqual(error.exception.code, 2)
                self.assertIn("requires macOS or Linux", errors.getvalue())
                output.assert_not_called()
                launch.assert_not_called()


class CleanupTests(unittest.TestCase):
    def test_inventory_failure_still_signals_only_recorded_identity_and_reaps(self):
        process = MagicMock(pid=101, returncode=-9)
        identity = (1, 101, ("kernel", 31), "R")
        runner = MagicMock()
        runner.processes = [process]
        runner.daemons = set()
        runner.names = {process: "test"}
        runner.owned = {process: {101: identity}}
        with patch.object(interop, "stop_owned", side_effect=interop.InteropFailure("inventory timed out")), \
                patch.object(interop, "signal_owned") as kill, \
                patch.object(lab, "signal") as signals:
            rows = lab.shutdown(runner, time.monotonic() + 1)
            kill.assert_called_once_with(101, identity, signals.SIGKILL)
        process.wait.assert_called_once()
        self.assertTrue(rows[0]["reaped"])
        self.assertFalse(rows[0]["observed_stopped"])
        self.assertEqual(rows[0]["returncode"], -9)

    @requires_posix_runner
    def test_early_inventory_rejection_retains_failed_receipt_and_cleanup(self):
        # Execute a real short Python child, never Rust. A zero exit with a
        # stale/empty libtest inventory must fail before generating credentials.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake_binary = root / "stale-test"
            fake_binary.write_text(f"#!{sys.executable}\nprint('0 tests, 0 benchmarks')\n")
            fake_binary.chmod(0o700)
            output = root / "evidence"
            with redirect_stdout(io.StringIO()):
                result = lab.main(["--output", str(output), "--test-binary", str(fake_binary)])
            report = json.loads((output / "result.json").read_text())
            self.assertEqual(result, 1)
            self.assertEqual(report["outcome"], "failed")
            self.assertIn("inventory is missing", report["error"])
            self.assertTrue(report["cleanup"]["owned_processes_stopped"])
            self.assertTrue(report["cleanup"]["temporary_directory_removed"])
            self.assertTrue(report["cleanup"]["processes"][0]["reaped"])
            self.assertEqual(report["scenarios"], [])

    @requires_posix_runner
    def test_gate_child_stops_and_is_reaped_with_a_live_connection(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / "gate.json"
            port = interop.reserve_port()
            lab.write_json(config, {
                "port": port, "target_port": 1,
                "delay_ms_per_write": 0, "bytes_per_second": 0,
            })
            child = subprocess.Popen(
                [sys.executable, str(Path(lab.__file__).resolve()), "--gate", str(config)],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
            )
            peer = None
            try:
                until = time.monotonic() + 4
                while time.monotonic() < until:
                    if child.poll() is not None:
                        self.fail("Gate exited before readiness")
                    try:
                        peer = socket.create_connection(("127.0.0.1", port), timeout=0.1)
                        break
                    except OSError:
                        time.sleep(0.02)
                self.assertIsNotNone(peer)
                child.terminate()
                _, stderr = child.communicate(timeout=3)
                self.assertEqual(child.returncode, 0, stderr.decode(errors="replace"))
                peer.settimeout(1)
                self.assertEqual(peer.recv(1), b"")
            finally:
                if peer is not None:
                    peer.close()
                if child.poll() is None:
                    child.kill()
                child.communicate(timeout=2)


if __name__ == "__main__":
    unittest.main()
