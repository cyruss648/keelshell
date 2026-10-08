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
