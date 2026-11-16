#!/usr/bin/env python3
"""Offline contract tests and actual host child controls, not Windows emulation."""

import base64
import copy
import ctypes
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import time
import types
import unittest
from unittest import mock

import windows_monitor_lab as lab


def resource_data():
    return {"boot_time": "1000", "uptime_seconds": 10.5, "cpu_percent": None,
            "memory_total_bytes": "1073741824", "memory_free_bytes": "536870912",
            "network_count": 1, "networks": [{"name": "本地网络", "received_bytes": "101",
                                            "transmitted_bytes": "201"}],
            "capacity_status": "ok", "filesystem_count": 1, "filesystems": [{
                "source": "C:\\", "mount": "C:\\", "total_bytes": "1000000000",
                "total_free_bytes": "600000000", "available_bytes": "500000000"}]}


def independent_resources():
    """Synthetic, identity-bearing brackets; never native Windows execution evidence."""
    before = {"memory_total_bytes": 1073741824, "memory_free_bytes": 536870910,
              "boot_unix": 1000, "uptime_seconds": 9,
              "networks": [{"name": "本地网络", "received_bytes": 100, "transmitted_bytes": 200,
                            "interface_index": 12, "interface_luid": 123456,
                            "interface_guid": "00112233-4455-6677-8899-aabbccddeeff"}],
              "filesystems": [{"mount": "C:\\", "total_bytes": 1000000000,
                               "total_free_bytes": 600000000, "available_bytes": 500000000,
                               "volume_guid": "\\\\?\\Volume{11111111-2222-3333-4444-555555555555}\\",
                               "volume_serial": 42}],
              "capacity_issues": []}
    after = copy.deepcopy(before)
    after.update(memory_free_bytes=536870914, uptime_seconds=12)
    after["networks"][0].update(received_bytes=102, transmitted_bytes=202)
    return resource_data(), before, after


def frame(operation="probe", data=None):
    if data is None:
        data = {"probe": {"os": "Win32NT"}, "resources": resource_data(),
                "processes": {"count": 0, "processes": []},
                "sockets": {"count": 0, "sockets": []}}[operation]
    return {"version": 1, "operation": operation, "platform": "Windows",
            "runtime": "WindowsPowerShell", "runtime_version": "5.1.26100.123",
            "units": "bytes seconds percent", "status": "ok", "data": data}


def raw(value):
    return json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode("utf-8")


def fixture_value(pid=1234):
    return {"version": 1, "pid": pid, "endpoints": [
        {"protocol": protocol, "address": address, "port": str(40000 + index), "pid": str(pid)}
        for index, (protocol, address) in enumerate([
            ("tcp", "127.0.0.1"), ("udp", "127.0.0.1"), ("tcp", "::1"), ("udp", "::1")])],
        "connected_only": [{"protocol": "tcp", "address": address, "port": str(41000 + index),
                            "pid": str(pid)} for index, address in enumerate(("127.0.0.1", "::1"))]}


class FrameTests(unittest.TestCase):
    def parse(self, value, operation=None, suffix=b"", stderr=b"", code=0):
        return lab.parse_frame(raw(value) + suffix, stderr, code, operation or value["operation"])

    def test_successful_probe_lf_crlf_and_no_newline(self):
        for suffix in (b"", b"\n", b"\r\n"):
            with self.subTest(suffix=suffix):
                self.assertEqual(self.parse(frame(), suffix=suffix)["data"], {"os": "Win32NT"})

    def test_empty_arrays_and_singletons(self):
        for operation, key, singleton in (
            ("processes", "processes", {"pid": "0", "name": "Idle", "working_set_bytes": None}),
            ("sockets", "sockets", {"protocol": "udp", "address": "::1", "port": "65535", "pid": "0"}),
        ):
            for rows in ([], [singleton]):
                with self.subTest(operation=operation, rows=rows):
                    self.parse(frame(operation, {"count": len(rows), key: rows}))

    def test_unicode_process_and_network_names(self):
        self.parse(frame("resources"))
        self.parse(frame("processes", {"count": 1, "processes": [
            {"pid": "1", "name": "中文进程 α", "working_set_bytes": "18446744073709551615"}]}))

    def test_duplicate_top_level_key_rejected(self):
        payload = raw(frame()).replace(b'"version":1', b'"version":1,"version":1')
        with self.assertRaises(lab.LabError):
            lab.parse_frame(payload, b"", 0, "probe")

    def test_duplicate_nested_key_rejected(self):
        payload = raw(frame()).replace(b'"os":"Win32NT"', b'"os":"Win32NT","os":"Win32NT"')
        with self.assertRaises(lab.LabError):
            lab.parse_frame(payload, b"", 0, "probe")

    def test_unknown_field_wrong_operation_units_and_platform_rejected(self):
        for field, value in (("extra", 1), ("operation", "sockets"), ("platform", "Linux"),
                             ("runtime", "PowerShellCore"), ("units", "kilobytes"),
                             ("version", True), ("version", 2)):
            with self.subTest(field=field, value=value):
                item = frame()
                item[field] = value
                with self.assertRaises(lab.LabError):
                    self.parse(item, operation="probe")

    def test_invalid_runtime_versions_rejected(self):
        for version in ("7.1.1.1", "5.1", "5.1.1", "05.1.1.1", "5.1.01.1",
                        "5.1.4294967296.1", "5.1.1.1\n"):
            with self.subTest(version=version):
                item = frame()
                item["runtime_version"] = version
                with self.assertRaises(lab.LabError):
                    self.parse(item)

    def test_absent_nonzero_and_boolean_exits_rejected(self):
        for code in (None, 1, -1, True):
            with self.subTest(code=code):
                with self.assertRaises(lab.LabError):
                    self.parse(frame(), code=code)

    def test_stderr_invalid_utf8_and_bom_rejected(self):
        for payload, stderr in ((raw(frame()), b"warning"), (b"\xff", b""),
                                (b"\xef\xbb\xbf" + raw(frame()), b""),
                                (raw(frame()).replace(b"Win32NT", b"Win32\xffNT"), b"")):
            with self.subTest(payload=payload, stderr=stderr):
                with self.assertRaises(lab.LabError):
                    lab.parse_frame(payload, stderr, 0, "probe")

    def test_banners_trailing_space_extra_frame_and_truncation_rejected(self):
        valid = raw(frame())
        for payload in (b"banner\n" + valid, b" " + valid, valid + b" ", valid + b"\n\n",
                        valid + valid, valid + b"\nnoise", valid[:-1], b""):
            with self.subTest(payload=payload):
                with self.assertRaises(lab.LabError):
                    lab.parse_frame(payload, b"", 0, "probe")

    def test_combined_output_limit_rejected(self):
        with self.assertRaises(lab.LabError):
            lab.parse_frame(b"{" + b"x" * lab.MAX_FRAME, b"", 0, "probe")

    def test_all_typed_failures_are_non_success(self):
        for status in lab.FAILURES:
            for code in (0, 1):
                with self.subTest(status=status, code=code):
                    item = frame()
                    item.update(status=status, data=None)
                    with self.assertRaises(lab.LabError):
                        self.parse(item, code=code)

    def test_count_type_and_mismatch_rejected(self):
        for count in (1, -1, True, "0", 20001):
            with self.subTest(count=count):
                with self.assertRaises(lab.LabError):
                    self.parse(frame("processes", {"count": count, "processes": []}))

    def test_duplicate_or_unsorted_process_pids_rejected(self):
        for ids in (("1", "1"), ("2", "1")):
            rows = [{"pid": pid, "name": "python", "working_set_bytes": None} for pid in ids]
            with self.assertRaises(lab.LabError):
                self.parse(frame("processes", {"count": 2, "processes": rows}))

    def test_noncanonical_and_overwide_pid_rejected(self):
        for pid in ("01", "-1", "+1", "4294967296", "1.0", " 1", 1):
            with self.subTest(pid=pid):
                with self.assertRaises(lab.LabError):
                    self.parse(frame("processes", {"count": 1, "processes": [
                        {"pid": pid, "name": "python", "working_set_bytes": None}]}))

    def test_invalid_working_set_rejected(self):
        for value in ("18446744073709551616", "-1", 123, "00"):
            with self.subTest(value=value):
                with self.assertRaises(lab.LabError):
                    self.parse(frame("processes", {"count": 1, "processes": [
                        {"pid": "1", "name": "python", "working_set_bytes": value}]}))

    def test_control_escape_and_overlong_display_rejected(self):
        for name in ("a\x1b[31m", "a\n", "a\u0085", "a\u202e", "", "x" * 513):
            with self.subTest(name=name):
                with self.assertRaises(lab.LabError):
                    self.parse(frame("processes", {"count": 1, "processes": [
                        {"pid": "1", "name": name, "working_set_bytes": None}]}))

    def test_numeric_ipv4_ipv6_and_scopes(self):
        for address in ("127.0.0.1", "::1", "fe80::1%12", "::ffff:127.0.0.1"):
            with self.subTest(address=address):
                self.parse(frame("sockets", {"count": 1, "sockets": [
                    {"protocol": "tcp", "address": address, "port": "1", "pid": "4"}]}))

    def test_invalid_ip_scope_port_and_protocol_rejected(self):
        row = {"protocol": "tcp", "address": "::1", "port": "1", "pid": "4"}
        for field, value in (
            ("address", "localhost"), ("address", "127.0.0.1%2"), ("address", "::1%eth0"),
            ("address", "::1%0"), ("address", "::1%01"), ("address", "::1%4294967296"),
            ("address", "1.2.3.4/24"), ("port", "0"), ("port", "65536"), ("port", "01"),
            ("protocol", "established"), ("pid", "4294967296"),
        ):
            with self.subTest(field=field, value=value):
                with self.assertRaises(lab.LabError):
                    self.parse(frame("sockets", {"count": 1, "sockets": [{**row, field: value}]}))

    def test_duplicate_socket_rejected(self):
        row = {"protocol": "tcp", "address": "::1", "port": "1", "pid": "4"}
        with self.assertRaises(lab.LabError):
            self.parse(frame("sockets", {"count": 2, "sockets": [row, row]}))

    def test_unknown_cpu_is_preserved(self):
        result = self.parse(frame("resources"))
        self.assertIsNone(result["data"]["cpu_percent"])

    def test_explicit_empty_resource_arrays_remain_legal_frames(self):
        data = resource_data()
        data.update(networks=[], network_count=0, filesystems=[], filesystem_count=0)
        self.assertEqual(self.parse(frame("resources", data))["data"], data)

    def test_partial_capacity_failure_and_skip_preserve_core(self):
        for status in lab.FAILURES | {"skipped"}:
            with self.subTest(status=status):
                data = resource_data()
                data.update(capacity_status=status, filesystems=[], filesystem_count=0)
                self.parse(frame("resources", data))

    def test_capacity_failure_cannot_carry_success_rows(self):
        data = resource_data()
        data["capacity_status"] = "permission_denied"
        with self.assertRaises(lab.LabError):
            self.parse(frame("resources", data))

    def test_invalid_resource_metrics_rejected(self):
        for field, value in (("boot_time", "01"), ("uptime_seconds", -1),
                             ("cpu_percent", 100.1), ("cpu_percent", True),
                             ("memory_total_bytes", "0"), ("memory_free_bytes", "1073741825"),
                             ("memory_total_bytes", "18446744073709551616"),
                             ("network_count", 0), ("filesystem_count", True),
                             ("capacity_status", "invented")):
            with self.subTest(field=field, value=value):
                data = resource_data()
                data[field] = value
                with self.assertRaises(lab.LabError):
                    self.parse(frame("resources", data))

    def test_nonfinite_json_rejected(self):
        for token in (b"NaN", b"Infinity", b"-Infinity", b"1e999"):
            payload = raw(frame("resources")).replace(b'"cpu_percent":null', b'"cpu_percent":' + token)
            with self.subTest(token=token):
                with self.assertRaises(lab.LabError):
                    lab.parse_frame(payload, b"", 0, "resources")

    def test_duplicate_network_and_filesystem_rejected(self):
        for rows, count in (("networks", "network_count"), ("filesystems", "filesystem_count")):
            data = resource_data()
            data[rows] *= 2
            data[count] = 2
            with self.assertRaises(lab.LabError):
                self.parse(frame("resources", data))

    def test_free_and_quota_available_are_distinct(self):
        result = self.parse(frame("resources"))
        disk = result["data"]["filesystems"][0]
        self.assertNotEqual(disk["total_free_bytes"], disk["available_bytes"])
        data = resource_data()
        data["filesystems"][0]["available_bytes"] = "600000001"
        with self.assertRaises(lab.LabError):
            self.parse(frame("resources", data))


class AssemblyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="keelshell-monitor-assembly-test-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        for name, text in (("common.ps1", "# 前缀\r\n"), ("probe.ps1", "# probe\n")):
            (self.directory / name).write_bytes(text.encode("utf-8"))

    def test_exact_utf16le_base64_and_file_hash_binding(self):
        command, evidence = lab.assemble("probe", self.directory)
        decoded = base64.b64decode(command[len(lab.POWERSHELL):]).decode("utf-16le")
        self.assertEqual(decoded, "$op='probe';$skip=$false;\n# 前缀\r\n# probe\n" + lab.SUFFIX)
        self.assertEqual(evidence["wire_command_ascii_bytes"], len(command))
        self.assertEqual([row["file"] for row in evidence["parts"]], ["common.ps1", "probe.ps1"])

    def test_fixed_skip_capacity_variant(self):
        (self.directory / "resources.ps1").write_bytes(b"# resources\n")
        command, _ = lab.assemble("resources-skipped", self.directory)
        decoded = base64.b64decode(command[len(lab.POWERSHELL):]).decode("utf-16le")
        self.assertTrue(decoded.startswith("$op='resources';$skip=$true;\n"))

    def test_unknown_operation_rejected(self):
        with self.assertRaises(lab.LabError):
            lab.assemble("arbitrary", self.directory)

    def test_path_escape_rejected(self):
        with self.assertRaises(lab.LabError):
            lab.assemble("../probe", self.directory)

    def test_missing_part_rejected(self):
        (self.directory / "common.ps1").unlink()
        with self.assertRaises(lab.LabError):
            lab.assemble("probe", self.directory)

    def test_wire_command_must_be_strictly_below_8000_bytes(self):
        (self.directory / "common.ps1").write_bytes(b"#" * 4000)
        with self.assertRaisesRegex(lab.LabError, "cmd.exe"):
            lab.assemble("probe", self.directory)

    def test_actual_production_assembly_is_verbatim_and_below_limit(self):
        for operation, (prefix, body) in lab.ASSEMBLY.items():
            with self.subTest(operation=operation):
                command, evidence = lab.assemble(operation)
                decoded = base64.b64decode(command[len(lab.POWERSHELL):]).decode("utf-16le")
                expected = prefix + (lab.COLLECTORS / "common.ps1").read_bytes().decode("utf-8")
                expected += (lab.COLLECTORS / body).read_bytes().decode("utf-8") + lab.SUFFIX
                self.assertEqual(decoded, expected)
                self.assertLess(evidence["wire_command_ascii_bytes"], 8000)
                print(f"PRODUCTION_COMMAND {operation} ascii_bytes={evidence['wire_command_ascii_bytes']}")

    def test_actual_resource_variants_bind_same_source_and_only_change_fixed_skip_literal(self):
        normal, normal_binding = lab.assemble("resources")
        skipped, skipped_binding = lab.assemble("resources-skipped")
        decoded = lambda wire: base64.b64decode(wire[len(lab.POWERSHELL):]).decode("utf-16le")
        self.assertEqual(decoded(normal).replace("$skip=$false;\n", "$skip=$true;\n", 1),
                         decoded(skipped))
        self.assertEqual(normal_binding["parts"], skipped_binding["parts"])
        for part in normal_binding["parts"]:
            self.assertEqual(part["sha256"], lab.hashlib.sha256(
                (lab.COLLECTORS / part["file"]).read_bytes()).hexdigest())

    def test_complete_production_command_is_one_actual_cmd_argument(self):
        for operation in lab.ASSEMBLY:
            command, _ = lab.assemble(operation)
            argv = lab.cmd_argv(command, r"C:\Windows\System32\cmd.exe")
            self.assertEqual(argv, [r"C:\Windows\System32\cmd.exe", "/d", "/s", "/c", command])
            self.assertLess(len(lab.subprocess.list2cmdline(argv)), 8191)

    def test_cmd_boundary_rejects_extra_text_and_oversize(self):
        for command in ("echo not-a-production-command", lab.POWERSHELL + "x" * 8000,
                        lab.POWERSHELL + "abc\n"):
            with self.assertRaises(lab.LabError):
                lab.cmd_argv(command, "cmd.exe")


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixture_value()
        self.processes = {"count": 1, "processes": [
            {"pid": "1234", "name": "python", "working_set_bytes": "20000000"}]}
        self.native_process = {"pid": 1234, "name": "Python", "working_set_bytes": 21000000}
        self.sockets = {"count": 4, "sockets": copy.deepcopy(self.fixture["endpoints"])}
        self.native_sockets = [
            {**row, "state": "LISTEN" if row["protocol"] == "tcp" else "BOUND"}
            for row in self.fixture["endpoints"]
        ] + [{**row, "state": "CONNECTED_OR_OTHER"} for row in self.fixture["connected_only"]]

    def compare(self):
        return lab.compare_owned(self.fixture, self.processes, self.sockets,
                                 self.native_process, self.native_sockets)

    def test_owned_pid_endpoints_and_independent_apis_agree(self):
        result = self.compare()
        self.assertEqual(result["owned_endpoints_matched"], 4)
        self.assertEqual(result["connected_tcp_excluded"], 2)

    def test_connected_tcp_is_never_a_listener(self):
        self.sockets["sockets"].append(self.fixture["connected_only"][0])
        with self.assertRaisesRegex(lab.LabError, "connected TCP"):
            self.compare()

    def test_connected_negative_control_must_exist_in_independent_table(self):
        self.native_sockets = self.native_sockets[:4]
        with self.assertRaisesRegex(lab.LabError, "connected TCP"):
            self.compare()

    def test_production_listener_missing(self):
        self.sockets["sockets"].pop()
        with self.assertRaises(lab.LabError):
            self.compare()

    def test_independent_listener_missing(self):
        self.native_sockets.pop(0)
        with self.assertRaises(lab.LabError):
            self.compare()

    def test_owned_pid_and_process_name_are_required(self):
        for field, value in (("pid", 9999), ("name", "unrelated")):
            with self.subTest(field=field):
                original = dict(self.native_process)
                self.native_process[field] = value
                with self.assertRaises(lab.LabError):
                    self.compare()
                self.native_process = original

    def test_unknown_or_divergent_working_set_fails(self):
        for value in (None, "999999999"):
            with self.subTest(value=value):
                self.processes["processes"][0]["working_set_bytes"] = value
                with self.assertRaises(lab.LabError):
                    self.compare()

    def test_fixture_pid_must_match_original_process(self):
        with self.assertRaises(lab.LabError):
            lab.validate_fixture(self.fixture, 9999)

    def test_fixture_requires_dual_stack_and_connected_controls(self):
        for field in ("endpoints", "connected_only"):
            value = copy.deepcopy(self.fixture)
            value[field].pop()
            with self.assertRaises(lab.LabError):
                lab.validate_fixture(value, 1234)

    def test_fixture_rejects_nonloopback_or_foreign_pid(self):
        for field, value in (("address", "0.0.0.0"), ("pid", "9999")):
            item = copy.deepcopy(self.fixture)
            item["endpoints"][0][field] = value
            with self.assertRaises(lab.LabError):
                lab.validate_fixture(item, 1234)

    def resources(self):
        return independent_resources()

    def test_independent_resources_and_unknown_cpu(self):
        result = lab.compare_resources(*self.resources(), [10, 20, 20], [15, 30, 30])
        self.assertEqual(result["independent_cpu_percent"], 75)
        self.assertIsNone(result["production_cpu_percent"])
        self.assertTrue(result["cpu_difference_is_informational"])

    def test_cpu_counter_regression_fails(self):
        with self.assertRaises(lab.LabError):
            lab.compare_resources(*self.resources(), [10, 20, 20], [9, 30, 30])

    def test_cumulative_network_bytes_must_be_bracketed(self):
        data, before, after = self.resources()
        data["networks"][0]["received_bytes"] = "99"
        with self.assertRaisesRegex(lab.LabError, "bracket"):
            lab.compare_resources(data, before, after, [10, 20, 20], [15, 30, 30])

    def test_missing_independent_interface_fails(self):
        data, before, after = self.resources()
        after["networks"] = []
        with self.assertRaises(lab.LabError):
            lab.compare_resources(data, before, after, [10, 20, 20], [15, 30, 30])

    def test_independent_memory_uptime_and_capacity_mismatches_fail(self):
        for field, value in (("memory_total_bytes", "1"), ("boot_time", "0"),
                             ("uptime_seconds", 9999)):
            with self.subTest(field=field):
                data, before, after = self.resources()
                data[field] = value
                with self.assertRaises(lab.LabError):
                    lab.compare_resources(data, before, after, [10, 20, 20], [15, 30, 30])
        data, before, after = self.resources()
        after["filesystems"][0]["total_bytes"] += 1
        with self.assertRaises(lab.LabError):
            lab.compare_resources(data, before, after, [10, 20, 20], [15, 30, 30])


class ResourceInventoryTests(unittest.TestCase):
    def setUp(self):
        self.data, self.before, self.after = independent_resources()

    def compare(self):
        lab.validate_data("resources", self.data)
        return lab.compare_resources(self.data, self.before, self.after, [10, 20, 20], [15, 30, 30])

    def second_network(self):
        for sample in (self.before, self.after):
            sample["networks"].append({**sample["networks"][0], "name": "第二接口",
                                       "interface_index": 13, "interface_luid": 123457,
                                       "interface_guid": "11223344-5566-7788-99aa-bbccddeeff00"})
        self.data["networks"].append({**self.data["networks"][0], "name": "第二接口"})
        self.data["network_count"] = 2

    def second_disk(self):
        for sample in (self.before, self.after):
            sample["filesystems"].append({**sample["filesystems"][0], "mount": "D:\\",
                                         "volume_guid": "\\\\?\\Volume{22222222-2222-3333-4444-555555555555}\\",
                                         "volume_serial": 43})
        self.data["filesystems"].append({**self.data["filesystems"][0], "source": "D:\\", "mount": "D:\\"})
        self.data["filesystem_count"] = 2

    def skip_capacity(self, status="skipped"):
        self.data.update(capacity_status=status, filesystems=[], filesystem_count=0)

    def test_all_networks_omitted_fail_against_nonempty_independent_inventory(self):
        self.data.update(networks=[], network_count=0)
        with self.assertRaisesRegex(lab.LabError, "omitted stable network"):
            self.compare()

    def test_partial_network_omission_fails_by_identity(self):
        self.second_network()
        self.data["networks"].pop()
        self.data["network_count"] = 1
        with self.assertRaisesRegex(lab.LabError, "第二接口"):
            self.compare()

    def test_all_ready_fixed_volumes_omitted_fail_against_independent_inventory(self):
        self.data.update(filesystems=[], filesystem_count=0)
        with self.assertRaisesRegex(lab.LabError, "omitted stable capacity"):
            self.compare()

    def test_partial_ready_fixed_volume_omission_fails_by_identity(self):
        self.second_disk()
        self.data["filesystems"].pop()
        self.data["filesystem_count"] = 1
        with self.assertRaisesRegex(lab.LabError, r"omitted stable capacity.*d:"):
            self.compare()

    def test_equal_count_substitutions_cannot_hide_missing_identities(self):
        for facility, field in (("networks", "name"), ("filesystems", "mount")):
            with self.subTest(facility=facility):
                self.setUp()
                self.data[facility][0][field] = "Substitute" if field == "name" else "Z:\\"
                with self.assertRaisesRegex(lab.LabError, "omitted stable"):
                    self.compare()

    def test_extra_rows_without_independent_evidence_fail(self):
        for facility, count, changed in (
                ("networks", "network_count", {"name": "Unexpected"}),
                ("filesystems", "filesystem_count", {"mount": "Z:\\", "source": "Z:\\"})):
            with self.subTest(facility=facility):
                self.setUp()
                self.data[facility].append({**self.data[facility][0], **changed})
                self.data[count] = 2
                with self.assertRaisesRegex(lab.LabError, "absent from independent stable inventory"):
                    self.compare()

    def test_stable_empty_inventories_allow_legal_empty_observation(self):
        self.data.update(networks=[], network_count=0, filesystems=[], filesystem_count=0)
        for sample in (self.before, self.after):
            sample.update(networks=[], filesystems=[])
        result = self.compare()
        self.assertEqual(result["network_inventory"]["required"], [])
        self.assertEqual(result["capacity_inventory"]["required"], [])

    def test_required_identity_evidence_is_saved_and_order_is_irrelevant(self):
        self.second_network()
        self.second_disk()
        self.data["networks"].reverse()
        self.after["filesystems"].reverse()
        self.data["filesystems"][0].update(source="c:\\", mount="c:\\")
        result = self.compare()
        self.assertEqual(result["network_inventory"]["required"], ["本地网络", "第二接口"])
        self.assertEqual(result["capacity_inventory"]["required"], ["c:\\", "d:\\"])
        self.assertEqual(len(result["network_inventory"]["stable_identities"]), 2)
        self.assertEqual(result["capacity_inventory"]["before_only"], [])

    def test_interface_addition_or_removal_holds_even_when_stable_intersection_matches(self):
        self.second_network()
        for sample in (self.before, self.after):
            with self.subTest(changed_side="before" if sample is self.before else "after"):
                missing = sample["networks"].pop()
                with self.assertRaisesRegex(lab.Hold, "network_inventory_changed"):
                    self.compare()
                sample["networks"].append(missing)

    def test_same_name_interface_replacement_or_rename_holds(self):
        for field, value in (("interface_index", 99), ("interface_luid", 98765),
                             ("interface_guid", "11223344-5566-7788-99aa-bbccddeeff00"),
                             ("name", "重命名接口")):
            with self.subTest(field=field):
                self.setUp()
                self.after["networks"][0][field] = value
                with self.assertRaisesRegex(lab.Hold, "network_inventory_changed"):
                    self.compare()

    def test_counter_reset_is_hold_even_if_interface_identity_survives(self):
        self.after["networks"][0]["received_bytes"] = 0
        with self.assertRaisesRegex(lab.Hold, "counters_reset"):
            self.compare()

    def test_stable_omission_is_not_exempted_by_unrelated_interface_churn(self):
        self.second_network()
        self.after["networks"].pop()
        self.data["networks"].pop(0)
        self.data["network_count"] = 1
        with self.assertRaisesRegex(lab.LabError, "omitted stable network"):
            self.compare()

    def test_drive_addition_or_removal_holds_without_claiming_complete_inventory(self):
        self.second_disk()
        for sample in (self.before, self.after):
            with self.subTest(changed_side="before" if sample is self.before else "after"):
                missing = sample["filesystems"].pop()
                with self.assertRaisesRegex(lab.Hold, "capacity_inventory_changed"):
                    self.compare()
                sample["filesystems"].append(missing)

    def test_same_root_volume_replacement_or_resize_holds(self):
        for field, value in (("volume_serial", 999),
                             ("volume_guid", "\\\\?\\Volume{22222222-2222-3333-4444-555555555555}\\"),
                             ("total_bytes", 1100000000)):
            with self.subTest(field=field):
                self.setUp()
                self.after["filesystems"][0][field] = value
                with self.assertRaises(lab.Hold):
                    self.compare()

    def test_stable_volume_omission_is_not_exempted_by_unrelated_volume_churn(self):
        self.second_disk()
        self.after["filesystems"].pop()
        self.data["filesystems"].pop(0)
        self.data["filesystem_count"] = 1
        with self.assertRaisesRegex(lab.LabError, "omitted stable capacity"):
            self.compare()

    def test_missing_or_ambiguous_independent_identity_cannot_sign_a_bracket(self):
        for facility, field in (("networks", "interface_luid"), ("filesystems", "volume_guid")):
            with self.subTest(facility=facility):
                self.setUp()
                del self.before[facility][0][field]
                with self.assertRaisesRegex(lab.Hold, "identity_missing"):
                    self.compare()
                self.setUp()
                self.after[facility].append(copy.deepcopy(self.after[facility][0]))
                with self.assertRaisesRegex(lab.Hold, "identity_ambiguous"):
                    self.compare()

    def test_incomplete_capacity_evidence_holds_and_still_checks_known_stable_roots(self):
        self.after["capacity_issues"] = [{"mount": "D:\\", "reason": "readiness_unconfirmed"}]
        with self.assertRaisesRegex(lab.Hold, "capacity_inventory_changed_or_incomplete"):
            self.compare()
        self.data.update(filesystems=[], filesystem_count=0)
        with self.assertRaisesRegex(lab.LabError, "omitted stable capacity"):
            self.compare()

    def test_unknown_capacity_with_no_comparable_rows_is_hold_not_mismatch_or_pass(self):
        for sample in (self.before, self.after):
            sample["filesystems"] = []
            sample["capacity_issues"] = [{"reason": "drive_enumeration_incomplete"}]
        with self.assertRaisesRegex(lab.Hold, "capacity_inventory_changed_or_incomplete"):
            self.compare()

    def test_capacity_skip_exempts_only_capacity(self):
        self.skip_capacity()
        self.after["filesystems"] = []
        self.after["capacity_issues"] = [{"reason": "drive_enumeration_incomplete"}]
        result = self.compare()
        self.assertEqual(result["capacity_inventory"], {"status": "not_compared", "reason": "skipped"})
        self.assertEqual(result["capacity_tolerances"], [])
        self.data.update(networks=[], network_count=0)
        with self.assertRaisesRegex(lab.LabError, "omitted stable network"):
            self.compare()

    def test_capacity_skip_cannot_exempt_partial_interface_omission(self):
        self.second_network()
        self.skip_capacity()
        self.data["networks"].pop()
        self.data["network_count"] = 1
        with self.assertRaisesRegex(lab.LabError, "omitted stable network"):
            self.compare()

    def test_capacity_skip_cannot_exempt_interface_changes_or_core_mismatch(self):
        for change in ("network", "memory"):
            with self.subTest(change=change):
                self.setUp()
                self.skip_capacity()
                if change == "network":
                    self.after["networks"][0]["interface_luid"] += 1
                    with self.assertRaises(lab.Hold):
                        self.compare()
                else:
                    self.data["memory_total_bytes"] = "1"
                    with self.assertRaises(lab.LabError):
                        self.compare()

    def test_capacity_failure_preserves_core_comparison_without_signing_capacity(self):
        self.skip_capacity("permission_denied")
        result = self.compare()
        self.assertEqual(result["capacity_inventory"]["reason"], "permission_denied")
        self.assertEqual(result["network_inventory"]["required"], ["本地网络"])

    def test_quota_available_and_total_free_have_separate_bracket_checks(self):
        for field in ("total_free_bytes", "available_bytes"):
            with self.subTest(field=field):
                self.setUp()
                self.data["filesystems"][0][field] = "0"
                if field == "total_free_bytes":
                    self.data["filesystems"][0]["available_bytes"] = "0"
                with self.assertRaisesRegex(lab.LabError, "capacity free/available"):
                    self.compare()


class NativeInventoryTests(unittest.TestCase):
    """Execute Python adapter/drive readers against controlled buffers, never Win32 APIs."""

    def setUp(self):
        self.oracle = object.__new__(lab.NativeOracle)
        self.oracle.budget = lab.Budget(40)
        self.oracle.kernel, self.oracle.ip = mock.Mock(), mock.Mock()
        self.roots = ["C:\\", "D:\\", "E:\\", "R:\\"]
        self.kinds = {"C:\\": 3, "D:\\": 3, "E:\\": 5, "R:\\": 4}
        self.serials = {"C:\\": 42, "D:\\": 43}
        self.guids = {"C:\\": "\\\\?\\Volume{11111111-2222-3333-4444-555555555555}\\",
                      "D:\\": "\\\\?\\Volume{22222222-2222-3333-4444-555555555555}\\"}
        self.last_error = 5
        self.get_error = mock.patch.object(lab.C, "get_last_error",
                                           side_effect=lambda: self.last_error, create=True)
        self.get_error.start()
        self.addCleanup(self.get_error.stop)
        kernel = self.oracle.kernel
        kernel.GetThreadErrorMode.return_value = 0x20
        kernel.SetThreadErrorMode.side_effect = self.error_mode
        kernel.GetLogicalDriveStringsW.side_effect = self.drive_strings
        kernel.GetDriveTypeW.side_effect = self.kinds.__getitem__
        kernel.GetVolumeInformationW.side_effect = self.volume_info
        kernel.GetVolumeNameForVolumeMountPointW.side_effect = self.volume_name
        kernel.GetDiskFreeSpaceExW.side_effect = self.space

    @staticmethod
    def error_mode(mode, previous):
        if previous is not None:
            previous._obj.value = 0x20
        return True

    def drive_strings(self, capacity, target):
        text = "\0".join(self.roots) + "\0\0"
        for index, char in enumerate(text):
            target[index] = char
        return len(text) - 1

    def volume_info(self, mount, label, label_size, serial, component, flags, filesystem, fs_size):
        serial._obj.value = self.serials[mount]
        return True

    def volume_name(self, mount, target, capacity):
        target.value = self.guids[mount]
        return True

    @staticmethod
    def space(mount, available, total, free):
        available._obj.value, total._obj.value, free._obj.value = 500000000, 1000000000, 600000000
        return True

    def test_independently_enumerates_every_ready_fixed_root_and_retains_volume_identity(self):
        result = self.oracle.filesystems()
        self.assertEqual([row["mount"] for row in result["filesystems"]], ["C:\\", "D:\\"])
        self.assertEqual(result["capacity_issues"], [])
        self.assertEqual(result["filesystems"][1]["volume_serial"], 43)
        self.assertEqual(result["filesystems"][1]["volume_guid"], self.guids["D:\\"].casefold())
        self.assertEqual(result["filesystems"][0]["total_free_bytes"], 600000000)
        self.assertEqual(result["filesystems"][0]["available_bytes"], 500000000)
        self.assertEqual([call.args[0] for call in self.oracle.kernel.GetDiskFreeSpaceExW.call_args_list],
                         ["C:\\", "D:\\"])
        self.oracle.kernel.SetThreadErrorMode.assert_has_calls(
            [mock.call(0x21, mock.ANY), mock.call(0x20, None)])

    def test_not_ready_fixed_drive_is_an_explicit_observation(self):
        self.oracle.kernel.GetVolumeInformationW.side_effect = None
        self.oracle.kernel.GetVolumeInformationW.return_value = False
        self.last_error = 21
        result = self.oracle.filesystems()
        self.assertEqual(result["filesystems"], [])
        self.assertEqual(result["capacity_issues"], [])
        self.assertEqual([(row["mount"], row["ready"]) for row in result["drive_inventory"]],
                         [("C:\\", False), ("D:\\", False), ("E:\\", None), ("R:\\", None)])
        self.oracle.kernel.GetDiskFreeSpaceExW.assert_not_called()

    def test_readiness_identity_or_space_failure_is_incomplete_evidence(self):
        for name, reason in (("GetVolumeInformationW", "readiness_unconfirmed"),
                             ("GetVolumeNameForVolumeMountPointW", "volume_identity_unconfirmed"),
                             ("GetDiskFreeSpaceExW", "capacity_unconfirmed")):
            with self.subTest(api=name):
                function = getattr(self.oracle.kernel, name)
                saved = function.side_effect
                function.side_effect, function.return_value = None, False
                result = self.oracle.filesystems()
                self.assertEqual(result["filesystems"], [])
                self.assertEqual({issue["reason"] for issue in result["capacity_issues"]}, {reason})
                self.oracle.kernel.SetThreadErrorMode.assert_called_with(0x20, None)
                function.side_effect = saved

    def test_enumeration_or_error_mode_failure_is_never_an_unqualified_empty_inventory(self):
        for result in (0, 128, 200):
            with self.subTest(length=result):
                self.oracle.kernel.GetLogicalDriveStringsW.side_effect = None
                self.oracle.kernel.GetLogicalDriveStringsW.return_value = result
                reading = self.oracle.filesystems()
                self.assertTrue(reading["capacity_issues"])
                self.assertEqual(reading["filesystems"], [])
                self.oracle.kernel.SetThreadErrorMode.assert_called_with(0x20, None)
        self.oracle.kernel.SetThreadErrorMode.side_effect = None
        self.oracle.kernel.SetThreadErrorMode.return_value = False
        self.assertEqual(self.oracle.filesystems()["capacity_issues"],
                         [{"reason": "disk_error_mode_unavailable"}])

    def test_malformed_duplicate_or_unc_roots_fail_and_restore_thread_error_mode(self):
        for roots in (["C:\\", "c:\\"], ["\\\\server\\share\\"], ["C:\\nested\\"], [""]):
            with self.subTest(roots=roots):
                self.roots = roots
                with self.assertRaises(lab.LabError):
                    self.oracle.filesystems()
                self.oracle.kernel.SetThreadErrorMode.assert_called_with(0x20, None)

    def test_invalid_volume_guid_or_error_mode_restore_failure_cannot_pass(self):
        self.guids["C:\\"] = "not-a-volume-guid"
        with self.assertRaises(lab.LabError):
            self.oracle.filesystems()
        self.oracle.kernel.SetThreadErrorMode.assert_called_with(0x20, None)
        self.guids["C:\\"] = self.guids["D:\\"]
        self.oracle.kernel.SetThreadErrorMode.side_effect = [True, False]
        with self.assertRaisesRegex(lab.LabError, "restore failed"):
            self.oracle.filesystems()

    def test_resources_has_no_production_mount_input_and_keeps_capacity_unknown_separate(self):
        def memory(target):
            target._obj.total, target._obj.free = 1073741824, 536870912
            return True
        self.oracle.kernel.GlobalMemoryStatusEx.side_effect = memory
        self.oracle.kernel.GetTickCount64.return_value = 12000
        self.oracle.kernel.GetDiskFreeSpaceExW.side_effect = None
        self.oracle.kernel.GetDiskFreeSpaceExW.return_value = False
        with mock.patch.object(self.oracle, "networks", return_value=[]):
            result = self.oracle.resources()
            self.assertEqual(result["memory_total_bytes"], 1073741824)
            self.assertTrue(result["capacity_issues"])
            self.assertEqual(len(result["drive_inventory"]), 4)
            with self.assertRaises(TypeError):
                self.oracle.resources(["C:\\"])

    def adapter_buffer(self, identities, fault=None):
        def read(family, flags, reserved, storage, length):
            self.assertEqual((family, flags, reserved), (0, 0xc0, None))
            start = ctypes.addressof(storage)
            size = ctypes.sizeof(lab.AdapterPrefix)
            length._obj.value = size * len(identities)
            for index, identity in enumerate(identities):
                row = lab.AdapterPrefix.from_buffer(storage, index * size)
                row.alignment = size
                row.luid = identity
                row.next = start + (index + 1) * size if index + 1 < len(identities) else None
                if fault == "cycle":
                    row.next = start
                elif fault == "pointer":
                    row.next = start + len(storage) + 1
                elif fault == "truncated":
                    row.alignment = size - 1
            return 0
        return read

    def test_ip_adapter_enumeration_retains_luids_and_has_explicit_empty_status(self):
        self.oracle.ip.GetAdaptersAddresses.side_effect = self.adapter_buffer([123456, 123457])
        self.assertEqual(self.oracle.adapter_luids(), {123456, 123457})
        self.oracle.ip.GetAdaptersAddresses.side_effect = None
        self.oracle.ip.GetAdaptersAddresses.return_value = 232
        self.assertEqual(self.oracle.adapter_luids(), set())

    def test_adapter_growth_retry_is_bounded_and_rechecks_the_original_budget(self):
        attempts = []
        def grow(family, flags, reserved, storage, length):
            attempts.append(len(storage))
            length._obj.value += 256
            return 111
        self.oracle.ip.GetAdaptersAddresses.side_effect = grow
        with self.assertRaisesRegex(lab.Hold, "kept_growing"):
            self.oracle.adapter_luids()
        self.assertEqual(len(attempts), 5)
        self.assertEqual(attempts, sorted(set(attempts)))
        self.oracle.budget = lab.Budget(5)
        with self.assertRaises(TimeoutError):
            self.oracle.adapter_luids()
        self.assertEqual(len(attempts), 5)

    def test_adapter_growth_then_success_and_non_success_are_distinct(self):
        success = self.adapter_buffer([123456])
        def grow_once(family, flags, reserved, storage, length):
            length._obj.value = 16000
            self.oracle.ip.GetAdaptersAddresses.side_effect = success
            return 111
        self.oracle.ip.GetAdaptersAddresses.side_effect = grow_once
        self.assertEqual(self.oracle.adapter_luids(), {123456})
        self.assertEqual(self.oracle.ip.GetAdaptersAddresses.call_count, 2)
        self.oracle.ip.GetAdaptersAddresses.side_effect = None
        self.oracle.ip.GetAdaptersAddresses.return_value = 5
        with self.assertRaisesRegex(lab.Hold, "enumeration_incomplete"):
            self.oracle.adapter_luids()

    def test_adapter_bad_links_truncation_duplicate_or_missing_identity_fail_closed(self):
        for identities, fault in (([123456], "cycle"), ([123456], "pointer"),
                                  ([123456], "truncated"), ([123456, 123456], None), ([0], None)):
            with self.subTest(identities=identities, fault=fault):
                self.oracle.ip.GetAdaptersAddresses.side_effect = self.adapter_buffer(identities, fault)
                with self.assertRaises(lab.LabError):
                    self.oracle.adapter_luids()

    def interface_table(self, luids):
        offset = (ctypes.sizeof(lab.U32) + ctypes.alignment(lab.IfRow) - 1) // \
            ctypes.alignment(lab.IfRow) * ctypes.alignment(lab.IfRow)
        storage = ctypes.create_string_buffer(offset + ctypes.sizeof(lab.IfRow) * len(luids))
        lab.U32.from_buffer(storage).value = len(luids)
        for index, luid in enumerate(luids):
            row = lab.IfRow.from_buffer(storage, offset + ctypes.sizeof(lab.IfRow) * index)
            row.luid, row.index, row.alias = luid, index + 12, "接口" + str(index)
            row.in_octets, row.out_octets = 100 + index, 200 + index
            guid = lab.uuid.UUID(int=index + 1)
            ctypes.memmove(ctypes.byref(row.guid), guid.bytes_le, 16)
        def read(pointer):
            pointer._obj.value = ctypes.addressof(storage)
            return 0
        self.oracle.ip.GetIfTable2.side_effect = read
        return storage

    def test_counter_inventory_joins_ip_enabled_luids_and_saves_stable_identity(self):
        table = self.interface_table([123456, 123457, 999999])
        with mock.patch.object(self.oracle, "adapter_luids", return_value={123456, 123457}):
            rows = self.oracle.networks()
        self.assertEqual([(row["interface_luid"], row["interface_index"]) for row in rows],
                         [(123456, 12), (123457, 13)])
        self.assertEqual(rows[0]["interface_guid"], "00000000-0000-0000-0000-000000000001")
        self.assertEqual(rows[1]["received_bytes"], 101)
        self.oracle.ip.FreeMibTable.assert_called_once()
        self.assertGreater(len(table), 0)  # Keep the backing allocation live through the reader.

    def test_adapter_disappearing_before_counter_read_holds_and_frees_table(self):
        table = self.interface_table([123456])
        with mock.patch.object(self.oracle, "adapter_luids", return_value={123456, 123457}):
            with self.assertRaisesRegex(lab.Hold, "inventory_changed"):
                self.oracle.networks()
        self.oracle.ip.FreeMibTable.assert_called_once()
        self.assertGreater(len(table), 0)


class DirectResourceOracleTests(unittest.TestCase):
    """Exercise real source assembly and gate routing with explicit offline doubles."""

    def setUp(self):
        self.data, self.before, self.after = independent_resources()
        self.ready = fixture_value()
        self.native = mock.Mock()
        self.native.resources.side_effect = [self.before, self.after]
        self.native.cpu_times.side_effect = [[10, 20, 20], [15, 30, 30]]
        self.native.process.return_value = {"pid": 1234, "name": "Python", "working_set_bytes": 21000000}
        self.native.sockets.return_value = [
            {**row, "state": "LISTEN" if row["protocol"] == "tcp" else "BOUND"}
            for row in self.ready["endpoints"]
        ] + [{**row, "state": "CONNECTED_OR_OTHER"} for row in self.ready["connected_only"]]
        self.fixture = mock.Mock(stdout=bytearray(), stderr=bytearray())
        self.fixture.ready.return_value = self.ready
        self.fixture.process.pid = 1234
        self.fixture.process.poll.return_value = None
        self.fixture.stop_fixture.return_value = 0
        self.fixture.close.return_value = {"errors": [], "observed_exit": 0,
                                          "terminal": {"direct_reaped": True, "active_processes": 0,
                                                       "pipes_drained": True, "errors": []}}
        self.temporary = tempfile.TemporaryDirectory(prefix="keelshell-offline-oracle-routing-")
        self.addCleanup(self.temporary.cleanup)
        self.evidence = lab.Evidence(Path(self.temporary.name) / "output")
        self.evidence.run = mock.Mock(side_effect=self.collect)

    def collect(self, label, argv, seconds, required_image):
        operation = "resources" if label == "resources-skipped" else label
        values = {"resources": self.data, "probe": {"os": "Win32NT"},
                  "processes": {"count": 1, "processes": [
                      {"pid": "1234", "name": "python", "working_set_bytes": "20000000"}]},
                  "sockets": {"count": 4, "sockets": self.ready["endpoints"]}}
        value = copy.deepcopy(values[operation])
        if label == "resources-skipped":
            value.update(capacity_status="skipped", filesystems=[], filesystem_count=0)
        command, _ = lab.assemble(label)
        self.assertEqual(argv[-1], command)
        return lab.CommandResult(raw(frame(operation, value)), b"", 0, self.fixture.close.return_value)

    def gate(self):
        with mock.patch.object(lab, "os", types.SimpleNamespace(name="nt")), \
                mock.patch.object(lab, "powershell_path", return_value=sys.executable), \
                mock.patch.object(lab, "cmd_path", return_value="offline-cmd-boundary"), \
                mock.patch.object(lab, "OwnedChild", return_value=self.fixture), \
                mock.patch.object(lab, "NativeOracle", return_value=self.native):
            return lab.direct_gate(self.evidence)

    def test_both_oracle_snapshots_are_independent_of_production_mount_arguments(self):
        self.assertEqual(self.gate()["status"], "PASS")
        self.assertEqual(self.native.resources.call_args_list, [mock.call(), mock.call()])
        saved = json.loads((self.evidence.path / "independent.json").read_text())
        self.assertEqual(saved["before"]["filesystems"], self.before["filesystems"])
        self.assertEqual(saved["after"]["networks"], self.after["networks"])
        self.assertEqual(self.evidence.run.call_count, 5)
        self.fixture.stop_fixture.assert_called_once()
        self.assertTrue(lab.terminal_ok(self.evidence.children[0]))

    def test_gate_rejects_a_volume_absent_only_from_production_and_retains_readings_and_cleanup(self):
        for sample in (self.before, self.after):
            sample["filesystems"].append({**sample["filesystems"][0], "mount": "D:\\",
                                         "volume_guid": "\\\\?\\Volume{22222222-2222-3333-4444-555555555555}\\",
                                         "volume_serial": 43})
        with self.assertRaisesRegex(lab.LabError, "omitted stable capacity"):
            self.gate()
        self.assertEqual(self.native.resources.call_args_list, [mock.call(), mock.call()])
        self.assertTrue((self.evidence.path / "independent-before.json").is_file())
        self.assertTrue((self.evidence.path / "independent.json").is_file())
        self.fixture.close.assert_called_once()
        self.assertTrue(lab.terminal_ok(self.evidence.children[0]))

    def test_gate_keeps_capacity_failure_as_hold_after_core_checks_and_fixture_terminal(self):
        self.data.update(capacity_status="permission_denied", filesystems=[], filesystem_count=0)
        with self.assertRaisesRegex(lab.Hold, "capacity_permission_denied_core_frames_retained"):
            self.gate()
        self.fixture.stop_fixture.assert_called_once()
        self.assertTrue(lab.terminal_ok(self.evidence.children[0]))


class LifecycleTests(unittest.TestCase):
    def child(self, source, seconds=12):
        child = lab.OwnedChild([sys.executable, "-u", "-c", source], lab.Budget(seconds))
        self.addCleanup(child.close)
        child.start()
        return child

    def test_real_child_preserves_raw_bytes_and_actual_exit(self):
        child = self.child("import os;os.write(1,b'\\xff\\r\\n');os.write(2,b'error\\n');raise SystemExit(7)")
        self.assertEqual(child.wait(), 7)
        receipt = child.close()
        self.assertEqual(bytes(child.stdout), b"\xff\r\n")
        self.assertEqual(bytes(child.stderr), b"error\n")
        self.assertEqual(receipt["observed_exit"], 7)
        self.assertTrue(lab.terminal_ok(receipt))

    def test_stream_closure_does_not_prove_process_exit(self):
        child = self.child("import os,time;os.close(1);os.close(2);time.sleep(40)", seconds=6)
        with self.assertRaises(TimeoutError):
            child.wait()
        receipt = child.close()
        self.assertIsNone(receipt["observed_exit"])
        self.assertTrue(lab.terminal_ok(receipt))

    def test_live_child_timeout_has_bounded_original_process_terminal(self):
        started = time.monotonic()
        child = self.child("import time;time.sleep(40)", seconds=6)
        with self.assertRaises(TimeoutError):
            child.wait()
        receipt = child.close()
        self.assertLess(time.monotonic() - started, 8)
        self.assertTrue(lab.terminal_ok(receipt))
        self.assertIsNotNone(child.process.returncode)

    def test_parent_and_real_descendant_share_owned_cleanup(self):
        child = self.child(
            "import subprocess,sys,time;"
            "p=subprocess.Popen([sys.executable,'-c','import time;time.sleep(40)']);"
            "print(p.pid,flush=True);time.sleep(40)")
        while b"\n" not in child.stdout:
            child.drain()
            time.sleep(0.01)
        self.assertGreater(int(bytes(child.stdout).strip()), 0)
        receipt = child.close()
        self.assertTrue(lab.terminal_ok(receipt))
        if os.name == "nt":
            self.assertGreaterEqual(receipt["terminal"]["native_process_objects"]["retained_processes"], 2)

    def test_real_dual_stack_fixture_and_natural_exit(self):
        if not socket.has_ipv6:
            self.skipTest("host has no IPv6")
        try:
            with socket.socket(socket.AF_INET6, socket.SOCK_STREAM) as check:
                check.bind(("::1", 0))
        except OSError:
            self.skipTest("host IPv6 loopback unavailable")
        child = lab.OwnedChild([sys.executable, "-u", str(lab.FIXTURES / "owned_fixture.py"),
                                "--seconds", "10"], lab.Budget(14))
        self.addCleanup(child.close)
        child.start()
        ready = child.ready()
        self.assertEqual(ready["pid"], child.process.pid)
        self.assertEqual(len(ready["endpoints"]), 4)
        self.assertEqual(len(ready["connected_only"]), 2)
        self.assertEqual(child.stop_fixture(), 0)
        self.assertTrue(lab.terminal_ok(child.close()))

    def test_output_overflow_still_reaps_real_child(self):
        child = self.child("import os,time;os.write(1,b'x'*100000);time.sleep(40)")
        with mock.patch.object(lab, "MAX_FRAME", 8192):
            with self.assertRaises(lab.LabError):
                child.wait()
            receipt = child.close()
        self.assertIn("output_budget_exceeded", receipt["errors"])
        self.assertEqual(len(child.stdout) + len(child.stderr), 8192)
        self.assertTrue(receipt["terminal"]["direct_reaped"])
        self.assertEqual(receipt["terminal"]["active_processes"], 0)

    def test_evidence_preserves_fixed_capture_filename_bytes_and_receipt(self):
        with tempfile.TemporaryDirectory(prefix="keelshell-monitor-evidence-test-") as directory:
            evidence = lab.Evidence(Path(directory) / "output", 20)
            result = evidence.run("probe", [sys.executable, "-c",
                                           "import os;os.write(1,b'\\xff\\r\\n')"], 12)
            self.assertEqual((evidence.path / "probe.stdout").read_bytes(), b"\xff\r\n")
            self.assertEqual(result.stdout, b"\xff\r\n")
            self.assertTrue(lab.terminal_ok(evidence.children[0]))
            self.assertEqual(json.loads((evidence.path / "probe.receipt.json").read_text())["observed_exit"], 0)

    def test_terminal_missing_or_cleanup_errors_never_sign_success(self):
        for receipt in ({"errors": [], "terminal": None},
                        {"errors": ["failure"], "terminal": {"direct_reaped": True,
                         "active_processes": 0, "pipes_drained": True, "errors": []}}):
            self.assertFalse(lab.terminal_ok(receipt))

    def test_budget_never_extends_parent_deadline(self):
        deadline = time.monotonic() + 2
        budget = lab.Budget(10, deadline)
        self.assertEqual(budget.end, deadline)
        with self.assertRaises(TimeoutError):
            budget.remaining(reserve=3)


class SafetyAndWorkflowTests(unittest.TestCase):
    def test_shell_process_proof_cannot_fall_back_to_posix(self):
        if os.name == "nt":
            self.skipTest("negative non-Windows boundary only")
        with self.assertRaises(lab.Hold):
            lab.OwnedChild(["unexecuted"], lab.Budget(10), required_image="powershell.exe")

    def test_nonwindows_native_oracle_is_hold(self):
        if os.name == "nt":
            self.skipTest("negative non-Windows boundary only")
        with self.assertRaises(lab.Hold):
            lab.NativeOracle(lab.Budget(10))

    def test_cli_records_nonwindows_hold_without_running_commands(self):
        if os.name == "nt":
            self.skipTest("negative non-Windows boundary only")
        for mode in ("direct", "ssh"):
            with tempfile.TemporaryDirectory(prefix="keelshell-monitor-hold-test-") as directory:
                output = Path(directory) / "output"
                with mock.patch.object(lab.Evidence, "run", side_effect=AssertionError("no commands")):
                    self.assertEqual(lab.run_lab(mode, output, 10), lab.HOLD_EXIT)
                receipt = json.loads((output / "receipt.json").read_text())
                self.assertEqual(receipt["status"], "HOLD")
                self.assertEqual(receipt["reason"], "native_windows_required")
                self.assertEqual(receipt["children"], [])

    def test_existing_evidence_directory_is_never_overwritten(self):
        with tempfile.TemporaryDirectory(prefix="keelshell-monitor-existing-test-") as directory:
            with self.assertRaises(FileExistsError):
                lab.Evidence(directory)

    def test_evidence_path_escape_rejected(self):
        with tempfile.TemporaryDirectory(prefix="keelshell-monitor-path-test-") as directory:
            evidence = lab.Evidence(Path(directory) / "output")
            with self.assertRaises(lab.LabError):
                evidence.write_bytes("../escape", b"")

    def test_ssh_config_is_explicit_loopback_keys_and_forced_no_execution(self):
        config = lab.ssh_config(Path("owned-private"), 55001)
        for expected in ("Port 55001", "ListenAddress 127.0.0.1", "StrictModes yes",
                         'AuthorizedKeysFile "owned-private/authorized_keys"',
                         "PasswordAuthentication no", "DisableForwarding yes",
                         "ForceCommand cmd.exe /c exit 125"):
            self.assertIn(expected, config)
        for forbidden in ("Include ", "Match ", ".ssh/", "administrators_authorized_keys"):
            self.assertNotIn(forbidden, config)

    def test_ssh_config_rejects_low_port_and_path_injection(self):
        with self.assertRaises(lab.LabError):
            lab.ssh_config(Path("owned-private"), 22)
        with self.assertRaises(lab.Hold):
            lab.ssh_config(Path('owned"\nInclude dangerous'), 55001)

    def test_fixed_width_native_table_layouts(self):
        # These are ABI declarations, not Win32 calls or a native acceptance.
        self.assertEqual(ctypes.sizeof(lab.Tcp4), 24)
        self.assertEqual(ctypes.sizeof(lab.Tcp6), 56)
        self.assertEqual(ctypes.sizeof(lab.Udp4), 12)
        self.assertEqual(ctypes.sizeof(lab.Udp6), 28)
        self.assertEqual(ctypes.sizeof(lab.MemoryStatus), 64)
        self.assertEqual(ctypes.sizeof(lab.AdapterPrefix), 232 if ctypes.sizeof(ctypes.c_void_p) == 8 else 184)
        self.assertEqual(lab.AdapterPrefix.luid.offset, 224 if ctypes.sizeof(ctypes.c_void_p) == 8 else 176)
        if os.name == "nt":
            self.assertEqual(ctypes.sizeof(lab.IfRow), 1352)
            self.assertEqual(lab.IfRow.in_octets.offset, 1208)

    def test_workflow_has_independent_gates_setup_upload_and_docs_filter(self):
        text = (lab.ROOT / ".github/workflows/windows-monitor.yml").read_text()
        self.assertEqual(text.count("runs-on: windows-2025"), 2)
        self.assertEqual(text.count("uses: ./.github/actions/setup-build"), 2)
        self.assertIn("direct-api:", text)
        self.assertIn("isolated-ssh:", text)
        self.assertEqual(text.count("if: always()"), 3)
        self.assertIn("paths-ignore: ['docs/**', '**/*.md']", text)
        self.assertIn("persist-credentials: false", text)
        self.assertNotIn("continue-on-error", text)
        self.assertNotIn("cancel-in-progress", text)

    def test_workflow_replay_uses_exact_capture_and_cannot_silently_skip_hold(self):
        text = (lab.ROOT / ".github/workflows/windows-monitor.yml").read_text()
        self.assertIn("KEELSHELL_WINDOWS_MONITOR_CAPTURE_DIR:", text)
        self.assertIn("captured_windows_api_frames_are_accepted_through_authenticated_monitor_routing"
                      " -- --ignored --exact --test-threads=1", text)
        self.assertIn("CAPTURE_REPLAY_HOLD:", text)
        for operation in lab.OPERATIONS:
            self.assertIn(operation + ".stdout", text)


if __name__ == "__main__":
    unittest.main(verbosity=2)
