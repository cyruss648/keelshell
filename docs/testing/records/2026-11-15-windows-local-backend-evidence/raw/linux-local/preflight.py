"""One-shot, owned cached-image and registry preflight; no host Cargo."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import time
import tomllib
import uuid

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import run_linux_file_interop as fixture

OUT = Path(__file__).resolve().parent / "preflight-r1"
OUT.mkdir(exist_ok=False)
BASE = "a092dd5712c0a92ee4af18970fb0500fb230b760"
owner = uuid.uuid4().hex
receipt = {"baseline": BASE, "owner": owner, "steps": []}
cmd = fixture.Commands(OUT, receipt, lambda: None)
container = None
try:
    cmd.run("archive", [
        "git", "archive", "--format=tar", "--output=" + str(OUT / "source.tar"),
        BASE, "Cargo.toml", "Cargo.lock", "crates",
    ], timeout=30)
    receipt["archive_sha256"] = fixture.digest((OUT / "source.tar").read_bytes())
    with tarfile.open(OUT / "source.tar") as archive:
        lock = tomllib.loads(archive.extractfile("Cargo.lock").read().decode())
    cache = Path.home() / ".cargo/registry/cache/rsproxy.cn-e3de039b2554c837"
    src = Path.home() / ".cargo/registry/src/rsproxy.cn-e3de039b2554c837"
    packages = []
    for package in lock["package"]:
        if "source" not in package:
            continue
        name = package["name"] + "-" + package["version"]
        path = cache / (name + ".crate")
        item = {"name": name, "source": package["source"], "cached": path.is_file()}
        if path.is_file():
            item["checksum_matches"] = fixture.digest(path.read_bytes()) == package["checksum"]
        manifest = src / name / "Cargo.toml"
        if manifest.is_file():
            item["rust_version"] = tomllib.loads(manifest.read_text()).get("package", {}).get("rust-version")
        packages.append(item)
    receipt["packages"] = packages
    receipt["cache_missing"] = [p["name"] for p in packages if not p["cached"]]
    receipt["checksum_mismatches"] = [p["name"] for p in packages if p.get("checksum_matches") is False]
    name = "keelshell-local-probe-" + owner[:12]
    _, result = cmd.run("create", [
        "podman", "create", "--pull=never", "--http-proxy=false", "--network=none",
        "--label", "io.keelshell.fixture=linux-local-backend-probe",
        "--label", "io.keelshell.owner=" + owner, "--name", name,
        "docker.io/library/rust:1.98.1-slim-bookworm", "sh", "-ec",
        "uname -sm\nrustc -vV\ncargo -V\ncc --version\nldd --version\n",
    ], timeout=20)
    container = result.decode().strip()
    cmd.run("identity", [
        "podman", "inspect", "--type=container", "--format",
        '{"Id":{{json .Id}},"Name":{{json .Name}},"Labels":{{json .Config.Labels}}}',
        container,
    ], timeout=15)
    identity = json.loads((OUT / receipt["steps"][-1]["stdout_log"]).read_text())
    if identity["Id"] != container or identity["Labels"].get("io.keelshell.owner") != owner:
        raise RuntimeError("Ownership mismatch")
    cmd.run("toolchain", ["podman", "start", "--attach", container], timeout=30)
finally:
    if container:
        cmd.run("cleanup-identity", [
            "podman", "inspect", "--type=container", "--format",
            '{"Id":{{json .Id}},"Name":{{json .Name}},"Labels":{{json .Config.Labels}}}',
            container,
        ], timeout=10)
        data = json.loads((OUT / receipt["steps"][-1]["stdout_log"]).read_text())
        if data.get("Id") == container and data.get("Labels", {}).get("io.keelshell.owner") == owner:
            cmd.run("remove", ["podman", "rm", "-f", container], timeout=15)
            rc, _ = cmd.run("absent", ["podman", "container", "exists", container], timeout=10, check=False)
            receipt["container_removed"] = rc == 1
    receipt["owned_processes_stopped"] = cmd.finish()
    fixture.write_json(OUT / "receipt.json", receipt)
print(json.dumps({
    "archive_sha256": receipt["archive_sha256"],
    "registry_packages": len(receipt["packages"]),
    "cache_missing": receipt["cache_missing"],
    "checksum_mismatches": receipt["checksum_mismatches"],
    "toolchain_log": receipt["steps"][3]["stdout_log"],
    "container_removed": receipt.get("container_removed"),
    "owned_processes_stopped": receipt["owned_processes_stopped"],
}, indent=2))
