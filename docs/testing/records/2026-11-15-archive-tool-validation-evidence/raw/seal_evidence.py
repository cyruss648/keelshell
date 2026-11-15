"""Export this completed batch losslessly before removing its temporary copies."""

from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

batch = Path(__file__).resolve().parent
root = batch.parents[1]
evidence = root / "docs/testing/records/2026-11-15-archive-tool-validation-evidence"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, ensure_ascii=False)
        stream.write("\n")


def main():
    freeze = json.loads((batch / "frozen-inputs-r2.json").read_bytes())
    after = json.loads((batch / "post-gate-inputs-r2.json").read_bytes())
    assert freeze == after
    for path, digest in freeze["files"].items():
        assert sha((root / path).read_bytes()) == digest, path
    gate = json.loads((batch / "full-gate-r2.json").read_bytes())
    assert gate["exit_code"] == 0 and gate["reaped"] and not gate["timeout_or_interrupt"]
    assert sha((batch / "full-gate-r2.log").read_bytes()) == gate["log_sha256"]
    review = json.loads((batch / "review-session/review-closure-r2.json").read_bytes())
    app = json.loads((batch / "review-app/execution-summary-r2.json").read_bytes())
    assert review["freeze"]["recomputed_sha256"] == freeze["sha256"]
    assert app["input_sha256_before"] == app["input_sha256_after"] == freeze["sha256"]
    assert not any(review["open_findings"].values())
    assert app["rust_cases_passed"] == 7 and app["rust_cases_failed"] == 0
    write_json(batch / "engineering-conditions-satisfied-r3.json", {
        "recorded_utc": datetime.now(timezone.utc).isoformat(),
        "freeze_sha256": freeze["sha256"],
        "engineering_inputs": freeze["count"],
        "path_sets_and_contents_before_after_equal": True,
        "full_gate_exit": gate["exit_code"],
        "full_gate_reaped": gate["reaped"],
        "full_gate_log_sha256": gate["log_sha256"],
        "review_reports": ["review-session/report-r2.md", "review-app/report-r2.md"],
        "review_conditions_satisfied_by_parent": True,
        "historical_pending_reports_left_unchanged": True,
        "independent_rust_rebuild": False,
        "native_desktop_runs": {"macos": 0, "windows": 0, "linux": 0},
    })

    events = json.loads((batch / "container-events-readback-r3.json").read_bytes())
    containers = []
    for item in events:
        assert item["exit_code"] == 0, item["name"]
        rows = [json.loads(line) for line in item["stdout"].splitlines() if line.strip()]
        ids = {row["ID"] for row in rows if row.get("Name") == item["name"]}
        assert len(ids) == 1, (item["name"], ids)
        container_id = ids.pop()
        assert re.fullmatch(r"[a-f0-9]{64}", container_id)
        command = ["podman", "container", "exists", container_id]
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
        assert result.returncode == 1, (item["name"], result.returncode)
        containers.append({
            "name": item["name"], "id": container_id,
            "command": command, "exit_code": result.returncode,
            "absent": True, "event_statuses": [row["Status"] for row in rows],
        })
    assert len(containers) == 4
    author_path = root.parent.parent / "archive-tool-validation-20261115/keelshell"
    assert not author_path.exists()
    openssh = json.loads((batch / "openssh-r2/result.json").read_bytes())
    assert all(openssh["cleanup"][key] for key in (
        "owned_processes_stopped", "observed_processes_stopped", "temporary_directory_removed",
    ))
    write_json(batch / "resources-before-export-r3.json", {
        "recorded_utc": datetime.now(timezone.utc).isoformat(),
        "containers": containers,
        "cidfile_boundary": "Podman --rm removed the cidfiles; exact IDs recovered from owned-name events",
        "author_worktree_absent": True,
        "author_worktree": str(author_path),
        "openssh_owned_processes_stopped": True,
        "openssh_temporary_directory_removed": True,
        "other_worktrees_containers_images_and_unique_target_preserved": True,
    })

    assert not (evidence / "raw").exists(), "Use a new export, do not overwrite originals"
    evidence.mkdir(parents=True, exist_ok=True)
    (evidence / ".gitattributes").write_text(
        "# Preserve original report/log bytes, including line endings and whitespace.\n"
        "raw/** -text -diff -merge\n"
    )
    (evidence / ".gitignore").write_text("!*.log\n")
    originals = {}
    omitted = []
    for source in sorted(batch.rglob("*")):
        if not source.is_file():
            continue
        assert not source.is_symlink(), source
        relative = source.relative_to(batch).as_posix()
        data = source.read_bytes()
        if relative.startswith("stable-binaries/"):
            omitted.append({
                "path": relative, "bytes": len(data), "sha256": sha(data),
                "reason": "Regenerable prebuilt test copy; binding, complete inventory and execution receipts preserved",
            })
            continue
        assert source.suffix in {".json", ".log", ".md", ".py", ".txt"}, relative
        artifact = "raw/" + relative
        if len(data) > 65536:
            artifact += ".gz"
            encoded = gzip.compress(data, mtime=0)
            assert gzip.decompress(encoded) == data
        else:
            encoded = data
        target = evidence / artifact
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(encoded)
        assert target.read_bytes() == encoded
        originals[relative] = {
            "artifact": artifact, "source_bytes": len(data),
            "source_sha256": sha(data), "artifact_bytes": len(encoded),
            "artifact_sha256": sha(encoded),
        }
    write_json(evidence / "archive-manifest.json", {
        "schema": 1, "original_directory": batch.relative_to(root).as_posix(),
        "exported_utc": datetime.now(timezone.utc).isoformat(),
        "count": len(originals), "files": originals,
    })
    write_json(evidence / "omitted-artifacts.json", {
        "files": omitted, "total_bytes": sum(item["bytes"] for item in omitted),
    })
    print(json.dumps({
        "originals": len(originals),
        "source_bytes": sum(item["source_bytes"] for item in originals.values()),
        "stored_bytes": sum(item["artifact_bytes"] for item in originals.values()),
        "omitted_bytes": sum(item["bytes"] for item in omitted),
        "container_ids_all_absent": len(containers),
    }))


if __name__ == "__main__":
    main()
