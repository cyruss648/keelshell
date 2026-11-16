"""Verify the completed 95813a7 CI independently of the later fixture candidate."""

from collections import Counter
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

BASE = Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    assert len(packed) == manifest["archive_bytes"] and sha(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(row.isfile() and not row.issparse() for row in members)
        assert len(members) == manifest["count"]
        assert sorted(row.name for row in members) == sorted(row["path"] for row in manifest["entries"])
        for row in manifest["entries"]:
            raw = archive.extractfile(row["path"]).read()
            assert len(raw) == row["bytes"] and sha(raw) == row["sha256"], row["path"]
            files[row["path"]] = raw
    assert files["tools/verify.py"] == Path(__file__).read_bytes()
    final = json.loads(files["ci/source-ci-final-r1.log"])
    assert final["headSha"] == manifest["source"] == "95813a76f0ec9b59fd043cd982e33cdc01dd69f8"
    assert final["status"] == "completed" and final["conclusion"] == "failure"
    for label in ("source-ci-final-r1", "source-windows-job-r1",
                  "source-macos-active-log-r1", "source-ubuntu-job-r1"):
        receipt = json.loads(files[f"ci/{label}.json"])
        raw = files[f"ci/{label}.log"]
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0
        assert receipt["terminal"]["direct_reaped"] and not receipt["timed_out"]
        assert len(raw) == receipt["log_bytes"] and sha(raw) == receipt["log_sha256"]
    watch = json.loads(files["ci/source-ci-watch-r1.json"])
    assert watch["observed_exit"] == watch["controller_exit"] == 1
    assert watch["terminal"]["direct_reaped"] and not watch["timed_out"]
    assert sha(files["ci/source-ci-watch-r1.log"]) == watch["log_sha256"]
    namespace = {"__name__": "sealed_ci_parser"}
    exec(compile(files["tools/summarize_ci_r3.py"], "<sealed-ci-parser-r3>", "exec"), namespace)
    for platform, raw_name in (("macos", "source-macos-active-log-r1"), ("ubuntu", "source-ubuntu-job-r1")):
        report = json.loads(files[f"ci/source-{platform}-summary-r3.json"])
        recomputed = namespace["summarize"](files[f"ci/{raw_name}.log"])
        assert recomputed == {key: value for key, value in report.items() if key != "log"}
        assert len(report["suites"]) == 92
        assert sum(row["summary"]["passed"] for row in report["doctest_runs_unassigned"]) == 14
        app = next(row for row in report["suites"] if "keelshell_app-" in row["suite"])
        assert app["declared_tests"] == len(app["results"]) == (1343 if platform == "macos" else 1344)
        assert app["summary"]["failed"] == 0 and app["summary"]["ignored"] == 2
        job = next(row for row in final["jobs"] if platform in row["name"])
        assert job["conclusion"] == "success"
        for step in ("Rust quality gate", "OpenSSH interoperability (macOS and Linux)"):
            assert next(row for row in job["steps"] if row["name"] == step)["conclusion"] == "success"
    windows = json.loads(files["ci/source-windows-summary-r1.json"])
    assert len(windows["session_inventory"]) == len(set(windows["session_inventory"])) == 332
    assert set(windows["session_inventory"]) == {name for name, _ in windows["session_results"]}
    assert Counter(outcome for _, outcome in windows["session_results"]) == {"ok": 329, "ignored": 3}
    assert windows["zmodem_counts"] == {"ok": 43, "ignored": 2}
    assert windows["full_gate"]["full_workspace_rust_tests"] == "NOT_RUN"
    subprocess.run(
        [sys.executable, str(BASE.parent / "verify.py"), "--source", manifest["source"]],
        check=True, timeout=180,
    )
    print(json.dumps({
        "source": manifest["source"], "run": final["databaseId"],
        "members": len(files), "mismatches": 0, "engineering_inputs": 719,
        "windows_session": [329, 0, 3], "windows_full_gate": "FAILED_PYTHON_DISCOVERY",
        "macos_ubuntu": "SUCCESS_WITH_OPENSSH", "parser_revision": 3,
        "ci_app_evidence": "declared run counts and unique result names; no separate --list",
        "tests_replayed": 0,
    }))


if __name__ == "__main__":
    main()
