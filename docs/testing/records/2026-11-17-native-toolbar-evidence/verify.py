"""Verify native toolbar originals without launching an app or replaying actions."""
import json
from pathlib import Path
import runpy
import struct
import subprocess

BASE = Path(__file__).resolve().parent
common = runpy.run_path(str(BASE.parent / "2026-11-17-windows-openssh-monitor-evidence/repair/verify.py"))
digest = common["digest"]


def jpeg_size(raw):
    assert raw[:2] == b"\xff\xd8"
    offset = 2
    while offset < len(raw):
        assert raw[offset] == 255
        while raw[offset] == 255:
            offset += 1
        marker = raw[offset]
        offset += 1
        assert marker not in (0xD9, 0xDA), "JPEG size was not found before image data"
        length = struct.unpack_from(">H", raw, offset)[0]
        if marker in (0xC0, 0xC1, 0xC2):
            height, width = struct.unpack_from(">HH", raw, offset + 3)
            return [width, height]
        offset += length
    raise AssertionError("Missing JPEG frame")


def main():
    files = common["unpack"](BASE)
    observation = json.loads(files["native/observation.json"])
    assert len(observation["captures"]) == 10
    assert not observation["compact_window_acceptance"]
    assert not observation["original_full_toolbar_p2_closed"]
    for entry in observation["captures"]:
        image, ax = files["native/" + entry["image"]], files["native/" + entry["ax"]]
        assert len(image) == entry["image_bytes"] and digest(image) == entry["image_sha256"]
        assert len(ax) == entry["ax_bytes"] and digest(ax) == entry["ax_sha256"]
        assert jpeg_size(image) == entry["dimensions"] == [2880, 1866]
        assert entry["actual_format"] == "JPEG"
    receipt = json.loads(files["native/native-receipt.json"])
    assert receipt["observed_exit"] == receipt["controller_exit"] == 0
    assert not receipt["timed_out"] and not receipt["interrupted"] and not receipt["errors"]
    assert receipt["terminal"] == {
        "direct_reaped": True, "active_processes": 0, "pipes_drained": True, "errors": [],
    }
    binding = json.loads(files["native/package-binding.json"])
    package = json.loads(files["native/package-manifest.json"])
    assert digest(files["native/package-manifest.json"]) == binding["package_manifest_sha256"]
    assert package["build"]["commit"] == binding["source"] == observation["package_source"]
    assert receipt["executable_sha256"] == package["binary_sha256"]
    assert not package["installed"] and not observation["installed"]
    cleanup = json.loads(files["native/post-close-cleanup.json"])
    assert cleanup["pid"] == 2486 and cleanup["terminated"] and not cleanup["remaining"]
    assert cleanup["direct_reaped"] is None
    decision = json.loads(files["review/decision.json"])
    assert decision["decision"] == "APPROVE_SCOPED_NATIVE_TOOLBAR_EVIDENCE"
    assert not any(decision["finding_counts"].values()) and not decision["findings"]
    assert decision["visual_review"]["images_actually_opened"] == list(range(1, 11))
    assert decision["package_source"] == observation["package_source"]
    assert decision["bound_current_source"] == observation["current_source"]
    assert decision["compact_window"] == "HOLD" and not decision["full_native_finish"]
    assert not decision["old_p2"]["closed"]
    assert json.loads(files["agent-lifecycle.json"])["closed"]
    changes = subprocess.check_output([
        "git", "diff", "--name-only", observation["package_source"],
        observation["current_source"], "--", "crates", "Cargo.toml", "Cargo.lock",
    ], text=True, timeout=20).splitlines()
    assert changes == ["crates/keelshell-session/tests/windows_openssh_monitor.rs"]
    print(json.dumps({
        "members": len(files), "package_source": observation["package_source"],
        "unchanged_production_source_through": observation["current_source"],
        "raw_captures": 10, "format": "JPEG", "pixels": [2880, 1866],
        "original_exit": 0, "review": decision["decision"],
        "compact_window": "HOLD", "original_full_p2_closed": False,
        "full_native_finish": False, "actions_replayed": 0, "mismatches": [],
    }))


if __name__ == "__main__":
    main()
