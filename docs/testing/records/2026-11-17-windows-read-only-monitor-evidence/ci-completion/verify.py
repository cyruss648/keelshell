"""Verify exact CI, raw Windows API artifact and Git bytes without test replay."""
import argparse
import ast
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import zipfile

BASE = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE.parent))
from summarize_tests import normalize, summarize

SOURCE = "5d2df2fc5ac8566fe11182b7f4337076296dfa54"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def archive_files(base):
    manifest = json.loads((base / "manifest.json").read_text())
    raw = (base / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"] and sha(raw) == manifest["archive_sha256"]
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(m.isfile() and not PurePosixPath(m.name).is_absolute()
                   and ".." not in PurePosixPath(m.name).parts for m in members)
        files = {m.name: archive.extractfile(m).read() for m in members}
    assert len(files) == len(members) == manifest["count"]
    assert set(files) == {r["path"] for r in manifest["entries"]}
    for row in manifest["entries"]:
        assert len(files[row["path"]]) == row["bytes"] and sha(files[row["path"]]) == row["sha256"]
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default=SOURCE)
    args = parser.parse_args()
    assert args.source == SOURCE, "This record certifies one exact source"
    files = archive_files(BASE)

    def doc(name):
        return json.loads(files[name])

    watch = doc("watch.json")
    assert watch["source"] == SOURCE and watch["reason"] == "terminal"
    assert watch["remote_mutations"] == watch["manual_reruns"] == watch["manual_cancellations"] == 0
    for request in watch["requests"]:
        assert request["exit"] == 0 and not request["timed_out"]
        for stream in ("stdout", "stderr"):
            row = request[stream]
            assert len(files[row["path"]]) == row["bytes"]
            assert sha(files[row["path"]]) == row["sha256"]
    terminal = doc("terminal-summary-r1.json")
    assert terminal["source"] == SOURCE and terminal["tests_replayed"] == 0
    assert terminal["partial"] is False
    assert {r["databaseId"] for r in terminal["runs"]} == {43972364934, 43972365007}
    observed_jobs = {}
    for run in terminal["runs"]:
        assert run["headSha"] == SOURCE and run["status"] == "completed"
        assert run["conclusion"] == "success"
        observed = doc(f"run-{run['databaseId']}-latest.json")
        assert all(observed[key] == run[key] for key in run)
        requests = [row for row in watch["requests"]
                    if row["command"][1:4] == ["run", "view", str(run["databaseId"])]]
        assert requests and doc(requests[-1]["stdout"]["path"]) == observed
        for job in observed["jobs"]:
            assert job["databaseId"] not in observed_jobs
            observed_jobs[job["databaseId"]] = (run["workflowName"], job)
    assert len(terminal["jobs"]) == len(observed_jobs) == 5
    assert {job["job_id"] for job in terminal["jobs"]} == set(observed_jobs)
    quality = []
    direct = None
    for job in terminal["jobs"]:
        workflow, observed = observed_jobs[job["job_id"]]
        assert job["workflow"] == workflow
        assert all(job[key] == observed[key] for key in ("name", "status", "conclusion"))
        if job["conclusion"] == "skipped":
            assert job["workflow"] == "Windows monitor integration"
            assert job["actual_windows_ssh"] == "UNRUN/HOLD"
            continue
        assert job["status"] == "completed" and job["conclusion"] == "success"
        assert job["steps"] == observed["steps"]
        downloaded = doc(f"job-{job['job_id']}-download.json")
        raw = files[downloaded["raw_log"]]
        assert downloaded["exit"] == 0
        assert len(raw) == downloaded["raw_bytes"] and sha(raw) == downloaded["raw_sha256"]
        assert sha(files[f"job-{job['job_id']}-download.stderr.log"]) == downloaded["stderr_sha256"]
        if job["workflow"] == "Quality":
            parsed = summarize(raw)
            assert parsed == doc(f"job-{job['job_id']}-suites.json")
            assert all(s["summary"]["failed"] == 0 for s in parsed["suites"])
            assert all(s["summary"]["failed"] == 0 for s in parsed["doctest_runs_unassigned"])
            ordinary = {
                key: sum(s["summary"][key] for s in parsed["suites"])
                for key in ("passed", "failed", "ignored")
            }
            assert ordinary == job["ordinary"]
            app = next(s for s in parsed["suites"]
                       if "unittests src/main.rs " in s["suite"].replace("\\", "/")
                       and "keelshell_app" in s["suite"])
            windows = [(n, v) for n, v in app["results"] if n.startswith("monitor::tests::windows::")]
            assert len(windows) == 23 and all(v == "ok" for _, v in windows)
            assert job["python_main"]["run"] == 405 and job["python_main"]["failed"] == 0
            text = normalize(raw)
            gate = text.split("##[group]Run python scripts/check.py", 1)[1]
            python_section = gate.split("Running: cargo fmt --all --check", 1)[0]
            match = re.search(
                r"Ran (\d+) tests in ([\d.]+)s\s+OK \(skipped=(\d+)\)", python_section
            )
            assert match
            assert job["python_main"] == {
                "run": int(match[1]), "seconds": float(match[2]),
                "skipped": int(match[3]), "passed": int(match[1]) - int(match[3]),
                "failed": 0,
            }
            rustdoc = {key: sum(s["summary"][key] for s in parsed["doctest_runs_unassigned"])
                       for key in ("passed", "failed", "ignored")}
            assert rustdoc == job["rustdoc"]
            assert job["ordinary_suites"] == len(parsed["suites"])
            assert app["summary"] == job["app"]
            assert next(s for s in job["steps"] if s["name"] == "Rust quality gate")["conclusion"] == "success"
            if "windows" not in job["name"].lower():
                assert next(s for s in job["steps"]
                            if s["name"] == "OpenSSH interoperability (macOS and Linux)")["conclusion"] == "success"
            quality.append({"name": job["name"], "ordinary": ordinary, "app": app["summary"],
                            "python": job["python_main"]})
        else:
            direct = job
            text = normalize(raw)
            replay = "captured_windows_api_frames_are_accepted_through_authenticated_monitor_routing"
            assert re.search(rf"test {replay} \.\.\. ok", text)
            assert "1 passed; 0 failed; 0 ignored" in text
            python_results = re.findall(
                r"^(test_\S+ \([^)]+\)) \.\.\. (ok|skipped)(?: [^\n]*)?$",
                text, re.MULTILINE,
            )
            assert len(python_results) == len({name for name, _ in python_results}) == 111
            assert sum(value == "ok" for _, value in python_results) == 108
            assert sum(value == "skipped" for _, value in python_results) == 3
            assert job["lab_python_tests"]["results"] == [list(row) for row in python_results]
            assert job["lab_python_tests"]["passed"] == 108
            assert job["lab_python_tests"]["skipped"] == 3
            assert re.search(r"Ran 111 tests in [\d.]+s\s+OK \(skipped=3\)", text)
    assert len(quality) == 3 and direct is not None

    download = doc("direct-api-artifact-download-r1.json")
    zipped = files[download["archive"]]
    assert len(zipped) == download["archive_bytes"] and sha(zipped) == download["archive_sha256"]
    assert download["expected_github_digest"] == "sha256:" + sha(zipped)
    assert download["artifact"]["workflow_run"]["head_sha"] == SOURCE
    with zipfile.ZipFile(io.BytesIO(zipped)) as archive:
        members = [m for m in archive.infolist() if not m.is_dir()]
        assert len(members) == len({m.filename for m in members}) == 34
        for member in members:
            assert not PurePosixPath(member.filename).is_absolute()
            assert ".." not in PurePosixPath(member.filename).parts
            assert archive.read(member) == files["direct-artifact-r1/" + member.filename]
    api = doc("direct-artifact-r1/receipt.json")
    assert api["status"] == api["windows_api"] == "PASS"
    assert api["ssh"] == api["native_gui"] == "UNRUN"
    assert api["system"] == "Windows" and api["architecture"] == "AMD64"
    children = {r["label"]: r for r in api["children"]}
    assert set(children) == {"probe", "resources", "processes", "sockets", "resources-skipped", "fixture"}

    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10
    ).strip())

    def git_bytes(name):
        return subprocess.check_output(
            ["git", "show", f"{SOURCE}:{name}"], cwd=root, timeout=25
        )

    constants = {}
    tree = ast.parse(git_bytes("scripts/windows_monitor_lab.py"))
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id in ("ASSEMBLY", "POWERSHELL", "SUFFIX"):
                    constants[target.id] = ast.literal_eval(node.value)
    for label, child in children.items():
        assert child["observed_exit"] == 0 and not child["errors"]
        end = child["terminal"]
        assert end["direct_reaped"] and end["active_processes"] == 0
        assert end["pipes_drained"] and not end["errors"]
        if label == "fixture":
            continue
        shell = child["shell_execution"]
        assert shell["complete_original_object_census"] and shell["job_total_processes"] == 2
        power = shell["powershell_original_objects"]
        assert len(power) == 1 and power[0]["retained_while_live"]
        assert power[0]["natural_terminal_before_cleanup"] and power[0]["exit_code"] == 0
        native = end["native_process_objects"]
        assert native["admission_closed"] and native["identities_bound"]
        assert native["pending_process_objects"] == 0
        prefix, body = constants["ASSEMBLY"][label]
        parts = [
            git_bytes("crates/keelshell-session/src/monitor/windows/" + name)
            for name in ("common.ps1", body)
        ]
        script = prefix + b"".join(parts).decode("utf-8") + constants["SUFFIX"]
        wire = constants["POWERSHELL"] + base64.b64encode(script.encode("utf-16le")).decode("ascii")
        assert files[f"direct-artifact-r1/{label}.wire.txt"] == wire.encode("ascii")
        assert len(wire) < 8000 and shell["production_command_ascii_bytes"] == len(wire)
        assert child["command_chars"] < 8191
        assert files[f"direct-artifact-r1/{label}.stderr"] == b""
    compare = doc("direct-artifact-r1/comparison.json")
    assert compare["owned"]["owned_endpoints_matched"] == 4
    assert compare["owned"]["connected_tcp_excluded"] == 2
    assert compare["resources"]["networks_matched"] == 3
    assert len(compare["resources"]["capacity_inventory"]["stable_identities"]) == 2
    assert compare["resources_skipped"]["networks_matched"] == 3
    assert compare["resources_skipped"]["capacity_status"] == "skipped"

    with tarfile.open(fileobj=io.BytesIO(git_bytes(
        "docs/testing/records/2026-11-17-windows-read-only-monitor-evidence/originals.tar.gz"
    )), mode="r:gz") as archive:
        freeze = json.loads(archive.extractfile("integration/full-gate-inputs-r3.json").read())
    assert freeze["count"] == len(freeze["inputs"]) == 758
    assert sha("".join(f"{r['path']}\0{r['sha256']}\n" for r in freeze["inputs"]).encode()) == freeze["aggregate_sha256"]

    def verify_input(row):
        data = git_bytes(row["path"])
        assert len(data) == row["bytes"] and sha(data) == row["sha256"]

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(verify_input, freeze["inputs"]))
    print(json.dumps({
        "source": SOURCE, "members": len(files), "engineering_inputs": 758,
        "quality": quality, "actual_windows_api": "PASS (Server2025 AMD64)",
        "powershell_original_processes": 5, "fixture_terminal": True, "capture_replay_passed": 1,
        "actual_windows_ssh": "UNRUN/HOLD", "native_desktop": "UNRUN",
        "tests_replayed": 0, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
