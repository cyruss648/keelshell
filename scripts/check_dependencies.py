#!/usr/bin/env python3
"""Check direct Rust dependencies against live, official crates.io metadata.

Usage: python3 -B scripts/check_dependencies.py [--json-report NEW_FILE.json]

This is a read-only check: it neither runs Cargo nor updates manifests/locks.
It checks every workspace member, local path dependency, workspace declaration,
and normal/build/dev/target dependency, independent of the host or features.
Inheritance from nested or unconfirmed external workspaces fails explicitly.
Unused workspace declarations and non-member dev dependencies need a current
requirement, but have no edge in the consuming workspace's lockfile.

Only crates.io is supported; Git, alternative registries, patches and source
replacement are rejected. This checks repository inputs, not external Cargo
configuration or environment overrides. Python icons, Actions and toolchains
have separate upgrade procedures. Python 3.11+ and its standard library suffice.

Cargo's x.y requirement is a caret range, *not* an exact minor constraint:
https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
We separately require the latest major/minor AND the latest direct lock version.
Cargo.lock erases aliases/targets: every owner edge to the named package must
be latest. Ambiguous or missing edges fail, including optional dependencies.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, replace
from datetime import datetime, timezone
import glob
import hashlib
import json
import math
from pathlib import Path
import re
import sys
import tomllib
from typing import Callable
import urllib.error
import urllib.request


API_ROOT = "https://crates.io/api/v1/crates/"
LOCK_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
NAME = re.compile(r"[A-Za-z][A-Za-z0-9_-]{0,63}\Z", re.ASCII)
MINOR = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z", re.ASCII)
SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?\Z",
    re.ASCII,
)
KINDS = ("dependencies", "build-dependencies", "dev-dependencies")
LEGACY_KINDS = {"build_dependencies": "build-dependencies",
                "dev_dependencies": "dev-dependencies"}
MAX_BYTES = 16 * 1024 * 1024
MAX_JOBS = 8


class CheckFailure(Exception):
    """A fixed diagnostic code, never an exception or input string."""


@dataclass(frozen=True)
class Version:
    """Validated SemVer; stable precedence ignores build metadata."""

    text: str
    core: tuple[int, int, int]
    prerelease: bool

    @classmethod
    def parse(cls, value: object) -> Version:
        match = SEMVER.fullmatch(value) if isinstance(value, str) else None
        if not match:
            raise CheckFailure("invalid_version")
        pre = match[4]
        if pre and any(p.isdigit() and len(p) > 1 and p.startswith("0")
                       for p in pre.split(".")):
            raise CheckFailure("invalid_version")
        return cls(value, tuple(int(match[i]) for i in (1, 2, 3)), pre is not None)


def cargo_accepts(requirement: str, version: Version) -> bool:
    """Evaluate the allowed x.y subset of Cargo caret requirements.

    1.2 permits <2.0.0; 0.2 permits <0.3.0; notably 0.0 permits <0.1.0,
    because its omitted patch is unconstrained. Prereleases are never admitted.
    """
    match = MINOR.fullmatch(requirement)
    if not match:
        raise CheckFailure("requirement_must_be_x_y")
    major, minor = int(match[1]), int(match[2])
    ceiling = (major + 1, 0, 0) if major else (0, minor + 1, 0)
    return not version.prerelease and (major, minor, 0) <= version.core < ceiling


def valid_name(value: object) -> str:
    """Keep names safe for official URL construction and diagnostic output."""
    if not isinstance(value, str) or not NAME.fullmatch(value):
        raise CheckFailure("invalid_package_name")
    return value


def table(value: object) -> dict:
    if not isinstance(value, dict):
        raise CheckFailure("invalid_manifest_table")
    return value


class Snapshot:
    """Detect changed inputs while network requests are in flight."""

    def __init__(self) -> None:
        self.digests: dict[Path, bytes | None] = {}

    def read(self, path: Path, *, optional: bool = False) -> dict:
        try:
            if optional and not path.exists():
                if self.digests.get(path) is not None:
                    raise CheckFailure("inputs_changed_during_check")
                self.digests[path] = None
                return {}
            with path.open("rb") as stream:
                data = stream.read(MAX_BYTES + 1)
            if len(data) > MAX_BYTES:
                raise CheckFailure("input_too_large")
            digest = hashlib.sha256(data).digest()
            if path in self.digests and self.digests[path] != digest:
                raise CheckFailure("inputs_changed_during_check")
            self.digests[path] = digest
            return tomllib.loads(data.decode("utf-8"))
        except (OSError, ValueError, UnicodeError):
            raise CheckFailure("input_unreadable_or_invalid_toml") from None

    def verify(self) -> None:
        for path, digest in self.digests.items():
            try:
                if digest is None and not path.exists():
                    continue
                with path.open("rb") as stream:
                    data = stream.read(MAX_BYTES + 1)
                if digest is None or hashlib.sha256(data).digest() != digest:
                    raise CheckFailure("inputs_changed_during_check")
            except OSError:
                raise CheckFailure("inputs_changed_during_check") from None


@dataclass(frozen=True)
class Requirement:
    """One declaration/use, without exposing paths or target expressions."""

    owner: str | None
    owner_version: str | None
    alias: str
    name: str
    version: str
    kind: str
    target_specific: bool
    inherited: bool = False
    lock_required: bool = True

    def report(self) -> dict:
        return {
            "owner": self.owner or "workspace",
            "alias": self.alias,
            "package": self.name,
            "requirement": self.version,
            "kind": self.kind,
            "target_specific": self.target_specific,
            "inherited": self.inherited,
            "url": API_ROOT + self.name,
        }


@dataclass
class Inventory:
    requirements: list[Requirement]
    packages: set[tuple[str, str]]
    workspace_members: int
    locked_packages: set[tuple[str, str]]


def dependency_tables(data: dict):
    """Yield all dependency tables, without evaluating target predicates."""
    def in_scope(scope: dict, target: bool):
        for old, new in LEGACY_KINDS.items():
            if old in scope and new in scope:
                raise CheckFailure("duplicate_dependency_table")
        for key in (*KINDS, *LEGACY_KINDS):
            if key in scope:
                yield LEGACY_KINDS.get(key, key), target, table(scope[key])

    yield from in_scope(data, False)
    for scope in table(data.get("target", {})).values():
        yield from in_scope(table(scope), True)


def reject_overrides(data: dict) -> None:
    if any(data.get(key) for key in ("patch", "replace")):
        raise CheckFailure("dependency_overrides_not_supported")


def validate_dependency_options(spec: dict) -> None:
    """Reject malformed options even when they don't affect version selection."""
    for key in ("optional", "default-features", "default_features", "public"):
        if key in spec and type(spec[key]) is not bool:
            raise CheckFailure("invalid_dependency_option")
    if "features" in spec:
        features = spec["features"]
        if not isinstance(features, list) or any(
                not isinstance(feature, str) for feature in features):
            raise CheckFailure("invalid_dependency_option")
    if "default-features" in spec and "default_features" in spec:
        raise CheckFailure("duplicate_dependency_option")
    for key in ("path", "version"):
        if key in spec and (not isinstance(spec[key], str) or not spec[key]):
            raise CheckFailure("invalid_dependency_option")


def dependency_spec(alias: str, raw: object) -> tuple[str, dict]:
    valid_name(alias)
    spec = {"version": raw} if isinstance(raw, str) else table(raw)
    if any(key in spec for key in ("git", "branch", "tag", "rev", "registry-index")):
        raise CheckFailure("git_or_private_dependency_forbidden")
    if "registry" in spec and spec["registry"] != "crates-io":
        raise CheckFailure("alternative_registry_forbidden")
    allowed = {"version", "package", "path", "registry", "optional", "features",
               "default-features", "default_features", "public"}
    if set(spec) - allowed:
        raise CheckFailure("unsupported_dependency_specification")
    validate_dependency_options(spec)
    return valid_name(spec.get("package", alias)), spec


def collect_dependencies(root: Path, snapshot: Snapshot) -> Inventory:
    """Resolve workspace inheritance and the local path graph without Cargo."""
    root = root.resolve()
    root_file = root / "Cargo.toml"
    root_data = snapshot.read(root_file)
    workspace = table(root_data.get("workspace", {}))
    workspace_deps = table(workspace.get("dependencies", {}))
    workspace_package = table(workspace.get("package", {}))
    reject_overrides(root_data)

    # Never claim crates.io checking covers a repository's source replacement.
    for filename in ("config", "config.toml"):
        config = snapshot.read(root / ".cargo" / filename, optional=True)
        reject_overrides(config)
        if config.get("source") or config.get("registries") or config.get("paths"):
            raise CheckFailure("cargo_source_override_forbidden")
        registry = table(config.get("registry", {}))
        if registry.get("default", "crates-io") != "crates-io" or "index" in registry:
            raise CheckFailure("cargo_source_override_forbidden")

    def expanded(key: str) -> set[Path]:
        entries = workspace.get(key, [])
        if not isinstance(entries, list) or any(not isinstance(p, str) for p in entries):
            raise CheckFailure("invalid_workspace_members")
        result: set[Path] = set()
        for entry in entries:
            # Treat the root literally and include leading-dot members. The
            # member pattern alone supplies glob syntax, not checkout names.
            matches = glob.glob(entry, root_dir=root, recursive=True, include_hidden=True)
            if not matches and key == "members":
                raise CheckFailure("workspace_member_not_found")
            for match in matches:
                result.add((root / match).resolve() / "Cargo.toml")
        return result

    excluded = expanded("exclude")
    members = expanded("members") - excluded
    if "package" in root_data:
        members.add(root_file)
    if not members:
        raise CheckFailure("empty_workspace")
    requirements: list[Requirement] = []
    packages: set[tuple[str, str]] = set()
    package_paths: dict[tuple[str, str], Path] = {}
    path_dependencies: dict[Path, list[tuple[str, Path]]] = {}

    # Declarations are checked even when no member inherits them. A local path
    # declaration alone doesn't create an actual dependency or a lock edge.
    for alias, raw in workspace_deps.items():
        name, spec = dependency_spec(alias, raw)
        if "optional" in spec:
            raise CheckFailure("optional_workspace_declaration_forbidden")
        if "path" not in spec:
            requirement = spec.get("version")
            if not isinstance(requirement, str) or not MINOR.fullmatch(requirement):
                raise CheckFailure("requirement_must_be_x_y")
            requirements.append(Requirement(None, None, alias, name, requirement,
                                            "workspace.dependencies", False))

    pending = list(sorted(members))
    seen: set[Path] = set()
    while pending:
        manifest = pending.pop(0)
        if manifest in seen:
            continue
        seen.add(manifest)
        data = snapshot.read(manifest)
        reject_overrides(data)
        if manifest != root_file and "workspace" in data:
            raise CheckFailure("nested_workspace_not_supported")
        package = table(data.get("package"))
        name = valid_name(package.get("name"))
        owner_workspace = package.get("workspace")
        if owner_workspace is not None:
            if not isinstance(owner_workspace, str):
                raise CheckFailure("invalid_workspace_inheritance")
            if (manifest.parent / owner_workspace).resolve() != root:
                raise CheckFailure("external_workspace_not_supported")
        inheritance_checked = False

        def validate_inheritance_owner():
            """Never silently borrow declarations from another workspace."""
            nonlocal inheritance_checked
            if inheritance_checked:
                return
            if owner_workspace is None and manifest != root_file:
                if not manifest.is_relative_to(root):
                    raise CheckFailure("external_workspace_inheritance_not_supported")
                for ancestor in manifest.parent.parents:
                    if ancestor == root:
                        break
                    parent = snapshot.read(ancestor / "Cargo.toml", optional=True)
                    if "workspace" in parent:
                        raise CheckFailure("nested_workspace_inheritance_not_supported")
            inheritance_checked = True

        version = package.get("version", "0.0.0")
        if isinstance(version, dict):
            validate_inheritance_owner()
            if (set(version) != {"workspace"} or version.get("workspace") is not True
                    or "version" not in workspace_package):
                raise CheckFailure("invalid_workspace_inheritance")
            version = workspace_package["version"]
        version = Version.parse(version).text
        if (name, version) in packages:
            raise CheckFailure("duplicate_local_package")
        packages.add((name, version))
        package_paths[name, version] = manifest
        path_dependencies[manifest] = []

        for kind, target, dependencies in dependency_tables(data):
            for alias, raw in dependencies.items():
                inherited = isinstance(raw, dict) and "workspace" in raw
                if inherited:
                    validate_inheritance_owner()
                    allowed = {"workspace", "features", "optional", "default-features",
                               "default_features", "public"}
                    if raw["workspace"] is not True or set(raw) - allowed:
                        raise CheckFailure("invalid_workspace_inheritance")
                    validate_dependency_options(raw)
                    if alias not in workspace_deps:
                        raise CheckFailure("missing_workspace_dependency")
                    raw = workspace_deps[alias]
                dependency, spec = dependency_spec(alias, raw)
                if "path" in spec:
                    path = spec["path"]
                    if not isinstance(path, str) or not path:
                        raise CheckFailure("invalid_path_dependency")
                    base = root if inherited else manifest.parent
                    dependency_file = (base / path).resolve() / "Cargo.toml"
                    pending.append(dependency_file)
                    path_dependencies[manifest].append((kind, dependency_file))
                    continue
                requirement = spec.get("version")
                if not isinstance(requirement, str) or not MINOR.fullmatch(requirement):
                    raise CheckFailure("requirement_must_be_x_y")
                requirements.append(Requirement(name, version, alias, dependency,
                                                requirement, kind, target, inherited))

    # Inspect the whole declaration graph, but derive membership separately.
    # An excluded package cannot auto-enrol its own path dependencies. Resolve
    # membership first so traversal order cannot change whose dev edges count.
    pending = list(members) if "workspace" in root_data else []
    while pending:
        for _, dependency_file in path_dependencies[pending.pop()]:
            if (dependency_file.is_relative_to(root) and dependency_file not in excluded
                    and dependency_file not in members):
                members.add(dependency_file)
                pending.append(dependency_file)

    locked_manifests = set(members)
    pending = list(members)
    while pending:
        manifest = pending.pop()
        for kind, dependency_file in path_dependencies[manifest]:
            if kind == "dev-dependencies" and manifest not in members:
                continue
            if dependency_file not in locked_manifests:
                locked_manifests.add(dependency_file)
                pending.append(dependency_file)

    def requires_lock(requirement: Requirement) -> bool:
        if requirement.owner is None:
            return False
        manifest = package_paths[requirement.owner, requirement.owner_version]
        return (manifest in locked_manifests
                and (requirement.kind != "dev-dependencies" or manifest in members))

    requirements = [replace(requirement, lock_required=requires_lock(requirement))
                    for requirement in requirements]
    locked_packages = {identity for identity, manifest in package_paths.items()
                       if manifest in locked_manifests}
    return Inventory(requirements, packages, len(members), locked_packages)


@dataclass(frozen=True)
class LockedPackage:
    name: str
    version: Version
    source: str | None
    dependencies: tuple[str, ...]


class LockGraph:
    """Resolve direct edges, rather than searching for a version globally."""

    def __init__(self, data: dict, inventory: Inventory) -> None:
        if type(data.get("version")) is not int or data["version"] not in (3, 4):
            raise CheckFailure("unsupported_lock_format")
        records = data.get("package")
        if not isinstance(records, list):
            raise CheckFailure("invalid_lock_packages")
        self.packages: dict[str, list[LockedPackage]] = {}
        identities = set()
        for record in records:
            if not isinstance(record, dict):
                raise CheckFailure("invalid_lock_package")
            name = valid_name(record.get("name"))
            version = Version.parse(record.get("version"))
            source = record.get("source")
            if source is not None and source != LOCK_SOURCE:
                raise CheckFailure("non_crates_io_lock_source_forbidden")
            dependencies = record.get("dependencies", [])
            if not isinstance(dependencies, list) or any(
                    not isinstance(d, str) for d in dependencies):
                raise CheckFailure("invalid_lock_dependencies")
            identity = name, version.text, source
            if identity in identities:
                raise CheckFailure("duplicate_lock_package")
            identities.add(identity)
            self.packages.setdefault(name, []).append(
                LockedPackage(name, version, source, tuple(dependencies)))
        self.edges: dict[tuple[str, str], list[LockedPackage]] = {}
        for name, version in inventory.locked_packages:
            owners = [p for p in self.packages.get(name, [])
                      if p.version.text == version and p.source is None]
            if len(owners) != 1:
                raise CheckFailure("local_package_missing_from_lock")
            self.edges[name, version] = [self.resolve(d) for d in owners[0].dependencies]

    def resolve(self, edge: str) -> LockedPackage:
        # Cargo uses name, optional version, then optional parenthesized source.
        match = re.fullmatch(r"([^ ()]+)(?: ([^ ()]+))?(?: \(([^()]+)\))?", edge)
        if not match:
            raise CheckFailure("invalid_lock_edge")
        name = valid_name(match[1])
        version, source = match[2], match[3]
        if version:
            Version.parse(version)
        if source is not None and source != LOCK_SOURCE:
            raise CheckFailure("non_crates_io_lock_source_forbidden")
        candidates = [p for p in self.packages.get(name, [])
                      if (version is None or p.version.text == version)
                      and (source is None or p.source == source)]
        if len(candidates) != 1:
            raise CheckFailure("missing_or_ambiguous_lock_edge")
        return candidates[0]

    def direct_versions(self, requirement: Requirement) -> list[str]:
        if not requirement.lock_required:
            return []
        named = [p for p in self.edges[requirement.owner, requirement.owner_version]
                 if p.name == requirement.name]
        # A registry dependency shadowed by a local package must not pass.
        if any(p.source != LOCK_SOURCE for p in named):
            raise CheckFailure("direct_dependency_is_not_registry")
        if not any(cargo_accepts(requirement.version, p.version) for p in named):
            raise CheckFailure("compatible_direct_lock_edge_missing")
        # Do not discard incompatible versions: an obsolete owner edge can sit
        # alongside a current edge after a partial/hand-edited lock update.
        return sorted({p.version.text for p in named})


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def http_error_302(self, req, fp, code, msg, headers):
        # Reject before urllib parses the untrusted Location header, and retain
        # ownership of the response even on this exceptional path.
        try:
            fp.close()
        except Exception:
            pass
        raise CheckFailure("registry_redirect_rejected")

    http_error_301 = http_error_303 = http_error_307 = http_error_308 = http_error_302


def fetch_json(url: str, timeout: float) -> dict:
    """GET only official URLs, with a timeout, size limit and no redirects.

    Environment proxies and authentication handlers are intentionally unused.
    Never propagate network exceptions or response bodies into diagnostics.
    """
    if not url.startswith(API_ROOT) or not NAME.fullmatch(url[len(API_ROOT):]):
        raise CheckFailure("non_official_url_rejected")
    try:
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
        request = urllib.request.Request(
            url, method="GET",
            headers={"Accept": "application/json", "User-Agent": "KeelShell-dependency-check/1"},
        )
        with opener.open(request, timeout=timeout) as response:
            if response.status != 200 or response.geturl() != url:
                raise CheckFailure("registry_http_failure")
            body = response.read(MAX_BYTES + 1)
        if len(body) > MAX_BYTES:
            raise CheckFailure("registry_response_too_large")
        result = json.loads(body)
        if not isinstance(result, dict):
            raise CheckFailure("invalid_registry_metadata")
        return result
    except CheckFailure:
        raise
    except urllib.error.HTTPError as error:
        # HTTPError owns a response stream. Close it so a later resource warning
        # cannot print its URL/message after our redacted diagnostic.
        try:
            error.close()
        except Exception:
            pass
        raise CheckFailure("registry_request_failed") from None
    except Exception:
        raise CheckFailure("registry_request_failed") from None


def latest_stable(data: object) -> tuple[str, ...]:
    """Select from version records; max_version may be yanked or a prerelease.

    Equal-precedence build variants are all reported and accepted, but only if
    that exact, non-yanked version was present in the official response.
    """
    if not isinstance(data, dict) or not isinstance(data.get("versions"), list):
        raise CheckFailure("invalid_registry_metadata")
    stable: list[Version] = []
    seen: set[str] = set()
    for record in data["versions"]:
        if not isinstance(record, dict) or type(record.get("yanked")) is not bool:
            raise CheckFailure("invalid_registry_metadata")
        try:
            version = Version.parse(record.get("num"))
        except CheckFailure:
            raise CheckFailure("invalid_registry_metadata") from None
        if version.text in seen:
            raise CheckFailure("invalid_registry_metadata")
        seen.add(version.text)
        if not record["yanked"] and not version.prerelease:
            stable.append(version)
    if not stable:
        raise CheckFailure("no_stable_non_yanked_release")
    highest = max(v.core for v in stable)
    return tuple(sorted(v.text for v in stable if v.core == highest))


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def run_check(root: Path, *, fetch: Callable[[str, float], dict] = fetch_json,
              timeout: float = 15.0, jobs: int = 4) -> dict:
    """Return a safe report. Tests inject fetch; failures always fail the report."""
    report = {"schema_version": 1, "checked_at_utc": utc_now(), "status": "failed",
              "rust": [], "errors": []}
    snapshot = Snapshot()
    try:
        if (not math.isfinite(timeout) or not 0 < timeout <= 120
                or type(jobs) is not int or not 1 <= jobs <= MAX_JOBS):
            raise CheckFailure("invalid_request_limits")
        inventory = collect_dependencies(root, snapshot)
        lock = LockGraph(snapshot.read(root / "Cargo.lock"), inventory)
        names = sorted({r.name for r in inventory.requirements})
        report["counts"] = {"registry_packages": len(names),
                            "requirements": len(inventory.requirements),
                            "local_packages": len(inventory.packages),
                            "workspace_members": inventory.workspace_members}

        def query(name: str):
            try:
                return latest_stable(fetch(API_ROOT + name, timeout)), None
            except CheckFailure:
                # Fetch implementations are injectable, so their exception text
                # is untrusted too. Only our metadata parser exposes fixed codes.
                return (), "registry_check_failed"
            except Exception:
                return (), "registry_check_failed"

        with ThreadPoolExecutor(max_workers=jobs) as pool:
            releases = dict(zip(names, pool.map(query, names)))
        for requirement in inventory.requirements:
            row = requirement.report()
            latest, failure = releases[requirement.name]
            row.update(latest_stable_versions=list(latest), locked_versions=[],
                       errors=[], status="failed")
            if failure:
                row["errors"].append(failure)
            else:
                latest_version = Version.parse(latest[0])
                latest_minor = ".".join(str(n) for n in latest_version.core[:2])
                row["latest_requirement"] = latest_minor
                row["requirement_accepts_latest"] = cargo_accepts(
                    requirement.version, latest_version)
                if requirement.version != latest_minor:
                    row["errors"].append("requirement_not_latest_major_minor")
            try:
                versions = lock.direct_versions(requirement)
                row["locked_versions"] = versions
                row["lock_scope"] = "direct" if requirement.lock_required else "declaration_only"
                if latest and any(version not in latest for version in versions):
                    row["errors"].append("direct_lock_not_latest")
            except CheckFailure as error:
                row["errors"].append(str(error))
            row["status"] = "failed" if row["errors"] else "passed"
            report["rust"].append(row)
        snapshot.verify()
        if collect_dependencies(root, snapshot) != inventory:
            raise CheckFailure("inputs_changed_during_check")
        snapshot.verify()
        if all(row["status"] == "passed" for row in report["rust"]):
            report["status"] = "passed"
    except CheckFailure as error:
        report["errors"].append(str(error))
    except Exception:
        # Do not leak a filename, source URL, environment or credential through
        # a traceback, even for unexpected malformed input or internal failures.
        report["errors"].append("dependency_check_failed")
    report["completed_at_utc"] = utc_now()
    return report


class SafeArgumentParser(argparse.ArgumentParser):
    def error(self, message):
        self.exit(2, "Dependency check: invalid_arguments\n")


def main(argv: list[str] | None = None, *,
         fetch: Callable[[str, float], dict] = fetch_json) -> int:
    parser = SafeArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1],
                        help="Workspace root (defaults to this script's repository)")
    parser.add_argument("--timeout", type=float, default=15.0,
                        help="Timeout in seconds per request, >0 and <=120 (default: 15)")
    parser.add_argument("--jobs", type=int, default=4,
                        help="Concurrent requests, 1..8 (default: 4)")
    parser.add_argument("--json-report", type=Path,
                        help="Write a new .json report; existing files are never overwritten")
    options = parser.parse_args(argv)
    report = run_check(options.root, fetch=fetch, timeout=options.timeout, jobs=options.jobs)
    if options.json_report:
        try:
            if options.json_report.suffix.lower() != ".json":
                raise CheckFailure("report_requires_json_extension")
            # Exclusive creation also rejects existing symlinks and input files.
            with options.json_report.open("x", encoding="utf-8") as stream:
                json.dump(report, stream, indent=2, ensure_ascii=True)
                stream.write("\n")
        except Exception:
            print("Dependency check: report_write_failed", file=sys.stderr)
            return 1
    print(f"Dependency check: {report['status']}")
    for error in report["errors"]:
        print(f"  {error}")
    for row in report["rust"]:
        if row["status"] == "failed":
            print(f"  {row['owner']}/{row['alias']}: {', '.join(row['errors'])}")
    print(f"Checked {len(report['rust'])} Rust declarations/uses at {report['checked_at_utc']}")
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("Dependency check: interrupted", file=sys.stderr)
        sys.exit(130)
    except Exception:
        print("Dependency check: failed", file=sys.stderr)
        sys.exit(1)
