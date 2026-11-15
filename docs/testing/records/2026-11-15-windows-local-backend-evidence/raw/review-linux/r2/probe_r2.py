#!/usr/bin/env python3
"""Fresh reviewer probes; no old probe execution, Cargo, or real containers."""

import argparse
import copy
import io
import json
from pathlib import Path
import signal
import sys
import unittest
from unittest import mock

REVIEW = Path(__file__).resolve().parent
ROOT = REVIEW.parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import run_linux_local_backend as backend
import test_linux_local_backend as author


class ReviewerCases(unittest.TestCase):
    def run_case(self, label, primary=False, finish_error=False,
                 denied_write=None, optional_ignored=False, wrong_machine=False):
        output = REVIEW / "scenarios" / label
        output.parent.mkdir(exist_ok=True)
        self.assertFalse(output.exists(), "Never overwrite retained scenario evidence")
        events = []
        receipt_writes = 0
        actual_write = backend.write_json
        handlers = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}

        class Commands(author.SyntheticCommands):
            def run(self, name, argv, **kwargs):
                if primary and name == "resolve-revision":
                    raise backend.BackendFailure("reviewer primary failure")
                if wrong_machine and name.startswith("binary-elf-"):
                    rc, data = super().run(name, argv, **kwargs)
                    return rc, data.replace(b"AArch64", b"Advanced Micro Devices X86-64")
                if optional_ignored and name.startswith("inventory-"):
                    rc, data = super().run(name, argv, **kwargs)
                    return rc, data + b"zmodem::optional_selected: test\n"
                if optional_ignored and name.startswith("test-keelshell-session-"):
                    rc, data = super().run(name, argv, **kwargs)
                    return rc, data + b"test zmodem::optional_selected ... ignored\n"
                return super().run(name, argv, **kwargs)

            def finish(self):
                events.append({"event": "finish"})
                if finish_error:
                    raise PermissionError("reviewer finish signal denied")
                return True

        def remove(*args):
            existing = output / "receipt.json"
            events.append({
                "event": "remove",
                "provisional_receipt": json.loads(existing.read_text()) if existing.exists() else None,
            })
            return {"container_removed": True}

        def write(path, value):
            nonlocal receipt_writes
            if path.name == "receipt.json":
                receipt_writes += 1
            events.append({"event": "write", "file": path.name, "value": copy.deepcopy(value)})
            if (
                (denied_write == "provisional" and path.name == "receipt.json" and receipt_writes == 1)
                or (denied_write == "final-receipt" and path.name == "receipt.json" and receipt_writes == 2)
                or (denied_write == "cleanup" and path.name == "cleanup.json")
            ):
                raise PermissionError("reviewer write denied: " + str(denied_write))
            actual_write(path, value)

        args = argparse.Namespace(
            output=output, revision="a" * 40, image="synthetic", budget_seconds=60,
            registry_cache=REVIEW / "unused-cache", registry_index=REVIEW / "unused-index",
        )
        stdout = io.StringIO()
        with (
            mock.patch.object(backend, "Commands", Commands),
            mock.patch.object(backend, "SPECS", ((
                "keelshell-session", "keelshell_session", "zmodem::", (Commands.required,),
            ),)),
            mock.patch.object(backend, "registry_bundle", return_value={}),
            mock.patch.object(backend, "identity", return_value={}),
            mock.patch.object(backend, "remove_owned_container", remove),
            mock.patch.object(backend, "write_json", write),
            mock.patch("sys.stdout", stdout),
        ):
            result = backend.execute(args)
        self.assertEqual(handlers, {sig: signal.getsignal(sig) for sig in handlers})
        observed = {
            "exit": result, "stdout_summary": json.loads(stdout.getvalue()),
            "events": events, "handlers_restored": True,
        }
        (output / "review-observation.json").write_text(json.dumps(observed, indent=2) + "\n")
        durable = json.loads((output / "receipt.json").read_text())
        return result, durable, observed, output

    def test_primary_error_is_durable_before_cleanup_and_finish_error_is_preserved(self):
        result, receipt, observed, output = self.run_case(
            "primary-and-finish-error", primary=True, finish_error=True,
        )
        provisional = next(e for e in observed["events"] if e["event"] == "remove")["provisional_receipt"]
        self.assertEqual(provisional["status"], "FINALIZING")
        self.assertEqual(provisional["execution_status"], "FAIL")
        self.assertIn("reviewer primary failure", provisional["error"])
        self.assertEqual((result, receipt["status"]), (1, "FAIL"))
        self.assertIn("reviewer primary failure", receipt["error"])
        self.assertFalse(receipt["cleanup"]["owned_processes_stopped"])
        self.assertIn("PermissionError", receipt["cleanup"]["process_cleanup_error"])
        self.assertEqual(json.loads((output / "cleanup.json").read_text()), receipt["cleanup"])

    def test_optional_selected_ignored_is_rejected_even_when_the_required_test_passes(self):
        result, receipt, _, _ = self.run_case("optional-selected-ignored", optional_ignored=True)
        self.assertEqual((result, receipt["status"]), (1, "FAIL"))
        self.assertEqual(receipt["totals"], {"passed": 1, "failed": 0, "ignored": 1})
        self.assertIn("did not pass every selected test", receipt["error"])

    def test_provisional_write_failure_after_success_does_not_skip_cleanup_or_pass(self):
        result, receipt, observed, output = self.run_case("provisional-denied", denied_write="provisional")
        self.assertEqual((result, receipt["status"]), (1, "FAIL"))
        self.assertEqual(receipt["execution_status"], "PASS")
        self.assertTrue(receipt["cleanup"]["owned_processes_stopped"])
        self.assertTrue(receipt["cleanup"]["container_removed"])
        self.assertEqual(sum(e["event"] == "remove" for e in observed["events"]), 1)
        self.assertEqual(sum(e["event"] == "finish" for e in observed["events"]), 1)
        self.assertIn("PermissionError", receipt["persistence_errors"][0])
        self.assertTrue((output / "cleanup.json").exists())

    def test_final_cleanup_receipt_write_failure_still_saves_fail_and_restores_handlers(self):
        result, receipt, _, output = self.run_case("cleanup-receipt-denied", denied_write="cleanup")
        self.assertEqual((result, receipt["status"]), (1, "FAIL"))
        self.assertIn("cleanup.json: PermissionError", receipt["persistence_errors"][0])
        self.assertTrue(receipt["cleanup"]["owned_processes_stopped"])
        self.assertFalse((output / "cleanup.json").exists())

    def test_final_receipt_write_failure_leaves_finalizing_instead_of_false_pass(self):
        result, receipt, observed, output = self.run_case(
            "final-receipt-denied", denied_write="final-receipt",
        )
        self.assertEqual(result, 1)
        self.assertEqual(receipt["status"], "FINALIZING")
        self.assertEqual(observed["stdout_summary"]["status"], "FAIL")
        self.assertTrue(json.loads((output / "cleanup.json").read_text())["owned_processes_stopped"])

    def test_wrong_elf_machine_is_rejected_in_execute_before_selected_execution(self):
        result, receipt, _, _ = self.run_case("wrong-elf-machine", wrong_machine=True)
        self.assertEqual((result, receipt["status"]), (1, "FAIL"))
        self.assertIn("ELF architecture contradicts", receipt["error"])
        self.assertFalse(any(s["name"].startswith("test-keelshell-") for s in receipt["steps"]))
        self.assertEqual(receipt["totals"], {"passed": 0, "failed": 0, "ignored": 0})

    def test_retained_native_headers_satisfy_new_validator_without_relabeling_old_run(self):
        delivery = ROOT / "work/windows-local-backend-20261115/linux-local"
        old_run = json.loads((delivery / "native-r3/receipt.json").read_text())
        self.assertNotEqual(
            old_run["host_runner_inputs"]["scripts/run_linux_local_backend.py"],
            "b134be0b11b7b79b5c88baddf363a25bad3b5f4583265e1ca26ff8a35a4a55b0",
        )
        binary_files = sorted((delivery / "native-r3").glob("*-binary.json"))
        self.assertEqual(len(binary_files), 6)
        for path in binary_files:
            item = json.loads(path.read_text())
            with self.subTest(path=path.name):
                verified = backend.validate_binary(
                    item["sha256sum"], item["elf_header"], item["executable"], old_run["toolchain"],
                )
                self.assertEqual(verified["machine"], "AArch64")

    def test_successful_execution_with_failed_finish_cannot_pass(self):
        result, receipt, observed, _ = self.run_case("success-then-finish-error", finish_error=True)
        self.assertEqual((result, receipt["status"]), (1, "FAIL"))
        self.assertEqual(receipt["execution_status"], "PASS")
        provisional = next(e for e in observed["events"] if e["event"] == "remove")["provisional_receipt"]
        self.assertEqual(provisional["status"], "FINALIZING")
        self.assertFalse(receipt["cleanup"]["owned_processes_stopped"])
        self.assertIn("PermissionError", receipt["cleanup"]["process_cleanup_error"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
