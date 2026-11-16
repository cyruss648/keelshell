#!/usr/bin/python3
"""Owned Linux fixture; successful collectors always execute real Linux tools.

Only the fixed identity probe and monitor collectors are admitted by ForceCommand. A separate
token-authenticated protocol changes df/ps/ss visibility or reads counters. Neither
protocol is an application command or a general-purpose remote administration API.
"""

import argparse
from contextlib import contextmanager
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import time

LAB = Path("/run/keelshell-monitor")
STATE = LAB / "state"
HELPER = "/opt/keelshell-monitor/fixture.py"
USER = "ksmonitor"
MAGIC = "KS_MONITOR_LAB_V1"
TOOLS = ("df", "ps", "ss")
MODES = ("real", "missing", "unsupported", "permission")

# Exact Linux collectors, including the negative-cache df variant.
# Output is produced by /bin/sh and the image's /proc/tools, never these strings.
PREFIX = ('LC_ALL=C; export LC_ALL\n'
          'if [ "$(uname -s)" != Linux ]; then exit 64; fi\n')
DF = """df_status=0
if command -v df >/dev/null 2>&1; then
  df -Pk || df_status=$?
else
  df_status=127
fi
printf '\\n@@KS:df-status@@\\n%s\\n' "$df_status" """
DF = DF.rstrip()
SNAPSHOT = PREFIX + """printf '@@KS:platform@@\\nLinux\\n@@KS:stat@@\\n'
cat /proc/stat || exit 65
printf '\\n@@KS:meminfo@@\\n'
cat /proc/meminfo || exit 65
printf '\\n@@KS:loadavg@@\\n'
cat /proc/loadavg || exit 65
printf '\\n@@KS:uptime@@\\n'
cat /proc/uptime || exit 65
printf '\\n@@KS:net@@\\n'
cat /proc/net/dev || exit 65
printf '\\n@@KS:df@@\\n'
""" + DF + """
printf '\\n@@KS:bootid@@\\n'
if [ -r /proc/sys/kernel/random/boot_id ]; then
  cat /proc/sys/kernel/random/boot_id 2>/dev/null || printf '\\n!unavailable\\n'
fi
printf '\\n@@KS:diskstats@@\\n'
if [ -r /proc/diskstats ]; then
  cat /proc/diskstats 2>/dev/null || printf '\\n!unavailable\\n'
fi
printf '\\n@@KS:end@@\\n'
"""
CACHED_SNAPSHOT = SNAPSHOT.replace(DF, "printf '\\n@@KS:df-status@@\\ncached\\n'")
PLATFORM_SELECTOR = """ks_uname=''
for ks_candidate in /usr/bin/uname /bin/uname; do
  if [ -f "$ks_candidate" ] && [ -x "$ks_candidate" ]; then ks_uname="$ks_candidate"; break; fi
done
[ -n "$ks_uname" ] || exit 66
"""
PLATFORM_PROBE = """LC_ALL=C; export LC_ALL
""" + PLATFORM_SELECTOR + """ks_platform="$("$ks_uname" -s)" || exit 65
case "$ks_platform" in
  Linux|Darwin) ;;
  '') exit 65 ;;
  *) ks_platform=Unsupported ;;
esac
printf '@@KS:monitor-platform:1@@\\nplatform\\t%s\\nruntime\\tposix\\n@@KS:end@@\\n' "$ks_platform"
"""
PROCESSES = """LC_ALL=C; export LC_ALL
""" + PLATFORM_SELECTOR + """case "$("$ks_uname" -s)" in
  Linux) ;;
  Darwin)
    [ -x /bin/ps ] || exit 66
    /bin/ps -ww -axo pid=,user=,pcpu=,pmem=,args= || exit 65
    exit 0
    ;;
  *) exit 64 ;;
esac
command -v ps >/dev/null 2>&1 || exit 66
ps -ww -eo pid=,user=,pcpu=,pmem=,args= || exit 65
"""
SOCKETS = """LC_ALL=C; export LC_ALL
""" + PLATFORM_SELECTOR + """case "$("$ks_uname" -s)" in
  Linux) ;;
  Darwin)
    [ -f /usr/sbin/lsof ] && [ -x /usr/sbin/lsof ] || exit 66
    printf '@@KS:darwin-sockets:2@@\\n'
    /usr/sbin/lsof -nP -a -iTCP -sTCP:LISTEN -iUDP -F0pcftPnT -Ts
    ks_lsof_status=$?
    printf '@@KS:lsof-status@@\\n%s\\n@@KS:end@@\\n' "$ks_lsof_status"
    exit 0
    ;;
  *) exit 64 ;;
esac
command -v ss >/dev/null 2>&1 || exit 66
printf '@@KS:linux-sockets:1@@\\n'
ss -H -lntupn
ks_ss_status=$?
printf '@@KS:ss-status@@\\n%s\\n@@KS:end@@\\n' "$ks_ss_status"
"""
COLLECTORS = {
    PLATFORM_PROBE: ("identity", None),
    SNAPSHOT: ("core", "df"), CACHED_SNAPSHOT: ("core", None),
    PROCESSES: ("ps", "ps"), SOCKETS: ("ss", "ss"),
}

def collector_command(command):
    if command in COLLECTORS:
        return command
    start = PLATFORM_SELECTOR + 'case "$("$ks_uname" -s)" in\nDarwin)\n'
    separator = "\n;;\nLinux)\n"
    end = "\n;;\n*) exit 64 ;;\nesac\n"
    if command.startswith(start) and command.endswith(end):
        branches = command[len(start):-len(end)].rsplit(separator, 1)
        if len(branches) == 2 and branches[1] in (SNAPSHOT, CACHED_SNAPSHOT):
            # Only execute the exact known Linux body. The other platform's
            # unexecuted text cannot grant this fixture arbitrary shell access.
            return branches[1]
    raise PermissionError("Only fixed Linux monitor collectors are admitted")


def identity():
    stat = Path("/proc/1/stat").read_text()
    status = dict(line.split(":", 1) for line in Path("/proc/1/status").read_text().splitlines()
                  if ":" in line)
    return {
        "network_namespace": os.readlink("/proc/self/ns/net"),
        "pid1_birth_ticks": int(stat[stat.rfind(")") + 2:].split()[19]),
        "boot_id": Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
        "effective_caps_hex": status["CapEff"].strip(),
        "bounding_caps_hex": status["CapBnd"].strip(),
    }


def write_json(path, value):
    path.write_text(json.dumps(value, allow_nan=False) + "\n", encoding="utf-8")


@contextmanager
def locked_state():
    # Contract inspection is portable; entering the runtime still requires flock.
    import fcntl

    with (STATE / "lock").open("r+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        value = json.loads((STATE / "counts.json").read_text())
        yield value
        write_json(STATE / "counts.json", value)


def set_mode(value, tool, mode):
    if tool not in TOOLS or mode not in MODES:
        raise ValueError("Unrecognized fixed tool/mode")
    link = STATE / "bin" / tool
    link.unlink(missing_ok=True)
    if mode != "missing" and value["tools"][tool]["path"] is not None:
        # Direct Python shebang avoids looking for Python in the restricted PATH.
        link.symlink_to("/opt/keelshell-monitor/tool.py")
    value["modes"][tool] = mode


def control(command, token, value):
    """Parse fixed words; never pass a control request through a shell."""
    words = command.split(" ")
    if (len(words) < 3 or words[0] != MAGIC
            or not hmac.compare_digest(words[1], token)):
        raise PermissionError("Fixture control authentication rejected")
    if words[2:] == ["read-counts"]:
        return value
    if len(words) == 5 and words[2] == "set-mode":
        set_mode(value, words[3], words[4])
        value["mode_changes"] += 1
        return value
    raise ValueError("Unrecognized fixture control command")


def guard():
    import pwd

    if (not Path("/run/.containerenv").is_file()
            or os.getuid() != pwd.getpwnam(USER).pw_uid):
        raise PermissionError("Fixture requires its dedicated container account")
    current = identity()
    ready = json.loads((LAB / "ready.json").read_text())
    for name in ("network_namespace", "pid1_birth_ticks", "boot_id"):
        if current[name] != ready[name]:
            raise PermissionError("Fixture namespace/birth identity changed")


def forced():
    guard()
    command = os.environ.get("SSH_ORIGINAL_COMMAND", "")
    if command.startswith(MAGIC):
        with locked_state() as value:
            result = control(command, (STATE / "token").read_text(), value)
            print(json.dumps(result, allow_nan=False))
        return 0
    command = collector_command(command)
    operation, probe = COLLECTORS[command]
    with locked_state() as value:
        value["requests"][operation] += 1
        if probe is not None:
            value["probes"][probe] += 1
    environment = {
        "PATH": str(STATE / "bin"), "LC_ALL": "C", "LANG": "C",
        "HOME": "/home/" + USER,
    }
    os.execve("/bin/sh", ["sh", "-c", command], environment)


def tool(name, arguments):
    guard()
    with locked_state() as value:
        value["invocations"][name] += 1
        mode, executable = value["modes"][name], value["tools"][name]["path"]
        if mode == "real" and executable is not None:
            value["delegations"][name] += 1
    if mode == "unsupported":
        print(name + ": unrecognized option (owned fixture)", file=sys.stderr)
        return 2
    if mode == "permission":
        print(name + ": permission denied (owned fixture)", file=sys.stderr)
        return 126
    if mode != "real" or executable is None:
        print(name + ": command not found (owned fixture)", file=sys.stderr)
        return 127
    os.execve(executable, [name, *arguments], {
        "PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "HOME": "/home/" + USER,
    })


def config():
    return "\n".join([
        "Port 2222", "AddressFamily inet", "ListenAddress 0.0.0.0",
        f"HostKey {LAB}/keys/host", f"AuthorizedKeysFile {LAB}/keys/authorized_keys",
        f"PidFile {LAB}/sshd.pid", f"AllowUsers {USER}",
        "AuthenticationMethods publickey", "PubkeyAuthentication yes",
        "PasswordAuthentication no", "KbdInteractiveAuthentication no",
        "PermitEmptyPasswords no", "UsePAM no", "StrictModes yes", "UseDNS no",
        "PermitRootLogin no", "PermitUserEnvironment no",
        "AllowAgentForwarding no", "AllowStreamLocalForwarding no",
        "AllowTcpForwarding no", "PermitOpen none", "PermitTunnel no",
        "X11Forwarding no", "PermitTTY no", "MaxSessions 8", "MaxAuthTries 2",
        "LoginGraceTime 10", "LogLevel ERROR",
        f"ForceCommand /usr/bin/python3 {HELPER} force", "",
    ])


def prepare(owner):
    import pwd

    LAB.chmod(0o755)
    keys = LAB / "keys"
    keys.mkdir(mode=0o755)
    STATE.mkdir(mode=0o700)
    (STATE / "bin").mkdir(mode=0o700)
    (LAB / "owner").write_text(owner, encoding="ascii")
    for name in ("host", "client"):
        subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "",
                        "-f", str(keys / name)], stdin=subprocess.DEVNULL, check=True, timeout=4)
    (keys / "authorized_keys").write_text("restrict " + (keys / "client.pub").read_text())
    (keys / "authorized_keys").chmod(0o644)
    (STATE / "token").write_text(secrets.token_hex(32), encoding="ascii")
    (STATE / "token").chmod(0o600)
    (STATE / "lock").touch(mode=0o600)
    value = {
        "schema": 1, "owner": owner, "identity": identity(), "mode_changes": 0,
        "modes": {name: "real" for name in TOOLS},
        "requests": {name: 0 for name in ("identity", "core", "ps", "ss")},
        **{counter: dict.fromkeys(TOOLS, 0)
           for counter in ("probes", "invocations", "delegations")},
        "tools": {},
    }
    for name in TOOLS:
        found = shutil.which(name, path="/usr/sbin:/usr/bin:/sbin:/bin")
        path = Path(found).resolve() if found else None
        value["tools"][name] = {
            "path": str(path) if path else None,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest() if path else None,
        }
        set_mode(value, name, "real")
    for name in ("uname", "cat"):
        actual = shutil.which(name, path="/usr/bin:/bin")
        if actual is None:
            raise RuntimeError("CapabilityUnavailable: required core tool " + name)
        (STATE / "bin" / name).symlink_to(actual)
    write_json(STATE / "counts.json", value)
    account = pwd.getpwnam(USER)
    # Do not follow tool/core symlinks while changing only the owned tmpfs tree.
    for root, directories, files in os.walk(STATE):
        for path in [Path(root), *(Path(root) / name for name in directories + files)]:
            os.lchown(path, account.pw_uid, account.pw_gid)
    (LAB / "sshd_config").write_text(config())


def serve(owner):
    prepare(owner)
    subprocess.run(["/usr/sbin/sshd", "-t", "-f", str(LAB / "sshd_config")],
                   check=True, timeout=4, stdin=subprocess.DEVNULL)
    stopped = False

    def stop(_signal, _frame):
        nonlocal stopped
        stopped = True

    for sig in (signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, stop)
    process = subprocess.Popen(["/usr/sbin/sshd", "-D", "-e", "-f", str(LAB / "sshd_config")],
                               stdin=subprocess.DEVNULL)
    try:
        until = time.monotonic() + 5
        while not stopped:
            if process.poll() is not None:
                raise RuntimeError("Owned sshd exited before readiness")
            try:
                with socket.create_connection(("127.0.0.1", 2222), timeout=0.1):
                    break
            except OSError:
                if time.monotonic() >= until:
                    raise TimeoutError("Owned sshd readiness deadline")
                time.sleep(0.05)
        if not stopped:
            write_json(LAB / "ready.json", dict(identity(), owner=owner))
        while not stopped:
            if process.poll() is not None:
                raise RuntimeError("Owned sshd exited")
            time.sleep(0.05)
    finally:
        if process.poll() is None:
            process.terminate()
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("role", choices=("serve", "status", "force"))
    parser.add_argument("--owner")
    args = parser.parse_args()
    try:
        if args.role == "force":
            return forced()
        if not re.fullmatch(r"keelshell-monitor-[0-9a-f]{16}", args.owner or ""):
            raise ValueError("Invalid fixture owner")
        if args.role == "serve":
            if not Path("/run/.containerenv").is_file() or os.getuid() != 0:
                raise PermissionError("Only an owned Podman container may serve")
            serve(args.owner)
        else:
            if (LAB / "owner").read_text() != args.owner:
                raise PermissionError("Fixture owner mismatch")
            ready = json.loads((LAB / "ready.json").read_text())
            if ready != dict(identity(), owner=args.owner):
                raise PermissionError("Fixture identity changed")
            # counts omit credentials; only public key fingerprints leave tmpfs.
            public = json.loads((STATE / "counts.json").read_text())
            for name in ("host", "client"):
                pin = subprocess.run(["ssh-keygen", "-lf", str(LAB / "keys" / (name + ".pub"))],
                                     capture_output=True, text=True, check=True, timeout=3)
                public[name + "_pin"] = pin.stdout.split()[1]
            print(json.dumps(dict(public, sshd_config=config()), allow_nan=False))
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
        # Never echo SSH_ORIGINAL_COMMAND, the token or a credential payload.
        print('{"failure":{"type":"CapabilityUnavailable","message":"Owned fixture operation failed"}}',
              file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
