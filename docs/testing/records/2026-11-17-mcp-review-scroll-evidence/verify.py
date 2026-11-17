"""Read and verify this archive; never extract or execute its members."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def git_blob(commit, path):
    return subprocess.check_output(["git", "show", f"{commit}:{path}"], cwd=ROOT, timeout=20)


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--commit", help="Exact commit whose engineering and archived evidence blobs must match")
options = parser.parse_args()
if options.commit:
    assert re.fullmatch("[0-9a-f]{40}", options.commit)
manifest = json.loads((BASE / "manifest.json").read_text())
archive = BASE / "originals.tar.gz"
raw = archive.read_bytes()
assert len(raw) == manifest["bytes"] and digest(raw) == manifest["sha256"]
rows = {row["member"]: row for row in manifest["files"]}
assert len(rows) == manifest["members"] == len(manifest["files"])
saved = {}
with tarfile.open(archive, "r:gz") as source:
    members = source.getmembers()
    assert len(members) == len(rows)
    assert {member.name for member in members} == rows.keys()
    for member in members:
        path = PurePosixPath(member.name)
        assert member.isfile() and not path.is_absolute() and ".." not in path.parts
        content = source.extractfile(member).read()
        row = rows[member.name]
        assert len(content) == row["bytes"] and digest(content) == row["sha256"]
        saved[member.name] = content
assert sum(row["bytes"] for row in rows.values()) == manifest["logical_bytes"]
inputs = json.loads(saved["integration/inputs-r2.json"])
after = json.loads(saved["integration/inputs-after-r2.json"])
assert inputs["inputs"] == after["inputs"]
assert len(inputs["inputs"]) == manifest["final_engineering_inputs"]
assert digest(json.dumps(inputs["inputs"], sort_keys=True, separators=(",", ":")).encode()) == inputs["aggregate_sha256"] == manifest["final_engineering_aggregate"]
gate = json.loads(saved["integration/full-gate-r2/result.json"])
assert gate["exit_code"] == 0 and gate["reaped"] and not gate["timed_out"]
inventory = json.loads(saved["integration/full-gate-r2-inventory/summary.json"])
assert inventory["all_inventory_names_match"] and inventory["all_binary_digests_unchanged"]
assert inventory["failed"] == 0
native = json.loads(saved["scrollbar-native/normal-build-input-binding.json"])
changes = json.loads(saved["scrollbar-native/normal-build-final-test-differences.json"])
native_rows = {row["path"]: row for row in native["engineering_inputs"]}
final_rows = {row["path"]: row for row in inputs["inputs"]}
different = {name for name in native_rows.keys() | final_rows.keys() if native_rows.get(name) != final_rows.get(name)}
expected = {
    "crates/keelshell-app/src/workspace/tests/mcp_first_proposal.rs",
    "crates/keelshell-session/src/network_diagnostic/tests/test_icmp.py",
    "crates/keelshell-session/src/network_diagnostic/tests/test_traceroute.py",
}
assert different == expected == {row["path"] for row in changes["changed_test_inputs"]}
assert len(native_rows) == len(final_rows) == 777
assert changes["unchanged_inputs"] == 774
assert changes["native_build_aggregate"] == native["aggregate_sha256"]
assert changes["final_gate_aggregate"] == inputs["aggregate_sha256"]
client = json.loads(saved["scrollbar-native/mcp-native-client-result.json"])
assert client["status"] == client["file_readback"] == client["command_rejection"] == "PASS"
assert client["exit_code"] == 0 and client["reaped"] and len(client["calls"]) == 9
assert all(not call["is_error"] for call in client["calls"])
for name in ("native-app-r1/result.json", "fixture-cleanup-r1/result.json"):
    receipt = json.loads(saved["scrollbar-native/" + name])
    assert receipt["exit_code"] == 0 and receipt["reaped"] and not receipt["timed_out"]
if options.commit:
    for row in inputs["inputs"]:
        content = git_blob(options.commit, row["path"])
        assert len(content) == row["bytes"] and digest(content) == row["sha256"], row["path"]
    for path in sorted(BASE.iterdir()):
        if path.is_file():
            assert path.read_bytes() == git_blob(options.commit, str(path.relative_to(ROOT))), path.name
result = {
    "archive_members": len(rows), "archive_bytes": len(raw),
    "archive_sha256": digest(raw),
    "engineering_inputs": len(final_rows),
    "engineering_aggregate": inputs["aggregate_sha256"],
    "normal_native_unchanged_inputs": 774,
    "normal_native_test_only_differences": sorted(different),
    "ordinary_rust": {key: inventory[key] for key in ("suite_count", "passed", "failed", "ignored")},
    "commit": options.commit,
    "exact_git_blobs_match": True if options.commit else None,
    "boundary": "Explicit native scrollbar/file/rejection subset only. Wheel/root UNKNOWN, compact native, assistive technology, other-platform GUI and complete product/release remain open.",
}
print(json.dumps(result, indent=2))
