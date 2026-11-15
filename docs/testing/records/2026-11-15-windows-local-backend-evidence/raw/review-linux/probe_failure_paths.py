#!/usr/bin/env python3
"""Reviewer regression probes using synthetic commands, never real containers."""

import argparse
import gzip
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

REVIEW = Path(__file__).resolve().parent
ROOT = REVIEW.parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import run_linux_local_backend as backend

SOURCE = ROOT / (
    "work/windows-local-backend-20261115/linux-local/"
    "source-a092dd5712c0a92ee4af18970fb0500fb230b760.tar.gz"
)
REVISION = "a092dd5712c0a92ee4af18970fb0500fb230b760"


class SyntheticCommands:
    """Exercise execute's decisions without executing any external tool."""

    def __init__(self, output, receipt, deadline, interrupted):
        self.output = output
        self.receipt = receipt
        self.deadline = deadline
        self.cleaning = False
        self.inventory = {}
        for package, target, prefix, required in backend.SPECS:
            self.inventory.setdefault(package + "/" + target, []).extend(required)

    def run(self, name, argv, **kwargs):
        text = b""
        rc = 0
        if name == "resolve-revision":
            text = (REVISION + "\n").encode()
        elif name == "source-git-archive":
            (self.output / "source.tar").write_bytes(gzip.decompress(SOURCE.read_bytes()))
        elif name == "podman-vm-architecture":
            text = b"arm64\n"
        elif name == "create-container":
            text = ("a" * 64 + "\n").encode()
        elif name == "linux-toolchain":
            text = b"Linux aarch64\nrustc 1.98.1 (fixture)\nhost: aarch64-unknown-linux-gnu\n"
        elif name == "container-image-id":
            text = ("b" * 64 + "\n").encode()
        elif name == "container-identity":
            text = json.dumps({
                "Id": "a" * 64,
                "Name": backend.PREFIX + self.receipt["owner"][:12],
                "Labels": {
                    backend.LABEL: backend.LABEL_VALUE,
                    backend.OWNER_LABEL: self.receipt["owner"],
                },
            }).encode()
        elif name == "owned-container-absent":
            rc = 1
        elif name == "test-user":
            text = b"uid=1000 gid=1000\n"
        elif name.startswith("compile-"):
            package = name.removeprefix("compile-")
            values = []
            for key in self.inventory:
                item_package, target = key.split("/")
                if item_package == package:
                    values.append(json.dumps({
                        "reason": "compiler-artifact",
                        "profile": {"test": True},
                        "package_id": (
                            "path+file:///snapshot/crates/" + package + "#" + package + "@0.1.0"
                        ),
                        "target": {"name": target},
                        "executable": "/linux-target/debug/deps/" + target + "-abcd",
                    }))
            text = "\n".join(values).encode()
        elif name.startswith("inventory-"):
            key = name.removeprefix("inventory-").replace(
                "keelshell-session-", "keelshell-session/", 1
            ).replace("keelshell-core-", "keelshell-core/", 1)
            text = "\n".join(test + ": test" for test in self.inventory[key]).encode()
        elif name.startswith("binary-sha-"):
            text = (("0" * 64) + "  " + argv[-1] + "\n").encode()
        elif name.startswith("binary-elf-"):
            text = b"ELF Header:\n  Machine: AArch64\n"
        elif name.startswith("test-"):
            label = name.removeprefix("test-")
            if label.endswith("-archive-local"):
                label = label.removesuffix("-archive-local")
                prefix = "sftp::archive_transfer::local::tests::"
            elif label == "keelshell-session-keelshell_session":
                prefix = "zmodem::"
            else:
                prefix = ""
            key = label.replace(
                "keelshell-session-", "keelshell-session/", 1
            ).replace("keelshell-core-", "keelshell-core/", 1)
            selected = [test for test in self.inventory[key] if test.startswith(prefix)]
            text = "\n".join("test " + test + " ... ignored" for test in selected).encode()
        elif name == "source-after-run":
            manifest = json.loads((self.output / "source-manifest.json").read_text())
            text = "\n".join(
                value + "  " + key for key, value in manifest["files"].items()
            ).encode()
        self.receipt["steps"].append({
            "name": name, "argv": argv, "returncode": rc,
            "timed_out": False, "owned_processes_stopped": True,
        })
        return rc, text

    def finish(self):
        return True


class FailureProbes(unittest.TestCase):
    def test_required_inventory_ignored_must_fail_the_runner(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "all-ignored"
            args = argparse.Namespace(
                output=output, revision=REVISION, image="synthetic",
                budget_seconds=60, registry_cache=Path(directory),
                registry_index=Path(directory),
            )
            with mock.patch.object(backend, "Commands", SyntheticCommands):
                with mock.patch.object(
                    backend, "registry_bundle",
                    return_value={"packages": [], "index_entries": []},
                ):
                    with mock.patch("sys.stdout", new_callable=io.StringIO):
                        result = backend.execute(args)
            receipt = json.loads((output / "receipt.json").read_text())
            (REVIEW / "probe-all-ignored-receipt.json").write_text(
                json.dumps(receipt, indent=2) + "\n"
            )
            print("ALL_IGNORED_OBSERVED", json.dumps({
                "exit": result, "status": receipt["status"], "totals": receipt["totals"],
            }))
            self.assertEqual(result, 1, "Required tests were listed but none ran")

    def test_cleanup_identity_and_signal_error_must_preserve_failure_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "cleanup-error"
            args = argparse.Namespace(
                output=output, revision=REVISION, image="synthetic", budget_seconds=60,
            )
            process = mock.Mock(pid=123456)

            def fail_early(commands, *args, **kwargs):
                commands.processes.append(process)
                commands.identities[process] = {
                    process.pid: (1, process.pid, ("synthetic", 1), "R"),
                }
                raise backend.BackendFailure("Synthetic primary failure")

            raised = None
            with mock.patch.object(backend.Commands, "run", fail_early):
                with mock.patch.object(
                    backend.fixture.ownership, "stop_owned",
                    side_effect=backend.fixture.ownership.InteropFailure(
                        "Synthetic cleanup identity unavailable"
                    ),
                ):
                    with mock.patch.object(
                        backend.fixture.ownership, "signal_owned",
                        side_effect=PermissionError("Synthetic cleanup signal denied"),
                    ):
                        try:
                            with mock.patch("sys.stdout", new_callable=io.StringIO):
                                backend.execute(args)
                        except Exception as error:
                            raised = type(error).__name__ + ": " + str(error)
            observed = {
                "raised": raised,
                "receipt_exists": (output / "receipt.json").exists(),
                "cleanup_receipt_exists": (output / "cleanup.json").exists(),
            }
            (REVIEW / "probe-cleanup-error.json").write_text(
                json.dumps(observed, indent=2) + "\n"
            )
            print("CLEANUP_ERROR_OBSERVED", json.dumps(observed))
            self.assertIsNone(raised, "Expected cleanup errors must not bypass receipt persistence")
            self.assertTrue(observed["receipt_exists"])
            self.assertTrue(observed["cleanup_receipt_exists"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
