"""Companion staging and native-inspection contracts; headers are never executed."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import inspect_native
import package
import release
import test_release


class CompanionPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="keelshell-package-tests-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def stage(self, target: str) -> Path:
        platform = release.TARGETS[target][0]
        binary = self.root / (target + "-app")
        companion = self.root / (target + "-mcp")
        binary.write_bytes(test_release.ReleaseTests.binary(target))
        companion.write_bytes(test_release.ReleaseTests.binary(target) + b"MCP fixture")
        stage = self.root / (target + "-stage")
        result = subprocess.run(
            [sys.executable, package.__file__, platform, "--binary", str(binary),
             "--mcp-binary", str(companion), "--output", str(stage)],
            capture_output=True, text=True, timeout=30, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return stage

    def test_all_six_packages_include_companion_beside_app_with_bound_receipt(self) -> None:
        for target, (platform, _) in release.TARGETS.items():
            with self.subTest(target=target):
                stage = self.stage(target)
                receipt = json.loads((stage / release.MANIFEST).read_text())
                app = stage / release.BINARIES[platform]
                companion = stage / release.MCP_BINARIES[platform]
                self.assertEqual(app.parent, companion.parent)
                self.assertEqual(receipt["mcp_binary_sha256"], release.file_digest(companion))
                # Windows filesystem chmod cannot prove POSIX executable bits.
                if sys.platform != "win32" or platform == "windows":
                    release.validate_stage(stage, platform, target, release.version())
                else:
                    release.receipt((stage / release.MANIFEST).read_bytes(), platform, release.version(), target)

    def test_missing_companion_argument_or_input_fails_before_staging(self) -> None:
        binary = self.root / "app"
        binary.write_bytes(test_release.ReleaseTests.binary("aarch64-apple-darwin"))
        for arguments in ([], ["--mcp-binary", str(self.root / "missing")]):
            stage = self.root / "output"
            with self.subTest(arguments=arguments):
                result = subprocess.run(
                    [sys.executable, package.__file__, "macos", "--binary", str(binary),
                     "--output", str(stage), *arguments],
                    capture_output=True, text=True, timeout=30, check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(stage.exists())

    def test_wrong_platform_companion_fails_without_output(self) -> None:
        binary = self.root / "app"
        companion = self.root / "mcp"
        binary.write_bytes(test_release.ReleaseTests.binary("aarch64-apple-darwin"))
        companion.write_bytes(test_release.ReleaseTests.binary("x86_64-unknown-linux-gnu"))
        stage = self.root / "output"
        result = subprocess.run(
            [sys.executable, package.__file__, "macos", "--binary", str(binary),
             "--mcp-binary", str(companion), "--output", str(stage)],
            capture_output=True, text=True, timeout=30, check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Mach-O", result.stderr)
        self.assertFalse(stage.exists())

    @unittest.skipIf(sys.platform == "win32", "Synthetic Unix stage requires Unix executable metadata")
    def test_native_inspection_rejects_companion_above_macos_minimum(self) -> None:
        target = "aarch64-apple-darwin"
        stage = self.stage(target)
        def output(*command: str) -> str:
            return "minos 15.1\n" if command[-1].endswith("keelshell-mcp") else "minos 15.0\n"
        with patch.object(sys, "argv", ["inspect_native.py", "--platform", "macos", "--target", target,
                                        "--stage", str(stage)]), patch.object(sys, "platform", "darwin"), \
                patch.object(inspect_native, "run", side_effect=output):
            with self.assertRaisesRegex(ValueError, "deployment target"):
                inspect_native.main()

    @unittest.skipIf(sys.platform == "win32", "Synthetic Unix stage requires Unix executable metadata")
    def test_native_inspection_rejects_missing_linux_companion_dependency(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        stage = self.stage(target)
        def output(*command: str) -> str:
            return "libfixture.so => not found\n" if command[0] == "ldd" and command[-1].endswith("keelshell-mcp") else ""
        with patch.object(sys, "argv", ["inspect_native.py", "--platform", "linux", "--target", target,
                                        "--stage", str(stage)]), patch.object(sys, "platform", "linux"), \
                patch.object(inspect_native, "run", side_effect=output):
            with self.assertRaisesRegex(ValueError, "runtime libraries"):
                inspect_native.main()

    def test_windows_inspection_reads_both_dependencies_and_only_gui_resources(self) -> None:
        target = "x86_64-pc-windows-msvc"
        stage = self.stage(target)
        with patch.object(sys, "argv", ["inspect_native.py", "--platform", "windows", "--target", target,
                                        "--stage", str(stage)]), patch.object(sys, "platform", "win32"), \
                patch.object(inspect_native, "windows_resources") as resources, \
                patch.object(inspect_native, "windows_dependencies") as dependencies:
            inspect_native.main()
        resources.assert_called_once_with(stage / release.BINARIES["windows"])
        self.assertEqual([call.args[0] for call in dependencies.call_args_list],
                         [stage / release.BINARIES["windows"], stage / release.MCP_BINARIES["windows"]])


if __name__ == "__main__":
    unittest.main()
