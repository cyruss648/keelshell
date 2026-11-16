"""Recompute exact CI results from sealed originals without replaying tests."""
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import tarfile

BASE = Path(__file__).resolve().parent
SOURCE = "bd198a8ef2578ea9b96e0a603cd705bb2eadca13"
RUN = 43916079699


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    manifest = json.loads((BASE / "manifest.json").read_text())
    raw = (BASE / "originals.tar.gz").read_bytes()
    assert len(raw) == manifest["archive_bytes"]
    assert digest(raw) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        for member in archive:
            name = PurePosixPath(member.name)
            assert member.isfile() and not name.is_absolute() and ".." not in name.parts
            assert member.name not in files
            files[member.name] = archive.extractfile(member).read()
    assert len(files) == manifest["count"]
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        assert len(files[row["path"]]) == row["bytes"]
        assert digest(files[row["path"]]) == row["sha256"]

    def command(label, code=0):
        receipt = json.loads(files[label + ".json"])
        assert receipt["controller_exit"] == receipt["observed_exit"] == code
        assert not receipt["timed_out"] and not receipt["interrupted"]
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0
        assert terminal["pipes_drained"] and not terminal["errors"]
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"] and digest(raw) == receipt["log_sha256"]
        return raw

    parser = {"__name__": "sealed_ci_parser"}
    exec(compile(files["previous-ci/summarize_ci_r3.py"], "sealed_ci_parser", "exec"), parser)
    run = json.loads(command("ci-previous-final-r1"))
    assert run["headSha"] == SOURCE and run["databaseId"] == RUN
    assert run["status"] == "completed" and run["conclusion"] == "success"
    assert len(run["jobs"]) == 3
    assert all(job["status"] == "completed" and job["conclusion"] == "success"
               for job in run["jobs"])
    facts = json.loads(files["previous-ci/facts-r1.json"])
    assert facts["source"] == SOURCE and facts["run"] == RUN
    assert facts["tests_replayed"] == 0 and not facts["native_desktop_acceptance"]
    expected = {
        "windows": (133470464623, "r1", 3259, 49, 1356, 34, "skipped"),
        "macos": (133470464468, "r2", 3272, 51, 1367, 2, "success"),
        "ubuntu": (133470463980, "r2", 3265, 52, 1368, 2, "success"),
    }
    for platform, (job_id, revision, passed, ignored, app_passed, skipped, ssh) in expected.items():
        raw = command(f"ci-previous-{platform}-log-{revision}")
        summary = parser["summarize"](raw)
        saved = json.loads(files[f"previous-ci/{platform}-summary-r1.json"])
        summary["log"] = saved["log"]
        assert summary == saved
        suites = summary["suites"]
        docs = summary["doctest_runs_unassigned"]
        assert len(suites) == 92
        assert not summary["inventory_result_summary_mismatches"]
        assert all(row["summary"]["state"] == "ok" and row["summary"]["failed"] == 0
                   for row in suites + docs)
        assert [sum(row["summary"][key] for row in suites)
                for key in ("passed", "failed", "ignored")] == [passed, 0, ignored]
        assert sum(row["summary"]["passed"] for row in docs) == 14
        app = [row for row in suites if "keelshell_app" in row["suite"]]
        assert len(app) == 1
        assert [app[0]["summary"][key] for key in ("passed", "failed", "ignored")] == [
            app_passed, 0, 2,
        ]
        text = parser["normalize"](raw)
        discovery = text.split(" -m unittest discover -s scripts -p test_*.py", 1)[1].split(
            "Running: cargo fmt", 1,
        )[0]
        assert re.findall(r"Ran (\d+) tests? in", discovery) == ["293"]
        assert re.findall(r"OK \(skipped=(\d+)\)", discovery) == [str(skipped)]
        job = next(job for job in run["jobs"] if job["databaseId"] == job_id)
        step = next(step for step in job["steps"]
                    if step["name"] == "OpenSSH interoperability (macOS and Linux)")
        assert step["conclusion"] == ssh
        row = facts["platforms"][platform]
        assert row["job"] == job_id and row["ordinary_rust"] == {
            "passed": passed, "failed": 0, "ignored": ignored,
        }
        assert row["app"] == app[0]["summary"]
        assert row["python_run"] == 293 and row["python_skipped"] == skipped
        assert row["openssh_step"] == ssh and not row["compiled_app_inventory"]
    command("ci-previous-watch-r1")
    binding = json.loads(files["ci-previous-source-binding-r1.json"])
    assert binding["command"][-2:] == ["--source", SOURCE]
    assert SOURCE in command("ci-previous-source-binding-r1").decode()
    for platform in ("ubuntu", "macos"):
        assert "terminal escape sequences" in command(f"ci-previous-{platform}-log-r1", 1).decode()
    print(json.dumps({
        "source": SOURCE, "run": RUN, "members": len(files),
        "three_platforms_success": True, "mismatches": [],
        "tests_replayed": 0, "native_desktop_acceptance": False,
    }))


if __name__ == "__main__":
    main()
