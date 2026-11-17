#!/usr/bin/env python3
"""Offline admission/security contracts and real host-child environment controls."""

import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

import windows_monitor_lab as lab
from windows_monitor import ssh
from test_windows_monitor_lab import fixture_value, resource_data

PS = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"


def snapshot():
    rows = [{
        "pid": 100, "creation_time": 1000, "image": r"C:\Windows\System32\OpenSSH\sshd.exe",
        "command_line": "sshd.exe -D", "is_master": True,
        "retained_before_first_instruction": True, "natural_terminal_before_cleanup": False,
        "identity_bound_before_first_continue": True,
        "exit_code": None,
    }]
    for number, operation in enumerate(lab.OPERATIONS, 101):
        command, _ = lab.assemble(operation)
        rows.append({
            "pid": number, "creation_time": 1000 + number, "image": PS,
            "command_line": command, "is_master": False,
            "retained_before_first_instruction": True, "natural_terminal_before_cleanup": True,
            "identity_bound_before_first_continue": True,
            "exit_code": 0,
        })
    return {"errors": [], "admissions": rows, "complete_original_object_census": True,
            "admission_closed": True, "identities_bound": True, "evidence_kind": "NATIVE_WINDOWS"}


def commands():
    return {name: lab.assemble(name)[0] for name in lab.ASSEMBLY}


class ConfigurationTests(unittest.TestCase):
    def test_loopback_key_only_server_keeps_real_default_shell(self):
        config = ssh.server_config(Path("owned private"), 54321, "runneradmin")
        rows = config.splitlines()
        self.assertIn("ListenAddress 127.0.0.1", rows)
        self.assertIn('AuthorizedKeysFile "owned private/authorized_keys"', rows)
        self.assertIn("AuthenticationMethods publickey", rows)
        self.assertIn("PasswordAuthentication no", rows)
        self.assertIn("DisableForwarding yes", rows)
        self.assertIn("AllowUsers runneradmin", rows)
        self.assertIn("PermitTTY no", rows)
        directives = {row.split()[0].lower() for row in rows}
        self.assertTrue(directives.isdisjoint({
            "forcecommand", "include", "match", "strictmodes", "pidfile",
            "kbdinteractiveauthentication", "hostbasedauthentication",
            "permittunnel", "permituserenvironment",
        }))

    def test_invalid_high_port_cannot_create_configuration(self):
        for value in (0, -1, 49151, 65536, True, "54321"):
            with self.subTest(value=value), self.assertRaises(lab.LabError):
                ssh.server_config(Path("owned"), value, "runneradmin")

    def test_login_cannot_inject_options_or_select_other_accounts(self):
        for value in ("", "*", "runner*", "user@*", "user\nInclude shared", "two users",
                      "Runner", "user\x00", None):
            with self.subTest(value=value), self.assertRaises(lab.LabError):
                ssh.server_config(Path("owned"), 54321, value)

    def test_domain_login_is_explicit_and_lowercase(self):
        self.assertIn(r"AllowUsers ci\runner", ssh.server_config(Path("owned"), 54321, r"ci\runner"))

    def test_path_metacharacters_cannot_escape_key_directives(self):
        for value in ('owned" Include shared', "owned\nHostKey shared", "owned\rpath", "owned\x00"):
            with self.subTest(value=value), self.assertRaises(lab.Hold):
                ssh.server_config(Path(value), 54321, "runner")

    def test_host_fingerprint_hashes_decoded_key_not_display_comment(self):
        blob = b"\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20" + b"\x11" * 32
        wire = b"ssh-ed25519 " + base64.b64encode(blob)
        expected = "SHA256:" + base64.b64encode(hashlib.sha256(blob).digest()).decode().rstrip("=")
        self.assertEqual(ssh.host_fingerprint(wire), expected)
        self.assertEqual(ssh.host_fingerprint(wire + b" owned-lab\n"), expected)

    def test_wrong_key_algorithm_size_or_base64_is_rejected(self):
        for raw in (b"", b"ssh-rsa abc", b"ssh-ed25519 @!", b"ssh-ed25519 YQ==",
                    b"ssh-ed25519 YQ== two comments"):
            with self.subTest(raw=raw), self.assertRaises(lab.LabError):
                ssh.host_fingerprint(raw)

    def test_required_native_test_must_be_present_once(self):
        valid = (ssh.TEST_NAME + ": test\nanother_control: test\n").encode()
        self.assertEqual(ssh.check_inventory(valid), [ssh.TEST_NAME, "another_control"])
        for raw in (b"", b"unrelated: test\n", valid + valid):
            with self.subTest(raw=raw), self.assertRaises(lab.LabError):
                ssh.check_inventory(raw)

    def test_binary_path_rejects_other_targets_and_non_executables(self):
        with tempfile.TemporaryDirectory() as name:
            for basename, data in (("unrelated.exe", b"MZ"),
                                   ("windows_openssh_monitor-abc.exe", b"not-native")):
                path = Path(name) / basename
                path.write_bytes(data)
                with self.subTest(basename=basename), self.assertRaises(lab.Hold):
                    ssh.test_binary(path)


class AdmissionEvidenceTests(unittest.TestCase):
    def check(self, value):
        return ssh.validate_powershell_admissions(value, commands(), PS)

    def test_four_exact_original_objects_and_live_master(self):
        result = self.check(snapshot())
        self.assertEqual(sorted(row["operation"] for row in result["operations"]), sorted(lab.OPERATIONS))
        self.assertTrue(result["source_exact"] and result["original_processes_naturally_terminal"])

    def test_quoted_absolute_powershell_image_is_accepted(self):
        value = snapshot()
        for row in value["admissions"][1:]:
            row["command_line"] = '"' + PS + '"' + row["command_line"][len("powershell.exe"):]
        self.assertEqual(len(self.check(value)["operations"]), 4)

    def test_unclosed_admission_or_incomplete_census_never_signs_success(self):
        for field in ("admission_closed", "complete_original_object_census", "identities_bound"):
            for state in (False, None, 1, "true"):
                value = snapshot()
                value[field] = state
                with self.subTest(field=field, state=state), self.assertRaises(lab.Hold):
                    self.check(value)

    def test_missing_or_duplicate_operation_is_rejected(self):
        missing = snapshot()
        missing["admissions"].pop()
        duplicate = snapshot()
        duplicate["admissions"][-1]["command_line"] = duplicate["admissions"][1]["command_line"]
        for value in (missing, duplicate):
            with self.assertRaises(lab.Hold):
                self.check(value)

    def test_original_object_must_be_retained_before_first_instruction(self):
        for row_number in (0, 1):
            value = snapshot()
            value["admissions"][row_number]["retained_before_first_instruction"] = False
            with self.assertRaises(lab.Hold):
                self.check(value)

    def test_channel_completion_or_cleanup_kill_is_not_natural_exit(self):
        for state in (False, None, "true"):
            value = snapshot()
            value["admissions"][1]["natural_terminal_before_cleanup"] = state
            with self.assertRaises(lab.Hold):
                self.check(value)

    def test_command_prefix_injection_suffix_and_wrong_program_are_rejected(self):
        command = snapshot()["admissions"][1]["command_line"]
        for changed in ("evil -File bootstrap.ps1 " + command,
                        command + " trailing", command + '"', command.replace("powershell.exe", "other.exe")):
            value = snapshot()
            value["admissions"][1]["command_line"] = changed
            with self.subTest(changed=changed[:50]), self.assertRaises(lab.LabError):
                self.check(value)

    def test_source_changed_command_and_nonzero_exit_fail(self):
        for field, changed in (("command_line", "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand AAAA"),
                               ("exit_code", 1), ("exit_code", None)):
            value = snapshot()
            value["admissions"][1][field] = changed
            with self.subTest(field=field), self.assertRaises(lab.LabError):
                self.check(value)

    def test_unproven_extra_broker_descendant_cannot_disappear(self):
        value = snapshot()
        value["admissions"].append({
            "pid": 200, "creation_time": 1200, "image": "sshd-session.exe", "command_line": "session",
            "is_master": False, "retained_before_first_instruction": True,
            "identity_bound_before_first_continue": True,
            "natural_terminal_before_cleanup": False, "exit_code": None,
        })
        with self.assertRaises(lab.Hold):
            self.check(value)

    def test_pid_reuse_and_missing_creation_identity_are_rejected(self):
        reused = snapshot()
        reused["admissions"][2]["pid"] = reused["admissions"][1]["pid"]
        missing = snapshot()
        del missing["admissions"][1]["creation_time"]
        for value in (reused, missing):
            with self.assertRaises(lab.LabError):
                self.check(value)

    def test_debug_error_cannot_be_suppressed_by_complete_census_flag(self):
        value = snapshot()
        value["errors"] = ["lost_debug_event"]
        with self.assertRaises(lab.Hold):
            self.check(value)

    def test_master_count_is_bound_to_exactly_one_server(self):
        value = snapshot()
        value["admissions"][0]["is_master"] = False
        value["admissions"][0]["natural_terminal_before_cleanup"] = True
        with self.assertRaises(lab.Hold):
            self.check(value)

    def test_offline_control_cannot_be_relabelled_as_native_evidence(self):
        value = snapshot()
        value["evidence_kind"] = "OFFLINE_CONTROL_NOT_NATIVE"
        with self.assertRaises(lab.Hold):
            self.check(value)

    def test_metadata_binding_before_continue_is_required(self):
        value = snapshot()
        value["admissions"][1]["identity_bound_before_first_continue"] = False
        with self.assertRaises(lab.Hold):
            self.check(value)


def client_report():
    assertions = (
        "wrong_pin_rejected_before_authenticated_session",
        "independent_identity_is_Windows_PowerShell_5_1",
        "Windows_unknown_fields_and_CPU_gauge_are_not_invented",
        "owned_fixture_PID_and_real_working_set_are_observed",
        "owned_dual_stack_listener_or_UDP_binding_is_visible",
        "connected_only_TCP_endpoint_is_excluded",
        "cache_reads_and_clone_preserve_observations_and_authority",
        "pause_revokes_old_report_but_preserves_completed_observations",
        "clone_revocation_is_shared_and_old_reports_stay_revoked",
        "explicit_reset_clears_shared_cache_without_redetection",
    )
    return {
        "schema": 1, "status": "PASS", "test": ssh.TEST_NAME, "errors": [],
        "identity": {"platform": "Windows", "runtime": "WindowsPowerShell",
                     "runtime_version": "5.1.26100.1"},
        "assertion_evidence": [{"assertion": name, "passed": True} for name in assertions],
        "api_calls": {"snapshot": 1, "processes": 1, "listening_sockets": 1},
        "disconnect": {"attempted": True, "request_succeeded": True, "local_transport_closed": True},
        "fixture": fixture_value(),
        "windows_resources": resource_data(),
        "windows_processes": {"count": 0, "processes": []},
        "windows_sockets": {"count": 0, "sockets": []},
    }


class ClientReportTests(unittest.TestCase):
    def test_complete_structured_control_is_validated_without_signing_native_execution(self):
        value = client_report()
        self.assertIs(ssh.validate_client_report(value, fixture_value()), value)

    def test_pass_label_cannot_override_failed_or_missing_assertion(self):
        for changed in ("failed", "missing"):
            value = client_report()
            if changed == "failed":
                value["assertion_evidence"][0]["passed"] = False
            else:
                value["assertion_evidence"].pop(0)
            with self.subTest(changed=changed), self.assertRaises(lab.LabError):
                ssh.validate_client_report(value)

    def test_parent_fixture_is_not_selected_by_the_rust_report(self):
        value = client_report()
        value["fixture"]["pid"] += 1
        with self.assertRaises(lab.LabError):
            ssh.validate_client_report(value, fixture_value())

    def test_unproven_local_disconnect_is_rejected(self):
        for field in ("attempted", "request_succeeded", "local_transport_closed"):
            value = client_report()
            value["disconnect"][field] = False
            with self.subTest(field=field), self.assertRaises(lab.LabError):
                ssh.validate_client_report(value)

    def test_other_runtime_and_multiple_api_calls_cannot_pass(self):
        other_runtime = client_report()
        other_runtime["identity"]["runtime"] = "PowerShellCore"
        extra_calls = client_report()
        extra_calls["api_calls"]["snapshot"] = 2
        fake_version = client_report()
        fake_version["identity"]["runtime_version"] = "5.1.01.2"
        for value in (other_runtime, extra_calls, fake_version):
            with self.assertRaises(lab.LabError):
                ssh.validate_client_report(value)

    def test_held_capacity_is_not_an_empty_success(self):
        value = client_report()
        value["windows_resources"].update(capacity_status="permission_denied",
                                           filesystem_count=0, filesystems=[])
        with self.assertRaises(lab.Hold):
            ssh.validate_client_report(value)


class CleanupEvidenceTests(unittest.TestCase):
    def final(self):
        value = snapshot()
        value.update(state="PASS", terminal_proven=True, closed=True,
                     pending_process_objects=0, owned_handles_remaining=0, fallback_detaches=0)
        identities = {(row["pid"], row["creation_time"]) for row in value["admissions"]}
        return identities, value

    def test_terminal_service_cleanup_preserves_natural_collectors(self):
        admitted, value = self.final()
        ssh.validate_debug_cleanup(admitted, value)

    def test_late_unaccounted_admission_and_identity_replacement_are_rejected(self):
        admitted, value = self.final()
        extra = copy.deepcopy(value)
        extra["admissions"].append({**extra["admissions"][1], "pid": 900})
        replaced = copy.deepcopy(value)
        replaced["admissions"][1]["creation_time"] += 1
        for changed in (extra, replaced):
            with self.assertRaises(lab.LabError):
                ssh.validate_debug_cleanup(admitted, changed)

    def test_fallback_detach_live_handle_and_pending_objects_never_prove_terminal(self):
        for field in ("fallback_detaches", "owned_handles_remaining", "pending_process_objects"):
            admitted, value = self.final()
            value[field] = 1
            with self.subTest(field=field), self.assertRaises(lab.LabError):
                ssh.validate_debug_cleanup(admitted, value)

    def test_cleanup_killed_collector_cannot_replace_original_natural_exit(self):
        admitted, value = self.final()
        value["admissions"][1]["natural_terminal_before_cleanup"] = False
        with self.assertRaises(lab.LabError):
            ssh.validate_debug_cleanup(admitted, value)


class SourceBindingTests(unittest.TestCase):
    def evidence(self, head=None, status=None):
        evidence = mock.Mock()
        evidence.run.side_effect = [
            head or lab.CommandResult(b"a" * 40 + b"\n", b"", 0, {}),
            status or lab.CommandResult(b"", b"", 0, {}),
        ]
        return evidence

    def test_clean_exact_git_source_required(self):
        evidence = self.evidence()
        ssh.verify_source(evidence, "a" * 40)
        self.assertEqual(evidence.run.call_count, 2)

    def test_missing_or_nonexact_ref_is_rejected_before_any_git_command(self):
        for source in (None, "HEAD", "a" * 39, "A" * 40, "g" * 40):
            evidence = self.evidence()
            with self.subTest(source=source), self.assertRaises(lab.Hold):
                ssh.verify_source(evidence, source)
            evidence.run.assert_not_called()

    def test_dirty_source_wrong_head_and_failed_git_cannot_sign_native_run(self):
        cases = [
            self.evidence(head=lab.CommandResult(b"b" * 40 + b"\n", b"", 0, {})),
            self.evidence(status=lab.CommandResult(b" M scripts/changed.py\n", b"", 0, {})),
            self.evidence(head=lab.CommandResult(b"", b"failure", 1, {})),
            self.evidence(status=lab.CommandResult(b"", b"", 1, {})),
        ]
        for evidence in cases:
            with self.assertRaises(lab.Hold):
                ssh.verify_source(evidence, "a" * 40)


class HostBoundaryTests(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "negative non-Windows boundary only")
    def test_nonwindows_cli_preserves_explicit_hold_without_running_children(self):
        with tempfile.TemporaryDirectory() as name:
            output = Path(name) / "new-evidence"
            result = subprocess.run([
                sys.executable, "-B", str(Path(lab.ROOT) / "scripts/windows_monitor_ssh.py"),
                "--source", "a" * 40, "--test-binary", "never-executed.exe",
                "--output", str(output), "--seconds", "10",
            ], capture_output=True, timeout=15, check=False)
            self.assertEqual(result.returncode, lab.HOLD_EXIT)
            receipt = json.loads((output / "receipt.json").read_text())
            self.assertEqual((receipt["status"], receipt["ssh"], receipt["windows_api"]), ("HOLD", "HOLD", "UNRUN"))
            self.assertEqual(receipt["children"], [])
            self.assertEqual(receipt["native_gui"], "UNRUN")

    def test_explicit_child_environment_is_scoped_to_that_real_owned_child(self):
        child = lab.OwnedChild(
            [sys.executable, "-B", "-u", "-c", "import os; print(os.environ['KEELSHELL_LAB_CHILD_VALUE'])"],
            lab.Budget(12),
        )
        original = os.environ.get("KEELSHELL_LAB_CHILD_VALUE")
        environment = dict(os.environ, KEELSHELL_LAB_CHILD_VALUE="owned-only")
        try:
            child.start(environment=environment)
            self.assertEqual(child.wait(), 0)
        finally:
            receipt = child.close()
        self.assertTrue(lab.terminal_ok(receipt))
        self.assertEqual(bytes(child.stdout), b"owned-only\r\n" if os.name == "nt" else b"owned-only\n")
        self.assertEqual(os.environ.get("KEELSHELL_LAB_CHILD_VALUE"), original)

    def test_later_hold_preserves_partial_api_observations(self):
        def incomplete(evidence, _):
            evidence.write_json("independent-before.json", {"scope": "offline routing control"})
            raise lab.Hold("unproven_original_object")

        with tempfile.TemporaryDirectory() as name:
            output = Path(name) / "evidence"
            with mock.patch.object(ssh, "os", types.SimpleNamespace(name="nt")), \
                    mock.patch.object(ssh, "verify_source"), \
                    mock.patch.object(ssh, "_native_gate", side_effect=incomplete):
                self.assertEqual(ssh.run_live("never-executed", output, 10, "a" * 40), lab.HOLD_EXIT)
            receipt = json.loads((output / "receipt.json").read_text())
            self.assertEqual(receipt["status"], "HOLD")
            self.assertEqual(receipt["windows_api"], "PARTIAL")
            self.assertFalse(receipt["ssh_client_started"])
            self.assertEqual(receipt["children"], [])


if __name__ == "__main__":
    unittest.main()
