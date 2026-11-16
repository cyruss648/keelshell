#!/usr/bin/env python3
"""Bounded runner regression tests; no Cargo, image pulls or real containers."""

import argparse
import copy
import io
import json
from pathlib import Path
import platform
import subprocess
import sys
import signal
import tarfile
import tempfile
import time
import unittest
from unittest import mock

sys.dont_write_bytecode = True
import run_linux_local_backend as backend


requires_posix_runner = unittest.skipUnless(
    platform.system() in ("Darwin", "Linux"),
    "Lab runners require macOS/Linux kernel birth identities and POSIX signals",
)


class PureContracts(unittest.TestCase):
    def test_revision_accepts_only_an_exact_commit(self):
        value = "a092dd5712c0a92ee4af18970fb0500fb230b760"
        self.assertEqual(backend.revision(value), value)
        for invalid in ("HEAD", "main", value[:12], value.upper(), "--all", value + "\n"):
            with self.subTest(value=invalid), self.assertRaises(argparse.ArgumentTypeError):
                backend.revision(invalid)

    def test_sparse_index_paths_cover_all_name_lengths(self):
        for name, expected in (
            ("a", "1/a"), ("AB", "2/ab"), ("Syn", "3/s/syn"),
            ("serde_json", "se/rd/serde_json"), ("windows-sys", "wi/nd/windows-sys"),
        ):
            with self.subTest(name=name):
                self.assertEqual(backend.index_key(name), expected)

    def test_sparse_index_rejects_path_and_option_injection(self):
        for name in ("", "../crate", "a/b", "/root", ".cache", "-option", "a\nb", "a\\b"):
            with self.subTest(name=name), self.assertRaises(backend.BackendFailure):
                backend.index_key(name)

    def test_container_identity_requires_every_owner_field(self):
        owner = "b" * 32
        container = "a" * 64
        valid = {
            "Id": container, "Name": "/" + backend.PREFIX + owner[:12],
            "Labels": {backend.LABEL: backend.LABEL_VALUE, backend.OWNER_LABEL: owner},
        }
        self.assertTrue(backend.container_matches(valid, container, owner))
        candidates = [None, {}, {**valid, "Id": "c" * 64}, {**valid, "Name": None},
                      {**valid, "Name": backend.PREFIX + "other"}, {**valid, "Labels": None}]
        for label in (backend.LABEL, backend.OWNER_LABEL):
            value = copy.deepcopy(valid)
            value["Labels"][label] = "wrong"
            candidates.append(value)
        for info in candidates:
            with self.subTest(info=info):
                self.assertFalse(backend.container_matches(info, container, owner))
        self.assertFalse(backend.container_matches(valid, "a" * 12, owner))
        self.assertFalse(backend.container_matches(valid, container, "b" * 12))

    def test_toolchain_proves_matching_linux_kernel_compiler_and_vm(self):
        tools = "Linux aarch64\nrustc 1.98.1 (hash date)\nhost: aarch64-unknown-linux-gnu\n"
        value = backend.validate_tools(tools, "arm64\n")
        self.assertEqual(value["host"], "aarch64-unknown-linux-gnu")
        self.assertEqual(value["rustc"], "1.98.1")
        with self.assertRaises(backend.BackendFailure):
            backend.validate_tools(tools, "amd64")
        with self.assertRaises(backend.BackendFailure):
            backend.validate_tools(tools.replace("unknown-linux-gnu", "apple-darwin"), "arm64")
        with self.assertRaises(backend.BackendFailure):
            backend.validate_tools(tools.replace("Linux aarch64", "Linux x86_64"), "arm64")

    def test_linux_command_uses_only_container_owned_target_and_offline_cargo_home(self):
        command = backend.linux_exec("a" * 64, ["cargo", "test", "--locked", "--offline"], 40, user=True)
        for flag in ("--user=1000:1000", "--workdir=/snapshot",
                     "--env=CARGO_HOME=/linux-cargo", "--env=CARGO_TARGET_DIR=/linux-target",
                     "--env=CARGO_NET_OFFLINE=true"):
            self.assertIn(flag, command)
        self.assertNotIn("--ignore-rust-version", command)
        self.assertEqual(command[-4:], ["cargo", "test", "--locked", "--offline"])
        self.assertEqual(command[:2], ["podman", "exec"])


class ArchiveContracts(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def make_source(self, entries):
        path = self.root / "source.tar"
        with tarfile.open(path, "w") as archive:
            for name, data, kind in entries:
                entry = tarfile.TarInfo(name)
                if kind == "file":
                    entry.size = len(data)
                    archive.addfile(entry, io.BytesIO(data))
                else:
                    entry.type = tarfile.SYMTYPE
                    entry.linkname = "/outside"
                    archive.addfile(entry)
        return path

    def test_snapshot_hashes_exact_source_bytes(self):
        entries = [("Cargo.toml", b"[workspace]\n", "file"), ("Cargo.lock", b"# locked\n", "file"),
                   ("crates/example/src/lib.rs", b"pub fn example() {}\n", "file")]
        path = self.make_source(entries)
        result = backend.snapshot_manifest(path)
        self.assertEqual(result["file_count"], 3)
        self.assertEqual(result["files"]["Cargo.lock"], backend.sha256(b"# locked\n"))
        self.assertEqual(result, backend.snapshot_manifest(path))
        entries[-1] = ("crates/example/src/lib.rs", b"changed\n", "file")
        self.assertNotEqual(result["manifest_sha256"],
                            backend.snapshot_manifest(self.make_source(entries))["manifest_sha256"])

    def test_snapshot_rejects_escape_alias_control_link_and_duplicate_members(self):
        base = [("Cargo.toml", b"", "file"), ("Cargo.lock", b"", "file")]
        for member in (
            ("../escape", b"", "file"), ("/absolute", b"", "file"),
            ("crates/../alias", b"", "file"), ("crates/./alias", b"", "file"),
            ("crates/double//path", b"", "file"), ("crates/new\nline", b"", "file"),
            ("crates/link", b"", "symlink"), ("Cargo.lock", b"", "file"),
        ):
            with self.subTest(member=member), self.assertRaises(backend.BackendFailure):
                backend.snapshot_manifest(self.make_source(base + [member]))

    def test_snapshot_requires_workspace_and_lock(self):
        with self.assertRaises(backend.BackendFailure):
            backend.snapshot_manifest(self.make_source([("Cargo.toml", b"", "file")]))

    def prepare_registry(self, packages=None):
        cache, index = self.root / "cache", self.root / "index"
        cache.mkdir()
        index.mkdir()
        payload = b"public-registry-crate-archive"
        package = {"name": "example", "version": "1.2.3", "source": backend.OFFICIAL_SOURCE,
                   "checksum": backend.sha256(payload)}
        (cache / "example-1.2.3.crate").write_bytes(payload)
        metadata = index / ".cache/ex/am/example"
        metadata.parent.mkdir(parents=True)
        metadata.write_bytes(b"public sparse index metadata")
        return cache, index, {"package": packages or [package]}, package

    def test_registry_bundle_verifies_lock_hash_and_excludes_credentials_and_configs(self):
        cache, index, lock, package = self.prepare_registry()
        marker = b"private-fixture-marker-never-copy"
        for root in (cache, index):
            (root / "credentials.toml").write_bytes(marker)
            (root / "config.toml").write_bytes(marker)
        result = backend.registry_bundle(lock, cache, index, self.root / "registry.tar")
        self.assertEqual(result["missing_archives"], [])
        self.assertEqual(result["packages"][0]["sha256"], package["checksum"])
        self.assertNotIn(marker, (self.root / "registry.tar").read_bytes())
        with tarfile.open(self.root / "registry.tar") as archive:
            names = archive.getnames()
            self.assertEqual(len(names), 3)
            self.assertEqual(archive.extractfile(
                "registry/cache/" + backend.INDEX_DIRECTORY + "/example-1.2.3.crate"
            ).read(), b"public-registry-crate-archive")
            config = json.loads(archive.extractfile(
                "registry/index/" + backend.INDEX_DIRECTORY + "/config.json"
            ).read())
            self.assertEqual(config["dl"], "https://static.crates.io/crates")

    def test_registry_rejects_a_checksum_mismatch_before_cargo(self):
        cache, index, lock, _ = self.prepare_registry()
        (cache / "example-1.2.3.crate").write_bytes(b"changed archive")
        with self.assertRaisesRegex(backend.BackendFailure, "checksum"):
            backend.registry_bundle(lock, cache, index, self.root / "registry.tar")

    def test_registry_records_missing_archive_and_index_without_fetching(self):
        cache, index, lock, _ = self.prepare_registry()
        (cache / "example-1.2.3.crate").unlink()
        (index / ".cache/ex/am/example").unlink()
        result = backend.registry_bundle(lock, cache, index, self.root / "registry.tar")
        self.assertEqual(result["missing_archives"], ["example-1.2.3.crate"])
        self.assertEqual(result["missing_index"], ["ex/am/example"])
        self.assertFalse(result["packages"][0]["cached"])

    def test_registry_rejects_nonofficial_source_or_missing_checksum(self):
        cache, index, lock, package = self.prepare_registry()
        for change in ({"source": "registry+https://private.invalid/index"},
                       {"checksum": ""}, {"version": "../other"},
                       {"name": "../other"}):
            with self.subTest(change=change), self.assertRaises(backend.BackendFailure):
                backend.registry_bundle({"package": [{**package, **change}]},
                                        cache, index, self.root / "registry.tar")

    def test_registry_symlink_archive_is_not_followed(self):
        cache, index, lock, _ = self.prepare_registry()
        crate = cache / "example-1.2.3.crate"
        payload = crate.read_bytes()
        crate.unlink()
        outside = self.root / "outside"
        outside.write_bytes(payload)
        crate.symlink_to(outside)
        result = backend.registry_bundle(lock, cache, index, self.root / "registry.tar")
        self.assertEqual(result["missing_archives"], ["example-1.2.3.crate"])

    def test_registry_symlink_ancestor_cannot_escape_root(self):
        directory = self.root / "cache"
        directory.mkdir()
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "file").write_bytes(b"not a registry input")
        (directory / "alias").symlink_to(outside)
        with self.assertRaises(backend.BackendFailure):
            backend.cache_file(directory, "alias/file")

    def test_registry_multiple_locked_versions_copy_one_sparse_entry(self):
        cache, index, lock, package = self.prepare_registry()
        second = {**package, "version": "1.2.4"}
        (cache / "example-1.2.4.crate").write_bytes(b"public-registry-crate-archive")
        result = backend.registry_bundle({"package": [package, second]}, cache, index,
                                        self.root / "registry.tar")
        self.assertEqual(len(result["packages"]), 2)
        self.assertEqual(len(result["index_entries"]), 1)


class InventoryContracts(unittest.TestCase):
    def artifact(self, identity="path+file:///snapshot/crates/keelshell-session#keelshell-session@0.1.0",
                 executable="/linux-target/debug/deps/keelshell_session-abcd"):
        return {"reason": "compiler-artifact", "profile": {"test": True},
                "package_id": identity, "executable": executable,
                "target": {"name": "keelshell_session"}}

    def test_cargo_artifact_parser_selects_only_actual_package_tests(self):
        valid = self.artifact()
        dependency = {**valid, "package_id": "registry+https://index.crates.io/#other@1.2.3"}
        no_test = {**valid, "profile": {"test": False}}
        value = "\n".join(["non-json progress", json.dumps(valid), json.dumps(dependency), json.dumps(no_test)])
        self.assertEqual(backend.parse_artifacts(value, "keelshell-session"),
                         {"keelshell_session": valid["executable"]})

    def test_cargo_artifact_supports_path_id_and_later_project_versions(self):
        for identity in ("path+file:///snapshot/crates/keelshell-session#0.1.0",
                         "path+file:///snapshot/crates/keelshell-session#keelshell-session@0.2.0"):
            with self.subTest(identity=identity):
                self.assertEqual(len(backend.parse_artifacts(json.dumps(self.artifact(identity)),
                                                            "keelshell-session")), 1)

    def test_cargo_artifact_refuses_external_target_and_duplicate_executables(self):
        for executable in ("/host/target/test", "/linux-target/debug/deps/../other",
                           "/linux-target/debug/deps/test\nbad"):
            with self.subTest(executable=executable), self.assertRaises(backend.BackendFailure):
                backend.parse_artifacts(json.dumps(self.artifact(executable=executable)), "keelshell-session")
        artifact = json.dumps(self.artifact())
        with self.assertRaises(backend.BackendFailure):
            backend.parse_artifacts(artifact + "\n" + artifact, "keelshell-session")

    def test_named_inventory_preserves_every_name_and_rejects_empty_duplicates(self):
        self.assertEqual(backend.named_inventory("first: test\nnested::second: test\n2 tests, 0 benchmarks\n"),
                         ["first", "nested::second"])
        for invalid in ("", "no tests here\n", "one: test\none: test\n"):
            with self.subTest(invalid=invalid), self.assertRaises(backend.BackendFailure):
                backend.named_inventory(invalid)

    def test_named_results_require_exact_inventory_including_failed_and_ignored(self):
        text = "running 3 tests\ntest a ... ok\ntest b ... FAILED\ntest c ... ignored, fixture required\n"
        result = backend.named_results(text, ["a", "b", "c"])
        self.assertEqual((result["passed"], result["failed"], result["ignored"]), (1, 1, 1))
        for names in (["a", "b"], ["a", "b", "c", "missing"]):
            with self.subTest(names=names), self.assertRaises(backend.BackendFailure):
                backend.named_results(text, names)

    def test_green_summary_alone_cannot_certify_missing_named_tests(self):
        with self.assertRaises(backend.BackendFailure):
            backend.named_results("test result: ok. 5 passed; 0 failed\n", ["required"])
        with self.assertRaises(backend.BackendFailure):
            backend.named_results("test a ... ok\ntest a ... ok\n", ["a"])


class CleanupContracts(unittest.TestCase):
    def setUp(self):
        self.owner = "b" * 32
        self.container = "a" * 64
        self.name = backend.PREFIX + self.owner[:12]
        self.info = {"Id": self.container, "Name": self.name,
                     "Labels": {backend.LABEL: backend.LABEL_VALUE, backend.OWNER_LABEL: self.owner}}
        self.calls = []

    def fake(self, exists=0, absent=1, info=None):
        def run(name, argv, **kwargs):
            self.calls.append((name, argv, kwargs))
            if name == "container-identity":
                return 0, json.dumps(self.info if info is None else info).encode()
            if name == "ambiguous-create-exists":
                return exists, b""
            if name == "owned-container-absent":
                return absent, b""
            return 0, b""
        return mock.Mock(run=run)

    def test_cleanup_uses_verified_full_id_and_reads_absence(self):
        result = backend.remove_owned_container(self.fake(), self.container, self.name, self.owner)
        self.assertTrue(result["container_removed"])
        self.assertEqual([call[0] for call in self.calls],
                         ["container-identity", "remove-owned-container", "owned-container-absent"])
        self.assertEqual(self.calls[1][1], ["podman", "rm", "-f", self.container])

    def test_cleanup_recovers_created_container_after_client_timeout(self):
        result = backend.remove_owned_container(self.fake(), None, self.name, self.owner)
        self.assertTrue(result["container_removed"])
        self.assertEqual(result["container_id"], self.container)
        self.assertEqual([call[0] for call in self.calls[:3]],
                         ["ambiguous-create-exists", "container-identity", "container-identity"])

    def test_cleanup_rejects_foreign_owner_and_does_not_remove(self):
        foreign = {**self.info, "Labels": {**self.info["Labels"], backend.OWNER_LABEL: "c" * 32}}
        for container in (self.container, None):
            with self.subTest(container=container), self.assertRaises(backend.BackendFailure):
                backend.remove_owned_container(self.fake(info=foreign), container, self.name, self.owner)
        self.assertNotIn("remove-owned-container", [call[0] for call in self.calls])

    def test_cleanup_reports_never_created_without_inspecting_another_name(self):
        result = backend.remove_owned_container(self.fake(exists=1), None, self.name, self.owner)
        self.assertFalse(result["container_created"])
        self.assertTrue(result["container_removed"])
        self.assertEqual([call[0] for call in self.calls], ["ambiguous-create-exists"])
        self.calls.clear()
        result = backend.remove_owned_container(self.fake(), None, None, self.owner)
        self.assertIsNone(result["container_removed"])
        self.assertFalse(self.calls)

    def test_cleanup_does_not_treat_engine_error_or_unverified_absence_as_success(self):
        with self.assertRaises(backend.BackendFailure):
            backend.remove_owned_container(self.fake(exists=125), None, self.name, self.owner)
        with self.assertRaises(backend.BackendFailure):
            backend.remove_owned_container(self.fake(absent=125), self.container, self.name, self.owner)


class BudgetAndProcessContracts(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.receipt = {"steps": []}
        self.commands = None

    def tearDown(self):
        if self.commands:
            self.assertTrue(self.commands.finish())
        self.temporary.cleanup()

    def test_exhausted_overall_budget_never_starts_a_new_command(self):
        self.commands = backend.Commands(self.root, self.receipt, time.monotonic() - 1, lambda: None)
        with mock.patch.object(backend.fixture, "subprocess") as process:
            with self.assertRaisesRegex(backend.BackendFailure, "budget"):
                self.commands.run("should-not-start", [sys.executable, "-V"])
            process.Popen.assert_not_called()
        self.assertEqual(self.receipt["steps"], [])

    @requires_posix_runner
    def test_overall_budget_caps_real_command_and_reaps_its_owned_descendant(self):
        self.commands = backend.Commands(self.root, self.receipt, time.monotonic() + 0.8, lambda: None)
        child = self.root / "descendant.pid"
        script = (
            "import subprocess,sys,time,pathlib; "
            "p=subprocess.Popen([sys.executable,'-c','import time;time.sleep(30)']); "
            "pathlib.Path(sys.argv[1]).write_text(str(p.pid)); time.sleep(30)"
        )
        with self.assertRaises(backend.fixture.FixtureFailure):
            self.commands.run("overall-deadline", [sys.executable, "-c", script, str(child)], timeout=30)
        step = self.receipt["steps"][0]
        self.assertLessEqual(step["timeout_seconds"], 0.8)
        self.assertTrue(step["timed_out"])
        self.assertTrue(step["owned_processes_stopped"])
        self.assertIsNotNone(step["returncode"])
        pid = int(child.read_text())
        self.assertIn(pid, self.commands.identities[self.commands.processes[0]])
        info = backend.fixture.ownership.process_info(pid)
        self.assertTrue(info is None or info[3].startswith("Z"))
        self.assertEqual(json.loads((self.root / "progress.json").read_text())["steps"][0], step)

    @requires_posix_runner
    def test_nonzero_exit_keeps_actual_stderr_and_budget_receipt(self):
        self.commands = backend.Commands(self.root, self.receipt, time.monotonic() + 5, lambda: None)
        with self.assertRaises(backend.fixture.FixtureFailure):
            self.commands.run("failed-command", [sys.executable, "-c",
                "import sys; print('actual failure',file=sys.stderr); sys.exit(7)"], timeout=3)
        step = self.receipt["steps"][0]
        self.assertEqual(step["returncode"], 7)
        self.assertEqual(step["timeout_seconds"], 3)
        self.assertIn(b"actual failure", (self.root / step["stderr_log"]).read_bytes())
        self.assertTrue(step["owned_processes_stopped"])


class TopLevelFailureContracts(unittest.TestCase):
    def failed_run(self, failure):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "failure"
            args = argparse.Namespace(
                output=output, revision="a" * 40, image="cached-image", budget_seconds=60,
            )
            with mock.patch.object(backend.Commands, "run", side_effect=failure) as command:
                with mock.patch("sys.stdout", new_callable=io.StringIO):
                    result = backend.execute(args)
            receipt = json.loads((output / "receipt.json").read_text())
            self.assertEqual(result, 1)
            self.assertEqual(receipt["status"], "FAIL")
            self.assertIn(type(failure).__name__, receipt["error"])
            self.assertTrue(receipt["cleanup"]["owned_processes_stopped"])
            self.assertIsNone(receipt["cleanup"]["container_removed"])
            self.assertEqual(command.call_count, 1)
            self.assertTrue((output / "cleanup.json").is_file())

    def test_process_identity_failure_is_persisted_as_failure_before_container_creation(self):
        self.failed_run(backend.fixture.ownership.InteropFailure("identity unavailable"))

    def test_subprocess_observation_timeout_is_persisted_as_failure(self):
        self.failed_run(subprocess.TimeoutExpired(["ps"], 0.01))

    def test_cleanup_signal_failure_preserves_primary_error_and_restores_handlers(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "cleanup-error"
            args = argparse.Namespace(
                output=output, revision="a" * 40, image="synthetic", budget_seconds=60,
            )
            process = mock.Mock(pid=123456)
            identity = (1, process.pid, ("synthetic", 1), "R")
            handlers = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}

            def fail(commands, *unused_args, **unused_kwargs):
                commands.processes.append(process)
                commands.identities[process] = {process.pid: identity}
                raise backend.BackendFailure("primary source failure")

            with (
                mock.patch.object(backend.Commands, "run", fail),
                mock.patch.object(
                    backend.fixture.ownership, "stop_owned",
                    side_effect=backend.fixture.ownership.InteropFailure("identity unavailable"),
                ),
                mock.patch.object(
                    backend.fixture.ownership, "signal_owned",
                    side_effect=PermissionError("signal denied"),
                ) as signal_owned,
                # The signal is data passed to a mock, never a host capability.
                mock.patch.object(backend.fixture, "signal") as signals,
                mock.patch("sys.stdout", new_callable=io.StringIO),
            ):
                self.assertEqual(backend.execute(args), 1)
                signal_owned.assert_called_once_with(process.pid, identity, signals.SIGKILL)
            receipt = json.loads((output / "receipt.json").read_text())
            cleanup = json.loads((output / "cleanup.json").read_text())
            self.assertEqual(receipt["status"], "FAIL")
            self.assertIn("primary source failure", receipt["error"])
            self.assertIn("PermissionError", cleanup["process_cleanup_error"])
            self.assertFalse(cleanup["owned_processes_stopped"])
            self.assertEqual(handlers, {sig: signal.getsignal(sig) for sig in handlers})

    def test_unwritable_provisional_receipt_does_not_skip_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "write-error"
            args = argparse.Namespace(
                output=output, revision="a" * 40, image="synthetic", budget_seconds=60,
            )
            write_json = backend.write_json
            failed_once = False

            def write(path, value):
                nonlocal failed_once
                if path.name == "receipt.json" and not failed_once:
                    failed_once = True
                    raise PermissionError("provisional write denied")
                write_json(path, value)

            with (
                mock.patch.object(backend.Commands, "run", side_effect=OSError("primary failure")),
                mock.patch.object(backend.Commands, "finish", return_value=True) as finish,
                mock.patch.object(backend, "write_json", write),
                mock.patch("sys.stdout", new_callable=io.StringIO),
            ):
                self.assertEqual(backend.execute(args), 1)
            finish.assert_called_once()
            receipt = json.loads((output / "receipt.json").read_text())
            self.assertEqual(receipt["status"], "FAIL")
            self.assertIn("primary failure", receipt["error"])
            self.assertIn("PermissionError", receipt["persistence_errors"][0])
            self.assertTrue((output / "cleanup.json").is_file())


ELF_ARM64 = (
    "ELF Header:\n  Class: ELF64\n  Data: 2's complement, little endian\n"
    "  Type: DYN (Position-Independent Executable file)\n  Machine: AArch64\n"
)


class BinaryContracts(unittest.TestCase):
    executable = "/linux-target/debug/deps/selected-abcd"
    integrity = "0" * 64 + "  " + executable + "\n"

    def test_binary_hash_and_elf_architecture_are_bound_to_native_toolchain(self):
        result = backend.validate_binary(
            self.integrity, ELF_ARM64, self.executable, {"kernel_arch": "aarch64"},
        )
        self.assertEqual(result["sha256"], "0" * 64)
        self.assertEqual(result["machine"], "AArch64")

    def test_wrong_machine_is_rejected_even_if_the_vm_could_emulate_it(self):
        with self.assertRaises(backend.BackendFailure):
            backend.validate_binary(
                self.integrity, ELF_ARM64, self.executable, {"kernel_arch": "x86_64"},
            )

    def test_missing_or_duplicate_elf_fields_and_wrong_hash_target_are_rejected(self):
        for elf, integrity in (
            (ELF_ARM64.replace("Class: ELF64", "Class: ELF32"), self.integrity),
            (ELF_ARM64.replace("  Machine: AArch64\n", ""), self.integrity),
            (ELF_ARM64 + "  Machine: AArch64\n", self.integrity),
            (ELF_ARM64, self.integrity.replace("selected", "different")),
            (ELF_ARM64, "not-a-sha  " + self.executable),
        ):
            with self.subTest(elf=elf, integrity=integrity), self.assertRaises(backend.BackendFailure):
                backend.validate_binary(integrity, elf, self.executable, {"kernel_arch": "aarch64"})


class SyntheticCommands:
    """No external processes: exercise execute's final acceptance decisions."""

    outcome = "ok"
    required = "zmodem::selected_required_test"

    def __init__(self, output, receipt, deadline, interrupted):
        self.output, self.receipt, self.deadline = output, receipt, deadline
        self.cleaning = False

    def run(self, name, argv, **kwargs):
        data = b""
        if name == "resolve-revision":
            data = (self.receipt["revision"] + "\n").encode()
        elif name == "source-git-archive":
            with tarfile.open(self.output / "source.tar", "w") as archive:
                for path, content in (("Cargo.toml", b"[workspace]\n"), ("Cargo.lock", b"version=4\n")):
                    entry = tarfile.TarInfo(path)
                    entry.size = len(content)
                    archive.addfile(entry, io.BytesIO(content))
        elif name == "podman-vm-architecture":
            data = b"arm64\n"
        elif name == "create-container":
            data = ("b" * 64 + "\n").encode()
        elif name == "linux-toolchain":
            data = b"Linux aarch64\nrustc 1.98.1 (synthetic)\nhost: aarch64-unknown-linux-gnu\n"
        elif name == "container-image-id":
            data = ("c" * 64 + "\n").encode()
        elif name == "test-user":
            data = b"uid=1000 gid=1000\n"
        elif name == "compile-keelshell-session":
            data = json.dumps({
                "reason": "compiler-artifact", "profile": {"test": True},
                "package_id": "path+file:///snapshot#keelshell-session@0.1.0",
                "target": {"name": "keelshell_session"},
                "executable": BinaryContracts.executable,
            }).encode()
        elif name.startswith("inventory-"):
            data = (self.required + ": test\n").encode()
        elif name.startswith("binary-sha-"):
            data = BinaryContracts.integrity.encode()
        elif name.startswith("binary-elf-"):
            data = ELF_ARM64.encode()
        elif name.startswith("test-keelshell-session-"):
            data = ("test " + self.required + " ... " + self.outcome + "\n").encode()
        elif name == "source-after-run":
            manifest = json.loads((self.output / "source-manifest.json").read_text())
            data = "".join(value + "  " + key + "\n" for key, value in manifest["files"].items()).encode()
        self.receipt["steps"].append({
            "name": name, "argv": argv, "returncode": 0,
            "timed_out": False, "owned_processes_stopped": True,
        })
        return 0, data

    def finish(self):
        return True


class AcceptanceContracts(unittest.TestCase):
    def run_outcome(self, outcome):
        commands = type("OutcomeCommands", (SyntheticCommands,), {"outcome": outcome})
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "selected"
            args = argparse.Namespace(
                output=output, revision="a" * 40, image="synthetic", budget_seconds=60,
                registry_cache=Path(directory), registry_index=Path(directory),
            )
            with (
                mock.patch.object(backend, "Commands", commands),
                mock.patch.object(backend, "SPECS", ((
                    "keelshell-session", "keelshell_session", "zmodem::", (commands.required,),
                ),)),
                mock.patch.object(backend, "registry_bundle", return_value={}),
                mock.patch.object(backend, "identity", return_value={}),
                mock.patch.object(backend, "remove_owned_container", return_value={"container_removed": True}),
                mock.patch("sys.stdout", new_callable=io.StringIO),
            ):
                result = backend.execute(args)
            return result, json.loads((output / "receipt.json").read_text())

    def test_selected_required_test_must_actually_pass_not_only_appear_in_inventory(self):
        result, receipt = self.run_outcome("ignored, platform fixture unavailable")
        self.assertEqual(result, 1)
        self.assertEqual(receipt["status"], "FAIL")
        self.assertEqual(receipt["totals"], {"passed": 0, "failed": 0, "ignored": 1})
        self.assertIn("did not pass every selected test", receipt["error"])

    def test_executed_required_test_with_confirmed_cleanup_can_pass(self):
        result, receipt = self.run_outcome("ok")
        self.assertEqual(result, 0)
        self.assertEqual(receipt["status"], "PASS")
        self.assertEqual(receipt["totals"], {"passed": 1, "failed": 0, "ignored": 0})
        self.assertTrue(receipt["source_after_run_matches"])


if __name__ == "__main__":
    unittest.main()
