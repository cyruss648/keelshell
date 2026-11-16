#!/usr/bin/env python3
"""Check assembly bytes and synthetic fixtures; never execute Rust or PowerShell."""

import base64
import hashlib
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parent
PREFIX = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand "
SUFFIX = "}catch{F (E $_) $null 1}\n"
FIELDS = {
    "version",
    "operation",
    "platform",
    "runtime",
    "runtime_version",
    "units",
    "status",
    "data",
}


def pairs(items):
    result = {}
    for key, value in items:
        if key in result:
            raise ValueError("duplicate fixture key")
        result[key] = value
    return result


def reject_constant(_):
    raise ValueError("nonfinite fixture number")


def main():
    commands = []
    for operation, skip in [
        ("probe", False),
        ("resources", False),
        ("resources", True),
        ("processes", False),
        ("sockets", False),
    ]:
        literal = f"$op='{operation}';$skip=${str(skip).lower()};\n"
        sources = [(ROOT / "common.ps1").read_bytes(), (ROOT / f"{operation}.ps1").read_bytes()]
        for source in sources:
            assert source.endswith(b"\n") and b"\r" not in source
            assert not source.startswith(b"\xef\xbb\xbf")
        script = literal + "".join(source.decode("ascii") for source in sources) + SUFFIX
        encoded = base64.b64encode(script.encode("utf-16le")).decode("ascii")
        command = PREFIX + encoded
        assert command.isascii() and len(command) < 8000
        assert base64.b64decode(encoded, validate=True).decode("utf-16le") == script
        commands.append({
            "operation": operation,
            "skip_capacity": skip,
            "script_utf16_units": len(script.encode("utf-16le")) // 2,
            "command_ascii_bytes": len(command),
            "command_sha256": hashlib.sha256(command.encode("ascii")).hexdigest(),
        })
    fixtures = []
    for path in sorted((ROOT / "fixtures").glob("*.json")):
        raw = path.read_bytes()
        assert raw.endswith(b"\n") and b"\r" not in raw
        assert raw.startswith(b"{") and raw.endswith(b"}\n")
        frame = json.loads(
            raw.decode("utf-8"), object_pairs_hook=pairs, parse_constant=reject_constant
        )
        assert set(frame) == FIELDS
        assert frame["version"] == 1
        assert frame["platform"] == "Windows"
        assert frame["runtime"] == "WindowsPowerShell"
        assert frame["runtime_version"] == "5.1.26100.123"
        assert frame["units"] == "bytes seconds percent"
        assert frame["operation"] in {"probe", "resources", "processes", "sockets"}
        assert frame["status"] in {
            "ok", "command_missing", "permission_denied",
            "command_failed", "unsupported_options",
        }
        if frame["status"] == "ok":
            assert isinstance(frame["data"], dict)
        else:
            assert frame["data"] is None
        fixtures.append({
            "file": path.name,
            "bytes": len(raw),
            "expected_exit": 0 if frame["status"] == "ok" else "nonzero",
            "sha256": hashlib.sha256(raw).hexdigest(),
        })
    tests = re.findall(r"#\[test\]\nfn ([a-z0-9_]+)\(", (ROOT / "tests.rs").read_text())
    assert tests and len(tests) == len(set(tests))
    print(json.dumps({
        "scope": "static assembly and fixture checks only",
        "commands": commands,
        "fixtures": fixtures,
        "declared_rust_test_count": len(tests),
        "rust_tests_executed": 0,
        "powershell_commands_executed": 0,
        "test_names": tests,
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
