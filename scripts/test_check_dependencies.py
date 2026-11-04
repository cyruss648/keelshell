"""Offline dependency-checker tests. No registry or Cargo subprocess is used."""

from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch
import urllib.error

import check_dependencies as check


SOURCE = check.LOCK_SOURCE
SECRET = "PRIVATE_TOKEN_DO_NOT_DISCLOSE"


def metadata(*versions):
    """Strings are live versions; (version, yanked) specifies withdrawal."""
    return {"crate": {"max_version": "999.0.0"},
            "versions": [{"num": v if isinstance(v, str) else v[0],
                          "yanked": False if isinstance(v, str) else v[1]}
                         for v in versions]}


def package(name, version, dependencies=(), *, registry=False):
    result = {"name": name, "version": version, "dependencies": list(dependencies)}
    if registry:
        result["source"] = SOURCE
    return result


class OfflineCase(unittest.TestCase):
    def setUp(self):
        # Even an accidentally non-injected fetch cannot resolve DNS or connect.
        for target in ("socket.getaddrinfo", "socket.socket.connect", "socket.socket.connect_ex"):
            network = patch(target, side_effect=AssertionError("network forbidden"))
            network.start()
            self.addCleanup(network.stop)
        self.scratch = tempfile.TemporaryDirectory(prefix="keelshell-dependency-test-")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def lock(self, *packages):
        rows = ["version = 4\n"]
        for record in packages:
            rows.append("[[package]]")
            rows.extend(f"{key} = {json.dumps(value)}" for key, value in record.items())
        self.write("Cargo.lock", "\n".join(rows) + "\n")

    def basic(self, requirement="1.2", locked="1.2.3"):
        self.write("Cargo.toml", f"""
[package]
name = "app"
version = "0.1.0"
[dependencies]
sample = "{requirement}"
""")
        self.lock(package("app", "0.1.0", ["sample"]),
                  package("sample", locked, registry=True))

    def run_check(self, *versions, fetch=None, **kwargs):
        if fetch is None:
            fetch = lambda url, timeout: metadata(*(versions or ("1.2.3",)))
        return check.run_check(self.root, fetch=fetch, **kwargs)

    def assert_failed(self, report, error):
        self.assertEqual("failed", report["status"])
        codes = report["errors"] + [code for row in report["rust"] for code in row["errors"]]
        self.assertIn(error, codes)


class VersionTests(unittest.TestCase):
    def test_cargo_caret_ranges_including_zero_minor(self):
        cases = [
            ("1.2", "1.2.0", True), ("1.2", "1.99.9", True),
            ("1.2", "1.1.9", False), ("1.2", "2.0.0", False),
            ("0.2", "0.2.99", True), ("0.2", "0.3.0", False),
            ("0.2", "0.1.99", False), ("0.0", "0.0.99", True),
            ("0.0", "0.1.0", False), ("0.0", "0.0.1-alpha", False),
            ("1.2", "1.2.3+build.2", True), ("1.2", "1.3.0-rc.1", False),
        ]
        for requirement, version, accepted in cases:
            with self.subTest(requirement=requirement, version=version):
                self.assertEqual(accepted, check.cargo_accepts(
                    requirement, check.Version.parse(version)))

    def test_latest_ignores_max_version_prereleases_and_yanks(self):
        result = check.latest_stable(metadata(
            "0.9.99", "1.0.0", "1.10.2", "1.9.99",
            "2.0.0-rc.1", ("3.0.0", True)))
        self.assertEqual(("1.10.2",), result)

    def test_stable_build_metadata_does_not_change_precedence(self):
        result = check.latest_stable(metadata("1.2.0+spec-1.1.0",
                                             "1.2.0+other", "1.1.99+zzz"))
        self.assertEqual(("1.2.0+other", "1.2.0+spec-1.1.0"), result)

    def test_invalid_semver_rejected(self):
        for value in ("1.2", "01.2.3", "1.02.3", "1.2.3-01", "v1.2.3",
                      "1.2.3+", "1.2.3-rc..1", "1.2.3\n", 123):
            with self.subTest(value=value), self.assertRaises(check.CheckFailure):
                check.Version.parse(value)

    def test_no_stable_release_fails(self):
        for data in (metadata(), metadata("1.0.0-alpha"), metadata(("1.0.0", True))):
            with self.subTest(data=data), self.assertRaisesRegex(
                    check.CheckFailure, "no_stable_non_yanked_release"):
                check.latest_stable(data)

    def test_invalid_metadata_is_not_silently_ignored(self):
        for data in (None, {}, {"versions": {}}, {"versions": [None]},
                     {"versions": [{"num": "1.0.0"}]},
                     {"versions": [{"num": "1.0.0", "yanked": "false"}]},
                     metadata("1.0.0", "malformed"), metadata("1.0.0", "1.0.0")):
            with self.subTest(data=data), self.assertRaises(check.CheckFailure):
                check.latest_stable(data)


class CollectionTests(OfflineCase):
    def test_all_scopes_inheritance_renames_root_globs_and_auto_members(self):
        self.write("Cargo.toml", """
[workspace]
members = ["apps/*", "tools/nested/cli"]
exclude = ["apps/ignored"]
[workspace.package]
version = "0.1.0"
[workspace.dependencies]
renamed = { package = "actual", version = "2.3" }
unused = "1.4"
helper = { path = "libs/helper" }
[package]
name = "root"
version.workspace = true
[dependencies]
rootdep = "3.0"
""")
        self.write("apps/one/Cargo.toml", """
[package]
name = "one"
version.workspace = true
[dependencies]
renamed.workspace = true
helper.workspace = true
[build-dependencies]
builddep = "0.9"
[dev-dependencies]
devdep = "5.1"
[target.'cfg(windows)'.dependencies]
windep = { version = "0.2", optional = true }
[target.'cfg(unix)'.build-dependencies]
unixbuild = "1.0"
[target.'cfg(unix)'.dev-dependencies]
renamed.workspace = true
""")
        self.write("tools/nested/cli/Cargo.toml", """
[package]
name = "cli"
version = "0.2.0"
[dependencies]
renamed.workspace = true
""")
        self.write("libs/helper/Cargo.toml", """
[package]
name = "helper"
version.workspace = true
[dev_dependencies]
legacy = "0.0"
""")
        self.write("apps/ignored/Cargo.toml", "invalid TOML does not get read")
        inventory = check.collect_dependencies(self.root, check.Snapshot())
        self.assertEqual(4, inventory.workspace_members)
        self.assertEqual({("root", "0.1.0"), ("one", "0.1.0"),
                          ("cli", "0.2.0"), ("helper", "0.1.0")}, inventory.packages)
        self.assertEqual(11, len(inventory.requirements))
        actual = [r for r in inventory.requirements if r.name == "actual"]
        self.assertEqual(4, len(actual))
        self.assertEqual(3, sum(r.inherited for r in actual))
        self.assertEqual(3, sum(r.target_specific for r in inventory.requirements))
        self.assertEqual({"dependencies", "build-dependencies", "dev-dependencies"},
                         {r.kind for r in inventory.requirements if r.owner})
        self.assertFalse(any(r.name == "helper" for r in inventory.requirements))

    def test_unused_workspace_declaration_checked_without_a_lock_edge(self):
        self.write("Cargo.toml", """
[workspace]
members = ["app"]
[workspace.dependencies]
unused = "1.2"
""")
        self.write("app/Cargo.toml", '[package]\nname="app"\nversion="0.1.0"\n')
        self.lock(package("app", "0.1.0"))
        report = self.run_check()
        self.assertEqual("passed", report["status"])
        self.assertEqual("declaration_only", report["rust"][0]["lock_scope"])
        self.assert_failed(self.run_check("1.3.0"), "requirement_not_latest_major_minor")

    def test_globs_include_hidden_members_and_do_not_glob_the_root_path(self):
        self.root = self.root / "checkout[one]"
        self.write("Cargo.toml", '[workspace]\nmembers=["crates/*"]')
        for name, directory in (("visible", "visible"), ("hidden", ".hidden")):
            self.write(f"crates/{directory}/Cargo.toml",
                       f'[package]\nname="{name}"\nversion="0.1.0"\n'
                       '[dependencies]\nsample="1.2"')
        inventory = check.collect_dependencies(self.root, check.Snapshot())
        self.assertEqual({("visible", "0.1.0"), ("hidden", "0.1.0")}, inventory.packages)
        self.assertEqual(2, len(inventory.requirements))

    def test_local_path_closure_is_checked_even_when_excluded_from_members(self):
        self.write("Cargo.toml", """
[workspace]
exclude = ["helper"]
[package]
name = "app"
version = "0.1.0"
[dependencies]
helper = { path = "helper", version = "0.1.0" }
""")
        self.write("helper/Cargo.toml", """
[package]
name = "helper"
version = "0.1.0"
[dependencies]
sample = "1.2"
""")
        self.lock(package("app", "0.1.0", ["helper"]),
                  package("helper", "0.1.0", ["sample"]),
                  package("sample", "1.2.3", registry=True))
        report = self.run_check()
        self.assertEqual("passed", report["status"])
        self.assertEqual(1, report["counts"]["workspace_members"])
        self.assertEqual(2, report["counts"]["local_packages"])
        self.assertEqual("helper", report["rust"][0]["owner"])

    def test_duplicate_local_paths_and_cycles_do_not_repeat_collection(self):
        self.write("Cargo.toml", """
[package]
name = "app"
version = "0.1.0"
[dependencies]
helper = { path = "helper" }
[dev-dependencies]
helper = { path = "helper" }
""")
        self.write("helper/Cargo.toml", """
[package]
name = "helper"
version = "0.1.0"
[dev-dependencies]
app = { path = ".." }
""")
        inventory = check.collect_dependencies(self.root, check.Snapshot())
        self.assertEqual(2, len(inventory.packages))
        self.assertEqual(1, inventory.workspace_members)

    def excluded_helper(self, dependency_table):
        self.write("Cargo.toml", """
[workspace]
exclude = ["helper"]
[package]
name = "app"
version = "0.1.0"
[dependencies]
helper = { path = "helper" }
""")
        self.write("helper/Cargo.toml",
                   '[package]\nname="helper"\nversion="0.1.0"\n' + dependency_table)

    def test_excluded_package_dev_requirements_do_not_require_lock_edges(self):
        for heading in ("dev-dependencies", "dev_dependencies",
                        "target.'cfg(windows)'.dev-dependencies"):
            with self.subTest(heading=heading):
                self.excluded_helper(f'[{heading}]\nsample="1.2"\n')
                self.lock(package("app", "0.1.0", ["helper"]),
                          package("helper", "0.1.0"))
                report = self.run_check()
                self.assertEqual("passed", report["status"])
                self.assertEqual("helper", report["rust"][0]["owner"])
                self.assertEqual("declaration_only", report["rust"][0]["lock_scope"])
                self.assert_failed(self.run_check("1.3.0"), "requirement_not_latest_major_minor")

    def test_excluded_package_dev_path_closure_is_declaration_only(self):
        self.excluded_helper('[dev-dependencies]\ntoy={path="../toy"}\n')
        self.write("toy/Cargo.toml", """
[package]
name = "toy"
version = "0.1.0"
[dependencies]
sample = "1.2"
""")
        self.lock(package("app", "0.1.0", ["helper"]), package("helper", "0.1.0"))
        report = self.run_check()
        self.assertEqual("passed", report["status"])
        self.assertEqual(1, report["counts"]["workspace_members"])
        self.assertEqual(3, report["counts"]["local_packages"])
        self.assertEqual("declaration_only", report["rust"][0]["lock_scope"])
        self.assert_failed(self.run_check("1.3.0"), "requirement_not_latest_major_minor")

    def test_excluded_package_normal_path_closure_does_not_auto_enrol_members(self):
        for heading in ("dependencies", "build-dependencies"):
            with self.subTest(heading=heading):
                self.excluded_helper(f'[{heading}]\ntoy={{path="../toy"}}\n')
                self.write("toy/Cargo.toml", """
[package]
name = "toy"
version = "0.1.0"
[dependencies]
sample = "1.2"
[dev-dependencies]
devsample = "1.2"
""")
                self.lock(package("app", "0.1.0", ["helper"]),
                          package("helper", "0.1.0", ["toy"]),
                          package("toy", "0.1.0", ["sample"]),
                          package("sample", "1.2.3", registry=True))
                report = self.run_check()
                self.assertEqual("passed", report["status"])
                self.assertEqual(1, report["counts"]["workspace_members"])
                self.assertEqual({"sample": "direct", "devsample": "declaration_only"},
                                 {r["package"]: r["lock_scope"] for r in report["rust"]})
                self.lock(package("app", "0.1.0", ["helper"]),
                          package("helper", "0.1.0", ["toy"]),
                          package("toy", "0.1.0"))
                self.assert_failed(self.run_check(), "compatible_direct_lock_edge_missing")

    def test_independently_enrolled_member_keeps_dev_dependency_lock_checks(self):
        self.excluded_helper('[dev-dependencies]\ntoy={path="../toy"}\n')
        root = self.root / "Cargo.toml"
        root.write_text(root.read_text().replace(
            'exclude = ["helper"]', 'exclude = ["helper"]\nmembers = ["toy"]'))
        self.write("toy/Cargo.toml", """
[package]
name = "toy"
version = "0.1.0"
[dev-dependencies]
sample = "1.2"
""")
        self.lock(package("app", "0.1.0", ["helper"]),
                  package("helper", "0.1.0"), package("toy", "0.1.0", ["sample"]),
                  package("sample", "1.2.3", registry=True))
        report = self.run_check()
        self.assertEqual("passed", report["status"])
        self.assertEqual(2, report["counts"]["workspace_members"])
        self.assertEqual("direct", report["rust"][0]["lock_scope"])
        self.lock(package("app", "0.1.0", ["helper"]),
                  package("helper", "0.1.0"), package("toy", "0.1.0"))
        self.assert_failed(self.run_check(), "compatible_direct_lock_edge_missing")

    def test_rejects_non_minor_requirements(self):
        for requirement in ("^1.2", "=1.2", "1.2.3", "*", "1", "01.2", "0.2-beta",
                            "https://private.invalid/" + SECRET):
            with self.subTest(requirement=requirement):
                self.basic(requirement=requirement)
                report = self.run_check()
                self.assert_failed(report, "requirement_must_be_x_y")
                self.assertNotIn(SECRET, json.dumps(report))

    def test_rejects_private_git_registry_and_inheritance_overrides(self):
        specs = [
            '{git="https://user:' + SECRET + '@private.invalid/repo"}',
            '{version="1.2", registry="' + SECRET + '"}',
            '{version="1.2", registry-index="https://' + SECRET + '.invalid"}',
            '{workspace=true, version="1.2"}',
            '{version="1.2", unsupported="' + SECRET + '"}',
        ]
        for spec in specs:
            with self.subTest(spec=spec):
                self.basic()
                self.write("Cargo.toml", '[package]\nname="app"\nversion="0.1.0"\n'
                           '[dependencies]\nsample=' + spec)
                report = self.run_check()
                self.assertEqual("failed", report["status"])
                self.assertNotIn(SECRET, json.dumps(report))

    def test_crates_io_registry_and_package_alias_are_supported(self):
        self.basic()
        self.write("Cargo.toml", """
[package]
name="app"
version="0.1.0"
[dependencies]
renamed={package="sample", version="1.2", registry="crates-io"}
""")
        self.assertEqual("passed", self.run_check()["status"])

    def test_invalid_dependency_option_types_fail_for_all_declarations(self):
        invalid = ('optional="yes"', 'features=42', 'features=[false]',
                   'default-features="no"', 'public=1', 'version=42', 'path=42')
        for scope in ("direct", "workspace", "inherited"):
            for option in invalid:
                with self.subTest(scope=scope, option=option):
                    prefix = '[package]\nname="app"\nversion="0.1.0"\n'
                    # Avoid duplicate TOML keys so this probes option validation.
                    version = '' if option.startswith('version=') else 'version="1.2", '
                    if scope == "direct":
                        content = prefix + '[dependencies]\nsample={' + version + option + '}'
                    elif scope == "workspace":
                        content = prefix + '[workspace.dependencies]\nsample={' + version + option + '}'
                    else:
                        content = (prefix + '[workspace.dependencies]\nsample="1.2"\n'
                                   '[dependencies]\nsample={workspace=true, ' + option + '}')
                    self.write("Cargo.toml", content)
                    report = self.run_check()
                    self.assertEqual("failed", report["status"])
                    self.assertTrue(set(report["errors"]) & {
                        "invalid_dependency_option", "invalid_workspace_inheritance"})

    def test_optional_workspace_declaration_and_duplicate_option_fail(self):
        self.basic()
        self.write("Cargo.toml", """
[package]
name="app"
version="0.1.0"
[workspace.dependencies]
sample={version="1.2", optional=false}
""")
        self.assert_failed(self.run_check(), "optional_workspace_declaration_forbidden")
        self.write("Cargo.toml", """
[package]
name="app"
version="0.1.0"
[dependencies]
sample={version="1.2", default-features=false, default_features=false}
""")
        self.assert_failed(self.run_check(), "duplicate_dependency_option")

    def test_invalid_or_incomplete_workspace_fails_before_fetch(self):
        cases = [
            '[workspace]\nmembers=["missing"]',
            '[workspace]\nmembers=[]',
            '[workspace]\nmembers="app"',
            '[package]\nname="app"\nversion.workspace=true',
            '[package]\nname="app"\n[dependencies]\nsample.workspace=true',
            '[package]\nname="app"\n[dependencies]\nsample={workspace=false}',
            '[package]\nname="app"\n[dependencies]\nsample={optional=true}',
            '[package]\nname="app"\n[target]\nanything=1',
            '[package]\nname="app"\n[dev_dependencies]\n[dev-dependencies]',
            '[package]\nname="app"\nworkspace="../external"',
            'not valid TOML ' + SECRET,
        ]
        for content in cases:
            with self.subTest(content=content):
                self.write("Cargo.toml", content)
                report = self.run_check(fetch=lambda *args: self.fail("unexpected fetch"))
                self.assertEqual("failed", report["status"])
                self.assertNotIn(SECRET, json.dumps(report))

    def test_project_overrides_are_rejected(self):
        self.basic()
        self.write(".cargo/config.toml", '[source.crates-io]\nreplace-with="' + SECRET + '"')
        self.assert_failed(self.run_check(), "cargo_source_override_forbidden")
        (self.root / ".cargo/config.toml").unlink()
        with (self.root / "Cargo.toml").open("a") as stream:
            stream.write('\n[patch.crates-io]\nsample={path="' + SECRET + '"}')
        self.assert_failed(self.run_check(), "dependency_overrides_not_supported")

    def test_nested_workspace_fails_instead_of_using_wrong_inheritance(self):
        self.write("Cargo.toml", '[workspace]\nmembers=["app"]')
        self.write("app/Cargo.toml",
                   '[workspace]\n[package]\nname="app"\nversion="0.1.0"')
        self.assert_failed(self.run_check(), "nested_workspace_not_supported")

    def test_external_implicit_workspace_inheritance_fails_instead_of_false_pass(self):
        self.write("Cargo.toml", """
[workspace]
[workspace.dependencies]
shared={package="current", version="1.2"}
[package]
name="app"
version="0.1.0"
[dependencies]
helper={path="../vendor/helper"}
""")
        # The other workspace and its crate remain inside this test's scratch.
        vendor = self.root / "vendor"
        vendor.mkdir()
        (vendor / "Cargo.toml").write_text("""
[workspace]
members=["helper"]
[workspace.dependencies]
shared={package="obsolete", version="0.9"}
""")
        helper = vendor / "helper"
        helper.mkdir()
        (helper / "Cargo.toml").write_text("""
[package]
name="helper"
version="0.1.0"
[dependencies]
shared.workspace=true
current="1.2"
""")
        app = self.root / "app"
        app.mkdir()
        (self.root / "Cargo.toml").replace(app / "Cargo.toml")
        self.root = app
        self.lock(package("app", "0.1.0", ["helper"]),
                  package("helper", "0.1.0", ["obsolete", "current"]),
                  package("obsolete", "0.9.0", registry=True),
                  package("current", "1.2.3", registry=True))
        report = self.run_check(fetch=lambda *args: self.fail("must reject before fetching"))
        self.assert_failed(report, "external_workspace_inheritance_not_supported")

    def test_nested_ancestor_workspace_inheritance_is_rejected(self):
        self.write("Cargo.toml", """
[workspace]
[workspace.dependencies]
sample="1.2"
[package]
name="app"
version="0.1.0"
[dependencies]
helper={path="vendor/helper"}
""")
        self.write("vendor/Cargo.toml", """
[workspace]
members=["helper"]
[workspace.dependencies]
sample="0.9"
""")
        self.write("vendor/helper/Cargo.toml", """
[package]
name="helper"
version="0.1.0"
[dependencies]
sample.workspace=true
""")
        self.assert_failed(self.run_check(), "nested_workspace_inheritance_not_supported")

    def test_external_package_version_inheritance_requires_confirmed_owner(self):
        app = self.root / "app"
        self.write("app/Cargo.toml", """
[workspace]
members=["../helper"]
[workspace.package]
version="0.1.0"
""")
        self.write("helper/Cargo.toml", """
[package]
name="helper"
version.workspace=true
""")
        self.root = app
        self.assert_failed(self.run_check(), "external_workspace_inheritance_not_supported")
        with (app.parent / "helper/Cargo.toml").open("a") as stream:
            stream.write('workspace="../app"\n')
        self.lock(package("helper", "0.1.0"))
        self.assertEqual("passed", self.run_check()["status"])

    def test_package_version_workspace_requires_literal_boolean_true(self):
        self.write("Cargo.toml", """
[workspace]
members=["app"]
[workspace.package]
version="0.1.0"
""")
        self.lock(package("app", "0.1.0", ["sample"]),
                  package("sample", "1.2.3", registry=True))
        for value in ("1", "1.0", "false", '"true"', "true"):
            with self.subTest(value=value):
                self.write("app/Cargo.toml", '[package]\nname="app"\n'
                           f'version.workspace={value}\n[dependencies]\nsample="1.2"')
                report = self.run_check()
                if value == "true":
                    self.assertEqual("passed", report["status"])
                else:
                    self.assert_failed(report, "invalid_workspace_inheritance")


class LockTests(OfflineCase):
    def test_latest_direct_lock_passes(self):
        self.basic()
        report = self.run_check("1.2.0", "1.2.3", "2.0.0-beta", ("9.0.0", True))
        self.assertEqual("passed", report["status"])
        self.assertEqual(["1.2.3"], report["rust"][0]["locked_versions"])

    def test_compatible_minor_upgrade_still_requires_latest_x_y(self):
        self.basic(locked="1.3.0")
        report = self.run_check("1.3.0")
        self.assert_failed(report, "requirement_not_latest_major_minor")
        self.assertTrue(report["rust"][0]["requirement_accepts_latest"])
        self.assertNotIn("direct_lock_not_latest", report["rust"][0]["errors"])

    def test_patch_upgrade_requires_lock_update(self):
        self.basic(locked="1.2.2")
        self.assert_failed(self.run_check(), "direct_lock_not_latest")

    def test_new_transitive_version_cannot_mask_old_direct_version(self):
        self.basic()
        self.lock(package("app", "0.1.0", ["sample 1.2.2"]),
                  package("sample", "1.2.2", registry=True),
                  package("sample", "1.2.3", registry=True),
                  package("transitive", "0.1.0", ["sample 1.2.3"], registry=True))
        report = self.run_check()
        self.assert_failed(report, "direct_lock_not_latest")
        self.assertEqual(["1.2.2"], report["rust"][0]["locked_versions"])

    def test_two_compatible_direct_edges_require_both_to_be_latest(self):
        self.basic()
        self.lock(package("app", "0.1.0", ["sample 1.2.2", "sample 1.2.3"]),
                  package("sample", "1.2.2", registry=True),
                  package("sample", "1.2.3", registry=True))
        self.assert_failed(self.run_check(), "direct_lock_not_latest")

    def test_source_qualified_edges(self):
        self.basic()
        self.lock(package("app", "0.1.0", [f"sample 1.2.3 ({SOURCE})"]),
                  package("sample", "1.2.3", registry=True))
        self.assertEqual("passed", self.run_check()["status"])

    def test_latest_direct_edge_cannot_hide_incompatible_old_owner_edge(self):
        self.basic()
        for old in ("0.9.0", "1.1.9"):
            with self.subTest(old=old):
                self.lock(package("app", "0.1.0", ["sample 1.2.3", f"sample {old}"]),
                          package("sample", "1.2.3", registry=True),
                          package("sample", old, registry=True))
                report = self.run_check()
                self.assert_failed(report, "direct_lock_not_latest")
                self.assertEqual({old, "1.2.3"}, set(report["rust"][0]["locked_versions"]))

    def test_unqualified_ambiguous_edge_fails(self):
        self.basic()
        self.lock(package("app", "0.1.0", ["sample"]),
                  package("sample", "1.2.3", registry=True),
                  package("sample", "1.0.0", registry=True))
        self.assert_failed(self.run_check(), "missing_or_ambiguous_lock_edge")

    def test_yanked_lock_and_prerelease_lock_do_not_pass(self):
        self.basic(locked="1.2.4")
        self.assert_failed(self.run_check("1.2.3", ("1.2.4", True)), "direct_lock_not_latest")
        self.basic(locked="1.2.4-beta")
        self.assert_failed(self.run_check(), "compatible_direct_lock_edge_missing")

    def test_zero_major_wrong_minor_is_not_compatible(self):
        self.basic(requirement="0.2", locked="0.3.1")
        report = self.run_check("0.3.1")
        self.assert_failed(report, "requirement_not_latest_major_minor")
        self.assert_failed(report, "compatible_direct_lock_edge_missing")
        self.assertFalse(report["rust"][0]["requirement_accepts_latest"])

    def test_build_metadata_must_exist_in_registry(self):
        self.basic(locked="1.2.0+spec-1.1.0")
        self.assertEqual("passed", self.run_check("1.2.0+spec-1.1.0")["status"])
        self.assert_failed(self.run_check("1.2.0+spec-1.2.0"), "direct_lock_not_latest")

    def test_missing_optional_target_edge_fails(self):
        self.basic()
        self.write("Cargo.toml", """
[package]
name="app"
version="0.1.0"
[target.'cfg(windows)'.dependencies]
sample={version="1.2", optional=true}
""")
        self.lock(package("app", "0.1.0"), package("sample", "1.2.3", registry=True))
        self.assert_failed(self.run_check(), "compatible_direct_lock_edge_missing")

    def test_lock_source_override_and_path_shadow_fail(self):
        self.basic()
        bad = package("sample", "1.2.3", registry=True)
        bad["source"] = "git+https://user:" + SECRET + "@private.invalid/repo"
        self.lock(package("app", "0.1.0", ["sample"]), bad)
        report = self.run_check()
        self.assert_failed(report, "non_crates_io_lock_source_forbidden")
        self.assertNotIn(SECRET, json.dumps(report))
        self.lock(package("app", "0.1.0", ["sample"]), package("sample", "1.2.3"))
        self.assert_failed(self.run_check(), "direct_dependency_is_not_registry")

    def test_missing_malformed_duplicate_and_stale_lock_fail(self):
        self.basic()
        (self.root / "Cargo.lock").unlink()
        self.assert_failed(self.run_check(), "input_unreadable_or_invalid_toml")
        self.write("Cargo.lock", "malformed " + SECRET)
        report = self.run_check()
        self.assert_failed(report, "input_unreadable_or_invalid_toml")
        self.assertNotIn(SECRET, json.dumps(report))
        self.lock(package("app", "0.0.9"), package("sample", "1.2.3", registry=True))
        self.assert_failed(self.run_check(), "local_package_missing_from_lock")
        self.lock(package("app", "0.1.0", ["sample"]),
                  package("sample", "1.2.3", registry=True),
                  package("sample", "1.2.3", registry=True))
        self.assert_failed(self.run_check(), "duplicate_lock_package")


class FetchAndReportTests(OfflineCase):
    def test_injected_failures_are_redacted_and_fail_closed(self):
        self.basic()
        for error in (TimeoutError(SECRET), OSError(SECRET),
                      check.CheckFailure(SECRET), ValueError(SECRET)):
            def failed_fetch(url, timeout):
                raise error
            with self.subTest(error=type(error).__name__):
                report = self.run_check(fetch=failed_fetch)
                self.assert_failed(report, "registry_check_failed")
                self.assertNotIn(SECRET, json.dumps(report))

    def test_fetches_are_deduplicated_bounded_and_receive_timeout(self):
        self.write("Cargo.toml", """
[workspace]
[workspace.dependencies]
sample="1.2"
other="1.2"
[package]
name="app"
version="0.1.0"
[dependencies]
sample.workspace=true
other.workspace=true
[dev-dependencies]
sample.workspace=true
""")
        self.lock(package("app", "0.1.0", ["sample", "other"]),
                  package("sample", "1.2.3", registry=True),
                  package("other", "1.2.3", registry=True))
        calls = []
        guard = threading.Lock()
        rendezvous = threading.Barrier(2)
        def fetch(url, timeout):
            with guard:
                calls.append((url, timeout))
            rendezvous.wait(timeout=5)
            return metadata("1.2.3")
        report = self.run_check(fetch=fetch, jobs=2, timeout=7)
        self.assertEqual("passed", report["status"])
        self.assertEqual({(check.API_ROOT + "sample", 7), (check.API_ROOT + "other", 7)},
                         set(calls))
        self.assertEqual(2, len(calls))
        self.assertEqual(5, report["counts"]["requirements"])

    def test_input_change_and_new_source_config_during_fetch_fail(self):
        self.basic()
        def mutate_manifest(url, timeout):
            with (self.root / "Cargo.toml").open("a") as stream:
                stream.write("\n# concurrent edit\n")
            return metadata("1.2.3")
        self.assert_failed(self.run_check(fetch=mutate_manifest), "inputs_changed_during_check")
        def add_config(url, timeout):
            self.write(".cargo/config.toml", '[source.crates-io]\nreplace-with="private"')
            return metadata("1.2.3")
        self.assert_failed(self.run_check(fetch=add_config), "inputs_changed_during_check")

    def test_invalid_limits_fail_without_network(self):
        self.basic()
        for kwargs in ({"jobs": 0}, {"jobs": 9}, {"jobs": True}, {"timeout": 0},
                       {"timeout": -1}, {"timeout": 121}, {"timeout": float("nan")},
                       {"timeout": float("inf")}):
            with self.subTest(kwargs=kwargs):
                self.assert_failed(self.run_check(**kwargs), "invalid_request_limits")

    def test_glob_members_added_during_fetch_fail(self):
        self.write("Cargo.toml", '[workspace]\nmembers=["apps/*"]')
        self.write("apps/one/Cargo.toml",
                   '[package]\nname="one"\nversion="0.1.0"\n[dependencies]\nsample="1.2"')
        self.lock(package("one", "0.1.0", ["sample"]),
                  package("sample", "1.2.3", registry=True))
        def add_member(url, timeout):
            self.write("apps/two/Cargo.toml", '[package]\nname="two"\nversion="0.1.0"')
            return metadata("1.2.3")
        self.assert_failed(self.run_check(fetch=add_member), "inputs_changed_during_check")

    def test_cli_exit_code_and_safe_json_report(self):
        self.basic()
        report_path = self.root / "report.json"
        output = io.StringIO()
        with redirect_stdout(output), redirect_stderr(output):
            result = check.main(["--root", str(self.root), "--json-report", str(report_path)],
                                fetch=lambda *args: metadata("1.2.3"))
        self.assertEqual(0, result)
        report = json.loads(report_path.read_text())
        self.assertEqual("passed", report["status"])
        self.assertRegex(report["checked_at_utc"], r"^\d{4}-\d\d-\d\dT.*Z$")
        self.assertRegex(report["completed_at_utc"], r"^\d{4}-\d\d-\d\dT.*Z$")
        self.assertNotIn(str(self.root), report_path.read_text())
        self.assertEqual(check.API_ROOT + "sample", report["rust"][0]["url"])
        with redirect_stdout(output):
            result = check.main(["--root", str(self.root)],
                                fetch=lambda *args: metadata("1.3.0"))
        self.assertEqual(1, result)

    def test_report_does_not_overwrite_existing_files(self):
        self.basic()
        victim = self.write("victim.json", SECRET)
        for path in (victim, self.root / "Cargo.toml"):
            with self.subTest(path=path), redirect_stdout(io.StringIO()), \
                    redirect_stderr(io.StringIO()):
                result = check.main(["--root", str(self.root), "--json-report", str(path)],
                                    fetch=lambda *args: metadata("1.2.3"))
            self.assertEqual(1, result)
        self.assertEqual(SECRET, victim.read_text())

    def test_report_does_not_follow_symlinks(self):
        self.basic()
        victim = self.write("victim.json", SECRET)
        symlink = self.root / "link.json"
        try:
            symlink.symlink_to(victim)
        except (OSError, NotImplementedError):
            self.skipTest("This platform does not permit creation of symlinks")
        with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            result = check.main(["--root", str(self.root), "--json-report", str(symlink)],
                                fetch=lambda *args: metadata("1.2.3"))
        self.assertEqual(1, result)
        self.assertEqual(SECRET, victim.read_text())

    def test_cli_argument_error_does_not_echo_secret(self):
        output = io.StringIO()
        with redirect_stderr(output), self.assertRaises(SystemExit) as caught:
            check.main(["--timeout", SECRET])
        self.assertEqual(2, caught.exception.code)
        self.assertNotIn(SECRET, output.getvalue())

    def test_official_get_timeout_no_proxy_and_response_size_limit(self):
        url = check.API_ROOT + "sample"
        response = unittest.mock.MagicMock()
        response.__enter__.return_value = response
        response.status = 200
        response.geturl.return_value = url
        response.read.return_value = json.dumps(metadata("1.2.3")).encode()
        with patch.object(check.urllib.request, "build_opener") as builder:
            builder.return_value.open.return_value = response
            self.assertEqual(metadata("1.2.3"), check.fetch_json(url, 3.5))
            request = builder.return_value.open.call_args.args[0]
            self.assertEqual("GET", request.get_method())
            self.assertEqual(url, request.full_url)
            self.assertEqual({"timeout": 3.5}, builder.return_value.open.call_args.kwargs)
            self.assertEqual({}, builder.call_args.args[0].proxies)
            response.read.assert_called_once_with(check.MAX_BYTES + 1)
            response.read.return_value = b"x" * (check.MAX_BYTES + 1)
            with self.assertRaisesRegex(check.CheckFailure, "registry_response_too_large"):
                check.fetch_json(url, 3.5)

    def test_http_json_timeout_and_redirect_failures_do_not_leak(self):
        url = check.API_ROOT + "sample"
        with patch.object(check.urllib.request, "build_opener") as builder:
            for error in (TimeoutError(SECRET), urllib.error.URLError(SECRET),
                          urllib.error.HTTPError("https://" + SECRET, 429, SECRET, {}, None)):
                builder.return_value.open.side_effect = error
                with self.subTest(error=type(error).__name__), \
                        self.assertRaises(check.CheckFailure) as caught:
                    check.fetch_json(url, 1)
                self.assertNotIn(SECRET, str(caught.exception))
                self.assertIsNone(caught.exception.__cause__)
            builder.return_value.open.side_effect = None
            response = unittest.mock.MagicMock()
            response.__enter__.return_value = response
            response.status = 200
            response.geturl.return_value = url
            builder.return_value.open.return_value = response
            for body in (b"not JSON " + SECRET.encode(), b"[]", b"\xff"):
                response.read.return_value = body
                with self.assertRaises(check.CheckFailure) as caught:
                    check.fetch_json(url, 1)
                self.assertNotIn(SECRET, str(caught.exception))
            response.status = 503
            with self.assertRaisesRegex(check.CheckFailure, "registry_http_failure"):
                check.fetch_json(url, 1)
        for status in (301, 302, 303, 307, 308):
            with self.subTest(status=status):
                response = io.BytesIO(SECRET.encode())
                handler = getattr(check.NoRedirect(), f"http_error_{status}")
                with self.assertRaisesRegex(check.CheckFailure, "registry_redirect_rejected"):
                    handler(None, response, status, SECRET, {"Location": "https://" + SECRET})
                self.assertTrue(response.closed)

    def test_untrusted_urls_never_reach_the_network(self):
        for url in ("http://crates.io/api/v1/crates/sample",
                    "https://crates.io.evil.invalid/api/v1/crates/sample",
                    check.API_ROOT + "sample?token=" + SECRET,
                    check.API_ROOT + "../" + SECRET,
                    "https://user:" + SECRET + "@crates.io/api/v1/crates/sample"):
            with self.subTest(url=url), self.assertRaisesRegex(
                    check.CheckFailure, "non_official_url_rejected"):
                check.fetch_json(url, 1)


if __name__ == "__main__":
    unittest.main()
