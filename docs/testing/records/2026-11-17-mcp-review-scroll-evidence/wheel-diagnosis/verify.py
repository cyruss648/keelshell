"""Verify saved native observations without launching an app or replaying an action."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[4]
SOURCE = "39d9041f350e30788b39121959d2ae7f7efea043"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def git_blobs(commit, paths):
    assert re.fullmatch("[0-9a-f]{40}", commit)
    result = subprocess.run(
        ["git", "cat-file", "--batch"],
        input="".join(f"{commit}:{path}\n" for path in paths).encode(),
        cwd=ROOT, capture_output=True, timeout=45, check=True,
    )
    stream, values = io.BytesIO(result.stdout), {}
    for path in paths:
        header = stream.readline().split()
        assert len(header) == 3 and header[1] == b"blob", path
        value = stream.read(int(header[2]))
        assert len(value) == int(header[2]) and stream.read(1) == b"\n"
        values[path] = value
    assert stream.read() == b""
    return values


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--bind-source", action="store_true")
parser.add_argument("--commit", help="Also match the delivered files to this exact Git commit")
options = parser.parse_args()
manifest = json.loads((BASE / "manifest.json").read_bytes())
raw = (BASE / "originals.tar.gz").read_bytes()
assert len(raw) == manifest["bytes"] and sha(raw) == manifest["sha256"]
rows = {row["member"]: row for row in manifest["files"]}
assert len(rows) == len(manifest["files"]) == manifest["members"]
files = {}
with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
    members = archive.getmembers()
    assert len(members) == len(rows) and {m.name for m in members} == rows.keys()
    for member in members:
        path = PurePosixPath(member.name)
        assert member.isfile() and not path.is_absolute() and ".." not in path.parts
        value = archive.extractfile(member).read()
        row = rows[member.name]
        assert len(value) == row["bytes"] and sha(value) == row["sha256"], member.name
        files[member.name] = value
assert sum(map(len, files.values())) == manifest["logical_bytes"]
assert not any("/runtime-" in name or "/package-" in name for name in files)


def doc(name):
    return json.loads(files["probe/" + name])


binding = doc("normal-build-input-binding.json")
normal = binding["normal_build"]
inputs = {row["path"]: row for row in normal["engineering_inputs"]}
assert len(inputs) == normal["count"] == binding["current_input_count"] == 777
assert sha(json.dumps(list(inputs.values()), sort_keys=True, separators=(",", ":")).encode()) == normal["aggregate_sha256"]
changes = binding["final_test_differences"]
expected = {
    "crates/keelshell-app/src/workspace/tests/mcp_first_proposal.rs",
    "crates/keelshell-session/src/network_diagnostic/tests/test_icmp.py",
    "crates/keelshell-session/src/network_diagnostic/tests/test_traceroute.py",
}
assert {row["path"] for row in changes["changed_test_inputs"]} == expected
for row in changes["changed_test_inputs"]:
    assert inputs[row["path"]] == row["native_build"]
    inputs[row["path"]] = row["final_gate"]
canonical = json.dumps(list(inputs.values()), sort_keys=True, separators=(",", ":")).encode()
assert sha(canonical) == binding["current_input_aggregate"] == changes["final_gate_aggregate"]
assert changes["unchanged_inputs"] == 774 and changes["production_scope_equal"]
actions = doc("native-wheel-actions.json")
assert actions["source_base"] == SOURCE and actions["production_changes"] == 0
assert actions["package_reused_unchanged"] and not actions["scrollbar_dragging"]
assert len(actions["sequence"]) == 10
for step in actions["sequence"]:
    name = "probe/captures/" + step["capture"]
    assert files[name + ".jpg"].startswith(b"\xff\xd8")
    assert "KeelShell" in files[name + ".ax.txt"].decode()
first = doc("native-client-r1/result.json")
assert first["exit_code"] == 1 and first["reaped"] and not first["timed_out"]
assert b"native file review deadline" in files["probe/native-client-r1/command.log"]
first_child = doc("mcp-native-client-result.json")
assert first_child["exit_code"] == 0 and first_child["reaped"]
assert first_child.get("status") != "PASS"
second = doc("mcp-native-readback-r2-result.json")
assert second["status"] == second["unchanged_file_readback"] == second["file_proposal_rejection"] == "PASS"
assert second["exit_code"] == 0 and second["reaped"]
assert [row["tool"] for row in second["calls"]] == [
    "keelshell_list_sessions", "keelshell_get_action_status", "keelshell_sftp_read",
]
assert all(not row["is_error"] for row in second["calls"])
readback = doc("mcp-file-rejected-readback-r2.json")
pending = doc("mcp-file-proposal-pending.json")
assert readback["action_id"] == pending["action_id"]
assert readback["state"] == "rejected" and readback["full_original_unchanged"]
original = "KeelShell native acceptance\n原始内容：仅供本次审核式文件操作。\n".encode()
assert readback["sha256"] == pending["original_sha256"] == sha(original)
assert readback["bytes"] == len(original)
for name in ("native-readback-r2/result.json", "native-app-r1/result.json", "fixture-cleanup-r1/result.json"):
    receipt = doc(name)
    assert receipt["exit_code"] == 0 and receipt["reaped"] and not receipt["timed_out"]
cleanup = doc("final-fixture-cleanup.json")
assert cleanup["private_directory_removed"]
revoked_ax = files["probe/captures/09-all-grants-revoked.ax.txt"].decode()
assert "(disabled) Copy temporary launch settings" in revoked_ax
assert "(disabled) Copy configuration template" in revoked_ax
assert "Revoke session" not in revoked_ax
review = json.loads(files["review/review.json"])
assert review["decision"] == "APPROVE_BOUNDED"
assert review["open_issue_counts"] == {"P0": 0, "P1": 0, "P2": 0, "P3": 0}
assert review["historical_p2"]["closed_in_exact_package_scope"]
assert review["historical_p2"]["direction_cause"] == "UNKNOWN"
checks = json.loads(files["review/checks-r3.json"])
assert checks["summary"]["assertions"] == checks["summary"]["passed"] == len(checks["checks"]) == 907
assert checks["summary"]["failed"] == 0 and all(row["passed"] for row in checks["checks"])
review_receipt = json.loads(files["review/offline-r3/result.json"])
assert review_receipt["exit_code"] == 0 and review_receipt["reaped"] and not review_receipt["timed_out"]
source_bound = False
if options.bind_source or options.commit:
    blobs = git_blobs(SOURCE, list(inputs))
    for path, row in inputs.items():
        assert len(blobs[path]) == row["bytes"] and sha(blobs[path]) == row["sha256"], path
    source_bound = True
if options.commit:
    paths = [p.relative_to(ROOT).as_posix() for p in sorted(BASE.iterdir()) if p.is_file()]
    blobs = git_blobs(options.commit, paths)
    assert all(blobs[path] == (ROOT / path).read_bytes() for path in paths)
print(json.dumps({
    "members": len(files), "bytes": len(raw), "sha256": sha(raw),
    "engineering_source": SOURCE, "engineering_inputs": 777,
    "exact_source_blobs_match": source_bound,
    "delivery_commit": options.commit,
    "raw_capture_pairs": 10, "production_changes": 0,
    "independent_final_assertions": 907,
    "first_client_deadline_failure_preserved": True,
    "second_client_read_only_calls": 3,
    "file_proposal": "rejected; entire original unchanged",
    "normal_app_exit": "0/reaped",
    "boundary": "Observed bidirectional content movement using CUA up/down labels; direction mapping cause UNKNOWN. Physical input, compact native size, full GUI matrix and release unverified.",
}, ensure_ascii=False, indent=2))
