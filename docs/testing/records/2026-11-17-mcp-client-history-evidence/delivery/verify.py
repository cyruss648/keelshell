"""Verify committed delivery/review originals and their recorded native provenance."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

MAIN = "docs/testing/records/2026-11-17-mcp-client-history-evidence"
CI = "docs/testing/records/2026-11-17-tunnel-transport-lifetime-evidence/ci-completion"
SOURCE = "f910e0013832643f09b112b08443675a956d5551"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def git_blob(root, commit, path):
    return subprocess.check_output(
        ["git", "cat-file", "blob", f"{commit}:{path}"], cwd=root, timeout=30
    )


def verify_archive(root, commit, directory):
    original = git_blob(root, commit, f"{directory}/originals.tar.gz")
    manifest_raw = git_blob(root, commit, f"{directory}/manifest.json")
    assert original == (root / directory / "originals.tar.gz").read_bytes()
    assert manifest_raw == (root / directory / "manifest.json").read_bytes()
    manifest = json.loads(manifest_raw)
    assert len(original) == manifest["bytes"] and sha(original) == manifest["sha256"]
    rows = {row["member"]: row for row in manifest["files"]}
    assert len(rows) == manifest["members"]
    contents = {}
    with tarfile.open(fileobj=io.BytesIO(original)) as archive:
        members = archive.getmembers()
        assert len(members) == len(rows)
        assert {member.name for member in members} == set(rows)
        for member in members:
            assert member.isfile()
            raw = archive.extractfile(member).read()
            row = rows[member.name]
            assert len(raw) == row["bytes"] and sha(raw) == row["sha256"]
            contents[member.name] = raw
    assert sum(len(raw) for raw in contents.values()) == manifest["logical_bytes"]
    return {
        "path": directory, "members": len(contents),
        "sha256": manifest["sha256"], "all_members_equal": True,
    }, contents


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    assert re.fullmatch(r"[0-9a-f]{40}", args.commit)
    root = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True, timeout=10
    ).strip())
    base = subprocess.run(
        ["python3", "-B", str(root / MAIN / "verify_git_binding.py"),
         "--commit", args.commit],
        cwd=root, capture_output=True, text=True, timeout=60, check=True,
    )
    binding = json.loads(base.stdout)
    review, _ = verify_archive(root, args.commit, f"{CI}/review")
    delivery, content = verify_archive(root, args.commit, f"{MAIN}/delivery")
    observed = json.loads(content["parent/native-observations.json"])
    package = json.loads(content["parent/normal-package-manifest.json"])
    start = json.loads(content["parent/native-app-start.json"])
    end = json.loads(content["parent/native-app-result.json"])
    assert observed["source"] == package["build"]["commit"] == start["source"] == end["source"] == SOURCE
    assert start["pid"] == end["pid"] == observed["native_process"]["pid"]
    assert end["exit_code"] == 0 and end["reaped"] and not end["timed_out"]
    assert package["installed"] is False
    assert package["binary_sha256"] == start["binary_sha256"] == end["binary_sha256"]
    assert len(observed["images"]) == 11
    for image in observed["images"]:
        raw = content[f"parent/native-captures/{image['image']}"]
        ax = content[f"parent/native-captures/{image['ax']}"]
        assert len(raw) == image["bytes"] and sha(raw) == image["sha256"]
        assert sha(ax) == image["ax_sha256"]
        assert raw.startswith(b"\xff\xd8")
        assert image["pixel_dimensions"] == {"width": 2880, "height": 1866}
    for path, digest in package["files"].items():
        assert observed["package_files"][path]["sha256"] == digest
    screenshot_directory = "docs/design/screenshots/2026-11-17-mcp"
    screenshot_manifest = json.loads(git_blob(
        root, args.commit, f"{screenshot_directory}/manifest.json"
    ))
    assert screenshot_manifest["source"] == SOURCE
    assert len(screenshot_manifest["files"]) == 3
    for image in screenshot_manifest["files"]:
        raw = git_blob(root, args.commit, f"{screenshot_directory}/{image['file']}")
        assert raw == content[f"parent/native-captures/{image['capture']}"]
        assert len(raw) == image["bytes"] and sha(raw) == image["sha256"]
    print(json.dumps({
        "commit": args.commit, "engineering_binding": binding,
        "additional_archives": [review, delivery],
        "all_archived_members": binding["members"] + review["members"] + delivery["members"],
        "native_source": SOURCE, "native_original_image_ax_pairs": 11,
        "committed_unmodified_design_previews": 3,
        "native_process_exit": 0, "native_process_reaped": True,
        "boundary": "Offline exact committed original/provenance checks. No product rerun, shared target, GUI, network, installed-client or release acceptance.",
    }))


if __name__ == "__main__":
    main()
