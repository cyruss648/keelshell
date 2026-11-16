"""Kernel wire decoding, strict evidence, isolation and real bounded process tests."""

import asyncio
from contextlib import ExitStack, redirect_stderr, redirect_stdout
import copy
import io
import json
from pathlib import Path
import platform
import socket
import struct
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import MagicMock, patch

import openssh_interop as interop
import ssh_path_netem_lab as lab
from ssh_path_netem import container as service, qdisc


requires_posix_runner = unittest.skipUnless(
    platform.system() in ("Darwin", "Linux"),
    "Lab runners require macOS/Linux kernel birth identities and POSIX signals",
)


def complete_report():
    samples = []
    for number in (1, 2, 3):
        for kind in lab.KINDS:
            for side in (("B", "A") if number == 2 else ("A", "B")):
                samples.append({
                    "index": len(samples), "round": number, "kind": kind, "path": side,
                    "outcome": "Passed", "failure": None, "elapsed_ns": number * 1_000_000,
                    "bytes": 0 if kind == "Exec" else 262144,
                })
    return {
        "schema": 1, "scenario": "baseline", "status": "Complete",
        "cleanup_unconfirmed": False, "wall_elapsed_ns": 36_000_000,
        "limits": {"rounds": 3, "sample_seconds": 8, "comparison_seconds": 60,
                   "bytes_per_transfer": 262144, "total_payload_bytes": 3145728},
        "samples": samples,
        "median_ns": {side: {kind: 2_000_000 for kind in lab.KINDS} for side in ("A", "B")},
    }


def kernel_message(delay=20_000_000, loss=42949673, packets=101, drops=2):
    options = qdisc.options(delay, loss) + qdisc.attribute(14, struct.pack("=Q", 1234567))
    stats = (
        qdisc.attribute(1, struct.pack("=QI", 262144, packets))
        + qdisc.attribute(3, struct.pack("=5I", 0, 0, drops, 0, 0))
    )
    payload = (
        qdisc.TCMSG.pack(socket.AF_UNSPEC, 0, 0, 2, qdisc.HANDLE, qdisc.ROOT, 0)
        + qdisc.attribute(1, b"netem\0") + qdisc.attribute(2, options)
        + qdisc.attribute(7, stats)
    )
    return qdisc.NLMSG.pack(16 + len(payload), 36, 0, 71, 0) + payload


def snapshot(condition=None):
    expected = condition or lab.conditions("direct-impaired", 20_000_000, 42949673)["A"]
    message = kernel_message(expected["delay_ns"], expected["loss_u32"])
    return {
        "owner": "fixture", "network_namespace": "net:[100]",
        "interface": "eth0", "ifindex": 2, "qdiscs": [qdisc.decode_qdisc(message)],
    }


class NetlinkTests(unittest.TestCase):
    def test_public_uapi_layout_and_original_bytes_survive_decode(self):
        wire = kernel_message()
        row = qdisc.decode_qdisc(wire)
        self.assertEqual(row["raw_netlink_hex"], wire.hex())
        self.assertEqual(row["options"]["delay_ns"], 20_000_000)
        self.assertEqual(row["options"]["loss_u32"], 42949673)
        self.assertEqual(row["options"]["kernel_prng_seed"], 1234567)
        self.assertEqual(row["counters"]["drops"], 2)
        self.assertEqual(row["counters"]["packets"], 101)
        self.assertEqual(qdisc.TCMSG.size, 20)

    def test_attributes_reject_truncation_invalid_lengths_and_duplicate_fields(self):
        for wire in (b"\x01", b"\x02\x00\x01\x00", b"\x08\x00\x01\x00",
                     qdisc.attribute(1, b"a") + qdisc.attribute(1, b"b")):
            with self.subTest(wire=wire), self.assertRaises(ValueError):
                qdisc.attributes(wire)

    def test_packet_counter_64_takes_precedence_without_losing_queue_drops(self):
        wire = kernel_message()
        payload = wire[16:]
        attrs = qdisc.attributes(payload[20:])
        counters = attrs[7] + qdisc.attribute(8, struct.pack("=Q", 2**40))
        payload = payload[:20] + b"".join(qdisc.attribute(k, v) for k, v in attrs.items() if k != 7)
        payload += qdisc.attribute(7, counters)
        wire = qdisc.NLMSG.pack(16 + len(payload), 36, 0, 1, 0) + payload
        row = qdisc.decode_qdisc(wire)
        self.assertEqual(row["counters"]["packets"], 2**40)
        self.assertEqual(row["counters"]["drops"], 2)

    def test_kernel_errors_and_interrupted_dump_are_never_empty_success(self):
        for error, flags in ((-1, 0), (0, 0x10)):
            with self.subTest(error=error, flags=flags):
                fake = MagicMock()
                wire = qdisc.NLMSG.pack(20, 2, flags, 77, 0) + struct.pack("=i", error)
                fake.recvmsg.return_value = (wire, [], 0, (0, 0))
                with patch.object(qdisc.socket, "socket") as factory, \
                        patch.multiple(qdisc.socket, AF_NETLINK=16, NETLINK_ROUTE=0, create=True), \
                        patch.object(qdisc.time, "monotonic_ns", return_value=77):
                    factory.return_value.__enter__.return_value = fake
                    with self.assertRaises((OSError, ValueError)):
                        qdisc.exchange(36, 5, b"")

    def test_non_kernel_sender_and_sequence_mismatch_fail_closed(self):
        for sender, seq in (((999, 0), 77), ((0, 0), 78)):
            with self.subTest(sender=sender, sequence=seq):
                fake = MagicMock()
                wire = qdisc.NLMSG.pack(20, 2, 0, seq, 0) + struct.pack("=i", 0)
                fake.recvmsg.return_value = (wire, [], 0, sender)
                with patch.object(qdisc.socket, "socket") as factory, \
                        patch.multiple(qdisc.socket, AF_NETLINK=16, NETLINK_ROUTE=0, create=True), \
                        patch.object(qdisc.time, "monotonic_ns", return_value=77):
                    factory.return_value.__enter__.return_value = fake
                    with self.assertRaises(ValueError):
                        qdisc.exchange(36, 5, b"")

    def test_host_invocation_is_rejected_before_opening_any_netlink_socket(self):
        with patch.object(qdisc.Path, "is_file", return_value=False), \
                patch.object(qdisc.socket, "socket") as socket_factory:
            with self.assertRaises(PermissionError):
                qdisc.replace("not-a-container", 0, 0)
            socket_factory.assert_not_called()


class EvidenceTests(unittest.TestCase):
    def test_all_eighteen_successful_rows_are_required_for_signed_observation(self):
        result = lab.validate_report(complete_report())
        self.assertEqual(result["Upload"]["B_minus_A_ns"], 0)
        mutations = {
            "missing": lambda r: r["samples"].pop(),
            "failure": lambda r: r["samples"][4].update(outcome="Failed", failure="TimedOut"),
            "unknown_cleanup": lambda r: r.update(cleanup_unconfirmed=True),
            "cancelled": lambda r: r.update(status="Cancelled"),
            "fake_median": lambda r: r["median_ns"]["A"].update(Exec=1),
            "reordered_with_same_groups": lambda r: r["samples"].__setitem__(
                slice(0, 4), [r["samples"][0], r["samples"][2], r["samples"][1], r["samples"][3]],
            ),
            "duplicate_index": lambda r: r["samples"][1].update(index=0),
            "bool_index": lambda r: r["samples"][1].update(index=True),
            "wrong_budget": lambda r: r["limits"].update(sample_seconds=9),
            "wrong_payload": lambda r: r["samples"][2].update(bytes=262143),
        }
        for name, change in mutations.items():
            with self.subTest(name=name):
                report = complete_report()
                change(report)
                with self.assertRaises((lab.LabFailure, interop.InteropFailure)):
                    lab.validate_report(report)

    def test_same_kernel_configuration_is_verified_for_both_control_and_damage(self):
        for scenario in lab.SCENARIOS:
            conditions = lab.conditions(scenario, 20_000_000, 42949673)
            for side in ("A", "B"):
                expected = conditions[side]
                row = snapshot(expected)
                accepted = lab.check_qdisc(row, expected, "fixture", "net:[100]")
                self.assertEqual(accepted["options"]["loss_u32"], expected["loss_u32"])
                for mutation in (
                    lambda r: r.update(network_namespace="net:[101]"),
                    lambda r: r.update(owner="another-task"),
                    lambda r: r["qdiscs"][0].update(parent=0),
                    lambda r: r["qdiscs"][0]["options"].update(delay_ns=None),
                    lambda r: r["qdiscs"][0].pop("counters"),
                    lambda r: r["qdiscs"].append(copy.deepcopy(r["qdiscs"][0])),
                ):
                    damaged = copy.deepcopy(row)
                    mutation(damaged)
                    with self.assertRaises(lab.LabFailure):
                        lab.check_qdisc(damaged, expected, "fixture", "net:[100]")

    def test_no_packets_no_drops_counter_reset_and_dirty_control_are_typed_failures(self):
        before = snapshot()["qdiscs"][0]
        before["counters"].update(bytes=0, packets=0, drops=0)
        after = copy.deepcopy(before)
        after["counters"].update(bytes=262144, packets=101, drops=2)
        condition = lab.conditions("direct-impaired", 20_000_000, 42949673)["A"]
        self.assertEqual(lab.check_counters(before, after, condition)["drops"], 2)
        cases = [
            ({"packets": 0}, condition, "QdiscNotTraversed"),
            ({"drops": 0}, condition, "NoObservedIpDrops"),
            ({"qlen": 1}, condition, "QdiscQueueNotDrained"),
            ({"overlimits": 1}, condition, "DropCauseAmbiguous"),
            ({"packets": 1000}, condition, "DropCauseAmbiguous"),
            ({}, dict(condition, loss_u32=0), "ControlPathDroppedPackets"),
        ]
        for update, expected, kind in cases:
            damaged = copy.deepcopy(after)
            damaged["counters"].update(update)
            with self.subTest(kind=kind), self.assertRaises(lab.LabFailure) as error:
                lab.check_counters(before, damaged, expected)
            self.assertEqual(error.exception.kind, kind)
        reset = copy.deepcopy(before)
        reset["counters"]["drops"] = 3
        with self.assertRaises(lab.LabFailure) as error:
            lab.check_counters(reset, after, condition)
        self.assertEqual(error.exception.kind, "QdiscCounterReset")

    def test_scenario_reversal_changes_only_the_declared_path(self):
        baseline = lab.conditions("baseline", 20_000_000, 42949673)
        direct = lab.conditions("direct-impaired", 20_000_000, 42949673)
        relay = lab.conditions("relay-impaired", 20_000_000, 42949673)
        self.assertEqual(direct["B"], baseline["B"])
        self.assertEqual(relay["A"], baseline["A"])
        self.assertEqual(direct["A"], relay["B"])
        self.assertEqual(direct["topology"], relay["topology"])
        self.assertNotIn("sleep", direct["gate_behavior"])


class SafetyTests(unittest.TestCase):
    def check_late_creation(self, interrupted, client_exit=None):
        """A stopped client cannot cancel an already accepted daemon request."""
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory)
            receipt = {"resources": [], "commands": [], "processes": [], "steps": []}
            runner = lab.stream_lab.LabRunner(
                root, time.monotonic() + 8, stack, receipt, lambda: None,
            )
            api = lab.PodmanLab(runner, "fixture", receipt, "mock-podman")
            original_start, original_wait = runner.start, runner.wait_bounded
            # Only the CLI client is real. The separate delayed daemon below
            # models a request that survives losing its client connection.
            client_program = "import time; time.sleep(5)"
            if client_exit is not None:
                client_program = ("import os,signal; os.kill(os.getpid(), signal.SIGTERM)"
                                  if client_exit < 0 else f"raise SystemExit({client_exit})")
            runner.start = lambda name, argv, env=None: original_start(
                name, [sys.executable, "-B", "-c", client_program], env,
            )
            runner.wait_bounded = lambda name, process, seconds: original_wait(
                name, process, 0.03 if client_exit is None else 2,
            )
            if interrupted:
                def interrupt_after_start():
                    if runner.processes:
                        raise interop.Interrupted("Owned test signal")
                runner.check_interrupt = interrupt_after_start
            marker = root / "late-resource"
            release = threading.Event()

            def daemon():
                if release.wait(2):
                    marker.write_text("owned")

            worker = threading.Thread(target=daemon)
            worker.start()
            try:
                expected = interop.Interrupted if interrupted else lab.LabFailure
                with self.assertRaises(expected):
                    api.create_network()
                client = runner.processes[0]
                stopped_before_resource_check = client.poll() is not None
                if client_exit is not None:
                    self.assertEqual(client.returncode, client_exit)
                checks = []

                def resource_command(label, args, **kwargs):
                    checks.append((label, client.poll() is not None))
                    if args[:2] == ["network", "rm"]:
                        marker.unlink()
                        return 0, ""
                    return (0 if marker.exists() else 1), ""

                api.command = resource_command
                api.bind = lambda resource: "a" * 64
                first = api.cleanup()
                release.set()
                worker.join(timeout=2)
                self.assertFalse(worker.is_alive())
                self.assertTrue(marker.exists())
                second = api.cleanup()
                self.assertFalse(marker.exists())
                self.assertTrue(stopped_before_resource_check)
                self.assertTrue(all(stopped for _, stopped in checks))
                for rows in (first, second):
                    self.assertFalse(rows[0]["absent"])
                    self.assertEqual(rows[0]["failure"]["type"], "MutationCompletionUnknown")
            finally:
                runner.check_interrupt = lambda: None
                release.set()
                worker.join(timeout=2)
                lab.stream_lab.shutdown(runner, time.monotonic() + 3)

    @requires_posix_runner
    def test_timeout_reaps_creator_before_checks_and_keeps_late_creation_unknown(self):
        self.check_late_creation(False)

    @requires_posix_runner
    def test_interrupt_reaps_creator_before_checks_and_keeps_late_creation_unknown(self):
        self.check_late_creation(True)

    @requires_posix_runner
    def test_nonzero_creator_exit_does_not_settle_the_daemon_request(self):
        self.check_late_creation(False, 125)

    @requires_posix_runner
    def test_signaled_creator_exit_does_not_settle_the_daemon_request(self):
        self.check_late_creation(False, -15)

    def test_actual_attachment_and_kernel_caps_override_ambiguous_podman_metadata(self):
        row = {
            "role": "servers",
            "security": {"privileged": False, "network_mode": "bridge", "cap_add": [],
                         "readonly": True, "security_opt": ["no-new-privileges"]},
            "network": {"Networks": {"owned": {"NetworkID": "a" * 64}}},
            "identity": {"effective_caps_hex": "400c0", "bounding_caps_hex": "400c0"},
        }
        network = {"id": "a" * 64}
        lab.check_container_isolation(row, network)
        for mutation in (
            lambda r: r["identity"].update(effective_caps_hex="ffffffff"),
            lambda r: r["identity"].update(bounding_caps_hex="ffffffff"),
            lambda r: r["network"]["Networks"]["owned"].update(NetworkID="b" * 64),
            lambda r: r["network"]["Networks"].update(foreign={"NetworkID": "b" * 64}),
            lambda r: r["security"].update(network_mode="host"),
        ):
            damaged = copy.deepcopy(row)
            mutation(damaged)
            with self.assertRaises(lab.LabFailure):
                lab.check_container_isolation(damaged, network)

    def test_unsupported_kernel_stays_a_typed_failure_in_the_runner(self):
        receipt = {"resources": [], "commands": []}
        api = lab.PodmanLab(MagicMock(), "fixture", receipt, "podman")
        api.command = MagicMock(return_value=(
            1, '{"failure":{"type":"KernelNetemUnavailable","message":"Operation not permitted","errno":1}}',
        ))
        with self.assertRaises(lab.LabFailure) as error:
            api.qdisc({"role": "A", "id": "a" * 64})
        self.assertEqual(error.exception.kind, "KernelNetemUnavailable")

    def test_container_network_permissions_and_publications_have_exact_scope(self):
        for role in ("servers", "A", "B"):
            args = lab.container_arguments("owned", "owned-name", "image", "owned-net", role, "10.1.1.2")
            self.assertIn("--cap-drop=ALL", args)
            self.assertIn("--read-only", args)
            self.assertIn("--pull=never", args)
            self.assertIn("--security-opt=no-new-privileges", args)
            self.assertNotIn("--privileged", args)
            self.assertNotIn("host", args)
            caps = {args[i + 1] for i, arg in enumerate(args) if arg == "--cap-add"}
            if role == "servers":
                self.assertEqual(caps, {"SETUID", "SETGID", "SYS_CHROOT"})
                self.assertNotIn("--publish", args)
            else:
                self.assertEqual(caps, {"NET_ADMIN"})
                self.assertEqual(args[args.index("--publish") + 1], "127.0.0.1::8080")

    def test_foreign_owner_is_never_deleted_and_other_cleanup_is_still_attempted(self):
        receipt = {"resources": [
            {"kind": "container", "name": "ours", "id": "a" * 64},
            {"kind": "container", "name": "foreign", "id": "b" * 64},
        ], "commands": []}
        api = lab.PodmanLab(MagicMock(), "fixture", receipt, "podman")
        calls = []

        def command(label, args, **_):
            calls.append(args)
            return (1 if label.startswith("verify-") else 0), ""

        def bind(resource):
            if resource["name"] == "foreign":
                raise lab.LabFailure("OwnershipMismatch", "Foreign label")
            return resource["id"]

        api.command = command
        api.bind = bind
        rows = api.cleanup()
        self.assertEqual(rows[0]["failure"]["type"], "OwnershipMismatch")
        self.assertFalse(rows[0]["absent"])
        self.assertTrue(rows[1]["absent"])
        removals = [args for args in calls if args[0] == "rm"]
        self.assertEqual(removals, [["rm", "--force", "--time=2", "a" * 64]])

    def test_unexpected_cleanup_error_cannot_skip_other_owned_resources(self):
        receipt = {"resources": [
            {"kind": "container", "name": "ours", "id": "a" * 64},
            {"kind": "container", "name": "error", "id": "b" * 64},
        ], "commands": []}
        api = lab.PodmanLab(MagicMock(), "fixture", receipt, "podman")
        calls = []

        def command(label, args, **_):
            calls.append(args)
            if args[-1] == "b" * 64:
                raise RuntimeError("Unexpected backend exception")
            return (1 if label.startswith("verify-") else 0), ""

        api.command = command
        api.bind = lambda resource: resource["id"]
        rows = api.cleanup()
        self.assertEqual(rows[0]["failure"]["type"], "RuntimeError")
        self.assertFalse(rows[0]["absent"])
        self.assertTrue(rows[1]["absent"])
        self.assertTrue(any(args[0] == "rm" and args[-1] == "a" * 64 for args in calls))

    def test_bad_cli_and_nonfinite_loss_never_launch_a_process(self):
        base = ["--output", "/unused", "--test-binary", sys.executable,
                "--test-binary-sha256", "a" * 64, "--base-image", "existing"]
        for flag, value in (("--loss-percent", "nan"), ("--loss-percent", "-1"),
                            ("--loss-percent", "0"), ("--timeout", "inf"),
                            ("--timeout", "601"), ("--delay-ms", "nan")):
            with self.subTest(flag=flag, value=value), redirect_stderr(io.StringIO()), \
                    patch.object(lab.platform, "system", return_value="Linux"), \
                    patch.object(interop.subprocess, "Popen") as start:
                with self.assertRaises(SystemExit) as error:
                    lab.main([*base, flag, value])
                self.assertEqual(error.exception.code, 2)
                start.assert_not_called()

    def test_digest_rejection_preserves_a_failed_receipt_without_launching(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "digest-failure"
            with redirect_stdout(io.StringIO()), \
                    patch.object(lab.platform, "system", return_value="Linux"), \
                    patch.object(interop.subprocess, "Popen") as start:
                result = lab.main([
                    "--output", str(output), "--test-binary", sys.executable,
                    "--test-binary-sha256", "0" * 64, "--base-image", "existing",
                ])
            receipt = json.loads((output / "result.json").read_text())
            self.assertEqual(result, 1)
            self.assertEqual(receipt["failure"]["type"], "PrebuiltDigestMismatch")
            self.assertFalse(receipt["observations_admissible"])
            self.assertTrue(receipt["cleanup"]["confirmed"])
            start.assert_not_called()

    @requires_posix_runner
    def test_zero_inventory_real_child_is_reaped_without_creating_resources(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "zero-tests"
            binary.write_text(f"#!{sys.executable}\nprint('0 tests, 0 benchmarks')\n")
            binary.chmod(0o700)
            output = root / "inventory-failure"
            with redirect_stdout(io.StringIO()), patch.object(interop, "executable", return_value=sys.executable):
                result = lab.main([
                    "--output", str(output), "--test-binary", str(binary),
                    "--test-binary-sha256", lab.stream_lab.digest(binary), "--base-image", "existing",
                ])
            receipt = json.loads((output / "result.json").read_text())
            self.assertEqual(result, 1)
            self.assertEqual(receipt["failure"]["type"], "PrebuiltInventoryMismatch")
            self.assertEqual(receipt["resources"], [])
            self.assertTrue(receipt["cleanup"]["confirmed"])
            self.assertTrue(receipt["cleanup"]["processes"][0]["reaped"])

    def test_reusing_output_preserves_original_failed_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "failed"
            output.mkdir()
            original = output / "result.json"
            original.write_text('{"status":"FAILED"}')
            with self.assertRaises(FileExistsError):
                interop.create_output(str(output))
            self.assertEqual(original.read_text(), '{"status":"FAILED"}')

    def test_publication_rejects_nonlocal_and_multiple_host_bindings(self):
        for bindings in ([{"HostIp": "0.0.0.0", "HostPort": "1234"}], [],
                         [{"HostIp": "127.0.0.1", "HostPort": "1234"}] * 2):
            with self.subTest(bindings=bindings), self.assertRaises(lab.LabFailure):
                lab.gate_endpoint({"network": {"Ports": {"8080/tcp": bindings}}})

    def test_unsupported_host_is_rejected_before_output_or_process_creation(self):
        for system in ("Windows", "FreeBSD"):
            with self.subTest(system=system), redirect_stderr(io.StringIO()) as errors, \
                    patch.object(lab.platform, "system", return_value=system), \
                    patch.object(interop, "create_output") as output, \
                    patch.object(interop.subprocess, "Popen") as start:
                with self.assertRaises(SystemExit) as error:
                    lab.main([
                        "--output", "/unused", "--test-binary", sys.executable,
                        "--test-binary-sha256", "0" * 64, "--base-image", "existing",
                    ])
                self.assertEqual(error.exception.code, 2)
                self.assertIn("supports macOS and Linux", errors.getvalue())
                output.assert_not_called()
                start.assert_not_called()


class GateTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.writers, self.tasks = [], set()
        self.connections = 0

        async def echo(reader, writer):
            self.connections += 1
            self.writers.append(writer)
            while chunk := await reader.read(65536):
                writer.write(chunk)
                await writer.drain()
            writer.write_eof()
            writer.close()

        self.target = await asyncio.start_server(echo, "127.0.0.1", 0)
        self.port = self.target.sockets[0].getsockname()[1]

        def accept(reader, writer):
            task = asyncio.create_task(service.tunnel(reader, writer, "127.0.0.1", self.port))
            self.tasks.add(task)
            task.add_done_callback(self.tasks.discard)

        self.gate = await asyncio.start_server(accept, "127.0.0.1", 0, limit=4096)

    async def asyncTearDown(self):
        for writer in self.writers:
            writer.close()
        for task in list(self.tasks):
            task.cancel()
        await asyncio.gather(*self.tasks, return_exceptions=True)
        for server in (self.gate, self.target):
            server.close()
            await server.wait_closed()

    async def request(self, authority=None, payload=b""):
        reader, writer = await asyncio.open_connection("127.0.0.1", self.gate.sockets[0].getsockname()[1])
        self.writers.append(writer)
        authority = authority or f"127.0.0.1:{self.port}"
        writer.write(f"CONNECT {authority} HTTP/1.1\r\n\r\n".encode() + payload)
        await writer.drain()
        header = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 2)
        return reader, writer, header

    async def test_unpaced_gate_preserves_binary_bytes_and_half_close(self):
        payload = bytes(range(256)) * 1024
        reader, writer, header = await self.request(payload=payload)
        self.assertIn(b"200", header)
        self.assertEqual(await asyncio.wait_for(reader.readexactly(len(payload)), 3), payload)
        writer.write_eof()
        self.assertEqual(await asyncio.wait_for(reader.read(), 2), b"")

    async def test_other_authority_is_rejected_without_opening_an_upstream(self):
        reader, _, header = await self.request(authority="example.invalid:22")
        self.assertIn(b"403", header)
        self.assertEqual(await asyncio.wait_for(reader.read(), 2), b"")
        self.assertEqual(self.connections, 0)


if __name__ == "__main__":
    unittest.main()
