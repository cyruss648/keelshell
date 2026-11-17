"""Owned, opt-in Windows OpenSSH integration; no service or host configuration edits."""

from contextlib import ExitStack
import base64
import ctypes
import hashlib
import json
import ntpath
import os
from pathlib import Path
import platform
import re
import socket
import subprocess
import sys
import tempfile
import time

import windows_monitor_lab as lab

TEST_NAME = "actual_windows_openssh_monitor_matches_owned_fixture"
CONFIG_ENV = "KEELSHELL_WINDOWS_SSH_LAB"
USERNAME = re.compile(r"[a-z0-9_.-]+(?:\\[a-z0-9_.-]+)?", re.ASCII)


def username():
    """Read the current Windows token's login name, without environment fallbacks."""
    if os.name != "nt":
        raise lab.Hold("native_windows_required")
    from ctypes import wintypes as W
    secur32 = ctypes.WinDLL("secur32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    get_name = secur32.GetUserNameExW
    get_name.argtypes = [ctypes.c_int, W.LPWSTR, ctypes.POINTER(W.ULONG)]
    get_name.restype = W.BOOL
    get_computer = kernel32.GetComputerNameW
    get_computer.argtypes = [W.LPWSTR, ctypes.POINTER(W.DWORD)]
    get_computer.restype = W.BOOL
    text = ctypes.create_unicode_buffer(512)
    length = W.ULONG(len(text))
    if not get_name(2, text, ctypes.byref(length)):
        raise lab.Hold("current_token_login_name_unavailable")
    value = text.value.lower()
    computer = ctypes.create_unicode_buffer(512)
    length = W.DWORD(len(computer))
    if not get_computer(computer, ctypes.byref(length)):
        raise lab.Hold("current_computer_identity_unavailable")
    local = computer.value.lower() + "\\"
    if value.startswith(local):
        value = value[len(local):]
    if not USERNAME.fullmatch(value):
        raise lab.Hold("current_login_name_outside_lab_contract")
    return value


def server_config(directory, port, login):
    """Use only explicit keys, a loopback listener and the unmodified default shell."""
    lab.integer(port, 65535, 49152)
    if not isinstance(login, str) or not USERNAME.fullmatch(login):
        raise lab.LabError("invalid_isolated_login")

    def quoted(name):
        text = str(Path(directory) / name).replace("\\", "/")
        if any(c in text for c in ('"', "\r", "\n", "\x00")):
            raise lab.Hold("unsafe_ephemeral_path")
        return '"' + text + '"'

    # Windows does not implement several similarly named Unix sshd options,
    # including StrictModes, PidFile and KbdInteractiveAuthentication. No
    # inherited config/Match block can redirect this explicit key file.
    return "\n".join([
        f"Port {port}", "ListenAddress 127.0.0.1",
        "HostKey " + quoted("host"),
        "AuthorizedKeysFile " + quoted("authorized_keys"),
        "PubkeyAuthentication yes", "AuthenticationMethods publickey",
        "PasswordAuthentication no", "DisableForwarding yes", "PermitTTY no",
        "AllowUsers " + login, "MaxSessions 1", "MaxStartups 3",
        "LoginGraceTime 10", "LogLevel ERROR", "",
    ])


def default_shell():
    """Observe only three relevant registry values; never change the SSH shell."""
    if os.name != "nt":
        raise lab.Hold("native_windows_required")
    import winreg
    values = {}
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\OpenSSH", 0,
                           winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as key:
            for name in ("DefaultShell", "DefaultShellCommandOption",
                         "DefaultShellEscapeArguments"):
                try:
                    value, kind = winreg.QueryValueEx(key, name)
                except FileNotFoundError:
                    continue
                if type(value) not in (str, int) or (
                    isinstance(value, str) and (
                        len(value) > 32768 or any(c in value for c in ("\x00", "\r", "\n"))
                    )
                ):
                    raise lab.Hold("unreadable_default_shell_contract")
                values[name] = {"value": value, "registry_type": kind}
    except FileNotFoundError:
        pass
    return {"registry_values": values, "changed": False,
            "inherited_default": "cmd.exe" if "DefaultShell" not in values else None}


def host_fingerprint(raw):
    """Compute the pinned SHA256 identity from this lab's Ed25519 public key."""
    fields = raw.strip().split()
    if len(fields) not in (2, 3) or fields[0] != b"ssh-ed25519":
        raise lab.LabError("invalid_ephemeral_public_key")
    try:
        blob = base64.b64decode(fields[1], validate=True)
    except ValueError as error:
        raise lab.LabError("invalid_ephemeral_public_key") from error
    if len(blob) != 51 or blob[:19] != b"\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20":
        raise lab.LabError("invalid_ephemeral_ed25519_blob")
    return "SHA256:" + base64.b64encode(hashlib.sha256(blob).digest()).decode("ascii").rstrip("=")


def test_binary(path):
    """Require a local native executable and the exact opt-in test identity."""
    path = Path(path).absolute()
    if (path.is_symlink() or not path.is_file()
            or not re.fullmatch(r"windows_openssh_monitor-[0-9a-f]+\.exe", path.name)):
        raise lab.Hold("exact_windows_ssh_test_binary_required")
    with path.open("rb") as stream:
        if stream.read(2) != b"MZ":
            raise lab.Hold("native_windows_ssh_test_binary_required")
    return path


def openssh_tools():
    """Use the installed Windows feature's tools without installation or downloads."""
    directory = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32/OpenSSH"
    paths = {name: directory / (name + ".exe") for name in ("sshd", "ssh-keygen")}
    if not all(p.is_file() and not p.is_symlink() for p in paths.values()):
        raise lab.Hold("openssh_binaries_missing_no_install_or_service_changes")
    return paths


def check_inventory(raw):
    names = re.findall(r"^(\S+): test$", raw.decode("utf-8-sig"), re.MULTILINE)
    if names.count(TEST_NAME) != 1 or len(names) != len(set(names)):
        raise lab.LabError("exact_native_ssh_test_absent_or_duplicate")
    return names


def verify_source(evidence, source):
    if not isinstance(source, str) or not re.fullmatch(r"[0-9a-f]{40}", source):
        raise lab.Hold("exact_committed_source_required")
    head = evidence.run("source-head", ["git", "rev-parse", "HEAD"])
    clean = evidence.run("source-status", [
        "git", "status", "--porcelain=v1", "--untracked-files=all", "--",
        "crates", "scripts", ".github", "Cargo.toml", "Cargo.lock",
    ])
    if (head.exit_code != 0 or head.stderr or head.stdout.strip() != source.encode("ascii")
            or clean.exit_code != 0 or clean.stdout or clean.stderr):
        raise lab.Hold("source_does_not_match_clean_committed_engineering_inputs")


def key_material(evidence, tools, directory, port, login):
    """Create only short-lived private files outside the uploaded evidence directory."""
    lab.secure_directory(directory)
    for name in ("host", "client"):
        result = evidence.run("live-keygen-" + name, [
            str(tools["ssh-keygen"]), "-q", "-t", "ed25519", "-N", "",
            "-C", "keelshell-owned-ssh-lab", "-f", str(directory / name),
        ])
        if result.exit_code != 0 or result.stdout or result.stderr:
            raise lab.Hold("ephemeral_ssh_key_generation_failed")
    public = (directory / "client.pub").read_bytes()
    host_public = (directory / "host.pub").read_bytes()
    fingerprint = host_fingerprint(host_public)
    host_fingerprint(public)
    (directory / "authorized_keys").write_bytes(b"restrict " + public)
    config = server_config(directory, port, login)
    (directory / "sshd_config").write_text(config, encoding="utf-8", newline="\n")
    evidence.write_json("isolated-server.json", {
        "port": port, "listen": "127.0.0.1", "login": login,
        "host_fingerprint": fingerprint, "config_sha256": hashlib.sha256(config.encode()).hexdigest(),
        "host_public_sha256": hashlib.sha256(host_public).hexdigest(),
        "client_public_sha256": hashlib.sha256(public).hexdigest(),
        "protected_temporary_directory": True, "private_keys_outside_output": True,
        "forced_command": False, "installed_service_changed": False,
        "registry_changed": False, "firewall_changed": False,
    })
    result = evidence.run("live-sshd-config", [
        str(tools["sshd"]), "-t", "-f", str(directory / "sshd_config"),
    ])
    if result.exit_code != 0 or result.stdout or result.stderr:
        raise lab.Hold("isolated_sshd_configuration_rejected")
    return fingerprint


def ready(server, port, budget):
    """A TCP readiness attempt is only a wait; pinned authentication proves the server."""
    while True:
        server.drain()
        if server.observed is not None:
            raise lab.Hold("owned_sshd_exited_before_authentication")
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as connection:
            connection.settimeout(min(0.05, budget.remaining(reserve=10)))
            if connection.connect_ex(("127.0.0.1", port)) == 0:
                return
        time.sleep(min(0.01, budget.remaining(reserve=10)))


def validate_client_report(value, expected_fixture=None):
    """The native Rust test supplies parsed production observations, never replay frames."""
    if (not isinstance(value, dict) or type(value.get("schema")) is not int
            or value["schema"] != 1 or value.get("status") != "PASS"
            or value.get("test") != TEST_NAME or value.get("errors") != []):
        raise lab.LabError("native_ssh_client_report_not_pass")
    identity = value.get("identity")
    if (not isinstance(identity, dict) or identity.get("platform") != "Windows"
            or identity.get("runtime") != "WindowsPowerShell"
            or not isinstance(identity.get("runtime_version"), str)
            or not re.fullmatch(r"5\.1\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)",
                                identity["runtime_version"])):
        raise lab.LabError("native_ssh_runtime_identity_unproven")
    for part in identity["runtime_version"].split("."):
        lab.integer(int(part), 0xffffffff)
    assertions = value.get("assertion_evidence")
    if (not isinstance(assertions, list) or not assertions
            or any(not isinstance(row, dict) or row.get("passed") is not True
                   or not isinstance(row.get("assertion"), str) for row in assertions)):
        raise lab.LabError("native_ssh_assertion_evidence_incomplete")
    required = {
        "wrong_pin_rejected_before_authenticated_session",
        "independent_identity_is_Windows_PowerShell_5_1",
        "Windows_unknown_fields_and_CPU_gauge_are_not_invented",
        "owned_fixture_PID_and_real_working_set_are_observed",
        "owned_dual_stack_listener_or_UDP_binding_is_visible",
        "connected_only_TCP_endpoint_is_excluded",
        "cache_reads_and_clone_preserve_observations_and_authority",
        "pause_revokes_old_report_but_preserves_completed_observations",
        "clone_revocation_is_shared_and_old_reports_stay_revoked",
        "explicit_reset_clears_shared_cache_without_redetection",
    }
    if not required.issubset({row.get("assertion") for row in assertions}):
        raise lab.LabError("native_ssh_required_assertions_absent")
    calls = value.get("api_calls")
    if (not isinstance(calls, dict) or set(calls) != {"snapshot", "processes", "listening_sockets"}
            or any(type(count) is not int or count != 1 for count in calls.values())):
        raise lab.LabError("native_ssh_collection_call_count_mismatch")
    disconnect = value.get("disconnect")
    if (not isinstance(disconnect, dict) or any(disconnect.get(name) is not True for name in
            ("attempted", "request_succeeded", "local_transport_closed"))):
        raise lab.LabError("native_ssh_local_disconnect_unproven")
    if expected_fixture is not None and value.get("fixture") != expected_fixture:
        raise lab.LabError("native_ssh_fixture_differs_from_original_child")
    for operation in ("resources", "processes", "sockets"):
        lab.validate_data(operation, value.get("windows_" + operation))
    if value["windows_resources"]["capacity_status"] != "ok":
        raise lab.Hold("windows_ssh_capacity_not_fully_observed")
    return value


def validate_debug_cleanup(admitted, final):
    """Cleanup cannot erase an admission or substitute forced exits for natural ones."""
    if (not isinstance(final, dict) or final.get("errors")
            or final.get("terminal_proven") is not True or final.get("closed") is not True
            or final.get("evidence_kind") != "NATIVE_WINDOWS"
            or final.get("state") != "PASS"
            or final.get("complete_original_object_census") is not True
            or final.get("admission_closed") is not True
            or final.get("identities_bound") is not True
            or any(type(final.get(name)) is not int or final[name] != 0 for name in (
                "pending_process_objects", "owned_handles_remaining", "fallback_detaches"))):
        raise lab.LabError("debug_originals_changed_or_nonterminal_during_cleanup")
    rows = final.get("admissions")
    if (not isinstance(rows, list)
            or len(rows) != len(admitted)
            or any(not isinstance(row, dict)
                   or type(row.get("pid")) is not int or type(row.get("creation_time")) is not int
                   or type(row.get("is_master")) is not bool for row in rows)
            or {(row["pid"], row["creation_time"]) for row in rows} != admitted
            or any(not row["is_master"] and row.get("natural_terminal_before_cleanup") is not True
                   for row in rows)):
        raise lab.LabError("debug_admissions_changed_during_cleanup")


def wait_client(client, server, budget):
    while True:
        server.drain()
        client.drain()
        if server.observed is not None:
            raise lab.LabError("owned_sshd_exited_during_authenticated_monitoring")
        if client.observed is not None and all(p.complete() for p in client.pumps):
            return client.observed
        time.sleep(min(0.01, budget.remaining(reserve=10)))


def validate_powershell_admissions(snapshot, commands, executable):
    """Require source-exact command lines and natural exits of original process objects."""
    if (not isinstance(snapshot, dict) or snapshot.get("errors")
            or snapshot.get("complete_original_object_census") is not True
            or snapshot.get("admission_closed") is not True
            or snapshot.get("identities_bound") is not True
            or snapshot.get("evidence_kind") != "NATIVE_WINDOWS"):
        raise lab.Hold("ssh_original_process_census_unproven")
    rows = snapshot.get("admissions", [])
    if not isinstance(rows, list) or not rows:
        raise lab.Hold("ssh_original_process_census_unproven")
    seen = set()
    masters = 0
    powershell = []
    expected = {command: name for name, command in commands.items() if name != "resources-skipped"}
    for row in rows:
        if not isinstance(row, dict) or type(row.get("is_master")) is not bool:
            raise lab.Hold("ssh_process_identity_unproven")
        identity = (lab.integer(row.get("pid"), 0xffffffff, 1),
                    lab.integer(row.get("creation_time"), 0xffffffffffffffff, 1))
        if (identity[0] in seen or row.get("retained_before_first_instruction") is not True
                or row.get("identity_bound_before_first_continue") is not True):
            raise lab.Hold("ssh_process_identity_unproven")
        seen.add(identity[0])
        if row["is_master"]:
            masters += 1
            continue
        if (row.get("retained_before_first_instruction") is not True
                or row.get("natural_terminal_before_cleanup") is not True):
            raise lab.Hold("ssh_admitted_process_terminal_unproven")
        if ntpath.normcase(row.get("image", "")) != ntpath.normcase(executable):
            continue
        # The DEBUG event supplies the original command line before admission.
        # OpenSSH may spell argv[0] as the filename or a full path.
        # cmd.exe may add spaces between argv[0] and the first argument. Only
        # that boundary is normalized; all production arguments stay exact.
        text = row.get("command_line", "")
        matched = re.fullmatch(
            r'(?:"([^"\r\n]+)"|([^"\s]+))'
            r" +(-NoLogo -NoProfile -NonInteractive -EncodedCommand [A-Za-z0-9+/]+=*)",
            text,
        )
        if matched is None:
            raise lab.Hold("ssh_powershell_command_line_binding_unproven")
        program = matched[1] or matched[2]
        if (ntpath.normcase(program) != "powershell.exe"
                and ntpath.normcase(program) != ntpath.normcase(executable)):
            raise lab.LabError("ssh_powershell_argv_identity_mismatch")
        command = "powershell.exe " + matched[3]
        if command not in expected or row.get("exit_code") != 0:
            raise lab.LabError("ssh_powershell_does_not_match_exact_production_command")
        powershell.append({"operation": expected[command], "pid": row["pid"],
                           "creation_time": row.get("creation_time"), "exit_code": 0,
                           "wire_sha256": hashlib.sha256(command.encode("ascii")).hexdigest()})
    if masters != 1 or sorted(row["operation"] for row in powershell) != [
            "probe", "processes", "resources", "sockets"]:
        raise lab.Hold("exact_ssh_production_operation_census_unproven")
    return {"operations": powershell, "source_exact": True,
            "original_processes_naturally_terminal": True}


def _native_gate(evidence, binary):
    from windows_monitor.debug_job import WindowsDebugJob

    class DebugChild(lab.OwnedChild):
        def drain(self):
            self.owner.pump_debug(self.budget)
            super().drain()

    tools = openssh_tools()
    binary = test_binary(binary)
    inventory = evidence.run("live-test-inventory", [str(binary), "--list"])
    if inventory.exit_code != 0 or inventory.stderr:
        raise lab.LabError("native_ssh_test_inventory_failed")
    names = check_inventory(inventory.stdout)
    evidence.write_json("test-binary.json", {
        "name": binary.name, "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "inventory": names, "executed": TEST_NAME,
    })
    evidence.write_json("default-shell.json", default_shell())
    evidence.write_json("openssh-tools.json", {
        name: {"name": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
        for name, path in tools.items()
    })
    commands = {}
    for operation in lab.ASSEMBLY:
        command, binding = lab.assemble(operation)
        evidence.write_json(operation + ".assembly.json", binding)
        commands[operation] = command
    fixture = None
    temporary = None
    try:
        with tempfile.TemporaryDirectory(prefix="keelshell-live-windows-ssh-") as name:
            temporary = Path(name)
            with ExitStack() as stack:
                reservation = stack.enter_context(socket.socket(socket.AF_INET, socket.SOCK_STREAM))
                reservation.bind(("127.0.0.1", 0))
                port = reservation.getsockname()[1]
                lab.integer(port, 65535, 49152)
                fingerprint = key_material(evidence, tools, temporary, port, username())
                reservation.close()
                server = DebugChild(
                    [str(tools["sshd"]), "-D", "-e", "-f", str(temporary / "sshd_config")],
                    lab.Budget(210, evidence.budget.end), owner=WindowsDebugJob(),
                )
                try:
                    server.start()
                    ready(server, port, evidence.budget)
                    fixture = lab.OwnedChild(
                        [sys.executable, "-B", "-u", str(lab.FIXTURES / "owned_fixture.py"),
                         "--seconds", "180"], lab.Budget(210, evidence.budget.end),
                    )
                    fixture.start()
                    owned = fixture.ready()
                    evidence.write_json("fixture.json", owned)
                    oracle = lab.NativeOracle(evidence.budget)
                    before, cpu_before = oracle.resources(), oracle.cpu_times()
                    evidence.write_json("independent-before.json", {"resources": before, "cpu": cpu_before})
                    config = {
                        "schema": 1, "host": "127.0.0.1", "port": port, "username": username(),
                        "private_key": str(temporary / "client"), "host_fingerprint": fingerprint,
                        "fixture": owned, "output_path": str(evidence.path.absolute() / "rust-client.json"),
                    }
                    private_config = temporary / "client.json"
                    private_config.write_text(json.dumps(config), encoding="utf-8")
                    environment = dict(os.environ, **{CONFIG_ENV: str(private_config)})
                    client = lab.OwnedChild(
                        [str(binary), TEST_NAME, "--ignored", "--exact", "--test-threads=1", "--nocapture"],
                        lab.Budget(110, evidence.budget.end),
                    )
                    try:
                        client.start(environment=environment)
                        code = wait_client(client, server, evidence.budget)
                    finally:
                        client_receipt = evidence.keep_child("native-ssh-client", client)
                    if code != 0 or not lab.terminal_ok(client_receipt):
                        raise lab.LabError("native_ssh_client_failed")
                    text = bytes(client.stdout).decode("utf-8", errors="strict")
                    if (not re.search(rf"test {TEST_NAME} \.\.\. ok", text)
                            or "test result: ok. 1 passed; 0 failed; 0 ignored;" not in text):
                        raise lab.LabError("exact_native_ssh_test_result_absent")
                    report_file = evidence.path / "rust-client.json"
                    if report_file.is_symlink():
                        raise lab.LabError("native_ssh_report_is_a_symlink")
                    with report_file.open("rb") as stream:
                        raw_report = stream.read(lab.MAX_FRAME + 1)
                    if len(raw_report) > lab.MAX_FRAME:
                        raise lab.LabError("native_ssh_report_exceeds_budget")
                    report = validate_client_report(lab.strict_json(raw_report.decode("utf-8")), owned)
                    after, cpu_after = oracle.resources(), oracle.cpu_times()
                    process, sockets = oracle.process(fixture.process.pid), oracle.sockets()
                    evidence.write_json("independent.json", {
                        "before": before, "after": after, "cpu_before": cpu_before, "cpu_after": cpu_after,
                        "process": process, "sockets": sockets,
                    })
                    comparison = {
                        "owned": lab.compare_owned(owned, report["windows_processes"], report["windows_sockets"],
                                                   process, sockets),
                        "resources": lab.compare_resources(report["windows_resources"], before, after,
                                                           cpu_before, cpu_after),
                    }
                    evidence.write_json("comparison.json", comparison)
                    snapshot = server.owner.seal_admission(evidence.budget, settle_seconds=3)
                    evidence.write_json("ssh-debug-originals.json", snapshot)
                    evidence.write_json("ssh-production-binding.json", validate_powershell_admissions(
                        snapshot, commands, lab.powershell_path()))
                    admitted = {(row["pid"], row["creation_time"]) for row in snapshot["admissions"]}
                finally:
                    try:
                        server_receipt = evidence.keep_child("owned-sshd", server)
                    finally:
                        # Keep the original-object failure even when closing
                        # the owned child cannot produce a terminal receipt.
                        final_debug = server.owner.terminal_evidence()
                        evidence.write_json("ssh-debug-cleanup.json", final_debug)
                if not lab.terminal_ok(server_receipt):
                    raise lab.LabError("owned_sshd_terminal_unproven")
                validate_debug_cleanup(admitted, final_debug)
                if fixture.stop_fixture() != 0:
                    raise lab.LabError("owned_fixture_did_not_stop")
    finally:
        fixture_receipt = evidence.keep_child("live-fixture", fixture) if fixture is not None else None
        evidence.write_json("ssh-cleanup.json", {
            "temporary_directory_removed": temporary is not None and not temporary.exists(),
            "private_config_uploaded": False, "keys_uploaded": False,
            "fixture_started": fixture is not None and fixture.process is not None,
        })
    if temporary is None or temporary.exists():
        raise lab.LabError("ephemeral_ssh_material_cleanup_unproven")
    if fixture_receipt is None or not lab.terminal_ok(fixture_receipt):
        raise lab.LabError("owned_fixture_terminal_unproven")
    return {"ssh": "PASS", "windows_api": "PASS",
            "scope": "actual pinned Windows OpenSSH / default shell / production monitor / owned fixtures"}


def run_live(binary, output, seconds=240, source=None):
    evidence = lab.Evidence(output, seconds)
    receipt = {"schema": 1, "status": "FAIL", "ssh": "UNRUN", "windows_api": "UNRUN",
               "native_gui": "UNRUN", "started_unix": time.time(), "children": evidence.children,
               "source": source, "system": platform.system(), "build": platform.version(),
               "architecture": platform.machine(), "python": sys.version}
    code = 1
    try:
        if os.name != "nt":
            raise lab.Hold("native_windows_required")
        verify_source(evidence, source)
        result = _native_gate(evidence, binary)
        receipt.update(result, status="PASS")
        code = 0
    except lab.Hold as error:
        receipt.update(status="HOLD", ssh="HOLD", reason=str(error))
        code = lab.HOLD_EXIT
    except (lab.LabError, OSError, ValueError, TimeoutError, lab.lifecycle.OwnershipError,
            subprocess.SubprocessError) as error:
        receipt.update(ssh="FAIL", reason=str(error) if isinstance(error, lab.LabError)
                       else type(error).__name__)
    finally:
        if receipt["windows_api"] == "UNRUN" and (evidence.path / "independent-before.json").is_file():
            receipt["windows_api"] = "PARTIAL"
        receipt["ssh_client_started"] = any(child["label"] == "native-ssh-client"
                                            for child in evidence.children)
        if any(not lab.terminal_ok(child) for child in evidence.children):
            receipt.update(status="FAIL", cleanup_failure=True)
            code = 1
        receipt["finished_unix"] = time.time()
        evidence.write_json("receipt.json", receipt)
    return code
