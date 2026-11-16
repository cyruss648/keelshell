"""Verify the completed source CI archive without replaying tests."""

import hashlib
import importlib.util
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import tempfile

BASE = Path(__file__).resolve().parent
SOURCE = "6f28f5c424752a221746fbfb621e319c20ff38b9"
JOBS = {"windows": 133420285732, "macos": 133420286347, "ubuntu": 133420286130}
OS = {"windows": "windows-2025", "macos": "macos-26", "ubuntu": "ubuntu-24.04"}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    manifest = json.loads((BASE / "manifest.json").read_bytes())
    packed = (BASE / "originals.tar.gz").read_bytes()
    assert manifest["source"] == SOURCE and manifest["run"] == 43901863960
    assert len(packed) == manifest["archive_bytes"]
    assert sha(packed) == manifest["archive_sha256"]
    files = {}
    with tarfile.open(fileobj=io.BytesIO(packed), mode="r:gz") as archive:
        members = archive.getmembers()
        assert len(members) == manifest["count"]
        for member in members:
            path = PurePosixPath(member.name)
            assert member.isfile() and not path.is_absolute() and ".." not in path.parts
            assert member.name not in files
            files[member.name] = archive.extractfile(member).read()
    assert set(files) == {row["path"] for row in manifest["entries"]}
    for row in manifest["entries"]:
        data = files[row["path"]]
        assert len(data) == row["bytes"] and sha(data) == row["sha256"], row["path"]

    def successful(label):
        receipt = json.loads(files[label + ".json"])
        assert receipt["observed_exit"] == receipt["controller_exit"] == 0, label
        assert not receipt["timed_out"] and not receipt["interrupted"], label
        terminal = receipt["terminal"]
        assert terminal["direct_reaped"] and terminal["active_processes"] == 0, label
        assert terminal["pipes_drained"] and not terminal["errors"], label
        raw = files[label + ".log"]
        assert len(raw) == receipt["log_bytes"] and sha(raw) == receipt["log_sha256"], label
        return raw

    successful("ci-watch-r1")
    successful("cleanup-docs-push-r1")
    final = json.loads(successful("ci-final-r1"))
    assert final["headSha"] == SOURCE and final["databaseId"] == 43901863960
    assert final["status"] == "completed" and final["conclusion"] == "success"
    assert {job["databaseId"] for job in final["jobs"]} == set(JOBS.values())
    for job in final["jobs"]:
        assert job["status"] == "completed" and job["conclusion"] == "success"
        steps = {step["name"]: step for step in job["steps"]}
        assert steps["Rust quality gate"]["conclusion"] == "success"
        interop = steps["OpenSSH interoperability (macOS and Linux)"]["conclusion"]
        assert interop == ("skipped" if job["databaseId"] == JOBS["windows"] else "success")

    normalization = json.loads(files["ci-prefix-normalization-r2.json"])
    assert normalization["primary_logs_unchanged"]
    assert [row["observed_exit"] for row in normalization["diagnostics"]] == [1, 1]
    facts = json.loads(files["ci-facts-r2.json"])
    assert facts["source"] == SOURCE and facts["run"] == 43901863960
    assert facts["status"] == "completed" and facts["conclusion"] == "success"
    escaped_ansi = re.compile(r"\^\[\[[0-?]*[ -/]*[@-~]")
    with tempfile.TemporaryDirectory(prefix="keelshell-reload-ci-") as scratch:
        directory = Path(scratch)
        for name in ("summarize_ci_r3.py", "summarize_source_ci.py"):
            (directory / name).write_bytes(files[name])
        spec = importlib.util.spec_from_file_location(
            "reload_completed_ci", directory / "summarize_source_ci.py"
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for key, system in OS.items():
            raw = successful(f"ci-{key}-log-r1")
            normalized = []
            escape_count = 0
            for line in raw.decode().splitlines():
                fields = line.split("\t", 2)
                assert len(fields) == 3 and fields[0] == f"native ({system})"
                assert re.match(r"^\ufeff?\d{4}-\d\d-\d\dT[\d:.]+Z(?: |$)", fields[2])
                clean, count = escaped_ansi.subn("", fields[2])
                normalized.append(clean)
                escape_count += count
            data = ("\n".join(normalized) + "\n").encode()
            name = f"ci-{key}-timestamps-r2.log"
            assert data == files[name]
            derivation = next(
                row for row in normalization["derived_logs"] if row["derived"] == name
            )
            assert derivation["primary_sha256"] == sha(raw)
            assert derivation["derived_sha256"] == sha(data)
            assert derivation["line_count"] == len(normalized)
            assert derivation["removed_fields_per_line"] == 2
            assert derivation["removed_literal_caret_ansi"] == escape_count
            path = directory / name
            path.write_bytes(data)
            report = module.summarize(path, key)
            assert report == json.loads(files[f"ci-{key}-summary-r2.json"])
            expected = facts["platforms"][key]
            for field in ("ordinary_rust", "rustdoc", "python"):
                assert report[field] == expected[field]
            assert report["app"]["summary"] == expected["app"]
            assert len(report["workspace"]["suites"]) == expected["ordinary_suites"] == 92

    # The original source seal binds all 721 inputs to this historical commit,
    # rather than to the newer working diff used by the next feature batch.
    source_binding = subprocess.check_output(
        [sys.executable, "-B", str(BASE.parent / "verify.py"), "--source", SOURCE],
        cwd=BASE.parents[4],
        timeout=120,
    )
    assert json.loads(source_binding)["mismatches"] == 0
    print(json.dumps({
        "source": SOURCE, "run": 43901863960, "status": "success",
        "sealed_members": manifest["count"], "source_inputs": 721,
        "platforms": facts["platforms"], "tests_replayed": 0,
        "native_desktop_acceptance": False,
    }))


if __name__ == "__main__":
    main()
