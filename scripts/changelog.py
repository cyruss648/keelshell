#!/usr/bin/env python3
"""Generate a bounded release changelog from conventional Git history.

The command is deterministic for a fixed repository and does not contact a
remote service. Release automation can call ``generate`` before packaging;
reviewers can also run it locally to inspect the exact release notes.
"""
from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HEADER = "# Changelog\n\nAll notable changes to KeelShell are documented here.\n\n"
COMMIT = re.compile(r"^(?P<kind>[a-z]+)(?:\([^)]*\))?!?:\s+(?P<subject>.+)$")
KINDS = {
    "feat": "Added",
    "fix": "Fixed",
    "perf": "Improved",
    "refactor": "Changed",
    "docs": "Documentation",
    "build": "Build",
    "ci": "Build",
    "test": "Testing",
}


def run(*args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=ROOT, text=True, encoding="utf-8", stderr=subprocess.STDOUT
    ).strip()


def commits(previous: str | None) -> list[tuple[str, str, str]]:
    revision = f"{previous}..HEAD" if previous else "HEAD"
    output = run("log", revision, "--format=%H%x09%s")
    result: list[tuple[str, str, str]] = []
    for row in output.splitlines():
        sha, subject = row.split("\t", 1)
        match = COMMIT.match(subject)
        if match:
            kind = KINDS.get(match.group("kind"), "Changed")
            text = match.group("subject")
        else:
            kind = "Changed"
            text = subject
        result.append((kind, text, sha[:8]))
    return result


def previous_tag(version: str) -> str | None:
    try:
        value = run("describe", "--tags", "--abbrev=0", "--match", "v*", "HEAD")
    except subprocess.CalledProcessError:
        return None
    return value if value != f"v{version}" else None


def render(version: str, entries: list[tuple[str, str, str]], existing: str = "") -> str:
    grouped: dict[str, list[tuple[str, str]]] = {}
    for kind, subject, sha in entries:
        grouped.setdefault(kind, []).append((subject, sha))
    body = [f"## [{version}]", "", f"Release notes generated from Git history.", ""]
    for kind in ("Added", "Fixed", "Improved", "Changed", "Documentation", "Build", "Testing"):
        if kind not in grouped:
            continue
        body.extend((f"### {kind}", ""))
        body.extend(f"- {subject} ([{sha}])" for subject, sha in grouped[kind])
        body.append("")
    block = "\n".join(body).rstrip() + "\n"
    remainder = existing
    marker = f"## [{version}]"
    if marker in remainder:
        before, after = remainder.split(marker, 1)
        next_release = after.find("\n## [")
        remainder = before + (after[next_release + 1 :] if next_release >= 0 else "")
    if remainder.startswith("# Changelog"):
        remainder = remainder.split("\n\n", 1)[1] if "\n\n" in remainder else ""
        if remainder.startswith("All notable changes"):
            remainder = remainder.split("\n\n", 1)[1] if "\n\n" in remainder else ""
    return HEADER + block + ("\n" + remainder.lstrip() if remainder.strip() else "")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["generate", "check"])
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "CHANGELOG.md")
    args = parser.parse_args()
    output = args.output if args.output.is_absolute() else ROOT / args.output
    if args.action == "check":
        if not output.exists() or f"## [{args.version}]" not in output.read_text(encoding="utf-8"):
            raise SystemExit(f"{output} has no release entry for {args.version}")
        print(f"Checked {output} for {args.version}")
        return
    existing = output.read_text(encoding="utf-8") if output.exists() else ""
    old = previous_tag(args.version)
    output.write_text(render(args.version, commits(old), existing), encoding="utf-8")
    print(f"Generated {output}")


if __name__ == "__main__":
    main()
