#!/usr/bin/env python3
"""Publish a verified six-target collection through gh; never replace assets.

Run only in a workflow serialized by repository/tag. GitHub has no atomic
compare-and-publish API, so another actor can still edit a release between checks.
Every mutation failure is treated as an unknown outcome: rerun the same inputs
to inspect/recover the draft, rather than deleting assets or retrying blindly.
Authentication is inherited from GH_TOKEN/GITHUB_TOKEN; command output is never
echoed, because CLI diagnostics can contain credentials or request headers.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile
from urllib.parse import quote

import release as release_tools

REPO = re.compile(r"[A-Za-z0-9][A-Za-z0-9-]{0,38}/[A-Za-z0-9_][A-Za-z0-9_.-]{0,99}", re.ASCII)
COMMIT = re.compile(r"[0-9a-f]{40}", re.ASCII)
HTTP = re.compile(r"HTTP/\S+ ([0-9]{3})(?: [^\r\n]*)?\r?\n", re.ASCII)
MAX_API_BYTES = 2 * 1024 * 1024
NOTES_MARKER = "<!-- keelshell-release-boundaries-v1 -->"
RELEASE_BOUNDARIES = f"""{NOTES_MARKER}
KeelShell 仍在开发中，完整的远程 SSH 管理与运维能力尚未完成。
这些 macOS 产物未使用 Developer ID 签名或经过 Apple 公证；Windows 产物未代码签名。
Linux 产物以 Ubuntu 24.04 为兼容基线。矩阵构建及自动测试不等于各平台原生桌面 GUI、真实服务器或生产环境验收。

KeelShell is in development; the full remote SSH management and operations scope is incomplete.
macOS builds are not Developer ID signed or Apple notarized; Windows builds are not code signed.
Linux builds target Ubuntu 24.04. Matrix builds and automated tests do not establish native GUI,
real-server interoperability, or production acceptance on every platform.
"""


class PublishError(ValueError):
    """A safe-to-display failure; raw gh output must not enter this exception."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PublishError(message)


@dataclass(frozen=True)
class Asset:
    path: Path
    size: int
    sha256: str


class GitHub:
    """Small gh adapter with bounded commands and explicit github.com routing."""

    def __init__(self, repo: str):
        require(REPO.fullmatch(repo) is not None, "Repository must be OWNER/NAME")
        self.repo = repo

    def run(self, arguments: list[str], *, cwd: Path | None = None,
            timeout: int = 120) -> subprocess.CompletedProcess:
        environment = dict(os.environ)
        environment.pop("GH_DEBUG", None)
        environment["GH_HOST"] = "github.com"
        environment["GH_PROMPT_DISABLED"] = "1"
        try:
            return subprocess.run(["gh", *arguments], cwd=cwd, env=environment,
                                  capture_output=True, timeout=timeout, check=False)
        except (OSError, subprocess.TimeoutExpired):
            # Do not interpolate error: TimeoutExpired may retain stdout/stderr.
            raise PublishError("gh could not complete; outcome may be unknown. Rerun the same inputs to inspect it.") from None

    def api(self, path: str, *, absent_ok: bool = False) -> object | None:
        result = self.run(["api", "--hostname", "github.com", "--include",
                           "--header", "Accept: application/vnd.github+json",
                           "--header", "X-GitHub-Api-Version: 2026-03-10",
                           f"repos/{self.repo}/{path}"])
        require(len(result.stdout) <= MAX_API_BYTES, "GitHub API response exceeds the size limit")
        try:
            output = result.stdout.decode("utf-8")
        except UnicodeDecodeError:
            raise PublishError("GitHub API returned invalid UTF-8") from None
        match = HTTP.match(output)
        require(match is not None, "GitHub API returned no recognizable HTTP status")
        status = int(match.group(1))
        if status == 404 and absent_ok:
            return None
        require(result.returncode == 0 and status == 200,
                f"GitHub API request failed (HTTP {status}); no response details were logged")
        _, separator, body = output.partition("\r\n\r\n")
        if not separator:
            _, separator, body = output.partition("\n\n")
        require(bool(separator), "GitHub API response has no body separator")
        try:
            return json.loads(body, object_pairs_hook=release_tools.no_duplicate_keys)
        except (ValueError, TypeError):
            raise PublishError("GitHub API returned invalid JSON") from None

    def mutate(self, arguments: list[str], *, cwd: Path | None = None,
               timeout: int = 120) -> None:
        result = self.run(["release", *arguments, "--repo", self.repo], cwd=cwd, timeout=timeout)
        require(result.returncode == 0,
                "gh release mutation failed; outcome may be unknown. Rerun the same inputs to inspect it.")

    def tag_commit(self, tag: str) -> str:
        reference = self.api(f"git/ref/tags/{quote(tag, safe='')}")
        require(isinstance(reference, dict) and reference.get("ref") == f"refs/tags/{tag}",
                "Remote tag reference does not match the requested tag")
        target = reference.get("object")
        visited: set[str] = set()
        for _ in range(9):
            require(isinstance(target, dict), "Remote tag object is invalid")
            checksum, kind = target.get("sha"), target.get("type")
            require(isinstance(checksum, str) and COMMIT.fullmatch(checksum) is not None,
                    "Remote tag object SHA is invalid")
            if kind == "commit":
                return checksum
            require(kind == "tag" and checksum not in visited and len(visited) < 8,
                    "Remote annotated tag chain is cyclic, too deep, or not a commit")
            visited.add(checksum)
            annotation = self.api(f"git/tags/{checksum}")
            require(isinstance(annotation, dict) and annotation.get("sha") == checksum,
                    "Remote annotated tag response does not match its SHA")
            target = annotation.get("object")
        raise PublishError("Remote tag nesting exceeds the limit")

    def release(self, tag: str) -> dict | None:
        item = self.api(f"releases/tags/{quote(tag, safe='')}", absent_ok=True)
        if item is None:
            # REST's by-tag endpoint only finds published releases. Listing
            # releases with a write-capable token also exposes drafts; a 404
            # above must never be treated as proof that no draft exists.
            matches = []
            for page in range(1, 101):
                items = self.api(f"releases?per_page=100&page={page}")
                require(isinstance(items, list) and len(items) <= 100
                        and all(isinstance(candidate, dict) for candidate in items),
                        "Remote release listing is invalid")
                matches.extend(candidate for candidate in items if candidate.get("tag_name") == tag)
                require(len(matches) <= 1, "Multiple releases use this tag; no release was selected")
                if len(items) < 100:
                    break
            else:
                raise PublishError("Remote release listing exceeds the search limit")
            if not matches:
                return None
            item = matches[0]
        require(isinstance(item, dict) and item.get("tag_name") == tag,
                "Remote release does not match the requested tag")
        require(type(item.get("id")) is int and item["id"] > 0
                and type(item.get("draft")) is bool and type(item.get("prerelease")) is bool,
                "Remote release metadata is invalid")
        return item

    def assets(self, release_id: int) -> list[dict]:
        # There are only 13 managed assets. A full 100-item page necessarily
        # contains foreign/duplicate assets and will fail closed below.
        items = self.api(f"releases/{release_id}/assets?per_page=100")
        require(isinstance(items, list), "Remote release assets response is invalid")
        return items


def local_assets(directory: Path, tag: str, manifest: Path, expected_commit: str) -> dict[str, Asset]:
    require(COMMIT.fullmatch(expected_commit) is not None, "Expected build commit must be a full lowercase SHA")
    value = release_tools.validate_ref(tag, manifest)
    require(not directory.is_symlink() and directory.is_dir(), "Collection must be a real directory")
    packages = {release_tools.package_name(value, target) for target in release_tools.TARGETS}
    names = packages | {name + ".sha256" for name in packages} | {"SHA256SUMS"}
    paths = list(directory.iterdir())
    require({path.name for path in paths} == names, "Collection must contain exactly six packages, their checksums, and SHA256SUMS")
    require(all(stat.S_ISREG(path.lstat().st_mode) for path in paths),
            "Collection must contain only regular files, without directories or symlinks")
    release_tools.verify_collection(directory, list(release_tools.TARGETS), manifest)
    # Integrity alone does not bind an archive to this release's source. Require
    # provenance in every already-validated receipt, including resumed drafts.
    for target, (platform, _) in release_tools.TARGETS.items():
        path = directory / release_tools.package_name(value, target)
        with release_tools.archive_members(path) as (members, _, open_member):
            with open_member(members[release_tools.MANIFEST][2]) as stream:
                receipt = release_tools.receipt(stream.read(release_tools.MAX_MANIFEST + 1), platform, value, target)
        require(receipt.get("target") == target, f"Package receipt must declare its exact target: {path.name}")
        build = receipt.get("build")
        require(isinstance(build, dict) and build.get("commit") == expected_commit,
                f"Package build commit is missing or does not match the release commit: {path.name}")
    return {path.name: Asset(path.resolve(), path.stat().st_size, release_tools.file_digest(path))
            for path in sorted(paths)}


def matching_assets(remote: list[dict], expected: dict[str, Asset], *, complete: bool) -> set[str]:
    found: set[str] = set()
    for item in remote:
        require(isinstance(item, dict), "Remote release asset is invalid")
        name = item.get("name")
        require(isinstance(name, str) and name in expected,
                "Release contains an unmanaged asset; it was not modified or removed")
        require(name not in found, "Release contains duplicate asset names")
        found.add(name)
        asset = expected[name]
        require(item.get("state") == "uploaded", f"Remote asset is not fully uploaded: {name}")
        require(type(item.get("size")) is int and item["size"] == asset.size,
                f"Remote asset size conflicts with the local file: {name}")
        require(item.get("digest") == f"sha256:{asset.sha256}",
                f"Remote asset SHA-256 is missing or conflicts with the local file: {name}")
    require(not complete or found == set(expected), "Release is missing expected assets")
    return found


def publish(repo: str, tag: str, directory: Path, commit: str,
            manifest: Path = release_tools.ROOT / "Cargo.toml") -> str:
    require(COMMIT.fullmatch(commit) is not None, "Commit must be a full lowercase 40-character SHA")
    github = GitHub(repo)
    expected = local_assets(directory, tag, manifest, commit)
    prerelease = "-" in release_tools.validate_ref(tag, manifest)
    require(github.tag_commit(tag) == commit, "Remote tag does not point to the requested commit")
    item = github.release(tag)
    if item is None:
        github.mutate(["create", tag, "--draft", "--verify-tag", "--target", commit,
                       "--generate-notes", "--notes", RELEASE_BOUNDARIES,
                       f"--prerelease={str(prerelease).lower()}"])
        item = github.release(tag)
        require(item is not None, "Created draft is not visible; rerun to inspect its outcome")
    release_id = item["id"]
    existing = matching_assets(github.assets(release_id), expected, complete=not item["draft"])
    if not item["draft"]:
        require(item["prerelease"] == prerelease, "Published release prerelease status conflicts with the version")
        return "Existing published release matches every asset; no changes made."
    for name, asset in expected.items():
        if name in existing:
            continue
        # Recheck the draft identity before every upload, never upload to a
        # release that another actor has already published or replaced.
        current = github.release(tag)
        require(current is not None and current["id"] == release_id and current["draft"],
                "Release changed during upload; publication was not attempted")
        require(asset.path.stat().st_size == asset.size
                and release_tools.file_digest(asset.path) == asset.sha256,
                "Local release file changed after validation")
        github.mutate(["upload", tag, name], cwd=asset.path.parent, timeout=600)
    matching_assets(github.assets(release_id), expected, complete=True)
    require(local_assets(directory, tag, manifest, commit) == expected, "Collection changed during publication")
    require(github.tag_commit(tag) == commit, "Remote tag changed during upload; draft was not published")
    current = github.release(tag)
    require(current is not None and current["id"] == release_id and current["draft"],
            "Release changed before publication; publication was not attempted")
    body = current.get("body") or ""
    require(isinstance(body, str), "Remote release notes are invalid")
    # Preserve generated/user-authored notes when recovering a draft, while
    # ensuring the public release always has the project's acceptance boundary.
    notes = body if RELEASE_BOUNDARIES.strip() in body else RELEASE_BOUNDARIES + "\n" + body
    with tempfile.TemporaryDirectory(prefix="keelshell-release-notes-") as temporary:
        notes_path = Path(temporary) / "notes.md"
        notes_path.write_text(notes, encoding="utf-8")
        github.mutate(["edit", tag, "--draft=false", "--notes-file", str(notes_path),
                       f"--prerelease={str(prerelease).lower()}"])
    current = github.release(tag)
    require(current is not None and current["id"] == release_id and not current["draft"]
            and current["prerelease"] == prerelease, "Release publication could not be confirmed; rerun to inspect it")
    require(github.tag_commit(tag) == commit, "Remote tag changed; published release requires manual inspection")
    matching_assets(github.assets(release_id), expected, complete=True)
    return "Published release confirmed: exact tag commit and all 13 asset sizes/SHA-256 digests match."


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    arguments = parser.parse_args(argv)
    print(publish(arguments.repo, arguments.tag, arguments.directory, arguments.commit))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Release publication stopped: {error}", file=sys.stderr)
        sys.exit(1)
