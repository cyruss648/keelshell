"""Verify sealed CI bytes and named results offline; never execute archived helpers."""
import argparse
from collections import Counter
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import tarfile
import zipfile

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[4]
SOURCE = "6ae91d2e90668e7d9c04a2a7eea7c5de1bf6d3d4"
OLD = "docs/testing/records/2026-11-17-monitor-read-cancellation-evidence"
ROOTS = {
    "collector": "work/monitor-cancellation-ci-completion-20261117",
    "review": "work/monitor-cancellation-ci-review-20261117",
}
CHECKS = []


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def check(label, condition):
    CHECKS.append(label)
    if not condition:
        raise ValueError(label)


def safe(name):
    path = PurePosixPath(name)
    return bool(name) and not path.is_absolute() and ".." not in path.parts and \
        "\\" not in name and not any(ord(c) < 32 for c in name)


def source_files():
    files = {}
    for prefix, relative in ROOTS.items():
        for directory, dirs, names in os.walk(ROOT / relative, followlinks=False):
            check("no linked source directories", all(
                not (Path(directory) / name).is_symlink() for name in dirs))
            for name in names:
                path = Path(directory) / name
                check("regular source member", stat.S_ISREG(path.lstat().st_mode))
                key = prefix + "/" + path.relative_to(ROOT / relative).as_posix()
                check("safe source name", safe(key))
                files[key] = path.read_bytes()
    return files


def unpack(raw, manifest):
    check("archive digest", sha(raw) == manifest["sha256"])
    check("archive bytes", len(raw) == manifest["bytes"])
    rows = {r["member"]: r for r in manifest["files"]}
    check("unique manifest", len(rows) == len(manifest["files"]) == manifest["members"])
    files = {}
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
        members = archive.getmembers()
        check("exact archive member set", len(members) == len(rows) and
              {m.name for m in members} == set(rows))
        for member in members:
            check("safe regular archive member", safe(member.name) and member.isfile())
            value = archive.extractfile(member).read()
            row = rows[member.name]
            check("member length " + member.name, len(value) == row["bytes"])
            check("member digest " + member.name, sha(value) == row["sha256"])
            files[member.name] = value
    check("logical bytes", sum(map(len, files.values())) == manifest["logical_bytes"])
    return files


def normalized(raw):
    return [re.sub(r"^\ufeff?\d{4}-\d\d-\d\dT\S+ ", "",
                   re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", s))
            for s in raw.decode("utf-8-sig").splitlines()]


def rust(raw):
    """Classify at group start, because stderr can precede the prior summary."""
    kind, active, groups = "ordinary", None, []
    for number, text in enumerate(normalized(raw), 1):
        if text.lstrip().startswith("Doc-tests "):
            kind = "rustdoc"
        elif re.match(r"^\s*Running\s+(?:unittests|tests[/\\])", text):
            kind = "ordinary"
        start = re.fullmatch(r"running (\d+) tests?", text)
        if start:
            check("non-overlapping Rust group", active is None)
            active = {"kind": kind, "declared": int(start[1]), "tests": [],
                      "start_line": number}
        candidate = re.sub(r"^[.s]+(?=test .+ \.\.\. (?:ok|FAILED|ignored))", "", text)
        case = re.fullmatch(r"test (.+?) \.\.\. (ok|FAILED|ignored)(?:, .*)?", candidate)
        if case and active is not None:
            active["tests"].append({"name": case[1], "status": case[2], "line": number})
        result = re.match(
            r"^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
            r"(\d+) measured; (\d+) filtered out;", text)
        if result:
            check("result belongs to a group", active is not None)
            counts = dict(zip(("passed", "failed", "ignored", "measured", "filtered"),
                              map(int, result.groups()[1:])))
            check("named Rust counts", Counter(t["status"] for t in active["tests"]) ==
                  Counter({"ok": counts["passed"], "FAILED": counts["failed"],
                           "ignored": counts["ignored"]}))
            check("declared Rust count", active["declared"] ==
                  counts["passed"] + counts["failed"] + counts["ignored"])
            check("unique Rust names", len(active["tests"]) ==
                  len({t["name"] for t in active["tests"]}))
            check("successful Rust group", result[1] == "ok" and counts["failed"] == 0)
            active.update(counts, result_line=number)
            groups.append(active)
            active = None
    check("all groups closed", active is None)
    totals = {category: {
        "groups": sum(g["kind"] == category for g in groups),
        **{key: sum(g[key] for g in groups if g["kind"] == category)
           for key in ("passed", "failed", "ignored", "measured", "filtered")},
    } for category in ("ordinary", "rustdoc")}
    return totals, groups


def python_main(raw):
    section = "\n".join(normalized(raw)).split("Running: cargo fmt --all --check", 1)[0]
    runs = re.findall(r"^Ran (\d+) tests? in [\d.]+s$", section, re.M)
    ends = re.findall(r"^OK(?: \(skipped=(\d+)\))?$", section, re.M)
    check("one Python main terminal pair", len(runs) == len(ends) == 1)
    check("Python main no failure", not re.search(r"^(?:FAILED|FAIL:|ERROR:)", section, re.M))
    return {"run": int(runs[0]), "skipped": int(ends[0] or 0)}


def git_blobs(commit, paths):
    check("full Git commit", re.fullmatch(r"[0-9a-f]{40}", commit) is not None)
    result = subprocess.run(["git", "cat-file", "--batch"], cwd=ROOT,
                            input="".join(f"{commit}:{p}\n" for p in paths).encode(),
                            capture_output=True, timeout=45, check=True)
    stream, values = io.BytesIO(result.stdout), {}
    for path in paths:
        header = stream.readline().split()
        check("committed blob " + path, len(header) == 3 and header[1] == b"blob")
        value = stream.read(int(header[2]))
        check("blob framing", len(value) == int(header[2]) and stream.read(1) == b"\n")
        values[path] = value
    check("batch exhausted", stream.read() == b"")
    return values


def bind_source():
    paths = [OLD + "/manifest.json", OLD + "/originals.tar.gz"]
    blobs = git_blobs(SOURCE, paths)
    old_files = unpack(blobs[paths[1]], json.loads(blobs[paths[0]]))
    candidate = json.loads(old_files["parent/candidate-inputs-r3.json"])
    rows = candidate["inputs"]
    check("777 old engineering inputs", candidate["count"] == len(rows) == 777)
    check("input paths unique", len({r["path"] for r in rows}) == 777)
    canonical = json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()
    check("old source aggregate", sha(canonical) == candidate["aggregate_sha256"] ==
          "6d495b849c63e00dfc7ba8b867f3fa8b5ba3481de9e4b66669dee77f40fa22d5")
    inputs = git_blobs(SOURCE, [r["path"] for r in rows])
    for row in rows:
        value = inputs[row["path"]]
        check("old Git input length " + row["path"], len(value) == row["bytes"])
        check("old Git input digest " + row["path"], sha(value) == row["sha256"])
    return {"source": SOURCE, "engineering_inputs": 777,
            "aggregate_sha256": sha(canonical), "all_exact_git_blobs_equal": True}


def verify(files):
    def doc(name):
        return json.loads(files[name])

    source_counts, checksum_counts = {}, {}
    for prefix in ROOTS:
        subset = {k[len(prefix) + 1:]: v for k, v in files.items()
                  if k.startswith(prefix + "/")}
        source_counts[prefix] = {"members": len(subset), "bytes": sum(map(len, subset.values()))}
        listed = {}
        for line in subset["SHA256SUMS"].decode().splitlines():
            digest, name = line.split("  ", 1)
            check("checksum path unique and safe", name not in listed and safe(name))
            listed[name] = digest
            check("source checksum " + prefix + "/" + name, sha(subset[name]) == digest)
        check("complete source checksum coverage", set(listed) == set(subset) - {"SHA256SUMS"})
        checksum_counts[prefix] = len(listed)
    check("complete two sources", source_counts == {
        "collector": {"members": 195, "bytes": 1041746},
        "review": {"members": 254, "bytes": 12010610}})
    decision = doc("review/decision.json")
    check("independent limited decision", decision["decision"] == "APPROVE_CI_EVIDENCE_ONLY" and
          decision["reviewed_engineering_source"] == SOURCE and
          decision["independent_checks"] == {"passed": 3318, "failed": 0} and
          not any(decision["findings"].values()))
    checks = doc("review/independent-checks.json")["checks"]
    check("3318 saved successful checks", len(checks) == 3318 and all(c["passed"] for c in checks))
    for attempt, code in (("r1", 1), ("r2", 1), ("r3", 0)):
        receipt = doc(f"review/receipts/recompute-{attempt}-stage.json")
        check("helper attempt preserved " + attempt, receipt["returncode"] == code and
              receipt["reaped"] and receipt["natural_exit"] and not receipt["timed_out"])
        for channel in ("stdout", "stderr"):
            raw = files[f"review/receipts/recompute-{attempt}.{channel}"]
            check("helper output binding", len(raw) == receipt[channel + "_bytes"] and
                  sha(raw) == receipt[channel + "_sha256"])
    check("historical Quality pending preserved",
          doc("collector/snapshots/004/44040248864-run.json")["status"] == "in_progress")
    runs = {}
    for run_id, prefix in (
        (44040248864, "review/originals/quality-001/"),
        (44040248806, "collector/snapshots/004/"),
        (44040248746, "collector/snapshots/004/"),
    ):
        quality = run_id == 44040248864
        run = doc(prefix + ("run.json" if quality else f"{run_id}-run.json"))
        jobs = doc(prefix + ("jobs.json" if quality else f"{run_id}-jobs.json"))
        check("exact completed run", run["id"] == run_id and run["head_sha"] == SOURCE and
              run["run_attempt"] == 1 and run["status"] == "completed" and
              run["conclusion"] == "success" and
              run["repository"]["full_name"] == "cyruss648/keelshell")
        check("complete jobs", jobs["total_count"] == len(jobs["jobs"]))
        for job in jobs["jobs"]:
            individual = doc((prefix + f'job-{job["id"]}.json') if quality else
                             f'collector/jobs/{job["id"]}.json')
            check("original job matches list", job == individual)
            check("exact completed job", job["run_id"] == run_id and
                  job["head_sha"] == SOURCE and job["run_attempt"] == 1 and
                  job["status"] == "completed" and
                  job["conclusion"] == ("skipped" if job["id"] == 133892796479 else "success"))
        runs[str(run_id)] = {"status": run["status"], "conclusion": run["conclusion"],
                            "jobs": [j["id"] for j in jobs["jobs"]]}
    zip_rows, expanded = [], {}
    for row in doc("review/zip-inventory.json"):
        group = "collector" if row["group"] == "parent" else "review"
        relative = row["path"].split("/" + ROOTS[group] + "/", 1)[1]
        key = group + "/" + relative
        raw = files[key]
        check("original ZIP binding", len(raw) == row["bytes"] and sha(raw) == row["sha256"])
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            check("ZIP CRC", archive.testzip() is None)
            infos = [i for i in archive.infolist() if not i.is_dir()]
            expected = {r["member"]: r for r in row["members"]}
            check("ZIP exact member names", len(infos) == len(expected) ==
                  len(row["members"]) and {i.filename for i in infos} == set(expected))
            for item in infos:
                check("safe ZIP member", safe(item.filename) and
                      stat.S_IFMT(item.external_attr >> 16) in (0, stat.S_IFREG))
                value = archive.read(item)
                member = expected[item.filename]
                check("ZIP member binding", len(value) == member["bytes"] ==
                      item.file_size and sha(value) == member["sha256"])
                check("extracted original bytes", files["review/" + member["extracted"]] == value)
                expanded[(key, item.filename)] = value
        zip_rows.append({"path": key, "members": len(infos),
                         "bytes": len(raw), "sha256": sha(raw)})
    check("seven ZIPs and 172 members", len(zip_rows) == 7 and
          sum(r["members"] for r in zip_rows) == 172)
    artifacts = []
    for artifact_id, run_id in ((14308219775, 44040248746), (14308319951, 44040248806),
                                (14309783967, 44040248864), (14309968324, 44040248864)):
        if run_id == 44040248864:
            prefix = "review/originals/quality-001/"
            metadata = doc(prefix + f"artifact-{artifact_id}.json")
            listing = doc(prefix + "artifacts.json")["artifacts"]
            raw = files[prefix + f"artifact-{artifact_id}.zip"]
        else:
            metadata = doc(f"collector/artifacts/{artifact_id}.json")
            listing = doc(f"collector/snapshots/004/{run_id}-artifacts.json")["artifacts"]
            raw = files[f"collector/downloads/artifact-{artifact_id}.zip"]
        check("official artifact metadata", metadata == next(a for a in listing
              if a["id"] == artifact_id) and metadata["workflow_run"]["id"] == run_id and
              metadata["workflow_run"]["head_sha"] == SOURCE)
        check("official artifact digest and size", metadata["digest"] == "sha256:" + sha(raw)
              and metadata["size_in_bytes"] == len(raw))
        artifacts.append({"id": artifact_id, "run": run_id, "bytes": len(raw), "sha256": sha(raw)})
    quality_counts = {}
    for platform, passed, ignored, skipped in (
        ("windows-2025", 3453, 66, 53), ("macos-26", 3466, 68, 3),
        ("ubuntu-24.04", 3459, 69, 3),
    ):
        prefix = f"native ({platform})/"
        key = "review/originals/quality-001/run-logs.zip"
        raw = expanded[(key, prefix + "8_Rust quality gate.txt")]
        checkout, = [v for (z, name), v in expanded.items()
                     if z == key and name.startswith(prefix + "2_Run actions_checkout@")]
        checkout_lines = normalized(checkout)
        check("checkout exact log1 source", any(
            text.endswith(" log -1 --format=%H") and
            number + 1 < len(checkout_lines) and checkout_lines[number + 1] == SOURCE
            for number, text in enumerate(checkout_lines)))
        totals, groups = rust(raw)
        check("expected ordinary totals", totals["ordinary"] ==
              {"groups": 96, "passed": passed, "failed": 0, "ignored": ignored,
               "measured": 0, "filtered": 0})
        check("expected rustdoc totals", totals["rustdoc"] ==
              {"groups": 6, "passed": 15, "failed": 0, "ignored": 0,
               "measured": 0, "filtered": 0})
        saved = doc(f"review/inventories/{platform}-rust.json")
        check("saved inventory totals", saved["totals"] == totals)
        check("every saved name and line", len(groups) == len(saved["groups"]) and all(
            all(g[k] == s[k] for k in g) for g, s in zip(groups, saved["groups"])))
        py = python_main(raw)
        check("Python report agrees", py == {"run": 526, "skipped": skipped} and
              all(decision["quality"][platform]["python_main"][k] == v for k, v in py.items()))
        check("Rust report agrees", decision["quality"][platform]["rust"] == totals)
        quality_counts[platform] = {"rust": totals, "python_main": py}
    interop = []
    for (key, name), raw in expanded.items():
        if not key.startswith("review/") or not name.endswith("/result.json"):
            continue
        result = json.loads(raw)
        totals, groups = rust(expanded[(key, str(PurePosixPath(name).with_name("tests.log")))])
        expected = result["passed_tests"]
        check("interop 18 or forwarding 6", expected in (18, 6) and
              totals["ordinary"]["passed"] == expected and len(groups) == 1)
        check("interop outcome and cleanup", result["outcome"] == "passed" and
              all(s["returncode"] == 0 for s in result["steps"]) and
              result["cleanup"]["owned_processes_stopped"] and
              result["cleanup"]["temporary_directory_removed"] and
              not result["cleanup"]["ancestry_unverified"])
        interop.append({"platform": result["platform"], "passed": expected})
    check("four interop results", len(interop) == 4)
    ssh = "review/extracted/parent/artifact-14308219775/"
    seal, final = doc(ssh + "ssh-debug-originals.json"), doc(ssh + "ssh-debug-cleanup.json")
    check("seal HOLD remains", seal["state"] == "HOLD" and not seal["terminal_proven"])
    identities = lambda rows: {(r["pid"], r["creation_time"]) for r in rows}
    check("21 original identities preserved", len(final["admissions"]) ==
          len(identities(final["admissions"])) == 21 and
          identities(seal["admissions"]) == identities(final["admissions"]))
    nonmasters = [r for r in final["admissions"] if not r["is_master"]]
    counts = Counter(str(r["exit_code"]) for r in nonmasters)
    check("natural nonzero SSH terminals retained", counts == {"0": 14, "255": 5, "1": 1}
          and all(r["natural_terminal_before_cleanup"] for r in nonmasters))
    master, = [r for r in final["admissions"] if r["is_master"]]
    check("master cleanup125 non-natural retained", master["exit_code"] == 125 and
          not master["natural_terminal_before_cleanup"])
    check("SSH cleanup boundary", final["state"] == "PASS" and final["closed"] and
          all(final[k] == 0 for k in ("job_active_processes", "pending_process_objects",
                                     "owned_handles_remaining", "debug_connections")))
    for artifact_id in (14308219775, 14308319951):
        check("native UNRUN retained", doc(
            f"review/extracted/parent/artifact-{artifact_id}/receipt.json")["native_gui"] == "UNRUN")
    check("direct API SSH UNRUN retained", doc(
        "review/extracted/parent/artifact-14308319951/receipt.json")["ssh"] == "UNRUN")
    check("API isolated SSH skipped", doc("collector/jobs/133892796479.json")["conclusion"] == "skipped")
    return {"source": SOURCE, "sources": source_counts, "source_checksum_entries": checksum_counts,
            "runs": runs, "quality": quality_counts, "original_zips": zip_rows,
            "official_artifact_digests": artifacts, "interop": interop,
            "independent_decision": decision["decision"], "independent_saved_checks": 3318,
            "boundary": "CI evidence only. SSH HOLD, 255x5/1x1, master cleanup125 non-natural, "
                        "API isolated SSH skipped, native UNRUN; current MCP/native unapproved.",
            "product_tests_replayed": 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", action="store_true", help="read the two source trees before sealing")
    parser.add_argument("--bind-source", action="store_true", help="bind 777 inputs to the exact old engineering Git objects")
    parser.add_argument("--commit", help="also bind every delivered CI file and appended record to this full Git commit")
    args = parser.parse_args()
    check("commit mode uses archive", not (args.commit and args.sources))
    if not args.sources:
        listed = {}
        for line in (BASE / "SHA256SUMS").read_text().splitlines():
            digest, name = line.split("  ", 1)
            check("safe unique delivery checksum", safe(name) and name not in listed)
            listed[name] = digest
            check("delivery checksum " + name, sha((BASE / name).read_bytes()) == digest)
        check("complete delivery checksum set", set(listed) == {
            p.name for p in BASE.iterdir() if p.is_file() and p.name != "SHA256SUMS"})
    files = source_files() if args.sources else unpack(
        (BASE / "originals.tar.gz").read_bytes(), json.loads((BASE / "manifest.json").read_bytes()))
    result = verify(files)
    if args.bind_source or args.commit:
        result["old_engineering_binding"] = bind_source()
    if args.commit:
        paths = [p.relative_to(ROOT).as_posix() for p in sorted(BASE.iterdir()) if p.is_file()]
        paths.append(OLD.removesuffix("-evidence") + ".md")
        blobs = git_blobs(args.commit, paths)
        for path in paths:
            check("delivered committed bytes " + path, blobs[path] == (ROOT / path).read_bytes())
        result["delivery_commit"] = args.commit
    result.update(passed=len(CHECKS), failed=0, mismatches=[])
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
