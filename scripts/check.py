#!/usr/bin/env python3
"""Portable developer gate. Stops on the first failure and preserves command output."""
import argparse
from pathlib import Path
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]

def report_storage(phase, temporary, *, output=None):
    """Observe capacity without paths or error text; never replace a check result."""
    facts = {}
    for label, path in (("workspace", ROOT), ("temporary", temporary)):
        try:
            usage = shutil.disk_usage(path)
            facts[label] = {
                "available": True, "total_bytes": usage.total,
                "used_bytes": usage.used, "free_bytes": usage.free,
            }
        except OSError as error:
            facts[label] = {
                "available": False,
                "errno": error.errno if type(error.errno) is int else None,
                "winerror": value if type(value := getattr(error, "winerror", None)) is int else None,
            }
    try:
        print(json.dumps({"storage_observation": {"phase": phase, "locations": facts}}),
              file=sys.stdout if output is None else output, flush=True)
    except (OSError, ValueError):
        # Optional reporting must not hide a failed command, including when
        # stdout has already been closed by its caller.
        pass

def dependency_policy():
    for path in [ROOT / "Cargo.toml", *sorted((ROOT / "crates").glob("*/Cargo.toml"))]:
        data = tomllib.loads(path.read_text())
        def visit(value):
            if not isinstance(value, dict):
                return
            for key, table in value.items():
                if key in ("dependencies", "dev-dependencies", "build-dependencies"):
                    for name, spec in table.items():
                        version = spec if isinstance(spec, str) else spec.get("version")
                        if version is not None and not re.fullmatch(r"[0-9]+\.[0-9]+", version):
                            raise ValueError(f"{path.relative_to(ROOT)}: {name} must use x.y, found {version}")
                        if isinstance(spec, dict) and "git" in spec:
                            raise ValueError(f"Unreviewed Git dependency: {name}")
                visit(table)
        visit(data)
    print("Dependency version policy: passed", flush=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy-only", action="store_true", help="Validate manifest dependency versions only")
    parser.add_argument("--fresh-dependencies", action="store_true",
                        help="Check official latest stable registry versions before the deterministic gate (requires network)")
    options = parser.parse_args()
    dependency_policy()
    if options.fresh_dependencies:
        subprocess.run([sys.executable, str(ROOT / "scripts/check_dependencies.py")],
                       cwd=ROOT, check=True)
    if options.policy_only:
        return
    with tempfile.TemporaryDirectory(prefix="keelshell-check-") as scratch:
        env = dict(os.environ, TMPDIR=str(Path(scratch).resolve()))
        for label, command in [
            ("python-tests", [sys.executable, "-m", "unittest", "discover", "-s", "scripts", "-p", "test_*.py"]),
            ("format", ["cargo", "fmt", "--all", "--check"]),
            ("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]),
            ("workspace-tests", ["cargo", "test", "--workspace", "--locked", "--", "--test-threads=4"]),
            ("readiness-controller", ["cargo", "test", "-p", "keelshell-ai", "--test", "local_agent_process", "--locked", "--", "--readiness-error-tests"]),
            ("observer-controller", ["cargo", "test", "-p", "keelshell-ai", "--test", "local_agent_directory_process", "--locked", "--", "--observer-io-error-tests"]),
            ("small-stack-controller", ["cargo", "test", "-p", "keelshell-ai", "--test", "local_agent_process", "--locked", "--", "--controller-small-stack"]),
        ]:
            report_storage(f"before-{label}", scratch)
            print("Running: " + " ".join(command), flush=True)
            try:
                subprocess.run(command, cwd=ROOT, env=env, check=True)
            finally:
                report_storage(f"after-{label}", scratch)

if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
