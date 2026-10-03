"""Offline release-publication tests with real archives and a simulated gh API."""
from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import publish
import release as release_tools

SHA = "a" * 40
OTHER_SHA = "b" * 40
REPOSITORY = "example/keelshell"


def executable(target: str) -> bytes:
    """Minimal structurally valid headers; fixtures are never executed."""
    platform, architecture = release_tools.TARGETS[target]
    if platform == "macos":
        cpu = 0x100000C if architecture == "aarch64" else 0x1000007
        return b"\xcf\xfa\xed\xfe" + struct.pack("<IIIIIII", cpu, 0, 2, 1, 8, 0, 0) + struct.pack("<II", 0x1B, 8)
    if platform == "linux":
        output = bytearray(120)
        output[:7] = b"\x7fELF\x02\x01\x01"
        struct.pack_into("<HHI", output, 16, 2, 183 if architecture == "aarch64" else 62, 1)
        struct.pack_into("<QQ", output, 24, 0x1000, 64)
        struct.pack_into("<HHH", output, 52, 64, 56, 1)
        return bytes(output)
    output = bytearray(240)
    output[:2] = b"MZ"
    struct.pack_into("<I", output, 0x3C, 64)
    output[64:68] = b"PE\0\0"
    struct.pack_into("<HH", output, 68, 0xAA64 if architecture == "aarch64" else 0x8664, 1)
    struct.pack_into("<HHH", output, 84, 112, 2, 0x20B)
    return bytes(output)


class FakeGitHub:
    """Models persisted remote state across failed CLI calls and repeated runs."""

    def __init__(self, tag: str):
        self.tag = tag
        self.target = {"type": "commit", "sha": SHA}
        self.annotations: dict[str, dict] = {}
        self.item: dict | None = None
        self.assets: dict[str, dict] = {}
        self.calls: list[list[str]] = []
        self.mutations: list[list[str]] = []
        self.upload_count = 0
        self.fail_upload: int | None = None
        self.create_failure_after_accept = False
        self.edit_failure_after_accept = False
        self.corrupt_after_upload = False
        self.change_tag_after_upload = False
        self.api_status: int | None = None
        self.list_prefix: list[dict] = []
        self.environments: list[dict] = []

    def new_release(self, draft: bool = True) -> None:
        self.item = {"id": 42, "tag_name": self.tag, "draft": draft,
                     "prerelease": "-" in self.tag, "target_commitish": SHA}

    def with_assets(self, expected: dict[str, publish.Asset]) -> None:
        self.assets = {name: {"name": name, "size": asset.size, "state": "uploaded",
                              "digest": f"sha256:{asset.sha256}"}
                       for name, asset in expected.items()}

    def api_result(self, command: list[str], value: object, status: int = 200) -> subprocess.CompletedProcess:
        payload = f"HTTP/2.0 {status} Test\r\nContent-Type: application/json\r\n\r\n{json.dumps(value)}".encode()
        return subprocess.CompletedProcess(command, 0 if status == 200 else 1, payload, b"private diagnostic omitted")

    def __call__(self, command: list[str], **kwargs) -> subprocess.CompletedProcess:
        self.calls.append(command)
        self.environments.append(kwargs["env"])
        if command[:2] == ["gh", "api"]:
            endpoint = command[-1]
            if self.api_status is not None:
                return self.api_result(command, {"message": "sensitive"}, self.api_status)
            if endpoint.endswith(f"git/ref/tags/{self.tag}"):
                return self.api_result(command, {"ref": f"refs/tags/{self.tag}", "object": self.target})
            if "/git/tags/" in endpoint:
                object_sha = endpoint.rsplit("/", 1)[1]
                return self.api_result(command, {"sha": object_sha, "object": self.annotations[object_sha]})
            if endpoint.endswith(f"releases/tags/{self.tag}"):
                published = self.item if self.item and not self.item["draft"] else None
                return self.api_result(command, published, 200 if published else 404)
            if "releases?per_page=100&page=" in endpoint:
                page = int(endpoint.rsplit("=", 1)[1])
                items = self.list_prefix + ([self.item] if self.item else [])
                return self.api_result(command, items[(page - 1) * 100:page * 100])
            if endpoint.endswith("releases/42/assets?per_page=100"):
                return self.api_result(command, list(self.assets.values()))
            raise AssertionError(f"Unexpected API endpoint: {endpoint}")
        if command[:2] != ["gh", "release"]:
            raise AssertionError("Unexpected non-gh command")
        self.mutations.append(command)
        action = command[2]
        code = 0
        if action == "create":
            if self.item:
                return subprocess.CompletedProcess(command, 1, b"", b"already exists")
            self.new_release()
            self.item["body"] = command[command.index("--notes") + 1] + "\nGenerated changes."
            code = int(self.create_failure_after_accept)
        elif action == "upload":
            self.upload_count += 1
            if self.upload_count == self.fail_upload:
                return subprocess.CompletedProcess(command, 1, b"", b"request authorization: do-not-print")
            name = command[4]
            if name in self.assets:
                raise AssertionError("Attempted replacement of an existing asset")
            data = (Path(kwargs["cwd"]) / name).read_bytes()
            self.assets[name] = {"name": name, "size": len(data), "state": "uploaded",
                                 "digest": "sha256:" + hashlib.sha256(data).hexdigest()}
            if self.corrupt_after_upload:
                self.assets[name]["digest"] = None
            if self.change_tag_after_upload:
                self.target = {"type": "commit", "sha": OTHER_SHA}
        elif action == "edit":
            if self.item is None:
                raise AssertionError("Editing absent release")
            self.item["draft"] = False
            self.item["prerelease"] = "--prerelease=true" in command
            self.item["body"] = Path(command[command.index("--notes-file") + 1]).read_text(encoding="utf-8")
            code = int(self.edit_failure_after_accept)
        else:
            raise AssertionError(f"Forbidden release operation: {action}")
        return subprocess.CompletedProcess(command, code, b"", b"private diagnostic omitted")


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.manifest = self.root / "Cargo.toml"
        self.directory = self.root / "collection"
        self.make_collection("1.2.3")
        self.fake = FakeGitHub(self.tag)
        self.process = patch("publish.subprocess.run", side_effect=self.fake).start()
        self.addCleanup(patch.stopall)

    def make_collection(self, value: str) -> None:
        self.tag = f"v{value}"
        self.manifest.write_text(f'[workspace.package]\nversion = "{value}"\n')
        self.directory.mkdir(exist_ok=True)
        for path in self.directory.iterdir():
            path.unlink()
        for target, (platform, _) in release_tools.TARGETS.items():
            name = release_tools.BINARIES[platform]
            binary = executable(target)
            checksum = hashlib.sha256(binary).hexdigest()
            manifest = {"schema_version": 1, "platform": platform, "version": value,
                        "target": target, "build": {"commit": SHA},
                        "binary_sha256": checksum, "icon_source_sha256": "c" * 64,
                        "files": {name: checksum}, "installed": False,
                        "signed_by_packaging_script": False,
                        "native_acceptance": "not performed by this script"}
            files = {name: (binary, 0o755), release_tools.MANIFEST: (json.dumps(manifest).encode(), 0o644)}
            path = self.directory / release_tools.package_name(value, target)
            # Archive permissions are explicit fixture data. Windows chmod
            # cannot represent Unix execute bits; do not weaken validation or
            # depend on the test host's filesystem permission semantics.
            if platform == "linux":
                with tarfile.open(path, "w:gz") as archive:
                    for member_name, (data, mode) in files.items():
                        member = tarfile.TarInfo(member_name)
                        member.size, member.mode = len(data), mode
                        archive.addfile(member, io.BytesIO(data))
            else:
                with zipfile.ZipFile(path, "w") as archive:
                    for member_name, (data, mode) in files.items():
                        member = zipfile.ZipInfo(member_name)
                        member.create_system = 3
                        member.external_attr = (0o100000 | mode) << 16
                        archive.writestr(member, data)
            (self.directory / (path.name + ".sha256")).write_text(
                f"{release_tools.file_digest(path)}  {path.name}\n", encoding="ascii", newline="\n")
        verified = release_tools.verify_collection(self.directory, list(release_tools.TARGETS), self.manifest)
        (self.directory / "SHA256SUMS").write_text("".join(line for _, line in verified.values()), newline="\n")
        self.expected = publish.local_assets(self.directory, self.tag, self.manifest, SHA)

    def edit_archived_receipt(self, target: str, edit) -> None:
        """Change provenance while retaining valid archive/file/checksum integrity."""
        value = self.tag.removeprefix("v")
        path = self.directory / release_tools.package_name(value, target)
        files = {}
        with release_tools.archive_members(path) as (members, _, open_member):
            for name, (_, mode, member) in members.items():
                with open_member(member) as stream:
                    files[name] = (stream.read(), mode)
        receipt = json.loads(files[release_tools.MANIFEST][0])
        edit(receipt)
        files[release_tools.MANIFEST] = (json.dumps(receipt).encode(), 0o644)
        if path.name.endswith(".tar.gz"):
            with tarfile.open(path, "w:gz") as archive:
                for name, (data, mode) in files.items():
                    member = tarfile.TarInfo(name)
                    member.size, member.mode = len(data), mode & 0o777
                    archive.addfile(member, io.BytesIO(data))
        else:
            with zipfile.ZipFile(path, "w") as archive:
                for name, (data, mode) in files.items():
                    member = zipfile.ZipInfo(name)
                    member.create_system = 3
                    member.external_attr = (0o100000 | (mode & 0o777)) << 16
                    archive.writestr(member, data)
        path.with_name(path.name + ".sha256").write_text(
            f"{release_tools.file_digest(path)}  {path.name}\n", encoding="ascii", newline="\n")
        packages = sorted(release_tools.package_name(value, item) for item in release_tools.TARGETS)
        (self.directory / "SHA256SUMS").write_text("".join(
            f"{release_tools.file_digest(self.directory / name)}  {name}\n" for name in packages),
            encoding="ascii", newline="\n")

    def run_publish(self) -> str:
        return publish.publish(REPOSITORY, self.tag, self.directory, SHA, self.manifest)

    def assert_not_published(self):
        self.assertFalse(any(command[2] == "edit" for command in self.fake.mutations))
        if self.fake.item:
            self.assertTrue(self.fake.item["draft"])

    def test_complete_six_target_collection_publishes_only_after_verified_uploads(self):
        self.assertIn("Published release confirmed", self.run_publish())
        self.assertEqual(self.fake.upload_count, 13)
        self.assertEqual(self.fake.mutations[-1][2], "edit")
        self.assertIn("--draft=false", self.fake.mutations[-1])
        self.assertIn("--verify-tag", self.fake.mutations[0])
        self.assertIn("--target", self.fake.mutations[0])
        self.assertIn(SHA, self.fake.mutations[0])
        self.assertIn("--generate-notes", self.fake.mutations[0])
        self.assertFalse(any("--clobber" in command for command in self.fake.calls))
        self.assertEqual(len(self.fake.assets), 13)

    def test_nested_annotated_tags_are_dereferenced_to_the_exact_commit(self):
        first, second = "d" * 40, "e" * 40
        self.fake.target = {"type": "tag", "sha": first}
        self.fake.annotations[first] = {"type": "tag", "sha": second}
        self.fake.annotations[second] = {"type": "commit", "sha": SHA}
        self.run_publish()
        self.assertFalse(self.fake.item["draft"])

    def test_annotated_tag_cycle_never_creates_a_release(self):
        checksum = "d" * 40
        self.fake.target = {"type": "tag", "sha": checksum}
        self.fake.annotations[checksum] = {"type": "tag", "sha": checksum}
        with self.assertRaisesRegex(publish.PublishError, "cyclic"):
            self.run_publish()
        self.assertEqual(self.fake.mutations, [])

    def test_wrong_tag_commit_never_creates_a_release(self):
        self.fake.target = {"type": "commit", "sha": OTHER_SHA}
        with self.assertRaisesRegex(publish.PublishError, "requested commit"):
            self.run_publish()
        self.assertEqual(self.fake.mutations, [])

    def test_partial_upload_failure_leaves_draft_and_next_run_resumes_missing_assets(self):
        self.fake.fail_upload = 5
        with self.assertRaisesRegex(publish.PublishError, "mutation failed"):
            self.run_publish()
        self.assert_not_published()
        self.assertEqual(len(self.fake.assets), 4)
        completed = set(self.fake.assets)
        call_count = len(self.fake.mutations)
        self.fake.fail_upload = None
        self.run_publish()
        later_uploads = {command[4] for command in self.fake.mutations[call_count:] if command[2] == "upload"}
        self.assertEqual(later_uploads, set(self.expected) - completed)

    def test_create_response_lost_recovers_existing_draft_without_recreating(self):
        self.fake.create_failure_after_accept = True
        with self.assertRaises(publish.PublishError):
            self.run_publish()
        self.assert_not_published()
        self.fake.create_failure_after_accept = False
        self.run_publish()
        self.assertEqual(sum(command[2] == "create" for command in self.fake.mutations), 1)

    def test_draft_on_a_later_api_page_is_found_without_duplicate_creation(self):
        self.fake.new_release()
        self.fake.list_prefix = [{"tag_name": f"v0.0.{index}"} for index in range(100)]
        self.run_publish()
        self.assertFalse(any(command[2] == "create" for command in self.fake.mutations))
        self.assertTrue(any("releases?per_page=100&page=2" in command[-1] for command in self.fake.calls))

    def test_multiple_drafts_for_same_tag_are_not_selected_arbitrarily(self):
        self.fake.new_release()
        self.fake.list_prefix = [{"tag_name": self.tag, "id": 17, "draft": True}]
        with self.assertRaisesRegex(publish.PublishError, "Multiple releases"):
            self.run_publish()
        self.assertEqual(self.fake.mutations, [])

    def test_resumed_draft_retains_existing_notes_and_adds_acceptance_boundaries(self):
        self.fake.new_release()
        self.fake.item["body"] = "Existing user-authored notes."
        self.run_publish()
        self.assertIn("Existing user-authored notes.", self.fake.item["body"])
        self.assertIn(publish.RELEASE_BOUNDARIES, self.fake.item["body"])

    def test_publication_response_lost_is_idempotent_on_next_run(self):
        self.fake.edit_failure_after_accept = True
        with self.assertRaises(publish.PublishError):
            self.run_publish()
        self.assertFalse(self.fake.item["draft"])
        self.fake.mutations.clear()
        self.assertIn("no changes made", self.run_publish())
        self.assertEqual(self.fake.mutations, [])

    def test_matching_published_release_is_read_only_success(self):
        self.fake.new_release(draft=False)
        self.fake.with_assets(self.expected)
        self.assertIn("no changes made", self.run_publish())
        self.assertEqual(self.fake.mutations, [])

    def test_conflicting_digest_refuses_published_and_draft_releases(self):
        for draft in (False, True):
            with self.subTest(draft=draft):
                self.fake.new_release(draft=draft)
                self.fake.with_assets(self.expected)
                self.fake.assets["SHA256SUMS"]["digest"] = "sha256:" + "0" * 64
                with self.assertRaisesRegex(publish.PublishError, "SHA-256"):
                    self.run_publish()
                self.assertEqual(self.fake.mutations, [])

    def test_conflicting_size_refuses_overwrite(self):
        self.fake.new_release()
        self.fake.with_assets(self.expected)
        self.fake.assets["SHA256SUMS"]["size"] += 1
        with self.assertRaisesRegex(publish.PublishError, "size conflicts"):
            self.run_publish()
        self.assertEqual(self.fake.mutations, [])

    def test_foreign_remote_asset_is_preserved_without_publishing(self):
        self.fake.new_release()
        self.fake.assets["unmanaged.txt"] = {"name": "unmanaged.txt"}
        with self.assertRaisesRegex(publish.PublishError, "unmanaged"):
            self.run_publish()
        self.assertEqual(self.fake.mutations, [])
        self.assertIn("unmanaged.txt", self.fake.assets)

    def test_incomplete_published_release_is_not_modified(self):
        self.fake.new_release(draft=False)
        with self.assertRaisesRegex(publish.PublishError, "missing expected"):
            self.run_publish()
        self.assertEqual(self.fake.mutations, [])

    def test_server_missing_digest_after_upload_does_not_publish(self):
        self.fake.corrupt_after_upload = True
        with self.assertRaisesRegex(publish.PublishError, "SHA-256"):
            self.run_publish()
        self.assert_not_published()

    def test_tag_moved_during_upload_does_not_publish(self):
        self.fake.change_tag_after_upload = True
        with self.assertRaisesRegex(publish.PublishError, "tag changed during upload"):
            self.run_publish()
        self.assert_not_published()

    def test_prerelease_follows_semver_not_an_external_flag(self):
        self.make_collection("1.2.3-rc.1")
        self.fake.tag = self.tag
        self.run_publish()
        self.assertIn("--prerelease=true", self.fake.mutations[0])
        self.assertIn("--prerelease=true", self.fake.mutations[-1])
        self.assertTrue(self.fake.item["prerelease"])

    def test_non_404_api_errors_never_trigger_create(self):
        self.fake.api_status = 403
        with self.assertRaisesRegex(publish.PublishError, "HTTP 403"):
            self.run_publish()
        self.assertEqual(self.fake.mutations, [])

    def test_invalid_local_inputs_stop_before_any_network_call(self):
        (self.directory / "unexpected.txt").write_text("data")
        with self.assertRaisesRegex(publish.PublishError, "exactly six"):
            self.run_publish()
        self.process.assert_not_called()

    def test_corrupt_archive_is_rejected_before_network(self):
        path = next(asset.path for name, asset in self.expected.items() if name.endswith(".zip"))
        path.write_bytes(b"corrupted")
        with self.assertRaisesRegex(ValueError, "Checksum mismatch"):
            self.run_publish()
        self.process.assert_not_called()

    def test_tag_must_match_workspace_version_before_network(self):
        with self.assertRaisesRegex(ValueError, "exactly match"):
            publish.publish(REPOSITORY, "v9.9.9", self.directory, SHA, self.manifest)
        self.process.assert_not_called()

    def test_one_package_from_another_commit_is_rejected_before_network(self):
        for target in release_tools.TARGETS:
            with self.subTest(target=target):
                self.make_collection("1.2.3")
                self.edit_archived_receipt(target, lambda receipt: receipt["build"].update(commit=OTHER_SHA))
                # Prove this is a provenance failure, not corrupt ZIP/TAR/checksum data.
                release_tools.verify_collection(self.directory, list(release_tools.TARGETS), self.manifest)
                with self.assertRaisesRegex(publish.PublishError, "build commit"):
                    self.run_publish()
                self.process.assert_not_called()

    def test_missing_build_or_commit_provenance_is_rejected_before_network(self):
        for field in ("build", "commit"):
            with self.subTest(field=field):
                self.make_collection("1.2.3")
                def remove(receipt):
                    if field == "build":
                        del receipt["build"]
                    else:
                        del receipt["build"]["commit"]
                self.edit_archived_receipt("x86_64-unknown-linux-gnu", remove)
                with self.assertRaisesRegex(publish.PublishError, "build commit"):
                    self.run_publish()
                self.process.assert_not_called()

    def test_missing_target_provenance_is_rejected_before_network(self):
        self.edit_archived_receipt("aarch64-apple-darwin", lambda receipt: receipt.pop("target"))
        with self.assertRaisesRegex(publish.PublishError, "exact target"):
            self.run_publish()
        self.process.assert_not_called()

    def test_wrong_target_provenance_is_rejected_before_network(self):
        self.edit_archived_receipt("aarch64-pc-windows-msvc", lambda receipt: receipt.update(target="x86_64-pc-windows-msvc"))
        with self.assertRaisesRegex(ValueError, "target mismatch"):
            self.run_publish()
        self.process.assert_not_called()

    def test_tokens_are_only_in_environment_and_debug_is_disabled(self):
        with patch.dict(os.environ, {"GH_TOKEN": "never-log-token", "GH_DEBUG": "api", "GH_HOST": "untrusted.invalid"}):
            self.run_publish()
        self.assertTrue(all(environment["GH_TOKEN"] == "never-log-token" for environment in self.fake.environments))
        self.assertTrue(all(environment["GH_HOST"] == "github.com" for environment in self.fake.environments))
        self.assertTrue(all("GH_DEBUG" not in environment for environment in self.fake.environments))
        self.assertNotIn("never-log-token", repr(self.fake.calls))

    def test_timeout_diagnostics_do_not_expose_request_headers(self):
        self.process.side_effect = subprocess.TimeoutExpired("gh", 120, output=b"token-in-output", stderr=b"secret")
        with self.assertRaises(publish.PublishError) as caught:
            self.run_publish()
        self.assertNotIn("token-in-output", str(caught.exception))
        self.assertNotIn("secret", str(caught.exception))
        self.assertEqual(self.fake.mutations, [])

    def test_duplicate_remote_names_and_pending_uploads_are_rejected(self):
        asset = self.expected["SHA256SUMS"]
        item = {"name": "SHA256SUMS", "size": asset.size, "state": "uploaded", "digest": f"sha256:{asset.sha256}"}
        with self.assertRaisesRegex(publish.PublishError, "duplicate"):
            publish.matching_assets([item, item], self.expected, complete=False)
        item["state"] = "starter"
        with self.assertRaisesRegex(publish.PublishError, "not fully uploaded"):
            publish.matching_assets([item], self.expected, complete=False)


if __name__ == "__main__":
    unittest.main()
