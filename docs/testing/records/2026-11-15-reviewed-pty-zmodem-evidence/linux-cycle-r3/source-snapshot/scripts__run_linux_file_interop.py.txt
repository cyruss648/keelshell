#!/usr/bin/env python3
"""Own a disposable Linux SSH/lrzsz/ACL fixture without Cargo or host installs.

Use a cached public image, for example::

    python3 scripts/run_linux_file_interop.py \
        --image rust:1.98.1-slim-bookworm --host-package-bridge --hold-seconds 900

The new output directory contains readiness.json as soon as SSH authentication
works. Its environment_file points at a private, temporary JSON file. Load that
file into a child process environment to run a supplied, already-built test:

    ... -- /absolute/path/to/copied-openssh-test \
        --ignored --exact \
        openssh_server_archive_packs_remote_sources_with_exact_manifest_and_readback

No test executable is discovered or compiled. Tests that directly manipulate
remote paths using the host filesystem do not work against this container; select
only transport-only tests explicitly. A parent's command is run with argv, not
an implicit shell. --hold-seconds permits another process to use the same fixture
until the owner token is written into readiness.json's release_file.

Raw PTY interface: request an SSH PTY, disable echo, canonical processing and
output translation, then exec `rz -b` in a new receive directory or `sz -b -- FILE`
in the send directory. Do not decode protocol bytes as terminal text. Paths,
tool availability, pinned key files and command templates are in readiness.json.
The built-in reference transfer connects two real OpenSSH raw PTYs to packaged
lrzsz. It validates the fixture, not the application's Zmodem implementation.

Every package/network command has a server-side 40-second timeout and host-side
45-second deadline. Failures remain in receipts. The runner never stops the
Podman machine, prunes images, mounts a host directory, or touches another
container. Container deletion verifies both full ID and ownership labels;
host-process cleanup checks kernel birth identities using openssh_interop.py.
"""

import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import platform
import re
import select
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

# Do not leave generated files beside checked-in scripts when importing helpers.
sys.dont_write_bytecode = True
import openssh_interop as ownership

ROOT = Path(__file__).resolve().parents[1]
LABEL = "io.keelshell.fixture"
LABEL_VALUE = "linux-file-interop"
OWNER_LABEL = "io.keelshell.owner"
CONTAINER_PREFIX = "keelshell-file-interop-"
ARCHIVE_SOURCE = ROOT / "crates/keelshell-core/src/server_archive.py"
REMOTE_SOURCE = "/fixture/server_archive.py"
REMOTE_ROOT = "/home/fixture/interop"
MAX_LOG_BYTES = 8 * 1024 * 1024


class FixtureFailure(Exception):
    """A failed fixture precondition, check, deadline, or ownership assertion."""


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def container_matches(info, container_id, owner):
    """Require the immutable ID and both labels, never merely a reusable name."""
    if not isinstance(info, dict) or not isinstance(info.get("Config"), dict):
        return False
    labels = info.get("Config", {}).get("Labels") or {}
    return (
        isinstance(labels, dict)
        and isinstance(info.get("Name"), str)
        and info.get("Id") == container_id
        and re.fullmatch(r"[0-9a-f]{64}", container_id or "") is not None
        and re.fullmatch(r"[0-9a-f]{32}", owner or "") is not None
        and labels.get(LABEL) == LABEL_VALUE
        and labels.get(OWNER_LABEL) == owner
        and (info.get("Name") or "").lstrip("/") == CONTAINER_PREFIX + owner[:12]
    )


def release_requested(path, owner):
    """A stray or stale release file must not end another owner's lease."""
    try:
        return path.is_file() and path.read_text(encoding="ascii").strip() == owner
    except (OSError, UnicodeError):
        return False


def ssh_argv(config, remote_command, pty=False):
    """Use only the fixture config and pinned key; PTY allocation is explicit."""
    return ["ssh", "-F", str(config), "-tt" if pty else "-T", "fixture", remote_command]


def safe_fixture_path(path):
    """Accept a lexical child of the disposable remote root, not its aliases."""
    return (
        isinstance(path, str)
        and path.startswith(REMOTE_ROOT + "/")
        and all(part not in ("", ".", "..") for part in path.split("/")[1:])
        and not any(character in path for character in "\r\n\0")
    )


def raw_pty_command(directory, program, names=()):
    """Build a literal remote raw-PTY command for the supported lrzsz pair."""
    if not safe_fixture_path(directory) or program not in ("rz", "sz"):
        raise FixtureFailure("Raw PTY command must use the disposable fixture tree")
    if program == "rz" and names:
        raise FixtureFailure("rz receives into its reviewed working directory")
    if program == "sz" and (not names or any(
        not name or "/" in name or name in (".", "..") or "\0" in name
        for name in names
    )):
        raise FixtureFailure("sz requires literal file basenames")
    command = [program, "-b"]
    if program == "sz":
        command.extend(["--", *names])
    return "cd " + shlex.quote(directory) + " && stty raw -echo -onlcr && exec " + shlex.join(command)


class Commands:
    """Bounded commands with private stdin and birth-checked owned children."""

    def __init__(self, output, receipt, check_interrupt):
        self.output = output
        self.receipt = receipt
        self.check_interrupt = check_interrupt
        self.processes = []
        self.identities = {}
        self.counter = 0

    def register(self, process):
        self.processes.append(process)
        self.identities[process] = {}
        info = ownership.process_info(process.pid)
        if info is not None:
            self.identities[process][process.pid] = info

    def capture(self, deadline):
        table = ownership.process_table(deadline)
        for process in self.processes:
            ownership.remember_owned(process, self.identities[process], table)

    def run(self, name, command, data=None, timeout=45, environment=None, check=True):
        self.check_interrupt()
        self.counter += 1
        stem = f"{self.counter:03d}-{name}"
        before = time.monotonic()
        deadline = before + timeout
        with tempfile.TemporaryFile() as stdin, (
            self.output / (stem + ".stdout.log")
        ).open("wb") as stdout, (
            self.output / (stem + ".stderr.log")
        ).open("wb") as stderr:
            if data is not None:
                stdin.write(data)
                stdin.seek(0)
            process = subprocess.Popen(
                command, cwd=ROOT, env=environment, stdin=stdin, stdout=stdout,
                stderr=stderr, start_new_session=True,
            )
            self.register(process)
            timed_out = False
            interrupted = False
            try:
                while process.poll() is None:
                    if time.monotonic() >= deadline:
                        timed_out = True
                        break
                    self.capture(deadline)
                    self.check_interrupt()
                    if stdout.tell() + stderr.tell() > MAX_LOG_BYTES:
                        raise FixtureFailure("Command exceeded the fixture log budget")
                    time.sleep(0.05)
            except BaseException:
                interrupted = True
                raise
            finally:
                clean = ownership.stop_owned(
                    process, time.monotonic() + 7, self.identities[process]
                )
                step = {
                    "name": name, "argv": command, "pid": process.pid,
                    "returncode": process.returncode, "timed_out": timed_out,
                    "interrupted": interrupted, "owned_processes_stopped": clean,
                    "elapsed_seconds": round(time.monotonic() - before, 3),
                    "stdout_log": stem + ".stdout.log",
                    "stderr_log": stem + ".stderr.log",
                }
                self.receipt["steps"].append(step)
                write_json(self.output / "progress.json", self.receipt)
                if not clean and not interrupted:
                    raise FixtureFailure("Could not verify owned command cleanup")
        result = (self.output / (stem + ".stdout.log")).read_bytes()
        if check and (timed_out or process.returncode):
            raise FixtureFailure(f"{name} failed; inspect {stem} logs")
        return process.returncode, result

    def finish(self):
        results = []
        for process in reversed(self.processes):
            try:
                results.append(ownership.stop_owned(
                    process, time.monotonic() + 7, self.identities[process]
                ))
            except (ownership.InteropFailure, OSError, subprocess.SubprocessError) as error:
                self.receipt.setdefault("cleanup_errors", []).append(type(error).__name__ + ": " + str(error))
                for pid, identity in self.identities[process].items():
                    ownership.signal_owned(pid, identity, signal.SIGKILL)
                results.append(False)
        return all(results)


def inspect_container(commands, container):
    _, data = commands.run("container-identity", [
        "podman", "inspect", "--type", "container", container,
    ])
    values = json.loads(data)
    if len(values) != 1:
        raise FixtureFailure("Container identity is ambiguous")
    return values[0]


def assert_container(commands, container_id, owner):
    info = inspect_container(commands, container_id)
    if not container_matches(info, container_id, owner):
        raise FixtureFailure("Container ownership changed; refusing mutation or cleanup")
    return info


def debian_upstream(path, token):
    """Map one APT URL to a fixed HTTPS origin, including encoded '+' versions."""
    try:
        decoded = urllib.parse.unquote(path, errors="strict")
    except UnicodeError:
        return None
    match = re.fullmatch(
        "/" + re.escape(token) + r"/(debian|debian-security)/"
        r"((?:dists|pool)/[A-Za-z0-9_+./~\-]+)", decoded,
    )
    if match is None or any(part in ("", ".", "..") for part in match[2].split("/")):
        return None
    return "https://deb.debian.org/" + match[1] + "/" + match[2]


def debian_host_bridge(commands, container_id):
    """Relay only public Debian files when VM egress is slow.

    APT's bundled trusted keys still verify repository/package signatures. This
    small server exposes no host files, supports no arbitrary destination and
    refuses paths outside public Debian release metadata/package directories.
    """
    token = uuid.uuid4().hex
    requests = []

    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, _format, *_args):
            pass

        def do_GET(self):
            url = debian_upstream(self.path, token)
            if url is None:
                self.send_error(404)
                return
            result = {"url": url}
            try:
                with urllib.request.urlopen(url, timeout=15) as response:
                    data = response.read(32 * 1024 * 1024 + 1)
                if len(data) > 32 * 1024 * 1024:
                    raise FixtureFailure("Public Debian file exceeds bridge budget")
                result.update({"bytes": len(data), "sha256": digest(data), "status": 200})
                self.send_response(200)
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
            except (OSError, urllib.error.URLError, FixtureFailure) as error:
                result.update({"error": type(error).__name__, "status": 502})
                self.send_error(502)
            finally:
                requests.append(result)

    # On Podman macOS, host.containers.internal reaches gvproxy's host loopback.
    # Verify the actual request; no host network setting is modified.
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        prefix = f"http://host.containers.internal:{server.server_port}/{token}/"
        commands.run("debian-bridge-repositories", [
            "podman", "exec", "-i", container_id, "sh", "-ec",
            "cat > /etc/apt/sources.list.d/debian.sources",
        ], "\n".join((
            "Types: deb", "URIs: " + prefix + "debian",
            "Suites: bookworm bookworm-updates", "Components: main",
            "Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg", "",
            "Types: deb", "URIs: " + prefix + "debian-security",
            "Suites: bookworm-security", "Components: main",
            "Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg", "",
        )).encode())
        options = [
            "-o", "Acquire::Retries=0", "-o", "Acquire::http::Timeout=8",
            "-o", "Acquire::Languages=none",
        ]
        commands.run("apt-index-host-bridge", [
            "podman", "exec", container_id, "timeout", "--kill-after=3", "40",
            "apt-get", *options, "update",
        ])
        commands.run("install-tools-host-bridge", [
            "podman", "exec", "--env", "DEBIAN_FRONTEND=noninteractive",
            container_id, "timeout", "--kill-after=3", "40",
            "apt-get", *options, "install", "--no-install-recommends", "-y",
            "python3", "openssh-server", "lrzsz", "acl",
        ])
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)
        write_json(commands.output / "public-package-bridge.json", {
            "requests": requests, "listener_closed": server.fileno() == -1,
            "thread_stopped": not thread.is_alive(),
            "signature_policy": "Normal apt trusted-key and package hash verification, no allow-unauthenticated",
        })


def provision(commands, container_id, prefer_host_bridge=False):
    # Both watchdogs apply to the actual package process, not only the remote
    # podman client; a lost client must not leave apt/apk running indefinitely.
    _, data = commands.run("linux-release", [
        "podman", "exec", container_id, "cat", "/etc/os-release",
    ])
    if b"ID=debian" in data or b"ID=ubuntu" in data:
        options = [
            "-o", "Acquire::Retries=0", "-o", "Acquire::http::Timeout=8",
            "-o", "Acquire::https::Timeout=8", "-o", "Acquire::Languages=none",
        ]
        if prefer_host_bridge:
            if b"ID=debian" not in data or b"bookworm" not in data:
                raise FixtureFailure("The public host bridge requires Debian bookworm")
            debian_host_bridge(commands, container_id)
            code = 0
        else:
            code, _ = commands.run("apt-index", [
                "podman", "exec", container_id, "timeout", "--kill-after=3", "40",
                "apt-get", *options, "update",
            ], check=False)
        if code:
            if b"ID=debian" not in data or b"bookworm" not in data:
                raise FixtureFailure("Direct APT timed out; public bridge supports Debian bookworm only")
            debian_host_bridge(commands, container_id)
        elif not prefer_host_bridge:
            code, _ = commands.run("install-tools", [
                "podman", "exec", "--env", "DEBIAN_FRONTEND=noninteractive",
                container_id, "timeout", "--kill-after=3", "40",
                "apt-get", *options, "install", "--no-install-recommends", "-y",
                "python3", "openssh-server", "lrzsz", "acl",
            ], check=False)
            if code:
                if b"ID=debian" not in data or b"bookworm" not in data:
                    raise FixtureFailure("Direct package install failed")
                debian_host_bridge(commands, container_id)
        create_user = "useradd --create-home --shell /bin/sh fixture"
    else:
        raise FixtureFailure("Only cached Debian and Ubuntu bases are supported")
    commands.run("create-fixture-user", [
        "podman", "exec", container_id, "sh", "-ec",
        create_user + "; passwd -d fixture >/dev/null; "
        "mkdir -p /fixture /run/sshd /home/fixture/.ssh " + REMOTE_ROOT + "; "
        "chmod 700 /fixture /home/fixture /home/fixture/.ssh " + REMOTE_ROOT + "; "
        "chown -R fixture:fixture /home/fixture",
    ])


def setup_ssh(commands, container_id, scratch):
    for name in ("host", "client"):
        commands.run("generate-" + name, [
            "ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "",
            "-f", str(scratch / name),
        ])
    for source, destination in (
        (scratch / "host", "/fixture/host"),
        (scratch / "client.pub", "/home/fixture/.ssh/authorized_keys"),
    ):
        commands.run("copy-key-file", ["podman", "cp", str(source), container_id + ":" + destination])
    config = "\n".join((
        "Port 2222", "ListenAddress 0.0.0.0", "AddressFamily inet",
        "HostKey /fixture/host", "PidFile /fixture/sshd.pid",
        "AllowUsers fixture", "AuthorizedKeysFile .ssh/authorized_keys",
        "PubkeyAuthentication yes", "AuthenticationMethods publickey",
        "PasswordAuthentication no", "KbdInteractiveAuthentication no",
        "PermitEmptyPasswords no", "PermitRootLogin no", "UsePAM no",
        "UseDNS no", "StrictModes yes", "PermitUserEnvironment no",
        "AllowAgentForwarding no", "AllowTcpForwarding no",
        "AllowStreamLocalForwarding no", "PermitTunnel no", "X11Forwarding no",
        "PermitTTY yes", "LogLevel ERROR", "MaxSessions 16",
        "Subsystem sftp internal-sftp", "",
    ))
    commands.run("sshd-config", [
        "podman", "exec", "-i", container_id, "sh", "-ec",
        "cat > /fixture/sshd_config; chmod 600 /fixture/host /fixture/sshd_config "
        "/home/fixture/.ssh/authorized_keys; "
        "chown fixture:fixture /home/fixture/.ssh/authorized_keys; "
        "/usr/sbin/sshd -t -f /fixture/sshd_config",
    ], config.encode())
    commands.run("start-sshd", [
        "podman", "exec", container_id, "/usr/sbin/sshd", "-f", "/fixture/sshd_config",
        "-E", "/fixture/sshd.log",
    ])
    _, data = commands.run("ssh-port", ["podman", "port", container_id, "2222/tcp"])
    match = re.fullmatch(rb"127\.0\.0\.1:(\d+)\s*", data)
    if match is None or not 0 < int(match[1]) < 65536:
        raise FixtureFailure("Expected exactly one loopback-only SSH mapping")
    port = int(match[1])
    # Read from the generated owned key, never accept a network keyscan as trust.
    (scratch / "known_hosts").write_text(
        "[127.0.0.1]:" + str(port) + " " + (scratch / "host.pub").read_text(),
        encoding="utf-8",
    )
    client_config = scratch / "ssh_config"
    client_config.write_text("\n".join((
        "Host fixture", "  HostName 127.0.0.1", f"  Port {port}", "  User fixture",
        "  IdentityFile " + ownership.config_value(scratch / "client"),
        "  UserKnownHostsFile " + ownership.config_value(scratch / "known_hosts"),
        "  GlobalKnownHostsFile /dev/null", "  StrictHostKeyChecking yes",
        "  BatchMode yes", "  IdentitiesOnly yes", "  IdentityAgent none",
        "  PasswordAuthentication no", "  KbdInteractiveAuthentication no",
        "  ConnectTimeout 5", "  ConnectionAttempts 1", "  LogLevel ERROR",
        "  ControlMaster no", "  ClearAllForwardings yes", "",
    )), encoding="utf-8")
    _, authenticated = commands.run("ssh-authentication", ssh_argv(
        client_config, "test -d " + REMOTE_ROOT + " && printf fixture-authenticated"
    ), timeout=12)
    if authenticated != b"fixture-authenticated":
        raise FixtureFailure("Pinned SSH authentication returned unexpected bytes")
    return port, client_config


TOOLS_PROBE = r'''
import json,os,platform,shutil,subprocess,sys
tools={}
for name,args in (("python3",["--version"]),("rz",["--version"]),
                  ("sz",["--version"]),("sshd",["-V"]),
                  ("setfacl",["--version"]),("getfacl",["--version"])):
    path=shutil.which(name) or ("/usr/sbin/sshd" if name=="sshd" else None)
    if not path: raise RuntimeError("Missing tool: "+name)
    result=subprocess.run([path]+args,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=5)
    tools[name]={"path":path,"version":result.stdout.decode("utf-8","replace").strip(),
                 "returncode":result.returncode}
old=shutil.which("python3.8")
print(json.dumps({"sys_platform":sys.platform,"os_name":os.name,
    "python_version":sys.version,"python_executable":sys.executable,
    "kernel":platform.uname()._asdict(),"tools":tools,
    "python38_available":old is not None,"python38_path":old}))
assert sys.platform.startswith("linux") and sys.version_info >= (3,8)
'''


ARCHIVE_PROBES = r'''
import base64,hashlib,importlib.util,io,json,os,platform,shutil,stat,subprocess,sys,tarfile,uuid,zipfile
from pathlib import Path
source=Path("/fixture/server_archive.py")
spec=importlib.util.spec_from_file_location("keelshell_server_archive_under_test",str(source))
helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
root=Path("/home/fixture/interop")/("archive-"+uuid.uuid4().hex)
root.mkdir(mode=0o700)
results=[]
def sha(data): return hashlib.sha256(data).hexdigest()
def command(args,**kw):
    p=subprocess.run(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=12,**kw)
    if p.returncode: raise AssertionError((args,p.returncode,p.stderr.decode("utf-8","replace")))
    return p.stdout
def case(name,fn):
    try:
        detail=fn()
        results.append({"name":name,"passed":True,"detail":detail})
    except Exception as error:
        results.append({"name":name,"passed":False,"error":type(error).__name__+": "+str(error)})
def directory(name):
    p=root/name;p.mkdir(mode=0o700);return p
def seed(p):
    (p/"binary.bin").write_bytes(bytes(range(256))*17+b"\x00\xff\r\n")
    (p/"tree").mkdir();(p/"tree"/"empty").mkdir()
    (p/"tree"/"text.txt").write_text("Linux archive \u4e2d\u6587\n",encoding="utf-8")
def request(p,fmt):
    return {"version":1,"sources":[str(p/"binary.bin"),str(p/"tree")],
            "destination":str(p/("packed."+fmt)),"format":fmt}
def encode(req): return base64.b64encode(json.dumps(req,ensure_ascii=False).encode()).decode()
def cli(mode,req,stage=None,data=None):
    args=[sys.executable,str(source),mode,encode(req)]
    if stage is not None: args.append(str(stage))
    return subprocess.run(args,input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=15)
def inspect(req):
    result=cli("inspect",req);assert result.returncode==0,result.stderr
    return json.loads(result.stdout)
def inherited_acl(p):
    command(["setfacl","-m","d:u::rwx,d:u:65534:rwx,d:g::r-x,d:m::rwx,d:o::---",str(p)])
    assert "system.posix_acl_default" in os.listxattr(str(p))
def rejected(fn):
    try: fn()
    except helper.Rejected: return
    raise AssertionError("accepted unsafe ACL state")
def pack_case(fmt,with_acl):
    p=directory(("default-acl-" if with_acl else "plain-")+fmt);seed(p)
    if with_acl: inherited_acl(p)
    parent_acl=command(["getfacl","-cp",str(p)]).decode()
    req=request(p,fmt);manifest=inspect(req)
    stage=p/(helper.PREFIX+str(uuid.uuid4()))
    result=cli("pack",req,stage,json.dumps(manifest).encode())
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout);path=Path(req["destination"]);data=path.read_bytes()
    assert receipt["sha256"]==sha(data) and receipt["archive_bytes"]==len(data)
    info=path.stat()
    assert stat.S_IMODE(info.st_mode)==0o600 and info.st_nlink==1
    assert not any("acl" in n.lower() for n in os.listxattr(str(path)))
    assert not stage.exists()
    assert command(["getfacl","-cp",str(p)]).decode()==parent_acl
    expected={m["path"]:m for m in manifest["members"]}
    if fmt=="zip":
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            members=archive.infolist();actual={}
            for m in members:
                payload=b"" if m.is_dir() else archive.read(m)
                actual[m.filename.rstrip("/")]=("directory" if m.is_dir() else "file",payload)
    else:
        with tarfile.open(fileobj=io.BytesIO(data),mode="r:gz") as archive:
            actual={}
            for m in archive.getmembers():
                assert m.isdir() or m.isfile()
                payload=b"" if m.isdir() else archive.extractfile(m).read()
                actual[m.name.rstrip("/")]=("directory" if m.isdir() else "file",payload)
    assert set(actual)==set(expected)
    for name,(kind,data) in actual.items():
        item=expected[name];assert kind==item["kind"]
        if kind=="file": assert len(data)==item["bytes"] and sha(data)==item["sha256"]
    return {"member_count":len(actual),"archive_sha256":receipt["sha256"],
            "archive_bytes":receipt["archive_bytes"],"destination_mode":oct(stat.S_IMODE(info.st_mode)),
            "parent_acl":parent_acl,"destination_acl":command(["getfacl","-cp",str(path)]).decode(),
            "stage_absent":True,"cli_returncode":result.returncode}
def fd_clear():
    p=directory("fd-clear");inherited_acl(p)
    stage=p/"inherited-stage";stage.mkdir(mode=0o700)
    payload=p/"inherited-payload";payload.write_bytes(b"synthetic fixture bytes");payload.chmod(0o600)
    before={};after={}
    for path in (stage,payload):
        fd=os.open(str(path),os.O_RDONLY|os.O_NOFOLLOW)
        try:
            before[path.name]=os.listxattr(fd)
            assert "system.posix_acl_access" in before[path.name]
            rejected(lambda:helper.private_acl(fd))
            helper.private_acl(fd,clear=True);helper.private_acl(fd)
            after[path.name]=os.listxattr(fd)
            assert not any("acl" in n.lower() for n in after[path.name])
        finally: os.close(fd)
    return {"before":before,"after":after}
def acl_injection(fmt,where):
    p=directory("inject-"+where+"-"+fmt);seed(p);inherited_acl(p)
    req=request(p,fmt);manifest=inspect(req);stage=p/(helper.PREFIX+str(uuid.uuid4()))
    original_scan=helper.scan;original_readback=helper.readback;injected=[]
    def scan(*args,**kwargs):
        writer=kwargs.get("writer",args[3] if len(args)>3 else None)
        if writer is not None and where=="stage" and not injected:
            inherited_acl(stage);injected.append(command(["getfacl","-cp",str(stage)]).decode())
        return original_scan(*args,**kwargs)
    def readback(*args,**kwargs):
        if where=="payload" and not injected:
            command(["setfacl","-m","u:65534:r--,m::---",str(stage/"payload")])
            assert stat.S_IMODE((stage/"payload").stat().st_mode)==0o600
            injected.append(command(["getfacl","-cp",str(stage/"payload")]).decode())
        return original_readback(*args,**kwargs)
    helper.scan=scan;helper.readback=readback
    try:
        rejected(lambda:helper.pack(req,str(p),Path(req["destination"]).name,str(stage),manifest))
    finally: helper.scan=original_scan;helper.readback=original_readback
    assert injected and not Path(req["destination"]).exists()
    assert stage.is_dir() and (stage/"payload").is_file()
    return {"injected_acl":injected[0],"destination_absent":True,
            "quarantine_stage_retained":True,
            "probe_kind":"observational wrapper around exact helper; real kernel ACL mutation"}
def unknown_acl():
    p=directory("unknown-acl");fd=os.open(str(p),os.O_RDONLY|os.O_DIRECTORY)
    try:
        os.setxattr(fd,"user.keelshell_unknown_acl",b"owned test")
        rejected(lambda:helper.private_acl(fd,clear=True))
        assert "user.keelshell_unknown_acl" in os.listxattr(fd)
        return {"unknown_acl_family_rejected":True,"unknown_xattr_preserved":True}
    finally: os.close(fd)
def changed_or_existing(fmt,mutation):
    p=directory(mutation+"-"+fmt);seed(p);req=request(p,fmt);manifest=inspect(req)
    stage=p/(helper.PREFIX+str(uuid.uuid4()))
    if mutation=="source-change": (p/"binary.bin").write_bytes(b"changed since review")
    else: Path(req["destination"]).write_bytes(b"keep existing target")
    result=cli("pack",req,stage,json.dumps(manifest).encode())
    assert result.returncode==65 and result.stdout==b"" and not stage.exists()
    if mutation=="destination-race": assert Path(req["destination"]).read_bytes()==b"keep existing target"
    else: assert not Path(req["destination"]).exists()
    return {"cli_returncode":result.returncode,"no_stage_created":True,"target_preserved":True}
def unsafe_source(kind):
    p=directory("unsafe-"+kind);seed(p)
    target=p/"binary.bin";target.unlink()
    if kind=="symlink": target.symlink_to(p/"tree"/"text.txt")
    elif kind=="hardlink": os.link(str(p/"tree"/"text.txt"),str(target))
    else: os.mkfifo(str(target))
    result=cli("inspect",request(p,"zip"))
    assert result.returncode==65 and result.stdout==b""
    return {"cli_returncode":65,"destination_absent":not (p/"packed.zip").exists()}
helper.capabilities()
for fmt in ("zip","tar.gz"):
    for acl in (False,True):
        case(("default-acl-" if acl else "plain-")+fmt,lambda f=fmt,a=acl:pack_case(f,a))
    for mutation in ("source-change","destination-race"):
        case(mutation+"-"+fmt,lambda f=fmt,m=mutation:changed_or_existing(f,m))
    for where in ("stage","payload"):
        case("late-acl-"+where+"-"+fmt,lambda f=fmt,w=where:acl_injection(f,w))
case("fd-inherited-access-default-acl-clear",fd_clear)
case("unknown-acl-family-fail-closed",unknown_acl)
for kind in ("symlink","hardlink","fifo"):
    case("unsafe-"+kind,lambda k=kind:unsafe_source(k))
result={"schema":1,"sys_platform":sys.platform,"os_name":os.name,"python":sys.version,
        "python_executable":sys.executable,"euid":os.geteuid(),"kernel":platform.uname()._asdict(),
        "source_sha256":sha(source.read_bytes()),
        "filesystem":command(["stat","-f","-c","%T",str(root)]).decode().strip(),
        "cases":results,"passed":sum(x["passed"] for x in results),
        "failed":sum(not x["passed"] for x in results),
        "boundary":"actual container Linux helper and ACL probes; no Rust or native GUI acceptance"}
print(json.dumps(result,ensure_ascii=False))
sys.exit(1 if result["failed"] else 0)
'''


def run_archive_probes(commands, container_id, python, output, receipt):
    code, data = commands.run("linux-archive-probes-" + python.replace(".", "-"), [
        "podman", "exec", "-i", "--user", "fixture", container_id,
        "timeout", "40", python, "-",
    ], ARCHIVE_PROBES.encode(), check=False)
    try:
        result = json.loads(data)
    except (UnicodeError, ValueError) as error:
        raise FixtureFailure("Linux archive probes produced no complete JSON receipt") from error
    write_json(output / ("archive-" + python + ".json"), result)
    if result.get("source_sha256") != digest(ARCHIVE_SOURCE.read_bytes()):
        raise FixtureFailure("Linux archive helper bytes do not match the copied source")
    receipt.setdefault("archive_runs", []).append({
        "python": python, "passed": result["passed"], "failed": result["failed"],
        "receipt": "archive-" + python + ".json",
    })
    if code or result["failed"]:
        raise FixtureFailure("Real Linux archive/ACL probes failed")


def reference_zmodem(commands, config, container_id, output):
    """Bridge two independently allocated real Linux OpenSSH raw PTYs."""
    stem = REMOTE_ROOT + "/zmodem-" + uuid.uuid4().hex
    source, destination = stem + "/send", stem + "/receive"
    name = "binary-all-bytes.bin"
    data = bytes(range(256)) * 1024 + b"\x18\x18\x00\xff\r\n" + "中文".encode()
    commands.run("zmodem-source", [
        "podman", "exec", "-i", "--user", "fixture", container_id, "python3", "-c",
        "import sys,pathlib; a=pathlib.Path(sys.argv[1]); a.mkdir(parents=True); "
        "pathlib.Path(sys.argv[2]).mkdir(); (a/sys.argv[3]).write_bytes(sys.stdin.buffer.read())",
        source, destination, name,
    ], data)
    processes = []
    stderr_handles = []
    started = time.monotonic()
    deadline = started + 35
    counters = {"sender_to_receiver": 0, "receiver_to_sender": 0}
    try:
        for label, directory, program, names in (
            ("sender", source, "sz", (name,)), ("receiver", destination, "rz", ()),
        ):
            stderr = (output / ("zmodem-" + label + ".stderr.log")).open("wb")
            stderr_handles.append(stderr)
            process = subprocess.Popen(
                ssh_argv(config, raw_pty_command(directory, program, names), pty=True),
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                start_new_session=True,
            )
            commands.register(process)
            processes.append(process)
        # Nonblocking queues keep a peer that stalls from blocking cleanup.
        pending = [bytearray(), bytearray()]
        readable = {process.stdout.fileno(): index for index, process in enumerate(processes)}
        writable = {process.stdin.fileno(): index for index, process in enumerate(processes)}
        for process in processes:
            os.set_blocking(process.stdout.fileno(), False)
            os.set_blocking(process.stdin.fileno(), False)
        while readable or any(process.poll() is None for process in processes):
            commands.check_interrupt()
            if time.monotonic() >= deadline:
                raise FixtureFailure("Reference lrzsz transfer exceeded 35 seconds")
            commands.capture(deadline)
            writers = [fd for fd, index in writable.items() if pending[index]]
            ready, write_ready, _ = select.select(list(readable), writers, [], 0.05)
            for fd in ready:
                block = os.read(fd, 64 * 1024)
                if not block:
                    del readable[fd]
                    continue
                index = readable[fd]
                pending[1 - index].extend(block)
                counters["sender_to_receiver" if index == 0 else "receiver_to_sender"] += len(block)
                if sum(map(len, pending)) > 2 * 1024 * 1024:
                    raise FixtureFailure("Reference lrzsz peer exceeded the pending-byte budget")
            for fd in write_ready:
                index = writable[fd]
                try:
                    count = os.write(fd, pending[index])
                except BrokenPipeError:
                    # Closing banners after both protocol tools exit may arrive
                    # after the other channel's stdin closed. File/readback and
                    # both exit statuses still independently decide success.
                    del writable[fd]
                    pending[index].clear()
                    continue
                del pending[index][:count]
        codes = [process.wait(timeout=1) for process in processes]
        if codes != [0, 0]:
            raise FixtureFailure("Reference sz/rz processes did not both exit successfully")
        _, verification = commands.run("zmodem-readback", [
            "podman", "exec", "--user", "fixture", container_id, "python3", "-c",
            "import sys,pathlib,hashlib,json; p=pathlib.Path(sys.argv[1]); "
            "b=p.read_bytes(); print(json.dumps({'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}))",
            destination + "/" + name,
        ])
        received = json.loads(verification)
        if received != {"bytes": len(data), "sha256": digest(data)}:
            raise FixtureFailure("Reference lrzsz complete-file readback mismatch")
        result = {
            "schema": 1, "outcome": "passed", "source_bytes": len(data),
            "source_sha256": digest(data), "received": received, "exit_codes": codes,
            "wire_bytes": counters, "elapsed_seconds": round(time.monotonic() - started, 3),
            "transport": "two real OpenSSH SSH PTYs with stty raw -echo -onlcr",
            "boundary": "packaged Linux lrzsz peer interoperability, not application acceptance",
        }
        write_json(output / "zmodem-reference.json", result)
        return result
    except (FixtureFailure, ownership.InteropFailure, OSError, ValueError, subprocess.SubprocessError) as error:
        write_json(output / "zmodem-reference.json", {
            "schema": 1, "outcome": "failed",
            "error": type(error).__name__ + ": " + str(error),
            "source_bytes": len(data), "source_sha256": digest(data),
            "wire_bytes": counters,
            "exit_codes_before_cleanup": [process.poll() for process in processes],
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "boundary": "Reference peer failure; not application acceptance",
        })
        raise
    finally:
        for process in reversed(processes):
            ownership.stop_owned(process, time.monotonic() + 7, commands.identities[process])
            for stream in (process.stdin, process.stdout):
                if stream is not None:
                    stream.close()
        for handle in stderr_handles:
            handle.close()


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--image", default="docker.io/library/rust:1.98.1-slim-bookworm", help="Cached public image; --pull=never always applies")
    parser.add_argument("--output", help="New receipt directory (never overwritten)")
    parser.add_argument("--hold-seconds", type=int, default=0, help="Lease after checks, 0..1800 seconds")
    parser.add_argument("--command-timeout", type=int, default=180, help="Caller-supplied test deadline, 1..600 seconds")
    parser.add_argument("--skip-archive", action="store_true")
    parser.add_argument("--skip-zmodem", action="store_true")
    parser.add_argument("--host-package-bridge", action="store_true", help="Use host HTTPS for public signed Debian bookworm packages")
    parser.add_argument("--default-acl-root", action="store_true", help="Install a real inheritable ACL on the supplied remote root")
    parser.add_argument("--prepared-container", help="Adopt only an already owned, provisioned fixture's full ID")
    parser.add_argument("--owner", help="Exact 32-hex owner token required when adopting")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="Explicit already-built command after --; Cargo is never invoked")
    result = parser.parse_args(argv)
    if not 0 <= result.hold_seconds <= 1800 or not 1 <= result.command_timeout <= 600:
        parser.error("Hold or command timeout is outside the bounded range")
    if bool(result.prepared_container) != bool(result.owner):
        parser.error("--prepared-container and --owner are required together")
    if result.prepared_container and (
        not re.fullmatch("[0-9a-f]{64}", result.prepared_container)
        or not re.fullmatch("[0-9a-f]{32}", result.owner)
    ):
        parser.error("Adoption requires a full container ID and exact owner token")
    if result.command[:1] == ["--"]:
        result.command = result.command[1:]
    if result.command and Path(result.command[0]).name in ("cargo", "rustc", "rustup"):
        parser.error("Supply an already-built test or parent command; no Rust build tools")
    return result


def main(argv=None):
    args = arguments(argv)
    if platform.system() not in ("Darwin", "Linux"):
        raise FixtureFailure("Host process-ownership checks support macOS/Linux only")
    os.umask(0o077)
    output = Path(args.output).absolute() if args.output else (
        ROOT / "work/zmodem-linux-interop-20261115" / (
            time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + uuid.uuid4().hex[:8]
        )
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.mkdir(mode=0o700)
    start = time.monotonic()
    owner = args.owner or uuid.uuid4().hex
    container_id = args.prepared_container
    container_name = CONTAINER_PREFIX + owner[:12]
    scratch = None
    requested_signal = None
    old_handlers = {}
    receipt = {
        "schema": 1, "outcome": "failed", "started_utc": ownership.utc_now(),
        "owner": owner, "container_name": container_name, "steps": [], "cleanup": {},
        "source_inputs": {
            str(path.relative_to(ROOT)): digest(path.read_bytes())
            for path in (Path(__file__).resolve(), ROOT / "scripts/openssh_interop.py", ARCHIVE_SOURCE)
        },
        "boundary": "owned Linux SSH/lrzsz/archive helper fixture; no Cargo, GUI, packaging or all-Linux acceptance",
    }
    snapshots = output / "source-snapshot"
    snapshots.mkdir()
    for name, expected in receipt["source_inputs"].items():
        data = (ROOT / name).read_bytes()
        if digest(data) != expected:
            raise FixtureFailure("Fixture source changed during snapshot capture")
        (snapshots / (name.replace("/", "__") + ".txt")).write_bytes(data)

    def interrupted(signum, _):
        nonlocal requested_signal
        requested_signal = signum

    def check_interrupt():
        if requested_signal is not None:
            raise FixtureFailure("Interrupted by signal " + str(requested_signal))

    commands = Commands(output, receipt, check_interrupt)
    code = 1
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            old_handlers[sig] = signal.signal(sig, interrupted)
        if container_id is None:
            _, data = commands.run("create-container", [
                "podman", "run", "--pull=never", "--http-proxy=false",
                "--label", LABEL + "=" + LABEL_VALUE,
                "--label", OWNER_LABEL + "=" + owner, "--name", container_name,
                "--detach", "--publish", "127.0.0.1::2222", "--entrypoint", "sleep",
                args.image, "infinity",
            ])
            container_id = data.decode("ascii").strip()
            assert_container(commands, container_id, owner)
            provision(commands, container_id, args.host_package_bridge)
        else:
            assert_container(commands, container_id, owner)
        receipt["container_id"] = container_id
        scratch = Path(tempfile.mkdtemp(prefix="keelshell-linux-interop-")).resolve()
        scratch.chmod(0o700)
        port, config = setup_ssh(commands, container_id, scratch)
        commands.run("copy-archive-source", [
            "podman", "cp", str(ARCHIVE_SOURCE), container_id + ":" + REMOTE_SOURCE,
        ])
        commands.run("readable-archive-source", [
            "podman", "exec", container_id, "chmod", "755", "/fixture",
        ])
        commands.run("private-fixture-files", [
            "podman", "exec", container_id, "chmod", "600", "/fixture/host", "/fixture/sshd_config",
        ])
        if args.default_acl_root:
            commands.run("remote-root-default-acl", [
                "podman", "exec", "--user", "fixture", container_id, "setfacl", "-m",
                "d:u::rwx,d:u:65534:rwx,d:g::r-x,d:m::rwx,d:o::---", REMOTE_ROOT,
            ])
            commands.run("remote-root-default-acl-readback", [
                "podman", "exec", "--user", "fixture", container_id,
                "getfacl", "-cp", REMOTE_ROOT,
            ])
        _, data = commands.run("tool-availability", [
            "podman", "exec", "-i", container_id, "python3", "-",
        ], TOOLS_PROBE.encode())
        tools = json.loads(data)
        write_json(output / "tools.json", tools)
        environment = {
            "KEELSHELL_OPENSSH_USER": "fixture", "KEELSHELL_OPENSSH_PORT": str(port),
            "KEELSHELL_OPENSSH_HOST_KEY": str(scratch / "host.pub"),
            "KEELSHELL_OPENSSH_CLIENT_KEY": str(scratch / "client"),
            "KEELSHELL_OPENSSH_ROOT": REMOTE_ROOT,
            "KEELSHELL_LINUX_CONTAINER": container_id,
            "KEELSHELL_LINUX_SSH_CONFIG": str(config),
        }
        env_file = scratch / "environment.json"
        write_json(env_file, environment)
        release_file = output / "parent-release"
        readiness = {
            "schema": 1, "owner": owner, "container_id": container_id,
            "endpoint": f"fixture@127.0.0.1:{port}",
            "environment_file": str(env_file), "ssh_config": str(config),
            "remote_root": REMOTE_ROOT, "archive_source": REMOTE_SOURCE,
            "archive_source_sha256": digest(ARCHIVE_SOURCE.read_bytes()),
            "release_file": str(release_file), "hold_seconds": args.hold_seconds,
            "default_acl_root": args.default_acl_root,
            "tools": tools,
            "raw_pty_receive_template": raw_pty_command(REMOTE_ROOT + "/YOUR-NEW-RECEIVE-DIR", "rz"),
            "raw_pty_send_template": raw_pty_command(REMOTE_ROOT + "/YOUR-SEND-DIR", "sz", ("FILE",)),
            "warning": "Keys and SSH config disappear during finally cleanup; readiness is historical after result.json exists.",
        }
        write_json(output / "readiness.json", readiness)
        print("LINUX_FIXTURE_READY " + str(output / "readiness.json"), flush=True)
        print("Endpoint " + readiness["endpoint"] + "; Python/lrzsz/OpenSSH/ACL tools installed", flush=True)
        if not args.skip_archive:
            run_archive_probes(commands, container_id, "python3", output, receipt)
            if tools["python38_available"]:
                run_archive_probes(commands, container_id, "python3.8", output, receipt)
            else:
                receipt["python38"] = "NOT_AVAILABLE; actual Python 3.8 runtime not accepted"
        if not args.skip_zmodem:
            receipt["zmodem_reference"] = reference_zmodem(commands, config, container_id, output)
        if args.command:
            environment = dict(os.environ, **environment)
            result, _ = commands.run(
                "parent-command", args.command, timeout=args.command_timeout,
                environment=environment, check=False,
            )
            if result:
                raise FixtureFailure("Supplied parent command failed")
        if args.hold_seconds:
            until = time.monotonic() + args.hold_seconds
            write_json(output / "lease.json", {
                "owner": owner, "seconds": args.hold_seconds,
                "started_utc": ownership.utc_now(), "release_file": str(release_file),
            })
            next_check = time.monotonic()
            while time.monotonic() < until and not release_requested(release_file, owner):
                check_interrupt()
                if time.monotonic() >= next_check:
                    info = assert_container(commands, container_id, owner)
                    if not info.get("State", {}).get("Running"):
                        raise FixtureFailure("Fixture container stopped while leased")
                    next_check = time.monotonic() + 10
                time.sleep(0.2)
            receipt["lease_release"] = "parent" if release_requested(release_file, owner) else "expired"
        check_interrupt()
        receipt["outcome"] = "passed"
        code = 0
    except (FixtureFailure, ownership.InteropFailure, OSError, ValueError, subprocess.SubprocessError) as error:
        receipt["error"] = type(error).__name__ + ": " + str(error)
        print(receipt["error"], file=sys.stderr, flush=True)
    finally:
        # Cleanup must complete even when a second Ctrl-C arrives.
        for sig in old_handlers:
            signal.signal(sig, signal.SIG_IGN)
        requested_signal = None
        receipt["cleanup"]["owned_host_processes_stopped"] = commands.finish()
        if container_id:
            try:
                assert_container(commands, container_id, owner)
                commands.run("sshd-final-log", [
                    "podman", "exec", container_id, "cat", "/fixture/sshd.log",
                ], check=False)
                commands.run("remove-owned-container", [
                    "podman", "rm", "--force", container_id,
                ])
                result, _ = commands.run("confirm-container-absent", [
                    "podman", "container", "exists", container_id,
                ], check=False)
                receipt["cleanup"]["owned_container_absent"] = result == 1
            except (FixtureFailure, ownership.InteropFailure, OSError, ValueError, subprocess.SubprocessError) as error:
                receipt["cleanup"]["owned_container_absent"] = False
                receipt["cleanup"]["error"] = type(error).__name__ + ": " + str(error)
        else:
            # A failed run may have created a container before its ID reached
            # stdout. Resolve only the unique name, then require both labels.
            try:
                result, data = commands.run("recover-create-identity", [
                    "podman", "inspect", "--type", "container", container_name,
                ], check=False)
                if result == 0:
                    info = json.loads(data)[0]
                    if not container_matches(info, info.get("Id"), owner):
                        raise FixtureFailure("Cannot recover owned container identity")
                    commands.run("remove-recovered-container", ["podman", "rm", "--force", info["Id"]])
                    result, _ = commands.run("confirm-recovered-absent", [
                        "podman", "container", "exists", info["Id"],
                    ], check=False)
                    receipt["cleanup"]["owned_container_absent"] = result == 1
                else:
                    result, _ = commands.run("confirm-create-absent", [
                        "podman", "container", "exists", container_name,
                    ], check=False)
                    receipt["cleanup"]["owned_container_absent"] = result == 1
            except (FixtureFailure, ownership.InteropFailure, OSError, ValueError, subprocess.SubprocessError) as error:
                receipt["cleanup"]["owned_container_absent"] = False
                receipt["cleanup"]["error"] = type(error).__name__ + ": " + str(error)
        if scratch is not None:
            try:
                shutil.rmtree(scratch)
            except OSError as error:
                receipt["cleanup"]["credential_removal_error"] = type(error).__name__ + ": " + str(error)
        receipt["cleanup"]["temporary_keys_removed"] = scratch is None or not scratch.exists()
        receipt["cleanup"]["owned_host_processes_stopped"] = (
            commands.finish() and receipt["cleanup"]["owned_host_processes_stopped"]
        )
        if not all(receipt["cleanup"].get(key, False) for key in (
            "owned_container_absent", "temporary_keys_removed", "owned_host_processes_stopped",
        )):
            receipt["outcome"] = "failed"
            receipt.setdefault("error", "Owned fixture cleanup could not be verified")
            code = 1
        receipt["finished_utc"] = ownership.utc_now()
        receipt["elapsed_seconds"] = round(time.monotonic() - start, 3)
        # Never preserve private file bytes. Receipt paths are historical only.
        receipt["files"] = {
            str(path.relative_to(output)): {"bytes": path.stat().st_size, "sha256": digest(path.read_bytes())}
            for path in sorted(output.rglob("*"))
            if not path.is_symlink() and path.is_file() and path.name != "result.json"
        }
        write_json(output / "result.json", receipt)
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
        print(f"Linux file interoperability {receipt['outcome']}: {output}", flush=True)
    return code


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (FixtureFailure, OSError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
