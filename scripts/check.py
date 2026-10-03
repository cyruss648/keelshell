#!/usr/bin/env python3
"""Portable developer gate. Stops on the first failure and preserves command output."""
import argparse
from pathlib import Path
import os
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]

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
    options = parser.parse_args()
    dependency_policy()
    if options.policy_only:
        return
    with tempfile.TemporaryDirectory(prefix="keelshell-check-") as scratch:
        env = dict(os.environ, TMPDIR=str(Path(scratch).resolve()))
        for command in [
            ["cargo", "fmt", "--all", "--check"],
            ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"],
            ["cargo", "test", "--workspace", "--locked", "--", "--test-threads=4"],
        ]:
            print("Running: " + " ".join(command), flush=True)
            subprocess.run(command, cwd=ROOT, env=env, check=True)

if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
