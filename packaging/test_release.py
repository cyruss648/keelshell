#!/usr/bin/env python3
"""Isolated stdlib release tests; synthetic headers are not native binaries.

Run: python3 -m unittest discover -s packaging -p 'test_release.py' -v
"""
from __future__ import annotations

import io
import json
from pathlib import Path
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

import release


class ReleaseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="keelshell-release-tests-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.manifest = self.root / "Cargo.toml"
        self.set_version("1.2.3")

    def set_version(self, value: str) -> None:
        self.manifest.write_text('[workspace.package]\nversion = ' + json.dumps(value) + '\n', encoding="utf-8")

    @staticmethod
    def binary(target: str) -> bytes:
        platform, architecture = release.TARGETS[target]
        if platform == "macos":
            cpu = 0x1000007 if architecture == "x86_64" else 0x100000C
            return struct.pack("<IIIIIIIIII", 0xFEEDFACF, cpu, 0, 2, 1, 8, 0, 0, 0x1B, 8)
        if platform == "linux":
            data = bytearray(120)
            data[:7] = b"\x7fELF\x02\x01\x01"
            struct.pack_into("<HHI", data, 16, 3, 62 if architecture == "x86_64" else 183, 1)
            struct.pack_into("<QQ", data, 24, 0x1000, 64)
            struct.pack_into("<HHH", data, 52, 64, 56, 1)
            return bytes(data)
        data = bytearray(240)
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 0x3C, 64)
        data[64:68] = b"PE\0\0"
        struct.pack_into("<HH", data, 68, 0x8664 if architecture == "x86_64" else 0xAA64, 1)
        struct.pack_into("<HHH", data, 84, 112, 2, 0x20B)
        return bytes(data)

    def stage(self, target: str) -> Path:
        platform, _ = release.TARGETS[target]
        stage = self.root / (target + "-stage")
        stage.mkdir()
        binary = stage / release.BINARIES[platform]
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_bytes(self.binary(target))
        binary.chmod(0o755)
        resource = stage / "resources" / "说明.txt"
        resource.parent.mkdir()
        resource.write_text("可审核的发布测试\n", encoding="utf-8")
        resource.chmod(0o644)
        receipt = {
            "schema_version": 1,
            "platform": platform,
            "version": release.version(self.manifest),
            "binary_sha256": release.file_digest(binary),
            "icon_source_sha256": "0" * 64,
            "files": {path.relative_to(stage).as_posix(): release.file_digest(path)
                      for path in (binary, resource)},
            "installed": False,
            "signed_by_packaging_script": False,
            "native_acceptance": "not performed by this script",
        }
        (stage / release.MANIFEST).write_text(json.dumps(receipt, ensure_ascii=False), encoding="utf-8")
        return stage

    def archive(self, target: str, output: Path | None = None) -> Path:
        platform = release.TARGETS[target][0]
        stage = self.stage(target)
        output = output or self.root / "artifacts"
        if sys.platform != "win32" or platform == "windows":
            return release.archive_stage(platform, target, stage, output, self.manifest)
        # Windows chmod cannot represent Unix execute bits. Construct actual
        # archive fixtures with explicit Unix metadata to exercise collection,
        # member/path/digest validation on Windows as well. Unix staging and
        # permission preservation are tested through archive_stage on POSIX.
        output.mkdir(parents=True, exist_ok=True)
        archive_path = output / release.package_name(release.version(self.manifest), target)
        files, _ = release.stage_files(stage)
        if platform == "linux":
            with tarfile.open(archive_path, "w:gz") as archive:
                for name, path in sorted(files.items()):
                    data = path.read_bytes()
                    item = tarfile.TarInfo(name)
                    item.mode = 0o755 if name == release.BINARIES[platform] else 0o644
                    item.size = len(data)
                    archive.addfile(item, io.BytesIO(data))
        else:
            with zipfile.ZipFile(archive_path, "w") as archive:
                for name, path in sorted(files.items()):
                    item = zipfile.ZipInfo(name)
                    item.create_system = 3
                    item.external_attr = (stat.S_IFREG | (0o755 if name == release.BINARIES[platform] else 0o644)) << 16
                    archive.writestr(item, path.read_bytes())
        archive_path.with_name(archive_path.name + ".sha256").write_text(
            f"{release.file_digest(archive_path)}  {archive_path.name}\n", encoding="ascii", newline="\n")
        return archive_path

    def edit_receipt(self, stage: Path, edit) -> None:
        path = stage / release.MANIFEST
        data = json.loads(path.read_text(encoding="utf-8"))
        edit(data)
        path.write_text(json.dumps(data), encoding="utf-8")

    def test_valid_release_and_prerelease_tags(self) -> None:
        for value in ("0.1.0", "1.2.3", "1.2.3-rc.1", "1.2.3-beta", "1.2.3-0.test-A"):
            with self.subTest(value=value):
                self.set_version(value)
                self.assertEqual(release.validate_ref("v" + value, self.manifest), value)

    def test_reject_invalid_versions_tags_and_shell_characters(self) -> None:
        for value in ("01.2.3", "1.02.3", "1.2", "1.2.3+build", "1.2.3-01", "1.2.3-rc..1",
                      "1.2.3;touch", "1.2.3$(id)", "1.2.3\n", "1.2.3-rc/1"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.set_version(value)
                release.version(self.manifest)
        self.set_version("1.2.3")
        for tag in ("1.2.3", "v1.2.4", "refs/tags/v1.2.3", "v1.2.3-rc.1", "v1.2.3+meta", "v1.2.3;echo bad"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_ref(tag, self.manifest)

    def test_all_six_archives_validate_and_preserve_executable_mode(self) -> None:
        for target, (platform, _) in release.TARGETS.items():
            with self.subTest(target=target):
                archive = self.archive(target)
                self.assertEqual(archive.name, release.package_name("1.2.3", target))
                release.validate_archive(archive, target, "1.2.3")
                with release.archive_members(archive) as (members, _, _):
                    expected_mode = 0o755
                    if sys.platform == "win32" and platform == "windows":
                        expected_mode = (self.root / (target + "-stage") / release.BINARIES[platform]).stat().st_mode & 0o777
                    self.assertEqual(members[release.BINARIES[platform]][1] & 0o777, expected_mode)
                    self.assertIn("resources/说明.txt", members)
                self.assertEqual(archive.with_name(archive.name + ".sha256").read_text(),
                                 f"{release.file_digest(archive)}  {archive.name}\n")

    def test_missing_unknown_modified_and_unknown_directory_are_rejected(self) -> None:
        target = "aarch64-apple-darwin"
        for scenario in ("missing", "extra", "tampered", "empty-directory"):
            with self.subTest(scenario=scenario):
                case = self.root / scenario
                case.mkdir()
                # A fresh staging tree for each independently corrupted case.
                original = self.root
                self.root = case
                stage = self.stage(target)
                self.root = original
                resource = stage / "resources" / "说明.txt"
                if scenario == "missing":
                    resource.unlink()
                elif scenario == "extra":
                    (stage / "unexpected").write_bytes(b"extra")
                elif scenario == "tampered":
                    resource.write_bytes(b"changed")
                else:
                    (stage / "unrecorded-directory").mkdir()
                with self.assertRaises(ValueError):
                    release.validate_stage(stage, "macos", target, "1.2.3")

    def test_manifest_rejects_traversal_duplicate_keys_and_false_acceptance(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        stage = self.stage(target)
        receipt_path = stage / release.MANIFEST
        original = receipt_path.read_bytes()
        for name in ("../escape", "/absolute", "a/../b", "a//b", "a\\b", "C:escape", "./name", "a\nname", "a/.. /escape", "CON", "nul.txt", "LPT1.log", "name.", "name "):
            with self.subTest(name=name):
                receipt_path.write_bytes(original)
                self.edit_receipt(stage, lambda data: data["files"].update({name: "0" * 64}))
                with self.assertRaises(ValueError):
                    release.validate_stage(stage, "linux", target, "1.2.3")
        receipt_path.write_bytes(original.replace(b'"schema_version": 1', b'"schema_version": 1, "schema_version": 1'))
        with self.assertRaises(ValueError):
            release.validate_stage(stage, "linux", target, "1.2.3")
        receipt_path.write_bytes(original)
        self.edit_receipt(stage, lambda data: data.update(native_acceptance="passed"))
        with self.assertRaises(ValueError):
            release.validate_stage(stage, "linux", target, "1.2.3")

    @unittest.skipUnless(hasattr(Path, "symlink_to"), "Symbolic links unavailable")
    def test_staged_symlinks_and_linked_stage_roots_are_rejected(self) -> None:
        target = "x86_64-apple-darwin"
        stage = self.stage(target)
        try:
            (stage / "link").symlink_to(stage / "resources", target_is_directory=True)
        except OSError:
            self.skipTest("This host does not permit creating symbolic links")
        with self.assertRaises(ValueError):
            release.validate_stage(stage, "macos", target, "1.2.3")
        (stage / "link").unlink()
        linked = self.root / "linked-stage"
        linked.symlink_to(stage, target_is_directory=True)
        with self.assertRaises(ValueError):
            release.validate_stage(linked, "macos", target, "1.2.3")

    def test_wrong_architecture_platform_truncated_or_library_binary_is_rejected(self) -> None:
        for target, (platform, architecture) in release.TARGETS.items():
            wrong = target.replace(architecture, "aarch64" if architecture == "x86_64" else "x86_64")
            with self.subTest(target=target):
                with self.assertRaises(ValueError):
                    release.check_binary(io.BytesIO(self.binary(wrong)), target)
                with self.assertRaises(ValueError):
                    release.check_binary(io.BytesIO(self.binary(target)[:12]), target)
                with self.assertRaises(ValueError):
                    release.target_info(target, "windows" if platform != "windows" else "linux")
        dll = bytearray(self.binary("x86_64-pc-windows-msvc"))
        struct.pack_into("<H", dll, 86, 0x2002)
        with self.assertRaises(ValueError):
            release.check_binary(io.BytesIO(dll), "x86_64-pc-windows-msvc")
        dylib = bytearray(self.binary("x86_64-apple-darwin"))
        struct.pack_into("<I", dylib, 12, 6)
        with self.assertRaises(ValueError):
            release.check_binary(io.BytesIO(dylib), "x86_64-apple-darwin")

    @unittest.skipIf(sys.platform == "win32", "Windows permission model does not expose Unix execute bits")
    def test_unix_executable_permission_cannot_be_lost(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        stage = self.stage(target)
        (stage / release.BINARIES["linux"]).chmod(0o644)
        with self.assertRaises(ValueError):
            release.validate_stage(stage, "linux", target, "1.2.3")

    def test_archive_rejects_traversal_symlink_and_unrecorded_members_without_extracting(self) -> None:
        target = "x86_64-apple-darwin"
        archive = self.archive(target)
        original = archive.read_bytes()
        for name, mode in (("../outside", stat.S_IFREG | 0o644), ("link", stat.S_IFLNK | 0o777),
                           ("unrecorded", stat.S_IFREG | 0o644)):
            with self.subTest(name=name):
                archive.write_bytes(original)
                with zipfile.ZipFile(archive, "a") as output:
                    info = zipfile.ZipInfo(name)
                    info.create_system = 3
                    info.external_attr = mode << 16
                    output.writestr(info, "outside")
                with self.assertRaises(ValueError):
                    release.validate_archive(archive, target, "1.2.3")
        self.assertFalse((self.root / "outside").exists())

    def test_tar_links_and_unknown_directories_are_rejected(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        archive = self.archive(target)
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.DIRTYPE):
            with self.subTest(kind=kind):
                with tarfile.open(archive, "w:gz") as output:
                    info = tarfile.TarInfo("unrecorded")
                    info.type = kind
                    info.linkname = "../../outside"
                    output.addfile(info)
                with self.assertRaises(ValueError):
                    release.validate_archive(archive, target, "1.2.3")

    def test_collection_requires_complete_matrix_and_generates_checked_flat_sums(self) -> None:
        source = self.root / "ci-artifacts"
        for target in release.TARGETS:
            self.archive(target, source / target)
        result = release.verify_collection(source, list(release.TARGETS), self.manifest)
        self.assertEqual(len(result), 6)
        output = self.root / "release"
        sums = release.collect(source, output, list(release.TARGETS), self.manifest)
        self.assertEqual(len(sums.read_text().splitlines()), 6)
        self.assertEqual(len(list(output.iterdir())), 13)
        release.verify_collection(output, list(release.TARGETS), self.manifest)
        sums.write_text("0" * 64 + "  fabricated.zip\n")
        with self.assertRaises(ValueError):
            release.verify_collection(output, list(release.TARGETS), self.manifest)

    def test_incomplete_duplicate_unknown_and_tampered_collection_are_rejected(self) -> None:
        target = "aarch64-pc-windows-msvc"
        archive = self.archive(target)
        source = archive.parent
        with self.assertRaises(ValueError):
            release.verify_collection(source, list(release.TARGETS), self.manifest)
        with self.assertRaises(ValueError):
            release.verify_collection(source, [target, target], self.manifest)
        (source / "unexpected.txt").write_text("unreviewed")
        with self.assertRaises(ValueError):
            release.verify_collection(source, [target], self.manifest)
        (source / "unexpected.txt").unlink()
        archive.write_bytes(archive.read_bytes() + b"tampered")
        with self.assertRaises(ValueError):
            release.verify_collection(source, [target], self.manifest)

    def test_valid_external_checksum_cannot_hide_changed_internal_content(self) -> None:
        target = "aarch64-pc-windows-msvc"
        archive = self.archive(target)
        with zipfile.ZipFile(archive) as original:
            entries = [(member, original.read(member)) for member in original.infolist()]
        with zipfile.ZipFile(archive, "w") as rewritten:
            for member, content in entries:
                rewritten.writestr(member, b"modified" if member.filename == "resources/说明.txt" else content)
        archive.with_name(archive.name + ".sha256").write_text(f"{release.file_digest(archive)}  {archive.name}\n", encoding="ascii", newline="\n")
        with self.assertRaises(ValueError):
            release.verify_collection(archive.parent, [target], self.manifest)

    def test_file_directory_case_collisions_are_rejected(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        stage = self.stage(target)
        path = stage / release.MANIFEST
        original = path.read_bytes()
        for name in ("Resources/other.txt", "PACKAGE-MANIFEST.JSON", "resources"):
            with self.subTest(name=name):
                path.write_bytes(original)
                self.edit_receipt(stage, lambda data: data["files"].update({name: "0" * 64}))
                with self.assertRaises(ValueError):
                    release.validate_stage(stage, "linux", target, "1.2.3")

    def test_malformed_load_command_table_is_rejected(self) -> None:
        target = "aarch64-apple-darwin"
        for command_count, command_bytes, command_size in ((2, 16, 8), (1, 8, 0), (1, 16, 8), (1, 16, 16)):
            with self.subTest(count=command_count, size=command_size):
                data = bytearray(self.binary(target))
                struct.pack_into("<II", data, 16, command_count, command_bytes)
                struct.pack_into("<I", data, 36, command_size)
                with self.assertRaises(ValueError):
                    release.check_binary(io.BytesIO(data), target)

    def test_optional_target_provenance_is_bound_to_requested_target(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        stage = self.stage(target)
        self.edit_receipt(stage, lambda data: data.update(target=target, build={"commit":"a" * 40, "workflow_run":"123", "rustc":"rustc fixture"}))
        if sys.platform != "win32":
            archive = release.archive_stage("linux", target, stage, self.root / "release", self.manifest)
            release.validate_archive(archive, target, "1.2.3")
        else:
            # Provenance parsing does not depend on the host permission model.
            release.receipt((stage / release.MANIFEST).read_bytes(), "linux", "1.2.3", target)
        self.edit_receipt(stage, lambda data: data.update(target="aarch64-unknown-linux-gnu"))
        with self.assertRaises(ValueError):
            release.validate_stage(stage, "linux", target, "1.2.3")

    def test_existing_output_is_preserved_and_nested_output_is_rejected(self) -> None:
        target = "x86_64-pc-windows-msvc"
        stage = self.stage(target)
        output = self.root / "artifacts"
        first = release.archive_stage("windows", target, stage, output, self.manifest)
        original = first.read_bytes()
        with self.assertRaises(ValueError):
            release.archive_stage("windows", target, stage, output, self.manifest)
        self.assertEqual(first.read_bytes(), original)
        with self.assertRaises(ValueError):
            release.archive_stage("windows", target, stage, stage / "nested", self.manifest)
        with self.assertRaises(ValueError):
            release.collect(output, output / "nested", [target], self.manifest)

    def test_cli_success_and_failure_exit_codes(self) -> None:
        command = [sys.executable, str(Path(release.__file__)), "validate-ref", "--manifest", str(self.manifest), "--tag"]
        valid = subprocess.run(command + ["v1.2.3"], capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(valid.returncode, 0, valid.stderr)
        self.assertEqual(valid.stdout.strip(), "1.2.3")
        invalid = subprocess.run(command + ["v1.2.3;echo bad"], capture_output=True, text=True, timeout=10, check=False)
        self.assertNotEqual(invalid.returncode, 0)
        self.assertIn("Release validation failed", invalid.stderr)
        missing = subprocess.run([sys.executable, str(Path(release.__file__)), "collect"], capture_output=True, text=True, timeout=10, check=False)
        self.assertNotEqual(missing.returncode, 0)


if __name__ == "__main__":
    unittest.main()
