"""Offline counterexamples for parsing, process failure propagation and budgets.

These controlled Python children are not CDB/Windows/application acceptance.
"""
import json
import os
from pathlib import Path
import sys
import tempfile
import signal
import subprocess
import threading
import time
import unittest
from unittest.mock import patch

import windows_native_crash as probe
owned = probe


NONCE = "a" * 32
NATIVE = f"""KEEL_{NONCE}_NATIVE_BEGIN_SECOND
Last event: 4d2.456: Security check failure - code c0000409 (second chance)
ExceptionAddress: 00007fff`00012345 (ucrtbase!abort+0x45)
ExceptionCode: c0000409
ExceptionFlags: 00000001
NumberParameters: 1
Parameter[0]: 0000000000000007
start             end                 module name
00007fff`00000000 00007fff`00100000   ucrtbase
 # Child-SP          RetAddr               Call Site
00 000000aa`000ff000 00007fff`00022222 ucrtbase!abort+0x45
01 000000aa`000ff008 00007fff`00033333 testapp!owned_test_worker+0x3
KEEL_{NONCE}_NATIVE_END_SECOND
"""
SUMMARY = "test result: ok. 704 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.00s"


class FakeOwner:
    """Real child lifetime, controlled target DWORD; no Win32 emulation claim."""

    def __init__(self, target_exit=0, fault=None):
        self.target_exit = target_exit
        self.fault = fault
        self.process = None
        self.opened = False
        self.closed = False
        self.terminated = False

    def spawn(self, command, cwd, env, deadline, merge_error=False):
        self.process = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                start_new_session=(os.name != 'nt'))
        return self.process

    def debugger(self, process):
        self.process = process
        if self.fault == "assign":
            raise probe.ProbeError("owned synthetic Job assignment failure")

    def open_target(self, pid):
        if self.fault == "open":
            raise probe.ProbeError("owned synthetic OpenProcess failure")
        if pid != 1234:
            raise probe.ProbeError("wrong synthetic PID")
        self.opened = True

    def exit_code(self):
        if self.fault == "query":
            raise probe.ProbeError("owned synthetic query failure")
        if self.opened and not self.terminated and self.process.poll() is not None:
            return self.target_exit
        return None

    def terminate(self):
        self.terminated = True
        if self.process.poll() is None and self.fault != "assign":
            self.process.kill()

    force = terminate

    def active(self, deadline):
        return int(self.process.poll() is None)

    def close(self):
        self.closed = True


class ParsingTests(unittest.TestCase):
    def test_distinct_whole_application_list_and_no_filter(self):
        text = "\n".join(f"tests::case_{index}: test" for index in range(706))
        self.assertEqual(len(probe.test_names(text)), 706)
        for bad in [text.replace("tests::case_705", "tests::case_704"), text + "\nextra: test", text.rsplit("\n", 1)[0]]:
            with self.assertRaises(probe.ProbeError):
                probe.test_names(bad)

    def test_exact_artifact_selection_rejects_ambiguity(self):
        artifact = {"reason": "compiler-artifact", "target": {"name": "keelshell-app"}, "profile": {"test": True}, "executable": "owned-test.exe"}
        self.assertEqual(probe.application_artifact(json.dumps(artifact)).name, "owned-test.exe")
        with self.assertRaises(probe.ProbeError):
            probe.application_artifact(json.dumps(artifact) + "\n" + json.dumps(artifact))

    def test_incremental_exception_context_and_parameter_are_actual_text(self):
        evidence = probe.TextEvidence(NONCE, {"tests::case"})
        content = (f"KEEL_{NONCE}_PID:1234\n" + NATIVE).encode()
        for byte in content:
            evidence.feed(bytes([byte]))
        evidence.finish()
        self.assertEqual(evidence.pid, 1234)
        self.assertTrue(evidence.exception_complete())
        self.assertEqual(evidence.exception_details(), {"exception_codes": ["0xc0000409"], "parameter_0": [7], "fault_module": "UCRTBASE", "native_frame_count": 2, "chances": ["SECOND"]})
        self.assertEqual(evidence.upload_events()[0]["parameters"], [7])
        self.assertFalse(evidence.errors)

    def test_secret_text_and_ignored_reason_never_enter_export(self):
        evidence = probe.TextEvidence(NONCE, {"tests::case"})
        evidence.feed(b"TOKEN=owned-secret\nAuthorization: Bearer owned-secret\ntest unknown ... ok\ntest tests::case ... ignored, TOKEN=owned-secret\n")
        evidence.finish()
        self.assertEqual(evidence.harness, ["test tests::case ... ignored"])
        self.assertNotIn("owned-secret", json.dumps(evidence.upload_events()) + "\n".join(evidence.harness))

    def test_fake_nonce_and_echoed_commands_cannot_open_native_block(self):
        evidence = probe.TextEvidence(NONCE, {"tests::case"})
        evidence.feed(("KEEL_wrong_NATIVE_BEGIN\nTOKEN=owned-secret\n0:000> .echo KEEL_" + NONCE + "_NATIVE_BEGIN\n").encode())
        evidence.finish()
        self.assertEqual(evidence.upload_events(), [])

    def test_incomplete_or_missing_context_is_not_success(self):
        evidence = probe.TextEvidence(NONCE, set())
        evidence.feed(NATIVE.rsplit("KEEL_", 1)[0].encode())
        evidence.finish()
        self.assertIn("incomplete_native_block", evidence.errors)
        self.assertFalse(evidence.exception_complete())

    def test_text_and_line_boundaries_are_bounded(self):
        evidence = probe.TextEvidence(NONCE, set())
        evidence.feed(b"x" * (probe.MAX_LINE + 1))
        self.assertIn("line_budget_exceeded", evidence.errors)
        self.assertEqual(evidence.pending, b"")
        evidence.feed(b"x" * probe.MAX_TEXT)
        self.assertIn("text_budget_exceeded", evidence.errors)
        self.assertEqual(evidence.upload_events(), [])

    def test_exact_deadline_boundary_and_no_budget_extension(self):
        clock = [100.0]
        deadline = probe.Deadline(1000 + 5400, clock=lambda: clock[0], wall=lambda: 1000)
        self.assertEqual(deadline.remaining(), 5400)
        clock[0] += 5395
        with self.assertRaises(TimeoutError):
            deadline.remaining(reserve=5)
        for epoch in [1000, 999, 6400.1]:
            with self.assertRaises(probe.ProbeError):
                probe.Deadline(epoch, clock=lambda: 100, wall=lambda: 1000)

    def test_failure_dword_dominates_diagnostics_and_timeout(self):
        for diagnostic_ok in [True, False]:
            for timed_out in [True, False]:
                self.assertEqual(probe.result_code(0xC0000409, diagnostic_ok, timed_out), 0xC0000409)
        self.assertEqual(probe.result_code(0, False), 125)
        self.assertEqual(probe.result_code(None, True), 125)
        self.assertEqual(probe.result_code(None, False, True), 124)
        self.assertEqual(probe.signed_exit(0xC0000409) & 0xffffffff, 0xC0000409)

    def test_generated_debugger_commands_preserve_unhandled_and_local_symbols(self):
        script = probe.debugger_script(NONCE)
        self.assertIn(".exr -1", script)
        self.assertIn("~* kn 0x40", script)
        self.assertIn("; gn", script)
        self.assertNotIn("; gh", script)
        self.assertNotIn(".dump", script)
        self.assertNotIn(".shell", script)
        self.assertNotIn("\ng\n", script)


class ProjectionTests(unittest.TestCase):
    def parse(self, content):
        evidence = probe.TextEvidence(NONCE, set())
        evidence.feed((f"KEEL_{NONCE}_PID:1234\n" + content).encode())
        evidence.finish()
        return evidence

    def test_first_then_second_chances_preserve_fields_parameters_and_stacks(self):
        first = NATIVE.replace("SECOND", "FIRST").replace("second chance", "first chance").replace("0000000000000007", "0000000000000009")
        evidence = self.parse(first + NATIVE)
        self.assertFalse(evidence.errors)
        events = evidence.upload_events()
        self.assertEqual([event["chance"] for event in events], ["FIRST", "SECOND"])
        self.assertEqual([event["parameters"] for event in events], [[9], [7]])
        for event in events:
            self.assertEqual(event["event_pid"], 1234)
            self.assertEqual(event["event_tid"], 0x456)
            self.assertEqual(event["exception_code"], 0xC0000409)
            self.assertEqual(event["exception_flags"], 1)
            self.assertEqual(event["fault_module"], "UCRTBASE")
            self.assertEqual(len(event["stacks"][0]["frames"]), 2)

    def test_all_thread_numeric_stacks_accept_official_case_and_decimal_thread_index(self):
        tail = (".  0 Id: 4d2.456 Suspend: 0 Teb: 000000aa`00100000 Unfrozen\n"
                " # Child-SP          RetAddr               Call Site\n"
                "00 000000aa`000ff000 00007fff`00022222 ucrtbase!abort+0x45\n"
                "  10 id: 4d2.457 Suspend: 0 Teb 000000aa`00200000 Unfrozen\n"
                " # Child-SP          RetAddr               Call Site\n"
                "00 000000aa`001ff000 00007fff`00044444 ntdll!OWNED_PRIVATE_SYMBOL\n")
        content = NATIVE.replace(f"KEEL_{NONCE}_NATIVE_END_SECOND", tail + f"KEEL_{NONCE}_NATIVE_END_SECOND")
        evidence = self.parse(content)
        self.assertFalse(evidence.errors)
        stacks = evidence.upload_events()[0]["stacks"]
        self.assertEqual(len(stacks), 3)
        self.assertEqual(stacks[-1]["debugger_thread_index"], 10)
        self.assertEqual(stacks[-1]["pid"], 1234)
        self.assertEqual(stacks[-1]["tid"], 0x457)
        self.assertEqual(stacks[-1]["frames"][0]["return_address"], 0x7fff00044444)
        self.assertNotIn("OWNED_PRIVATE_SYMBOL", json.dumps(evidence.upload_events()))

    def test_chance_hook_cannot_relabel_observed_chance(self):
        evidence = self.parse(NATIVE.replace("second chance", "first chance"))
        self.assertIn("native_required_fields_or_chance_missing", evidence.errors)
        self.assertEqual(evidence.upload_events(), [])

    def test_exception_hooks_include_both_chances_and_preserve_gn(self):
        command = next(line for line in probe.debugger_script(NONCE).splitlines() if line.endswith("0xc0000409"))
        self.assertIn('sxd -c "', command)
        self.assertIn('-c2 "', command)
        self.assertIn("NATIVE_BEGIN_FIRST", command)
        self.assertIn("NATIVE_BEGIN_SECOND", command)
        self.assertEqual(command.count("; gn"), 2)
        self.assertNotIn(" -h ", command)
        self.assertIn(".outmask- 0x80", probe.debugger_script(NONCE))

    def test_unknown_duplicate_out_of_order_and_missing_fields_reject_entire_export(self):
        variants = [
            NATIVE.replace("ExceptionFlags: 00000001", "ExceptionFlags: 00000001\nExceptionFlags: 00000001"),
            NATIVE.replace("ExceptionCode: c0000409\nExceptionFlags: 00000001", "ExceptionFlags: 00000001\nExceptionCode: c0000409"),
            NATIVE.replace("NumberParameters: 1", "NumberParameters: 2"),
            NATIVE.replace("Parameter[0]", "Parameter[1]"),
            NATIVE.replace("ExceptionFlags: 00000001\n", ""),
            NATIVE.replace("ucrtbase\n #", "unknown_sensitive_module\n #"),
            NATIVE.replace("00007fff`00100000   ucrtbase", "00007fff`00000000   ucrtbase"),
            NATIVE.replace("01 000000aa", "02 000000aa"),
            NATIVE.replace("ExceptionFlags: 00000001", "ExceptionFlags: 100000000"),
            NATIVE.replace(f"KEEL_{NONCE}_NATIVE_END_SECOND", "owned_sensitive_arbitrary_text\n" + f"KEEL_{NONCE}_NATIVE_END_SECOND"),
        ]
        for content in variants:
            with self.subTest(content_index=variants.index(content)):
                evidence = self.parse(content)
                self.assertTrue(evidence.errors)
                self.assertEqual(evidence.upload_events(), [])

    def test_symbol_description_and_disassembly_text_are_never_upload_fields(self):
        content = NATIVE.replace("ucrtbase!abort+0x45", "ucrtbase!OWNED_SENSITIVE_SYMBOL+0x45")
        evidence = self.parse(content)
        self.assertFalse(evidence.errors)
        uploaded = json.dumps(evidence.upload_events())
        self.assertNotIn("OWNED_SENSITIVE_SYMBOL", uploaded)
        self.assertNotIn("Security check failure", uploaded)
        self.assertEqual(evidence.exception_details()["fault_module"], "UCRTBASE")

    def test_private_raw_is_bounded_and_upload_never_contains_it(self):
        with tempfile.TemporaryDirectory(prefix="owned-private-raw-") as temp:
            path = Path(temp) / "raw.bin"
            evidence = probe.TextEvidence(NONCE, set(), raw_path=path)
            evidence.feed(b"OWNED_PRIVATE_SENTINEL\n" + b"x" * probe.MAX_TEXT)
            evidence.finish()
            self.assertEqual(path.stat().st_size, probe.MAX_TEXT)
            self.assertTrue(path.read_bytes().startswith(b"OWNED_PRIVATE_SENTINEL"))
            self.assertEqual(evidence.upload_events(), [])
            if os.name != "nt":
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)


def retain_control(name, receipt, raw=None, source=None):
    """Opt-in local evidence for owned fixtures; absent in the public workflow."""
    destination = os.environ.get("KEEL_CONTROL_EVIDENCE_DIR")
    if destination is None:
        return
    directory = Path(destination)
    directory.mkdir(parents=True, exist_ok=True)
    (directory / f"{name}.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if raw is not None:
        path = directory / f"{name}-owned-raw.bin"
        with os.fdopen(os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "wb") as output:
            output.write(raw)
    if source is not None:
        (directory / f"{name}-owned-helper.py").write_text(source)


def process_state(pid):
    """Bounded actual host state, never an assumed synthetic child result."""
    if os.name != "nt":
        result = subprocess.run(["ps", "-p", str(pid), "-o", "stat="], capture_output=True,
                                timeout=0.5, text=True)
        return "ABSENT" if result.returncode or not result.stdout.strip() else result.stdout.strip()
    import ctypes
    from ctypes import wintypes
    api = ctypes.WinDLL("kernel32", use_last_error=True)
    api.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    api.OpenProcess.restype = wintypes.HANDLE
    api.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    api.WaitForSingleObject.restype = wintypes.DWORD
    api.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = api.OpenProcess(0x00100000, False, pid)
    if not handle:
        return "ABSENT" if ctypes.get_last_error() == 87 else "QUERY_FAILED"
    try:
        status = api.WaitForSingleObject(handle, 0)
        return "TERMINAL" if status == 0 else "LIVE" if status == 258 else "QUERY_FAILED"
    finally:
        api.CloseHandle(handle)


def ready_pids(record, parent_pid):
    """Existence is not readiness: only a complete, expected publication qualifies."""
    try:
        pids = json.loads(record.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return None
    if (not isinstance(pids, dict) or set(pids) != {"parent", "child"}
            or any(type(pid) is not int or not 0 < pid <= 0xffffffff for pid in pids.values())
            or pids["parent"] != parent_pid or pids["child"] == parent_pid):
        raise AssertionError("unexpected owned PID publication")
    return pids


class NativeTreeHandles:
    """Bind fixture identities before cleanup; reopened PIDs are only extra evidence.

    The injected API in unit controls is a counterexample model, not Win32
    acceptance. Actual Windows controls use the owner's configured kernel32 API.
    """

    def __init__(self, api, job):
        self.api, self.job = api, job
        self.handles = {}
        self.membership = {}

    def bind(self, pids):
        from ctypes import byref
        from ctypes import wintypes
        try:
            for name, pid in pids.items():
                handle = self.api.OpenProcess(0x00100000 | 0x0400, False, pid)
                if not handle:
                    raise AssertionError("owned process handle unavailable before ACK")
                self.handles[name] = handle
                present = wintypes.BOOL()
                if not self.api.IsProcessInJob(handle, self.job, byref(present)) or not present.value:
                    raise AssertionError("fixture process is not in its retained Job")
                self.membership[name] = True
                if self.api.WaitForSingleObject(handle, 0) != 258:
                    raise AssertionError("fixture process was not live before ACK")
        except BaseException:
            self.close()
            raise

    def states(self):
        states = {}
        for name, handle in self.handles.items():
            status = self.api.WaitForSingleObject(handle, 0)
            states[name] = "TERMINAL" if status == 0 else "LIVE" if status == 258 else "QUERY_FAILED"
        return states

    def close(self):
        for handle in self.handles.values():
            self.api.CloseHandle(handle)
        self.handles.clear()


class PreparationTreeTests(unittest.TestCase):
    controls = []

    @classmethod
    def tearDownClass(cls):
        print(json.dumps({"actual_preparation_controls": cls.controls, "host": os.name}, sort_keys=True))

    def tree(self, stdio="inherited", direct_exit=None, interrupt=False, staged=False):
        with tempfile.TemporaryDirectory(prefix="owned-preparation-tree-") as temp:
            directory = Path(temp)
            helper = directory / "parent.py"
            record = directory / "pids.json"
            pending = directory / "pids.pending"
            ack = directory / "identity-bound"
            empty_seen = directory / "empty-observed"
            partial_seen = directory / "partial-observed"
            published = directory / "published"
            child_source = ("import os,signal,sys,time\n"
                            "if os.name != 'nt': signal.signal(signal.SIGTERM,signal.SIG_IGN)\n"
                            "print('OWNED_DESCENDANT_PIPE',flush=True)\n"
                            "time.sleep(30)\n")
            # The parent cannot exit until the observer owns the original child
            # identity. Publication is closed before rename and never partial.
            helper.write_text("import json,os,signal,subprocess,sys,time\n"
                              "from pathlib import Path\n"
                              "def await_marker(path):\n"
                              "    while not path.exists(): time.sleep(0.001)\n"
                              "if os.name != 'nt': signal.signal(signal.SIGTERM,signal.SIG_IGN)\n"
                              f"child=subprocess.Popen([sys.executable,'-c',{child_source!r}],stdin=subprocess.DEVNULL,"
                              + ("stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL" if stdio == "detached" else "")
                              + ")\n"
                              + (f"Path({str(record)!r}).touch()\n"
                                 f"await_marker(Path({str(empty_seen)!r}))\n"
                                 f"Path({str(record)!r}).write_text('{{')\n"
                                 f"await_marker(Path({str(partial_seen)!r}))\n" if staged else "")
                              + f"Path({str(pending)!r}).write_text(json.dumps({{'parent':os.getpid(),'child':child.pid}}))\n"
                              f"os.replace({str(pending)!r},{str(record)!r})\n"
                              f"Path({str(published)!r}).touch()\n"
                              f"await_marker(Path({str(ack)!r}))\n"
                              "print('OWNED_PARENT_READY',flush=True)\n"
                              + ("time.sleep(30)\n" if direct_exit is None else f"sys.exit({direct_exit})\n"))
            deadline = probe.Deadline(time.time() + (8 if interrupt else 5.4))
            owner = probe.owner_for_host()
            identities = NativeTreeHandles(owner.api, owner.job) if os.name == "nt" else None
            signalled = False
            bound_pids = None
            partial_observations = []
            def tick(process):
                nonlocal signalled, bound_pids
                if bound_pids is not None:
                    return
                # The staged counterexample deliberately exposes incomplete
                # contents. Once its partial phase is acknowledged, avoid a
                # competing Windows read handle while the producer renames.
                if staged and partial_seen.exists() and not published.exists():
                    return
                pids = ready_pids(record, process.pid)
                if pids is None:
                    if staged and record.exists():
                        content = record.read_text()
                        if content == "" and not empty_seen.exists():
                            partial_observations.append("empty")
                            empty_seen.touch()
                        elif content == "{" and not partial_seen.exists():
                            partial_observations.append("partial")
                            partial_seen.touch()
                    return
                if identities is not None:
                    identities.bind(pids)
                elif os.getpgid(pids["child"]) != process.pid:
                    raise AssertionError("fixture descendant is outside its owned group")
                bound_pids = pids
                ack.touch()
                if interrupt:
                    signalled = True
                    signal.raise_signal(signal.SIGINT)
            start = time.monotonic()
            try:
                result, receipt = probe.run_owned([sys.executable, str(helper)], directory, dict(os.environ),
                                                  deadline, owner=owner, tick=tick)
                elapsed = time.monotonic() - start
                self.assertIsNotNone(bound_pids, "actual identities must be bound before ACK")
                pids = ready_pids(record, owner.process.pid)
                self.assertEqual(pids, bound_pids)
                native_states = identities.states() if identities is not None else None
                pid_states = {key: process_state(pid) for key, pid in pids.items()}
                states = native_states if native_states is not None else pid_states
                self.controls.append({"stdio": stdio, "requested_parent_exit": direct_exit,
                                      "signal": "SIGINT" if interrupt else None,
                                      "signal_sent": signalled, "identity_bound_before_ack": True,
                                      "state_identity": "retained_native_handles" if identities is not None else "owned_group_pids",
                                      "job_membership": dict(identities.membership) if identities is not None else None,
                                      "partial_publications_rejected": partial_observations,
                                      "elapsed_seconds": round(elapsed, 6), "receipt": receipt,
                                      "pids": pids, "post_states": states, "post_pid_query_states": pid_states})
                retain_control(f"tree-{stdio}-exit{direct_exit}-sigint{interrupt}-staged{staged}", self.controls[-1],
                               raw=(result.stdout + result.stderr).encode(), source=helper.read_text())
                # No wait or grace is added to these post-return observations.
                # Original Windows handles must already be signalled; another
                # process reusing a PID cannot supply or defeat this assertion.
                self.assertTrue(all(state in ("ABSENT", "TERMINAL") or state.startswith("Z") for state in states.values()), states)
                self.assertLess(elapsed, 3)
                self.assertTrue(receipt["terminal"]["direct_reaped"])
                self.assertEqual(receipt["terminal"]["active_processes"], 0)
                self.assertTrue(receipt["terminal"]["pipes_drained"])
                self.assertFalse(receipt["errors"])
                if staged:
                    self.assertEqual(partial_observations, ["empty", "partial"])
                return result, receipt
            finally:
                if identities is not None:
                    identities.close()

    def test_real_timeout_owns_descendant_with_detached_stdout_and_stderr(self):
        result, receipt = self.tree(stdio="detached")
        self.assertEqual(result.returncode, 124)
        self.assertTrue(receipt["timed_out"])

    def test_real_timeout_drains_inherited_descendant_pipes_and_forces_tree(self):
        result, receipt = self.tree()
        self.assertEqual(result.returncode, 124)
        self.assertTrue(receipt["timed_out"])

    def test_real_sigint_owns_descendants_drains_and_reaps(self):
        result, receipt = self.tree(interrupt=True)
        self.assertEqual(result.returncode, 130)
        self.assertTrue(receipt["interrupted"])

    def test_real_sigint_rejects_empty_and_partial_publication_before_identity_ack(self):
        result, receipt = self.tree(interrupt=True, staged=True)
        self.assertEqual(result.returncode, 130)
        self.assertTrue(receipt["interrupted"])

    def test_old_exists_readiness_can_interrupt_an_actual_empty_publication(self):
        with tempfile.TemporaryDirectory(prefix="owned-old-publication-race-") as temp:
            directory = Path(temp)
            record = directory / "pids.json"
            helper = directory / "old-publisher.py"
            helper.write_text("import time\nfrom pathlib import Path\n"
                              f"with Path({str(record)!r}).open('w') as output:\n"
                              "    time.sleep(30)\n"
                              "    output.write('{\"parent\":1,\"child\":2}')\n")
            def old_tick(process):
                if record.exists():
                    signal.raise_signal(signal.SIGINT)
            start = time.monotonic()
            result, receipt = probe.run_owned([sys.executable, str(helper)], directory, dict(os.environ),
                                              probe.Deadline(time.time() + 8), tick=old_tick)
            elapsed = time.monotonic() - start
            self.assertEqual(result.returncode, 130)
            self.assertEqual(record.read_bytes(), b"")
            with self.assertRaises(json.JSONDecodeError):
                json.loads(record.read_text())
            self.assertTrue(receipt["interrupted"])
            self.assertTrue(receipt["terminal"]["direct_reaped"])
            self.assertEqual(receipt["terminal"]["active_processes"], 0)
            self.assertTrue(receipt["terminal"]["pipes_drained"])
            self.assertFalse(receipt["errors"])
            self.assertLess(elapsed, 3)
            retain_control("old-empty-publication-counterexample", {
                "actual_old_readiness_interrupt": True, "actual_record_bytes": 0,
                "actual_json_decode_error": True, "elapsed_seconds": elapsed, "receipt": receipt},
                raw=(result.stdout + result.stderr).encode(), source=helper.read_text())

    def test_real_parent_failure_keeps_exit7_even_with_descendant_pipe_timeout(self):
        result, receipt = self.tree(direct_exit=7)
        self.assertEqual(result.returncode, 7)
        self.assertEqual(receipt["observed_exit"], 7)
        self.assertTrue(receipt["timed_out"])

    def test_real_parent_success_cannot_leave_detached_descendant_running(self):
        result, receipt = self.tree(stdio="detached", direct_exit=0)
        self.assertEqual(result.returncode, 0)
        self.assertFalse(receipt["timed_out"])

    def test_expired_execution_allowance_starts_no_command(self):
        with tempfile.TemporaryDirectory(prefix="owned-no-launch-") as temp:
            marker = Path(temp) / "never-created"
            result, receipt = probe.run_owned([sys.executable, "-c", f"from pathlib import Path;Path({str(marker)!r}).touch()"],
                                              Path(temp), dict(os.environ), probe.Deadline(time.time() + 4.9))
            self.assertEqual(result.returncode, 124)
            self.assertTrue(receipt["timed_out"])
            self.assertFalse(marker.exists())

    def test_bounded_actual_nonzero_makes_following_stage_unreachable(self):
        with tempfile.TemporaryDirectory(prefix="owned-exit-stage-") as temp:
            following = False
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                probe.bounded([sys.executable, "-c", "import sys;sys.exit(19)"], Path(temp), dict(os.environ), probe.Deadline(time.time() + 10))
                following = True
            self.assertFalse(following)
            self.assertEqual(caught.exception.returncode, 19)
            self.assertEqual(caught.exception.owned_receipt["observed_exit"], 19)
            self.assertTrue(caught.exception.owned_receipt["terminal"]["direct_reaped"])


class FixtureIdentityTests(unittest.TestCase):
    def test_missing_empty_and_partial_records_do_not_authorize_ack(self):
        with tempfile.TemporaryDirectory(prefix="owned-publication-states-") as temp:
            record = Path(temp) / "pids.json"
            self.assertIsNone(ready_pids(record, 100))
            for content in ["", "{", '{"parent":100,']:
                record.write_text(content)
                self.assertIsNone(ready_pids(record, 100))
            record.write_text('{"parent":100,"child":200}')
            self.assertEqual(ready_pids(record, 100), {"parent": 100, "child": 200})
            for bad in [{"parent": 101, "child": 200}, {"parent": 100, "child": 100},
                        {"parent": 100, "child": True}, {"parent": 100, "child": 0},
                        {"parent": 100, "child": 0x100000000}, {"parent": 100}]:
                record.write_text(json.dumps(bad))
                with self.assertRaises(AssertionError):
                    ready_pids(record, 100)

    def api(self, child_member=True):
        class ModelApi:
            def __init__(self):
                self.open_by_pid = {100: 1001, 200: 2001}
                self.state_by_handle = {1001: 258, 2001: 258, 2002: 258}
                self.closed = []
            def OpenProcess(self, access, inheritable, pid):
                return self.open_by_pid[pid]
            def IsProcessInJob(self, handle, job, present):
                present._obj.value = handle == 1001 or child_member
                return 1
            def WaitForSingleObject(self, handle, milliseconds):
                return self.state_by_handle[handle]
            def CloseHandle(self, handle):
                self.closed.append(handle)
                return 1
        return ModelApi()

    def test_reused_pid_query_cannot_replace_retained_original_identity(self):
        api = self.api()
        identities = NativeTreeHandles(api, 55)
        identities.bind({"parent": 100, "child": 200})
        api.state_by_handle[1001] = api.state_by_handle[2001] = 0
        api.open_by_pid[200] = 2002  # a new unrelated process now has PID 200
        reopened = api.OpenProcess(0x00100000, False, 200)
        self.assertEqual(api.WaitForSingleObject(reopened, 0), 258)
        self.assertEqual(identities.states(), {"parent": "TERMINAL", "child": "TERMINAL"})
        identities.close()
        self.assertEqual(api.closed, [1001, 2001])

    def test_nonmember_child_is_rejected_and_original_handles_closed(self):
        api = self.api(child_member=False)
        identities = NativeTreeHandles(api, 55)
        with self.assertRaisesRegex(AssertionError, "not in its retained Job"):
            identities.bind({"parent": 100, "child": 200})
        self.assertEqual(api.closed, [1001, 2001])
        self.assertEqual(identities.handles, {})

    def test_original_handle_live_or_query_failure_remains_a_failed_terminal(self):
        api = self.api()
        identities = NativeTreeHandles(api, 55)
        identities.bind({"parent": 100, "child": 200})
        for native_status, expected in [(258, "LIVE"), (0xffffffff, "QUERY_FAILED")]:
            api.state_by_handle[2001] = native_status
            self.assertEqual(identities.states()["child"], expected)
            self.assertNotIn(expected, ("ABSENT", "TERMINAL"))
        identities.close()


class JobTerminalModelTests(unittest.TestCase):
    """Deterministic API counterexamples; these are never native Windows results."""

    def model(self, pids=(100, 200), pending=(258, 258, 0), fail=None):
        import ctypes
        class ApiModel:
            def __init__(self):
                self.error = 0
                self.closed = []
                self.calls = []
                self.pids = list(pids)
                self.total = 5 + len(pids)  # include prior already-terminated members
                self.wait_sequence = list(pending)
                self.last_wait = None
                self.terminated = False
                self.barrier = False
                self.limits = probe.JobExtendedLimits()
                self.limits.BasicLimitInformation.LimitFlags = 0x2000
                self.limits.BasicLimitInformation.PriorityClass = 0x20
                self.limits.ProcessMemoryLimit = 123456
            def QueryInformationJobObject(self, job, kind, info, size, returned):
                self.calls.append(("query", kind))
                if fail == f"query{kind}":
                    self.error = 5
                    return 0
                if kind == 9:
                    ctypes.memmove(info, ctypes.byref(self.limits), ctypes.sizeof(self.limits))
                elif kind == 3:
                    values = info._obj
                    if fail == "excessive_count":
                        values.assigned = probe.MAX_JOB_PROCESSES + 1
                        self.error = 234
                        return 0
                    values.assigned = len(self.pids)
                    values.listed = min(len(values.pids), len(self.pids))
                    for index, pid in enumerate(self.pids[:values.listed]):
                        values.pids[index] = pid
                    if fail == "bad_list_length":
                        values.listed = len(values.pids) + 1
                    if values.listed < values.assigned:
                        self.error = 234
                        return 0
                elif kind == 1:
                    info._obj.ActiveProcesses = 0 if self.terminated else len(self.pids)
                    info._obj.TotalProcesses = self.total
                return 1
            def SetInformationJobObject(self, job, kind, info, size):
                self.calls.append(("set", kind))
                if fail == "barrier":
                    return 0
                ctypes.memmove(ctypes.byref(self.limits), info, ctypes.sizeof(self.limits))
                self.barrier = bool(self.limits.BasicLimitInformation.LimitFlags & 8)
                if fail == "late_before_barrier":
                    self.pids.append(300)
                    self.total += 1
                if fail == "setter_removes_member":
                    self.pids.remove(200)
                return 1
            def OpenProcess(self, access, inherit, pid):
                self.calls.append(("open", pid))
                if pid == 200 and fail in ("gone", "access_denied"):
                    self.error = 87 if fail == "gone" else 5
                    return 0
                return pid + 1000
            def IsProcessInJob(self, handle, job, present):
                self.calls.append(("member", handle))
                present._obj.value = not (handle == 1200 and fail == "membership_changed")
                return 0 if fail == "membership_query" else 1
            def TerminateJobObject(self, job, code):
                self.calls.append(("terminate", job))
                self.terminated = True
                return 0 if fail == "terminate" else 1
            def WaitForSingleObject(self, handle, milliseconds):
                self.calls.append(("wait", handle, milliseconds))
                if handle == 1200:
                    self.last_wait = self.wait_sequence.pop(0) if self.wait_sequence else 0
                    return self.last_wait
                return 0
            def CloseHandle(self, handle):
                self.closed.append(handle)
                return 1
        api = ApiModel()
        owner = probe.WindowsJob.__new__(probe.WindowsJob)
        owner.api, owner.last_error, owner.job = api, lambda: api.error, 55
        owner.processes, owner.extra_handles, owner.terminal_handles = [], [], {}
        owner.verified_extra_handles = []
        owner.process = None
        owner.terminal_prepared = owner.admission_closed = False
        owner.terminal_pending = None
        owner.terminal_total = None
        return owner, api

    def root(self):
        class Root:
            returncode = 0
            stdin = stdout = stderr = None
            def poll(self):
                return 0
            def wait(self, timeout):
                if timeout <= 0:
                    raise subprocess.TimeoutExpired("model-root", timeout)
                return 0
        return Root()

    def test_old_accounting_only_cleanup_can_publish_false_terminal_with_original_handle_live(self):
        owner, api = self.model(pending=(258,))
        # Reproduce the original active() semantics, not a Windows emulator.
        def accounting_only(deadline):
            import ctypes
            values = probe.JobAccounting()
            api.QueryInformationJobObject(owner.job, 1, ctypes.byref(values), ctypes.sizeof(values), None)
            return values.ActiveProcesses
        owner.active = accounting_only
        terminal = probe.cleanup(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertEqual(terminal["active_processes"], 0)
        self.assertEqual(api.WaitForSingleObject(owner.terminal_handles[200], 0), 258)
        retain_control("accounting-zero-live-object-model-counterexample", {
            "evidence_kind": "API_MODEL_NOT_NATIVE_WINDOWS", "old_accounting_terminal": terminal,
            "original_child_handle_status": "LIVE"})
        owner.close()

    def test_cleanup_waits_for_all_original_objects_after_accounting_zero(self):
        owner, api = self.model()
        terminal = probe.cleanup(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertEqual(api.last_wait, 0)
        self.assertEqual(terminal["active_processes"], 0)
        self.assertEqual(terminal["native_process_objects"], {
            "admission_closed": True, "identities_bound": True,
            "retained_processes": 2, "pending_process_objects": 0,
            "original_handles_checked": 2, "known_direct_handles": 0,
            "verified_extra_handles": 0})
        self.assertTrue(all(call[-1] == 0 for call in api.calls if call[0] == "wait"))
        self.assertLess(api.calls.index(("open", 200)), api.calls.index(("set", 9)))
        self.assertLess(api.calls.index(("member", 1200)), api.calls.index(("terminate", 55)))
        retain_control("accounting-zero-original-object-terminal-model", {
            "evidence_kind": "API_MODEL_NOT_NATIVE_WINDOWS", "terminal": terminal, "api_calls": api.calls})
        owner.close()
        self.assertEqual(api.closed, [55, 1100, 1200])

    def test_all_members_including_nested_list_are_bound_and_other_limits_preserved(self):
        owner, api = self.model(pids=tuple(range(100, 132)), pending=(0,))
        owner.prepare_terminal(probe.Deadline(time.time() + 1))
        self.assertEqual(set(owner.terminal_handles), set(range(100, 132)))
        self.assertEqual(sum(call == ("query", 3) for call in api.calls), 4)
        self.assertTrue(api.barrier)
        self.assertEqual(api.limits.BasicLimitInformation.ActiveProcessLimit, 1)
        self.assertEqual(api.limits.BasicLimitInformation.PriorityClass, 0x20)
        self.assertEqual(api.limits.ProcessMemoryLimit, 123456)
        owner.close()
        self.assertEqual(len(api.closed), 33)

    def test_admission_during_or_after_binding_fails_closed_even_when_second_snapshot_captures_it(self):
        owner, api = self.model(fail="late_before_barrier")
        terminal, errors = probe.finish_owned(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertIsNone(terminal)
        self.assertIn("owned_cleanup_failed", errors)
        self.assertTrue(api.terminated)
        self.assertIn(1300, api.closed)
        owner, api = self.model(pending=(0,))
        owner.prepare_terminal(probe.Deadline(time.time() + 1))
        api.pids.append(300)
        api.total += 1
        terminal, errors = probe.finish_owned(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertIsNone(terminal)
        self.assertIn("owned_cleanup_failed", errors)
        self.assertTrue(api.terminated)
        self.assertEqual(api.closed, [55, 1100, 1200])

    def test_members_removed_by_limit_setting_keep_original_handles_until_signalled(self):
        owner, api = self.model(fail="setter_removes_member")
        terminal = probe.cleanup(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertNotIn(200, api.pids)
        self.assertEqual(api.last_wait, 0)
        self.assertEqual(terminal["native_process_objects"]["retained_processes"], 2)
        self.assertEqual(terminal["active_processes"], 0)
        owner.close()
        self.assertEqual(api.closed, [55, 1100, 1200])

    def test_every_listed_member_open_failure_including_87_fails_closed(self):
        for failure, native_error in (("gone", 87), ("access_denied", 5)):
            with self.subTest(failure=failure):
                owner, api = self.model(fail=failure)
                terminal, errors = probe.finish_owned(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
                self.assertIsNone(terminal)
                self.assertIn("owned_cleanup_failed", errors)
                self.assertFalse(owner.terminal_prepared)
                self.assertTrue(api.terminated)
                self.assertEqual(api.error, native_error)
                self.assertEqual(api.closed, [55, 1100])
                self.assertNotIn(("set", 9), api.calls)
                self.assertEqual(sum(call == ("open", 200) for call in api.calls), 1)
                # The old model's externally-held object may still be LIVE.
                # ERROR87 must not turn that unknown identity into a pass.
                self.assertEqual(api.WaitForSingleObject(1200, 0), 258)
                retain_control(f"listed-open-{native_error}-fail-closed-model", {
                    "evidence_kind": "API_MODEL_NOT_NATIVE_WINDOWS",
                    "real_Windows_sequence_reachable": "UNKNOWN", "terminal": terminal,
                    "errors": sorted(errors), "unbound_original_observer_status": 258,
                    "closed_handles": api.closed, "api_calls": api.calls})

    def test_known_direct_and_verified_debuggee_handles_are_checked_when_current_list_is_empty(self):
        owner, api = self.model(pids=(), pending=(258, 258, 0))
        direct = probe.WindowsProcess(api, 100, 1100, None, None, None, None)
        owner.processes = [direct]
        owner.extra_handles = [1200]
        owner.verified_extra_handles = [1200]
        terminal = probe.cleanup(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertEqual(terminal["active_processes"], 0)
        self.assertEqual(api.last_wait, 0)
        evidence = terminal["native_process_objects"]
        self.assertEqual(evidence["retained_processes"], 0)
        self.assertEqual(evidence["original_handles_checked"], 2)
        self.assertEqual(evidence["known_direct_handles"], 1)
        self.assertEqual(evidence["verified_extra_handles"], 1)
        self.assertTrue(any(call[0] == "wait" and call[1] == 1100 for call in api.calls))
        self.assertEqual(sum(call[0] == "wait" and call[1] == 1200 for call in api.calls), 3)
        retain_control("held-originals-absent-from-current-list-model", {
            "evidence_kind": "API_MODEL_NOT_NATIVE_WINDOWS", "real_Windows_sequence_reachable": "UNKNOWN",
            "terminal": terminal, "api_calls": api.calls})
        owner.close()
        self.assertEqual(api.closed, [55, 1100, 1200])

    def test_unverified_extra_is_closure_only_and_original_handles_use_handle_identity(self):
        owner, api = self.model(pids=(), pending=(258,))
        direct = probe.WindowsProcess(api, 200, 1100, None, None, None, None)
        owner.processes = [direct]
        owner.extra_handles = [1200, 1300]
        owner.verified_extra_handles = [1300]
        owner.terminal_handles = {200: 1400}  # a separately held identity, even with the same PID label
        self.assertEqual(owner.original_wait_handles(), [1400, 1100, 1300])
        owner.wait_retained(probe.Deadline(time.time() + 1))
        self.assertFalse(any(call[0] == "wait" and call[1] == 1200 for call in api.calls))
        owner.close()
        self.assertEqual(api.closed, [55, 1100, 1200, 1300, 1400])

    def test_debuggee_is_registered_for_terminal_wait_only_after_job_verification(self):
        for failure in (None, "membership_query"):
            with self.subTest(failure=failure):
                owner, api = self.model(pids=(), pending=(258,), fail=failure)
                debug_owner = probe.WindowsOwner.__new__(probe.WindowsOwner)
                debug_owner.__dict__.update(owner.__dict__)
                debug_owner.target = None
                if failure is None:
                    debug_owner.open_target(200)
                    self.assertEqual(debug_owner.original_wait_handles(), [1200])
                    self.assertEqual(debug_owner.verified_extra_handles, [1200])
                    self.assertIsNone(debug_owner.exit_code())
                    # Top-level success already required a signalled target;
                    # this does not excuse an incomplete terminal structure.
                    self.assertEqual(probe.result_code(None, True), 125)
                else:
                    with self.assertRaises(probe.OwnershipError):
                        debug_owner.open_target(200)
                    self.assertEqual(debug_owner.original_wait_handles(), [])
                    self.assertEqual(debug_owner.verified_extra_handles, [])
                    self.assertFalse(any(call[0] == "wait" and call[1] == 1200 for call in api.calls))
                self.assertEqual(debug_owner.extra_handles, [1200])
                debug_owner.close()
                self.assertEqual(api.closed, [55, 1200])

    def test_reused_nonmember_or_membership_error_fails_closed_and_closes_handles(self):
        for failure in ("membership_changed", "membership_query"):
            with self.subTest(failure=failure):
                owner, api = self.model(fail=failure)
                with self.assertRaisesRegex(probe.OwnershipError, "membership changed"):
                    owner.prepare_terminal(probe.Deadline(time.time() + 1))
                self.assertFalse(owner.terminal_prepared)
                owner.force()
                owner.wait_retained(probe.Deadline(time.time() + 1))
                unverified = 1200 if failure == "membership_changed" else 1100
                self.assertFalse(any(call[0] == "wait" and call[1] == unverified for call in api.calls))
                owner.close()
                self.assertTrue(api.terminated)
                self.assertEqual(api.closed[0], 55)
                self.assertTrue(all(handle in api.closed for handle in (1100, 1200) if ("open", handle - 1000) in api.calls))

    def test_barrier_list_length_and_query_failures_never_create_a_success_terminal(self):
        for failure in ("query1", "query9", "barrier", "query3", "bad_list_length", "excessive_count"):
            with self.subTest(failure=failure):
                owner, api = self.model(fail=failure)
                terminal, errors = probe.finish_owned(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
                self.assertIsNone(terminal)
                self.assertIn("owned_cleanup_failed", errors)
                self.assertTrue(api.terminated)
                self.assertEqual(api.closed, [55] + [call[1] + 1000 for call in api.calls if call[0] == "open"])

    def test_expired_original_budget_and_original_wait_failure_cannot_publish_terminal(self):
        owner, api = self.model(pending=(258,) * 100)
        start = time.monotonic()
        terminal, errors = probe.finish_owned(owner, self.root(), [], probe.Deadline(time.time() + 0.04), {})
        self.assertIsNone(terminal)
        self.assertIn("owned_cleanup_failed", errors)
        self.assertLess(time.monotonic() - start, 0.2)
        self.assertEqual(api.closed, [55, 1100, 1200])
        owner, api = self.model(pending=(0xffffffff,))
        terminal, errors = probe.finish_owned(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertIsNone(terminal)
        self.assertIn("owned_cleanup_failed", errors)
        self.assertEqual(api.closed, [55, 1100, 1200])

    def test_total_change_during_final_original_wait_prevents_success_terminal(self):
        owner, api = self.model(pending=(0,))
        original_wait = api.WaitForSingleObject
        original_query = api.QueryInformationJobObject
        child_waits = 0
        def wait_changes_total(handle, milliseconds):
            nonlocal child_waits
            status = original_wait(handle, milliseconds)
            if handle == 1200 and milliseconds == 0:
                child_waits += 1
                if child_waits == 2:
                    before = api.total
                    api.total += 1
                    api.calls.append(("total_change_in_final_wait", before, api.total))
            return status
        def accounting_trace(job, kind, info, size, returned):
            status = original_query(job, kind, info, size, returned)
            if kind == 1:
                api.calls.append(("accounting_return", info._obj.TotalProcesses, info._obj.ActiveProcesses))
            return status
        api.WaitForSingleObject = wait_changes_total
        api.QueryInformationJobObject = accounting_trace
        terminal, errors = probe.finish_owned(owner, self.root(), [], probe.Deadline(time.time() + 1), {})
        self.assertIsNone(terminal)
        self.assertEqual(errors, {"owned_cleanup_failed"})
        self.assertEqual(probe.result_code(0, not errors), 125)
        self.assertEqual(owner.terminal_total, 7)
        self.assertEqual(api.total, 8)
        self.assertIn(("accounting_return", 8, 0), api.calls)
        self.assertEqual(api.closed, [55, 1100, 1200])
        retain_control("post-wait-total-drift-fail-closed-model", {
            "evidence_kind": "API_MODEL_NOT_NATIVE_WINDOWS", "real_Windows_sequence_reachable": "UNKNOWN",
            "historical_native_cause": "UNPROVEN", "terminal": terminal, "errors": sorted(errors),
            "controller_if_original_zero": 125, "api_calls": api.calls, "closed_handles": api.closed})

    def test_active_changes_during_waits_preserve_both_accounting_samples(self):
        for before, after in ((0, 1), (1, 0)):
            with self.subTest(before=before, after=after):
                owner, api = self.model(pending=(0,))
                deadline = probe.Deadline(time.time() + 1)
                owner.prepare_terminal(deadline)
                api.terminated = True
                active = before
                original_wait = api.WaitForSingleObject
                original_query = api.QueryInformationJobObject
                def wait_changes_active(handle, milliseconds):
                    nonlocal active
                    status = original_wait(handle, milliseconds)
                    if handle == 1200:
                        active = after
                        api.calls.append(("active_change_in_original_wait", before, after))
                    return status
                def accounting_active(job, kind, info, size, returned):
                    status = original_query(job, kind, info, size, returned)
                    if kind == 1:
                        info._obj.ActiveProcesses = active
                        api.calls.append(("accounting_return", info._obj.TotalProcesses, active))
                    return status
                api.WaitForSingleObject = wait_changes_active
                api.QueryInformationJobObject = accounting_active
                self.assertEqual(owner.active(deadline), 1)
                samples = [call for call in api.calls if call[0] == "accounting_return"]
                self.assertEqual(samples, [("accounting_return", 7, before), ("accounting_return", 7, after)])
                owner.close()
                self.assertEqual(api.closed, [55, 1100, 1200])
                retain_control(f"post-wait-active-{before}-to-{after}-conservative-model", {
                    "evidence_kind": "API_MODEL_NOT_NATIVE_WINDOWS", "real_Windows_sequence_reachable": "UNKNOWN",
                    "active_result": 1, "samples": samples, "api_calls": api.calls, "closed_handles": api.closed})

    def test_trailing_accounting_query_failure_or_expiry_preserves_failed_terminal(self):
        for failure in ("query", "deadline"):
            with self.subTest(failure=failure):
                owner, api = self.model(pending=(0,))
                deadline = probe.Deadline(time.time() + 1)
                child_waits = 0
                armed = False
                original_wait = api.WaitForSingleObject
                original_query = api.QueryInformationJobObject
                def wait_arms_failure(handle, milliseconds):
                    nonlocal child_waits, armed
                    status = original_wait(handle, milliseconds)
                    if handle == 1200 and milliseconds == 0:
                        child_waits += 1
                        if child_waits == 2:
                            armed = True
                            if failure == "deadline":
                                deadline.end = time.monotonic() - 0.001
                            api.calls.append(("failure_armed_in_final_wait", failure))
                    return status
                def query_may_fail(job, kind, info, size, returned):
                    if kind == 1 and armed and failure == "query":
                        api.error = 5
                        api.calls.append(("trailing_accounting_query_failed", 5))
                        return 0
                    return original_query(job, kind, info, size, returned)
                api.WaitForSingleObject = wait_arms_failure
                api.QueryInformationJobObject = query_may_fail
                terminal, errors = probe.finish_owned(owner, self.root(), [], deadline, {})
                self.assertIsNone(terminal)
                self.assertIn("owned_cleanup_failed", errors)
                self.assertEqual(probe.result_code(0, not errors), 125)
                self.assertTrue(armed)
                self.assertTrue(api.terminated)
                self.assertEqual(api.closed, [55, 1100, 1200])
                if failure == "query":
                    self.assertIn(("trailing_accounting_query_failed", 5), api.calls)
                    self.assertEqual(errors, {"owned_cleanup_failed"})
                else:
                    self.assertIn("owned_fallback_reap_failed", errors)
                retain_control(f"post-wait-{failure}-fail-closed-model", {
                    "evidence_kind": "API_MODEL_NOT_NATIVE_WINDOWS", "real_Windows_sequence_reachable": "UNKNOWN",
                    "terminal": terminal, "errors": sorted(errors), "controller_if_original_zero": 125,
                    "api_calls": api.calls, "closed_handles": api.closed})

    def test_native_structures_keep_dword_width_and_actual_accounting_offset(self):
        import ctypes
        self.assertEqual(probe.JobAccounting.ActiveProcesses.offset, 40)
        self.assertEqual(ctypes.sizeof(probe.JobAccounting), 48)
        self.assertEqual(probe.JobBasicLimits.ActiveProcessLimit.offset, 40 if ctypes.sizeof(ctypes.c_size_t) == 8 else 28)
        if ctypes.sizeof(ctypes.c_size_t) == 8:
            self.assertEqual(ctypes.sizeof(probe.JobExtendedLimits), 144)


class NativeJobBarrierTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "nt", "new native Windows barrier control is UNRUN on this host")
    def test_actual_two_existing_nested_members_survive_barrier_and_late_createprocess_is_rejected(self):
        with tempfile.TemporaryDirectory(prefix="owned-native-Job-barrier-") as temp:
            directory = Path(temp)
            helper = directory / "parent.py"
            record, ack, late_record = (directory / name for name in ("pids.json", "ack", "late.json"))
            marker = directory / "late-process-must-not-execute"
            child_source = "import time;time.sleep(30)"
            late_source = f"from pathlib import Path;Path({str(marker)!r}).touch()"
            helper.write_text("import ctypes,json,os,subprocess,sys,time\n"
                              "from ctypes import wintypes\nfrom pathlib import Path\n"
                              "api=ctypes.WinDLL('kernel32',use_last_error=True)\n"
                              "api.CreateJobObjectW.argtypes=[ctypes.c_void_p,wintypes.LPCWSTR]\n"
                              "api.CreateJobObjectW.restype=wintypes.HANDLE\n"
                              "api.GetCurrentProcess.argtypes=[]\napi.GetCurrentProcess.restype=wintypes.HANDLE\n"
                              "api.AssignProcessToJobObject.argtypes=[wintypes.HANDLE,wintypes.HANDLE]\n"
                              "api.AssignProcessToJobObject.restype=wintypes.BOOL\n"
                              "api.IsProcessInJob.argtypes=[wintypes.HANDLE,wintypes.HANDLE,ctypes.POINTER(wintypes.BOOL)]\n"
                              "api.IsProcessInJob.restype=wintypes.BOOL\n"
                              "nested=api.CreateJobObjectW(None,None)\n"
                              "assert nested and api.AssignProcessToJobObject(nested,api.GetCurrentProcess())\n"
                              f"child=subprocess.Popen([sys.executable,'-c',{child_source!r}],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)\n"
                              "present=wintypes.BOOL()\n"
                              "assert api.IsProcessInJob(int(child._handle),nested,ctypes.byref(present)) and present.value\n"
                              f"pending=Path({str(record)!r}+'.pending')\n"
                              "pending.write_text(json.dumps({'parent':os.getpid(),'child':child.pid}))\n"
                              f"os.replace(pending,{str(record)!r})\n"
                              f"while not Path({str(ack)!r}).exists():time.sleep(0.001)\n"
                              "late={'nested_child_membership':True,'creation_rejected':False}\n"
                              "try:\n"
                              f"    attempt=subprocess.Popen([sys.executable,'-c',{late_source!r}],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)\n"
                              "except OSError as error:\n"
                              "    late['creation_rejected']=True;late['winerror']=error.winerror\n"
                              "else:\n"
                              "    late['unexpected_child_pid']=attempt.pid\n"
                              f"pending=Path({str(late_record)!r}+'.pending')\n"
                              "pending.write_text(json.dumps(late))\n"
                              f"os.replace(pending,{str(late_record)!r})\n")
            owner = probe.WindowsJob()
            identities = NativeTreeHandles(owner.api, owner.job)
            deadline = probe.Deadline(time.time() + 8)
            bound = None
            barrier = None
            def tick(process):
                nonlocal bound, barrier
                if bound is not None:
                    return
                pids = ready_pids(record, process.pid)
                if pids is None:
                    return
                identities.bind(pids)
                before = identities.states()
                owner.prepare_terminal(deadline)
                after = identities.states()
                # Observe setter effects before ACK allows any original exit or
                # attempted late child creation. Nested descendants must appear.
                self.assertEqual(before, {"parent": "LIVE", "child": "LIVE"})
                self.assertEqual(after, before)
                self.assertTrue(set(pids.values()) <= set(owner.terminal_handles))
                barrier = {"before_original_handle_states": before, "after_original_handle_states": after,
                           "nested_member_ids_captured": sorted(owner.terminal_handles),
                           "total_processes_at_binding": owner.terminal_total}
                bound = pids
                ack.touch()
            try:
                result, receipt = probe.run_owned([sys.executable, str(helper)], directory, dict(os.environ),
                                                  deadline, owner=owner, tick=tick)
                self.assertIsNotNone(bound)
                late = json.loads(late_record.read_text())
                original_states = identities.states()
                evidence = {"evidence_kind": "ACTUAL_NATIVE_WINDOWS", "barrier": barrier,
                            "late_createprocess": late, "late_marker_exists": marker.exists(),
                            "post_original_handle_states": original_states, "receipt": receipt}
                retain_control("native-nested-members-admission-barrier", evidence, source=helper.read_text())
                print(json.dumps({"actual_native_job_barrier": evidence}, sort_keys=True))
                self.assertTrue(late["nested_child_membership"])
                self.assertTrue(late["creation_rejected"])
                self.assertFalse(marker.exists())
                self.assertEqual(original_states, {"parent": "TERMINAL", "child": "TERMINAL"})
                # A rejected association may itself increment cumulative Job
                # accounting. Changed accounting must fail closed; unchanged
                # accounting may pass only with the full original-object proof.
                if "owned_cleanup_failed" in receipt["errors"]:
                    self.assertEqual(result.returncode, 125)
                    self.assertIsNone(receipt["terminal"])
                    self.assertNotIn("owned_fallback_reap_failed", receipt["errors"])
                    self.assertNotIn("owned_fallback_pipe_terminal_failed", receipt["errors"])
                else:
                    self.assertEqual(result.returncode, 0)
                    self.assertEqual(receipt["terminal"]["native_process_objects"]["pending_process_objects"], 0)
                    self.assertTrue(receipt["terminal"]["direct_reaped"])
                    self.assertTrue(receipt["terminal"]["pipes_drained"])
            finally:
                identities.close()


class ProcessTests(unittest.TestCase):
    def run_owned(self, output, target_exit=0, cdb_exit=0, fault=None, hang=False, seconds=10):
        with tempfile.TemporaryDirectory(prefix="owned-crash-controller-") as temp:
            child = Path(temp) / "debugger.py"
            child.write_text("import sys,time\n"
                             f"print('KEEL_{NONCE}_PID:1234',flush=True)\n"
                             "command=sys.stdin.readline()\n"
                             "assert command == 'g\\n'\n"
                             f"sys.stdout.write({output!r});sys.stdout.flush()\n"
                             + ("time.sleep(30)\n" if hang else "")
                             + f"sys.exit({cdb_exit})\n")
            owner = FakeOwner(target_exit, fault)
            evidence = probe.TextEvidence(NONCE, {"tests::case"})
            deadline = probe.Deadline(time.time() + seconds)
            receipt = probe.run_debugger([sys.executable, str(child)], Path(temp), dict(os.environ), deadline, evidence, owner)
            self.assertTrue(owner.closed)
            self.assertTrue(owner.terminated)
            self.assertIsNotNone(owner.process.returncode)
            self.assertTrue(receipt["owned_cleanup_complete"])
            return receipt, evidence

    def test_real_subprocess_handshake_and_success_summary(self):
        receipt, _ = self.run_owned("running 706 tests\ntest tests::case ... ok\n" + SUMMARY + "\n")
        self.assertEqual(receipt["controller_exit"], 0)
        self.assertEqual(receipt["diagnostic_errors"], [])
        self.assertTrue(receipt["test_started"])

    def test_real_subprocess_crash_dword_with_zero_debugger_exit(self):
        receipt, evidence = self.run_owned(NATIVE, target_exit=0xC0000409)
        self.assertEqual(receipt["debugger_exit"], 0)
        self.assertEqual(receipt["original_test_exit_hex"], "0xc0000409")
        self.assertEqual(receipt["controller_exit"], 0xC0000409)
        self.assertTrue(evidence.exception_complete())

    def test_missing_native_diagnostic_keeps_real_failure(self):
        receipt, _ = self.run_owned("", target_exit=0xC0000409)
        self.assertEqual(receipt["controller_exit"], 0xC0000409)
        self.assertIn("native_exception_context_incomplete", receipt["diagnostic_errors"])

    def test_mismatched_exception_record_cannot_relabel_original_exit(self):
        receipt, _ = self.run_owned(NATIVE.replace("ExceptionCode: c0000409", "ExceptionCode: c0000005"), target_exit=0xC0000409)
        self.assertEqual(receipt["controller_exit"], 0xC0000409)
        self.assertIn("native_exception_code_does_not_match_exit", receipt["diagnostic_errors"])

    def test_failed_debugger_cannot_turn_a_pass_into_success(self):
        receipt, _ = self.run_owned(SUMMARY + "\n", cdb_exit=17)
        self.assertEqual(receipt["controller_exit"], 125)
        self.assertIn("debugger_nonzero_exit", receipt["diagnostic_errors"])

    def test_debugger_echoing_failed_exit_is_separate_observed_evidence(self):
        receipt, _ = self.run_owned("", target_exit=7, cdb_exit=7)
        self.assertEqual(receipt["original_test_exit"], 7)
        self.assertEqual(receipt["debugger_exit"], 7)
        self.assertEqual(receipt["controller_exit"], 7)
        self.assertNotIn("debugger_nonzero_exit", receipt["diagnostic_errors"])

    def test_partial_success_or_filtered_summary_cannot_pass_full_suite(self):
        for summary in [SUMMARY.replace("704 passed", "400 passed"), SUMMARY.replace("0 filtered out", "1 filtered out")]:
            receipt, _ = self.run_owned(summary + "\n")
            self.assertEqual(receipt["controller_exit"], 125)
            self.assertIn("completed_suite_summary_missing", receipt["diagnostic_errors"])

    def test_job_assignment_failure_directly_reaps_unassigned_debugger(self):
        receipt, _ = self.run_owned("", fault="assign")
        self.assertFalse(receipt["test_started"])
        self.assertEqual(receipt["controller_exit"], 125)

    def test_open_failure_never_sends_g_and_reaps_debugger(self):
        receipt, _ = self.run_owned("", fault="open")
        self.assertEqual(receipt["controller_exit"], 125)
        self.assertFalse(receipt["test_started"])
        self.assertIsNone(receipt["original_test_exit"])

    def test_query_failure_still_closes_owned_process(self):
        receipt, _ = self.run_owned("", fault="query")
        self.assertEqual(receipt["controller_exit"], 125)
        self.assertIn("original_exit_query_failed", receipt["diagnostic_errors"])

    def test_real_concurrent_descendant_sentinel_invalidates_projection_and_keeps_failure(self):
        with tempfile.TemporaryDirectory(prefix="owned-concurrent-projection-") as temp:
            directory = Path(temp)
            ready = directory / "sentinel-ready"
            child = directory / "debugger-with-descendant.py"
            raw = directory / "private-shared.bin"
            sentinel = "Authorization: Bearer OWNED_CONCURRENT_SENTINEL"
            begin, rest = NATIVE.split("Last event:", 1)
            emitter = ("import sys,time\nfrom pathlib import Path\n"
                       f"print({sentinel!r},flush=True)\nPath({str(ready)!r}).touch()\n"
                       + f"for _ in range(3): print({sentinel!r},flush=True);time.sleep(.005)\n")
            child.write_text("import subprocess,sys,threading,time\nfrom pathlib import Path\n"
                             f"print('KEEL_{NONCE}_PID:1234',flush=True)\n"
                             "assert sys.stdin.readline() == 'g\\n'\n"
                             f"sys.stdout.write({begin!r});sys.stdout.flush()\n"
                             f"descendant=subprocess.Popen([sys.executable,'-c',{emitter!r}],stdin=subprocess.DEVNULL)\n"
                             "limit=time.monotonic()+1\n"
                             f"while not Path({str(ready)!r}).exists() and time.monotonic()<limit: time.sleep(.002)\n"
                             f"lines={'Last event:' + rest!r}.splitlines()\n"
                             "def response():\n"
                             "    for line in lines[:-1]: print(line,flush=True);time.sleep(.002)\n"
                             "worker=threading.Thread(target=response);worker.start();worker.join(timeout=1)\n"
                             "descendant.wait(timeout=1)\n"
                             "print(lines[-1],flush=True)\n")
            owner = FakeOwner(target_exit=0xC0000409)
            evidence = probe.TextEvidence(NONCE, set(), raw_path=raw)
            receipt = probe.run_debugger([sys.executable, str(child)], directory, dict(os.environ),
                                         probe.Deadline(time.time() + 10), evidence, owner, require_suite_summary=False)
            retain_control("concurrent-projection", receipt, raw=raw.read_bytes(), source=child.read_text())
            self.assertEqual(receipt["controller_exit"], 0xC0000409)
            self.assertTrue(receipt["owned_cleanup_complete"])
            self.assertIn("unexpected_native_text", receipt["diagnostic_errors"])
            self.assertEqual(evidence.upload_events(), [])
            self.assertIn(sentinel.encode(), raw.read_bytes())
            self.assertNotIn("OWNED_CONCURRENT_SENTINEL", json.dumps(receipt) + json.dumps(evidence.upload_events()))
            print(json.dumps({"actual_concurrent_projection": {"descendant_output_observed_in_private_raw": True,
                             "projection_rejected": True, "original_failure_preserved": receipt["controller_exit"],
                             "owned_cleanup_complete": receipt["owned_cleanup_complete"]}}))

    def test_real_hanging_child_is_killed_within_absolute_budget(self):
        start = time.monotonic()
        receipt, _ = self.run_owned("", hang=True, seconds=5.15)
        self.assertLess(time.monotonic() - start, 2)
        self.assertEqual(receipt["controller_exit"], 124)
        self.assertTrue(receipt["timed_out"])
        self.assertIsNone(receipt["original_test_exit"])


if __name__ == "__main__":
    unittest.main()
