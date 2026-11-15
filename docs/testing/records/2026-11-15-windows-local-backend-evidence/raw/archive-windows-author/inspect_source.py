"""Record source-only Windows archive delivery evidence; never build or run Rust."""

from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import re


EVIDENCE = Path(__file__).resolve().parent
ROOT = EVIDENCE.parents[2]
SOURCE = "crates/keelshell-session/src/sftp/archive_transfer/local_windows.rs"
TESTS = "crates/keelshell-session/src/sftp/archive_transfer/local_windows/tests.rs"
INPUTS = [
    SOURCE,
    TESTS,
    "crates/keelshell-platform/src/lib.rs",
    "crates/keelshell-platform/src/path.rs",
    "crates/keelshell-platform/src/windows/mod.rs",
    "crates/keelshell-platform/src/windows/native.rs",
    "crates/keelshell-platform/src/windows/security.rs",
    "crates/keelshell-session/src/sftp/archive_transfer.rs",
    "crates/keelshell-session/src/sftp/archive_transfer/local.rs",
    "crates/keelshell-session/src/sftp/mutation.rs",
    "crates/keelshell-session/src/sftp/queue/reservations.rs",
    "crates/keelshell-core/src/archive.rs",
]


def snapshot():
    result = []
    for name in INPUTS:
        data = (ROOT / name).read_bytes()
        result.append(
            dict(
                path=name,
                bytes=len(data),
                sha256=hashlib.sha256(data).hexdigest(),
                role="owned" if name in (SOURCE, TESTS) else "read_only_contract",
            )
        )
    return result


before = snapshot()
production = (ROOT / SOURCE).read_text()
tests = (ROOT / TESTS).read_text()
pattern = re.compile(
    r"#\[(?:tokio::)?test\](?P<attrs>(?:\s*#\[[^\n]+\])*)"
    r"\s*(?:async\s+)?fn\s+(?P<name>\w+)"
)
inventory = []
for match in pattern.finditer(tests):
    name = match["name"]
    inventory.append(
        dict(
            name=f"sftp::archive_transfer::local::tests::{name}",
            source_line=tests[: match.start()].count("\n") + 1,
            ignored_by_default="ignore =" in match["attrs"],
            platform="windows",
            result="NOT_RUN",
        )
    )

checks = {
    "unsafe_blocks_or_unsafe_functions_absent": not re.search(
        r"\bunsafe\s*(?:\{|fn|impl)", production + "\n" + tests
    ),
    "production_forbids_unsafe": "#![forbid(unsafe_code)]" in production,
    "production_panic_unwrap_expect_absent": not re.search(
        r"\b(?:unwrap|expect)\s*\(|\bpanic!", production
    ),
    "production_path_based_file_io_absent": not re.search(
        r"(?:std|tokio)::fs::(?:read|write|open|create|rename|remove|metadata)"
        r"|File::open|Directory::from_file",
        production,
    ),
    "external_tests_begin_with_inner_test_cfg": tests.startswith("#![cfg(test)]\n"),
    "external_tests_are_windows_only": "#![cfg(windows)]" in tests,
    "exact_test_names_are_unique": len(inventory)
    == len({test["name"] for test in inventory}),
    "test_inventory_is_29": len(inventory) == 29,
    "one_privileged_reparse_test_is_opt_in": sum(
        test["ignored_by_default"] for test in inventory
    )
    == 1,
    "required_manifest_traits_present": (
        "#[derive(Clone, Debug, Default, PartialEq, Eq)]\n"
        "pub(super) struct Manifest"
    )
    in production,
    "async_interface_names_present": all(
        f"pub(super) async fn {name}(" in production
        for name in ("destination", "scan", "extract")
    ),
    "sync_interface_names_present": all(
        f"pub(super) fn {name}(" in production
        for name in (
            "extraction_supported",
            "staging_path",
            "validate_destinations",
        )
    ),
    "owned_files_have_no_trailing_whitespace": all(
        line == line.rstrip()
        for text in (production, tests)
        for line in text.splitlines()
    ),
}
after = snapshot()
checks["read_only_contract_inputs_stable_during_inspection"] = before == after

record = {
    "captured_utc": datetime.now(timezone.utc).isoformat(),
    "host": platform.system(),
    "status": "SOURCE_ONLY_NOT_COMPILED",
    "checks": checks,
    "test_inventory": inventory,
    "test_count": len(inventory),
    "executed_test_count": 0,
    "windows_native": "NOT_RUN",
    "cargo": "NOT_RUN_USER_CONSTRAINT",
    "rustc": "NOT_RUN_USER_CONSTRAINT",
    "formatter": "NOT_RUN_USER_CONSTRAINT",
    "clippy": "NOT_RUN_USER_CONSTRAINT",
    "git": "NOT_READ_OR_CHANGED_USER_CONSTRAINT",
    "source_inputs": before,
    "limits": (
        "Text inspection and SHA-256 inventory only. No syntax parser, compiler, "
        "test execution, native acceptance, parent integration approval, or "
        "crash durability is established by this record."
    ),
}
output = EVIDENCE / "source-inspection.json"
output.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n")
if not all(checks.values()):
    failed = [name for name, value in checks.items() if not value]
    raise SystemExit("Source-only inspection failed: " + ", ".join(failed))
print(
    json.dumps(
        {
            "record": str(output.relative_to(ROOT)),
            "status": record["status"],
            "text_checks": len(checks),
            "test_inventory": len(inventory),
            "executed_tests": 0,
            "owned_files": [entry for entry in before if entry["role"] == "owned"],
        },
        indent=2,
    )
)
