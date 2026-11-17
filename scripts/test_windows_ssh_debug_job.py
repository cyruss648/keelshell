"""Fault-injection controls, explicitly NOT native Win32/OpenSSH acceptance.

The scripted boundary supplies observations and failures to the actual owner.
It does not implement another admission algorithm. A separately skipped native
test must exercise the real Win32 calls and inherited debug chain on Windows.
Run with -B; temporary evidence is isolated and removed by its control.
"""

import ctypes as C
from dataclasses import dataclass
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
import unittest
from unittest import mock

import windows_native_crash as lifecycle
import windows_monitor_lab as lab
from windows_monitor import debug_job as debug


class Budget:
    def __init__(self, seconds=10):
        self.end = time.monotonic() + seconds

    def remaining(self, reserve=0):
        left = self.end - time.monotonic() - reserve
        if left <= 0:
            raise TimeoutError("offline original budget expired")
        return left


@dataclass
class Object:
    pid: int
    handle: int
    thread: int
    file: int
    creation: int
    line: str = 'powershell.exe -NoProfile -NonInteractive -EncodedCommand QQ=='
    image: str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
    member: bool = True
    signaled: bool = False
    exited: bool = False
    exit_queued: bool = False
    signal_on_continue: bool = True
    code: int = 0


class ScriptedBoundary:
    """Win32-shaped call observations, never a Windows runtime emulator."""

    def __init__(self):
        self.calls = []
        self.objects = []
        self.handles = {}
        self.events = []
        self.borrowed = set()
        self.files = set()
        self.closed = set()
        self.current = None
        self.error = 0
        self.next_handle = 1000
        self.fail = set()
        self.close_failures = {}
        self.continue_failures = 0
        self.total_override = None
        self.active_override = None
        self.pids_override = None
        self.limits = lifecycle.JobExtendedLimits()
        self.limits.BasicLimitInformation.LimitFlags = 0x2000
        self.limits.ProcessMemoryLimit = 987654
        self.after_barrier = None
        self.stream_forever = False

    def object(self, pid=41, *, creation=None, member=True):
        self.next_handle += 10
        base = self.next_handle
        obj = Object(pid, base, base + 1, base + 2,
                     creation if creation is not None else 133000000000000000 + base,
                     member=member)
        self.objects.append(obj)
        self.handles[obj.handle] = obj
        self.borrowed.update((obj.handle, obj.thread))
        self.files.add(obj.file)
        event = self.event(debug.CREATE_PROCESS, obj)
        event.u.create_process.hProcess = obj.handle
        event.u.create_process.hThread = obj.thread
        event.u.create_process.hFile = obj.file
        self.events.append((event, obj))
        return obj

    def event(self, code, obj):
        event = debug.DebugEvent()
        event.code, event.pid, event.tid = code, obj.pid, obj.pid + 300
        return event

    def send(self, code, obj):
        event = self.event(code, obj)
        self.events.append((event, obj))
        return event

    def exit(self, obj, code=0, *, signal=True):
        obj.code, obj.signal_on_continue, obj.exit_queued = code, signal, True
        self.send(debug.EXIT_PROCESS, obj).u.exit_process.code = code

    def exception(self, obj, code, first=1):
        event = self.send(debug.EXCEPTION, obj)
        event.u.exception.record.code = code
        event.u.exception.first_chance = first

    def WaitForDebugEvent(self, pointer, milliseconds):
        self.calls.append(("wait_event", milliseconds))
        if "wait_event" in self.fail:
            self.error = 5
            return 0
        if not self.events:
            if not self.stream_forever:
                self.error = debug.ERROR_SEM_TIMEOUT
                return 0
            self.events.append((self.event(debug.DEBUG_STRING, self.objects[0]), self.objects[0]))
        event, obj = self.events.pop(0)
        C.memmove(pointer, C.byref(event), C.sizeof(event))
        self.current = event, obj
        return 1

    def ContinueDebugEvent(self, pid, tid, status):
        self.calls.append(("continue", pid, status))
        if self.continue_failures:
            self.continue_failures -= 1
            return 0
        event, obj = self.current
        if event.code == debug.EXIT_PROCESS:
            obj.exited = True
            obj.signaled = obj.signal_on_continue
        return 1

    def GetCurrentProcess(self):
        return -1

    def DuplicateHandle(self, source, handle, target, output, access, inherit, options):
        self.calls.append(("duplicate", handle, access, inherit, options))
        if "duplicate" in self.fail:
            return 0
        self.next_handle += 10
        output._obj.value = self.next_handle
        self.handles[self.next_handle] = self.handles[handle]
        return 1

    def GetProcessId(self, handle):
        self.calls.append(("pid", handle))
        return self.handles[handle].pid + int("pid" in self.fail)

    def IsProcessInJob(self, handle, job, output):
        self.calls.append(("member", handle))
        output._obj.value = self.handles[handle].member
        return int("member" not in self.fail)

    def AssignProcessToJobObject(self, job, handle):
        self.calls.append(("assign", handle))
        if "assign" in self.fail:
            return 0
        if "assignment_unconfirmed" not in self.fail:
            self.handles[handle].member = True
        return 1

    def QueryFullProcessImageNameW(self, handle, flags, output, length):
        self.calls.append(("image", handle))
        if "image" in self.fail:
            return 0
        raw = self.handles[handle].image.encode("utf-16-le")
        C.memmove(output, raw, len(raw))
        length._obj.value = len(raw) // 2
        return 1

    def GetProcessTimes(self, handle, creation, exit_time, kernel, user):
        self.calls.append(("creation", handle))
        if "creation" in self.fail:
            return 0
        value = self.handles[handle].creation
        creation._obj.low, creation._obj.high = value & 0xffffffff, value >> 32
        return 1

    def NtQueryInformationProcess(self, handle, kind, output, size, returned):
        self.calls.append(("command", handle, kind, size))
        header = C.sizeof(debug.UnicodeString)
        raw = self.handles[handle].line.encode("utf-16-le")
        needed = header + len(raw) + 2
        if output is None:
            returned._obj.value = needed
            if "command_size" in self.fail:
                returned._obj.value = header + debug.MAX_COMMAND_BYTES + 1
            return 0xC0000004 if "command_unavailable" not in self.fail else 0xC0000003
        if "command_read" in self.fail:
            return 0xC0000004
        C.memmove(C.addressof(output) + header, raw + b"\0\0", len(raw) + 2)
        value = debug.UnicodeString.from_buffer(output)
        value.Length, value.MaximumLength = len(raw), len(raw) + 2
        value.Buffer = C.addressof(output) + header
        returned._obj.value = needed
        if "command_pointer" in self.fail:
            value.Buffer = 1
        if "command_odd" in self.fail:
            value.Length -= 1
        if "command_returned" in self.fail:
            returned._obj.value = size + 1
        return 0

    def QueryInformationJobObject(self, job, kind, output, size, returned):
        self.calls.append(("job_query", kind))
        if f"query{kind}" in self.fail:
            return 0
        if kind == 9:
            C.memmove(output, C.byref(self.limits), C.sizeof(self.limits))
        elif kind == 1:
            values = output._obj
            values.TotalProcesses = (self.total_override if self.total_override is not None
                                     else sum(obj.member for obj in self.objects))
            values.ActiveProcesses = (self.active_override if self.active_override is not None
                                      else sum(obj.member and not obj.exited for obj in self.objects))
        elif kind == 3:
            pids = (self.pids_override if self.pids_override is not None
                    else [obj.pid for obj in self.objects if obj.member and not obj.exited])
            values = output._obj
            values.assigned = len(pids)
            values.listed = min(len(values.pids), len(pids))
            for index, pid in enumerate(pids[:values.listed]):
                values.pids[index] = pid
            if values.assigned > values.listed:
                self.error = 234
                return 0
        return 1

    def SetInformationJobObject(self, job, kind, output, size):
        self.calls.append(("job_set", kind))
        if "barrier" in self.fail:
            return 0
        if "barrier_ignored" not in self.fail:
            C.memmove(C.byref(self.limits), output, C.sizeof(self.limits))
        if self.after_barrier:
            callback, self.after_barrier = self.after_barrier, None
            callback()
        return 1

    def WaitForSingleObject(self, handle, milliseconds):
        self.calls.append(("wait_original", handle, milliseconds))
        if "original_wait" in self.fail:
            return 0xffffffff
        return debug.WAIT_OBJECT_0 if self.handles[handle].signaled else debug.WAIT_TIMEOUT

    def GetExitCodeProcess(self, handle, output):
        self.calls.append(("exit_query", handle))
        output._obj.value = self.handles[handle].code
        return int("exit_query" not in self.fail)

    def TerminateProcess(self, handle, code):
        self.calls.append(("kill_original", handle))
        if "terminate_process" in self.fail:
            return 0
        obj = self.handles[handle]
        if not obj.exit_queued:
            self.exit(obj, code)
        return 1

    def TerminateJobObject(self, job, code):
        self.calls.append(("kill_job", job))
        if "terminate_job" in self.fail:
            return 0
        for obj in self.objects:
            if obj.member and not obj.exit_queued:
                self.exit(obj, code)
        return 1

    def CloseHandle(self, handle):
        self.calls.append(("close", handle))
        if handle in self.borrowed:
            raise AssertionError("debug API's borrowed process/thread handle was closed")
        if self.close_failures.get(handle, 0):
            self.close_failures[handle] -= 1
            return 0
        if handle in self.closed:
            raise AssertionError("owned handle was closed twice")
        self.closed.add(handle)
        return 1

    def DebugActiveProcessStop(self, pid):
        self.calls.append(("detach", pid))
        return int("detach" not in self.fail)

    def DebugSetProcessKillOnExit(self, value):
        self.calls.append(("kill_on_debugger_exit", value))
        return 1


def fixture():
    api = ScriptedBoundary()
    owner = debug.WindowsDebugJob.__new__(debug.WindowsDebugJob)
    owner._initialize_debug_state(native=False)
    owner.api, owner.nt, owner.last_error, owner.job = api, api, lambda: api.error, 55
    owner.processes, owner.process, owner.extra_handles = [], None, []
    owner.verified_extra_handles, owner.terminal_handles = [], {}
    owner.terminal_prepared = owner.admission_closed = False
    owner.terminal_pending = owner.terminal_total = None
    owner._thread, owner._budget = threading.get_ident(), Budget()
    return owner, api


class AdmissionControls(unittest.TestCase):
    def test_offline_constructor_never_claims_native(self):
        if os.name != "nt":
            with self.assertRaisesRegex(lifecycle.OwnershipError, "native_windows"):
                debug.WindowsDebugJob()
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        api.exit(obj)
        owner.pump_debug(owner._budget)
        evidence = owner.terminal_evidence(owner._budget)
        self.assertTrue(evidence["terminal_proven"])
        self.assertEqual(evidence["evidence_kind"], "OFFLINE_CONTROL_NOT_NATIVE")
        self.assertEqual(evidence["state"], "HOLD")
        owner.close()

    def test_create_binds_original_before_continue_and_closes_only_event_file(self):
        owner, api = fixture()
        obj = api.object(member=False)
        owner.pump_debug(owner._budget)
        calls = [call[0] for call in api.calls]
        for action in ("duplicate", "assign", "member", "image", "creation", "command", "close"):
            self.assertLess(calls.index(action), calls.index("continue"))
        row = owner.admissions[0]
        self.assertTrue(row["retained_before_first_instruction"])
        self.assertTrue(row["identity_bound_before_first_continue"])
        self.assertEqual(row["creation_time"], obj.creation)
        self.assertGreater(row["creation_time"], 0xffffffff)
        self.assertEqual(row["command_line"], obj.line)
        with self.assertRaises(TypeError):
            row["pid"] = 999
        with self.assertRaises(AttributeError):
            owner.admissions = ()
        self.assertIn(obj.file, api.closed)
        self.assertTrue(api.borrowed.isdisjoint(api.closed))
        owner.close()
        self.assertEqual(owner.terminal_evidence()["owned_handles_remaining"], 0)
        self.assertNotIn("detach", [call[0] for call in api.calls])

    def test_command_query_bounds_and_native_identity_failures_hold_before_continue(self):
        failures = (
            "duplicate", "pid", "member", "assign", "assignment_unconfirmed",
            "image", "creation", "command_unavailable", "command_size",
            "command_read", "command_pointer", "command_odd", "command_returned",
        )
        for failure in failures:
            with self.subTest(failure=failure):
                owner, api = fixture()
                obj = api.object(member=False)
                api.fail.add(failure)
                with self.assertRaises(lifecycle.OwnershipError):
                    owner.pump_debug(owner._budget)
                self.assertNotIn("continue", [call[0] for call in api.calls])
                self.assertTrue(owner.terminal_evidence()["errors"])
                self.assertEqual(owner.terminal_evidence()["state"], "HOLD")
                # Failure teardown keeps the pending CREATE original stable;
                # clearing the injection is not allowed to erase HOLD.
                api.fail.clear()
                try:
                    owner.close()
                except lifecycle.OwnershipError:
                    pass
                self.assertTrue(api.borrowed.isdisjoint(api.closed))
                self.assertIn(obj.file, api.closed)
                self.assertEqual(owner._owned, {})

    def test_breakaway_and_lost_kill_on_close_are_rejected(self):
        for flags in (0, 0x2000 | 0x0800, 0x2000 | 0x1000):
            with self.subTest(flags=flags):
                owner, api = fixture()
                api.object()
                api.limits.BasicLimitInformation.LimitFlags = flags
                with self.assertRaisesRegex(lifecycle.OwnershipError, "policy_invalid"):
                    owner.pump_debug(owner._budget)
                self.assertFalse(any(call[0] == "continue" for call in api.calls))

    def test_duplicate_pid_cannot_overwrite_unterminated_original(self):
        owner, api = fixture()
        api.object(pid=41, creation=100)
        owner.pump_debug(owner._budget)
        api.object(pid=41, creation=200)
        with self.assertRaisesRegex(lifecycle.OwnershipError, "pid_reused"):
            owner.pump_debug(owner._budget)
        self.assertEqual(len(owner.admissions), 2)
        self.assertEqual(owner.admissions[0]["creation_time"], 100)
        self.assertFalse(owner.admissions[1]["identity_bound_before_first_continue"])

    def test_pid_reuse_after_original_terminal_preserves_both_original_objects(self):
        owner, api = fixture()
        first = api.object(pid=41, creation=100)
        owner.pump_debug(owner._budget)
        api.exit(first)
        owner.pump_debug(owner._budget)
        second = api.object(pid=41, creation=200)
        owner.pump_debug(owner._budget)
        api.exit(second)
        owner.pump_debug(owner._budget)
        evidence = owner.terminal_evidence()
        self.assertEqual([row["creation_time"] for row in evidence["admissions"]], [100, 200])
        self.assertEqual(len(owner.original_wait_handles()), 2)
        self.assertTrue(evidence["terminal_proven"])
        owner.close()

    def test_same_pid_and_creation_is_duplicate_identity_even_after_terminal(self):
        owner, api = fixture()
        first = api.object(creation=100)
        owner.pump_debug(owner._budget)
        api.exit(first)
        owner.pump_debug(owner._budget)
        api.object(creation=100)
        with self.assertRaisesRegex(lifecycle.OwnershipError, "duplicate_process_identity"):
            owner.pump_debug(owner._budget)

    def test_exception_policy_consumes_one_initial_breakpoint_and_passes_real_exceptions(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        for code, first in ((debug.BREAKPOINT, 1), (debug.BREAKPOINT, 1),
                            (0xC0000005, 1), (0xC0000005, 0), (debug.BREAKPOINT, 0)):
            api.exception(obj, code, first)
        owner.pump_debug(owner._budget)
        dispositions = [row["continue_status"] for row in owner.terminal_evidence()["exceptions"]]
        self.assertEqual(dispositions, [debug.DBG_CONTINUE] + [debug.DBG_EXCEPTION_NOT_HANDLED] * 4)
        api.exit(obj, 0xC0000005)
        owner.pump_debug(owner._budget)
        self.assertEqual(owner.admissions[0]["exit_code"], 0xC0000005)
        self.assertTrue(owner.admissions[0]["natural_terminal_before_cleanup"])
        owner.close()

    def test_unknown_pid_event_cannot_be_continued_or_signed(self):
        owner, api = fixture()
        obj = api.object()
        api.events.clear()
        api.send(debug.DEBUG_STRING, obj)
        with self.assertRaisesRegex(lifecycle.OwnershipError, "without_live_admission"):
            owner.pump_debug(owner._budget)
        self.assertFalse(any(call[0] == "continue" for call in api.calls))
        self.assertFalse(owner.terminal_evidence()["complete_original_object_census"])
        self.assertGreater(owner.terminal_evidence()["debug_connections"], 0)

    def test_unknown_exception_stays_not_handled_even_on_failure_cleanup(self):
        owner, api = fixture()
        obj = api.object()
        api.events.clear()
        api.exception(obj, 0xC0000005, 0)
        with self.assertRaises(lifecycle.OwnershipError):
            owner.pump_debug(owner._budget)
        self.assertEqual(owner._pending.status, debug.DBG_EXCEPTION_NOT_HANDLED)
        self.assertFalse(owner.terminal_evidence()["terminal_proven"])


class TerminalControls(unittest.TestCase):
    def test_exit_event_is_not_terminal_until_continued_original_signaled_and_exit_queried(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        api.exit(obj, signal=False)
        owner.pump_debug(owner._budget)
        evidence = owner.terminal_evidence()
        row = evidence["admissions"][0]
        self.assertTrue(row["exit_event_received"])
        self.assertTrue(row["exit_event_continued"])
        self.assertFalse(row["original_signaled"])
        self.assertFalse(row["natural_terminal_before_cleanup"])
        self.assertIsNone(row["exit_code"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertEqual(owner.active(owner._budget), 1)
        obj.signaled = True
        owner.pump_debug(owner._budget)
        self.assertTrue(owner.admissions[0]["natural_terminal_before_cleanup"])
        self.assertTrue(owner.terminal_evidence()["terminal_proven"])
        owner.close()

    def test_signal_and_exit_query_without_exit_event_is_not_natural_terminal(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        obj.signaled, obj.exited = True, True
        evidence = owner.terminal_evidence()
        self.assertTrue(evidence["admissions"][0]["original_signaled"])
        self.assertFalse(evidence["admissions"][0]["exit_event_received"])
        self.assertFalse(evidence["admissions"][0]["natural_terminal_before_cleanup"])
        self.assertFalse(evidence["terminal_proven"])

    def test_signaled_exit_259_is_an_actual_exit_not_still_active_inference(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        api.exit(obj, 259)
        owner.pump_debug(owner._budget)
        self.assertTrue(owner.admissions[0]["natural_terminal_before_cleanup"])
        self.assertEqual(owner.admissions[0]["exit_code"], 259)
        owner.close()

    def test_exit_query_or_original_wait_failure_cannot_be_hidden_by_zero_accounting(self):
        for failure in ("exit_query", "original_wait"):
            with self.subTest(failure=failure):
                owner, api = fixture()
                obj = api.object()
                owner.pump_debug(owner._budget)
                api.exit(obj)
                api.fail.add(failure)
                with self.assertRaises(lifecycle.OwnershipError):
                    owner.pump_debug(owner._budget)
                self.assertFalse(owner.terminal_evidence()["terminal_proven"])

    def test_event_exit_code_must_agree_with_retained_original(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        api.exit(obj, 0)
        obj.code = 3
        with self.assertRaisesRegex(lifecycle.OwnershipError, "exit_code_disagrees"):
            owner.pump_debug(owner._budget)
        self.assertFalse(owner.admissions[0]["natural_terminal_before_cleanup"])

    def test_missed_short_lived_admission_and_current_unknown_pid_fail_full_census(self):
        for mutation in ("total", "pids", "active"):
            with self.subTest(mutation=mutation):
                owner, api = fixture()
                obj = api.object()
                owner.pump_debug(owner._budget)
                api.exit(obj)
                owner.pump_debug(owner._budget)
                if mutation == "total":
                    api.total_override = 2
                elif mutation == "pids":
                    api.pids_override, api.active_override = [77], 1
                else:
                    api.active_override = 1
                evidence = owner.terminal_evidence()
                self.assertIn("debug_admissions_job_census_incomplete", evidence["errors"])
                self.assertFalse(evidence["complete_original_object_census"])
                self.assertFalse(evidence["terminal_proven"])

    def test_job_query_failure_never_returns_a_complete_census(self):
        for kind in (1, 3):
            with self.subTest(kind=kind):
                owner, api = fixture()
                api.object()
                owner.pump_debug(owner._budget)
                api.fail.add(f"query{kind}")
                self.assertFalse(owner.terminal_evidence()["complete_original_object_census"])

    def test_cleanup_pumps_exit_itself_and_cannot_manufacture_a_natural_result(self):
        owner, api = fixture()
        api.object()
        owner.pump_debug(owner._budget)
        owner.prepare_terminal(owner._budget)
        owner.terminate()
        self.assertEqual(owner.active(owner._budget), 0)
        self.assertFalse(owner.admissions[0]["natural_terminal_before_cleanup"])
        self.assertTrue(owner.terminal_evidence()["terminal_proven"])
        evidence = owner.terminal_evidence()
        owner.close()
        self.assertFalse(evidence["closed"])
        after = owner.terminal_evidence()
        self.assertTrue(after["closed"])
        self.assertEqual(after["owned_handles_remaining"], 0)
        self.assertEqual(after["fallback_detaches"], 0)

    def test_continue_failure_keeps_event_pending_and_cleanup_retries_without_false_success(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        api.exit(obj)
        api.continue_failures = 1
        with self.assertRaisesRegex(lifecycle.OwnershipError, "debug_continue_failed"):
            owner.pump_debug(owner._budget)
        self.assertFalse(owner.admissions[0]["exit_event_continued"])
        self.assertFalse(obj.signaled)
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        evidence = owner.terminal_evidence()
        self.assertTrue(evidence["closed"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertIn("debug_continue_failed", evidence["errors"])

    def test_file_close_failure_is_retained_and_does_not_continue_an_unchecked_create(self):
        owner, api = fixture()
        obj = api.object()
        api.close_failures[obj.file] = 1
        with self.assertRaisesRegex(lifecycle.OwnershipError, "owned_handle_close_failed"):
            owner.pump_debug(owner._budget)
        self.assertFalse(any(call[0] == "continue" for call in api.calls))
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        self.assertIn(obj.file, api.closed)
        self.assertFalse(owner.terminal_evidence()["terminal_proven"])
        self.assertTrue(api.borrowed.isdisjoint(api.closed))

    def test_load_dll_file_and_created_thread_contract_do_not_double_close_borrowed_handles(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        event = api.send(debug.LOAD_DLL, obj)
        event.u.load_dll.hFile = 771
        api.send(debug.CREATE_THREAD, obj).u.create_thread.hThread = 772
        api.borrowed.add(772)
        api.send(debug.EXIT_THREAD, obj)
        owner.pump_debug(owner._budget)
        self.assertIn(771, api.closed)
        self.assertNotIn(772, api.closed)
        owner.close()

    def test_final_owned_handle_close_failure_raises_and_keeps_hold_and_handle_inventory(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        api.exit(obj)
        owner.pump_debug(owner._budget)
        duplicate = owner.original_wait_handles()[0]
        api.close_failures[duplicate] = 1
        with self.assertRaisesRegex(lifecycle.OwnershipError, "owned_handle_close_failed"):
            owner.close()
        evidence = owner.terminal_evidence()
        self.assertFalse(evidence["closed"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertEqual(evidence["owned_handles_remaining"], 1)
        # A retry can release the handle; the failure remains and cannot PASS.
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        self.assertEqual(owner.terminal_evidence()["owned_handles_remaining"], 0)
        self.assertFalse(owner.terminal_evidence()["terminal_proven"])

    def test_persistent_continue_failure_is_bounded_and_detach_never_proves_terminal(self):
        owner, api = fixture()
        api.object()
        owner.pump_debug(owner._budget)
        owner._budget = Budget(0.01)
        api.continue_failures = 100000
        start = time.monotonic()
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        self.assertLess(time.monotonic() - start, 0.5)
        evidence = owner.terminal_evidence()
        self.assertTrue(evidence["pending_debug_event"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertEqual(evidence["owned_handles_remaining"], 0)
        self.assertTrue(evidence["errors"])

    def test_snapshot_before_cleanup_cannot_be_mutated_by_late_admissions(self):
        owner, api = fixture()
        api.object(pid=41)
        owner.pump_debug(owner._budget)
        before = owner.terminal_evidence()
        api.object(pid=42)
        owner.pump_debug(owner._budget)
        after = owner.terminal_evidence()
        self.assertEqual(len(before["admissions"]), 1)
        self.assertEqual(before["job_total_processes"], 1)
        self.assertEqual(len(after["admissions"]), 2)
        after["admissions"][0]["command_line"] = "modified caller snapshot"
        self.assertNotEqual(owner.admissions[0]["command_line"], "modified caller snapshot")
        owner.close()

    def test_job_termination_failure_uses_retained_originals_and_stays_failed(self):
        owner, api = fixture()
        api.object()
        owner.pump_debug(owner._budget)
        api.fail.add("terminate_job")
        start = time.monotonic()
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        self.assertLess(time.monotonic() - start, 0.5)
        self.assertTrue(any(call[0] == "kill_original" for call in api.calls))
        evidence = owner.terminal_evidence()
        self.assertTrue(evidence["closed"])
        self.assertEqual(evidence["fallback_detaches"], 0)
        self.assertFalse(evidence["terminal_proven"])


class SealAndBudgetControls(unittest.TestCase):
    def master(self):
        owner, api = fixture()
        master = api.object(pid=41)
        owner.process = type("RootIdentity", (), {"pid": master.pid})()
        owner.pump_debug(owner._budget)
        return owner, api, master

    def test_natural_seal_keeps_master_live_and_closes_admission(self):
        owner, api, master = self.master()
        child = api.object(pid=42)
        owner.pump_debug(owner._budget)
        api.exit(child)
        evidence = owner.seal_admission(owner._budget, settle_seconds=0)
        self.assertTrue(evidence["admission_closed"])
        self.assertTrue(evidence["complete_original_object_census"])
        self.assertFalse(master.signaled)
        self.assertEqual([row["is_master"] for row in evidence["admissions"]], [True, False])
        self.assertTrue(evidence["admissions"][1]["natural_terminal_before_cleanup"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertEqual(api.limits.BasicLimitInformation.ActiveProcessLimit, 1)
        self.assertEqual(api.limits.ProcessMemoryLimit, 987654)
        owner.close()

    def test_natural_seal_retries_changing_active_counts_before_exact_final_census(self):
        owner, api, master = self.master()
        child = api.object(pid=42)
        owner.pump_debug(owner._budget)
        api.exit(child)
        owner.pump_debug(owner._budget)
        accounting = owner.accounting
        calls = 0

        def changing(budget):
            nonlocal calls
            calls += 1
            value = accounting(budget)
            if calls == 1:
                value.ActiveProcesses = 2
            return value

        with mock.patch.object(owner, "accounting", side_effect=changing):
            evidence = owner.seal_admission(owner._budget, settle_seconds=0.1)
        self.assertTrue(evidence["admission_closed"])
        self.assertTrue(evidence["complete_original_object_census"])
        self.assertEqual(evidence["errors"], [])
        first = evidence["incomplete_census_observations"][0]
        self.assertEqual((first["active_before"], first["active_after"]), (2, 1))
        self.assertEqual(first["current_pids"], [master.pid])
        self.assertTrue(all(row["natural_terminal_before_cleanup"]
                            for row in evidence["admissions"] if not row["is_master"]))
        owner.close()

    def test_natural_seal_pumps_pending_create_before_signing_cumulative_total(self):
        owner, api, _ = self.master()
        child = api.object(pid=42)
        pump = owner.pump_debug
        calls = 0

        def delayed(budget):
            nonlocal calls
            calls += 1
            if calls == 1:
                return 0
            count = pump(budget)
            if not child.exit_queued:
                api.exit(child)
                count += pump(budget)
            return count

        with mock.patch.object(owner, "pump_debug", side_effect=delayed):
            evidence = owner.seal_admission(owner._budget, settle_seconds=0.1)
        first = evidence["incomplete_census_observations"][0]
        self.assertEqual((first["total_after"], first["admitted"]), (2, 1))
        self.assertEqual(first["unknown_pids"], [child.pid])
        self.assertEqual(evidence["job_total_processes"], len(evidence["admissions"]))
        self.assertTrue(evidence["complete_original_object_census"])
        self.assertTrue(evidence["admissions"][1]["identity_bound_before_first_continue"])
        self.assertTrue(evidence["admissions"][1]["natural_terminal_before_cleanup"])
        owner.close()

    def test_unresolved_cumulative_omission_exhausts_settle_without_sealing(self):
        owner, api, _ = self.master()
        api.total_override = 2
        with self.assertRaisesRegex(lifecycle.OwnershipError, "census_incomplete"):
            owner.seal_admission(owner._budget, settle_seconds=0)
        self.assertFalse(owner.admission_closed)
        evidence = owner.terminal_evidence()
        self.assertFalse(evidence["complete_original_object_census"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertEqual(evidence["incomplete_census_observations"][0]["admitted"], 1)
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()

    def test_asynchronous_termination_census_cannot_return_zero_until_stable(self):
        owner, _, _ = self.master()
        owner.seal_admission(owner._budget, settle_seconds=0)
        owner.terminate()
        accounting = owner.accounting
        calls = 0

        def changing(budget):
            nonlocal calls
            calls += 1
            value = accounting(budget)
            if calls == 1:
                value.ActiveProcesses = 1
            return value

        with mock.patch.object(owner, "accounting", side_effect=changing):
            self.assertGreater(owner.active(owner._budget), 0)
            self.assertEqual(owner.active(owner._budget), 0)
        self.assertEqual(owner.terminal_evidence()["errors"], [])
        owner.close()

    def test_settling_never_tolerates_a_changed_sealed_cumulative_total(self):
        owner, api, _ = self.master()
        owner.seal_admission(owner._budget, settle_seconds=0)
        api.total_override = 2
        with self.assertRaisesRegex(lifecycle.OwnershipError, "census_incomplete"):
            owner.active(owner._budget)
        self.assertFalse(owner.terminal_evidence()["terminal_proven"])
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()

    def test_census_snapshots_do_not_expose_mutable_internal_pid_lists(self):
        owner, api, master = self.master()
        api.active_override = 2
        self.assertFalse(owner._census(owner._budget, settling=True))
        api.active_override = None
        evidence = owner.terminal_evidence()
        evidence["incomplete_census_observations"][0]["current_pids"].clear()
        evidence["last_census_observation"]["current_pids"].clear()
        fresh = owner.terminal_evidence()
        self.assertEqual(fresh["incomplete_census_observations"][0]["current_pids"], [master.pid])
        self.assertEqual(fresh["last_census_observation"]["current_pids"], [master.pid])
        owner.close()

    def test_live_nonmaster_cannot_be_sealed_or_killed_to_pass_natural_snapshot(self):
        owner, api, _ = self.master()
        api.object(pid=42)
        with self.assertRaisesRegex(lifecycle.OwnershipError, "natural_settle_expired"):
            owner.seal_admission(owner._budget, settle_seconds=0)
        self.assertFalse(owner.admission_closed)
        self.assertFalse(any(call[0] == "kill_job" for call in api.calls))

    def test_seal_race_retains_late_admission_and_holds_before_any_continue(self):
        owner, api, _ = self.master()
        late = []
        api.after_barrier = lambda: late.append(api.object(pid=42))
        with self.assertRaisesRegex(lifecycle.OwnershipError, "creation_after_admission_seal"):
            owner.seal_admission(owner._budget, settle_seconds=0)
        self.assertEqual(len(owner.admissions), 2)
        self.assertTrue(owner.admissions[1]["identity_bound_before_first_continue"])
        self.assertFalse(any(call[:2] == ("continue", 42) for call in api.calls))
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        self.assertEqual(len(owner.terminal_evidence()["admissions"]), 2)
        self.assertFalse(owner.terminal_evidence()["terminal_proven"])

    def test_barrier_failure_or_ignored_setter_is_not_a_seal(self):
        for fault in ("barrier", "barrier_ignored"):
            with self.subTest(fault=fault):
                owner, api, _ = self.master()
                api.fail.add(fault)
                with self.assertRaises(lifecycle.OwnershipError):
                    owner.seal_admission(owner._budget, settle_seconds=0)
                self.assertFalse(owner.admission_closed)
                self.assertTrue(owner.terminal_evidence()["errors"])

    def test_child_created_after_successful_seal_is_still_recorded_and_killed_before_continue(self):
        owner, api, _ = self.master()
        before = owner.seal_admission(owner._budget, settle_seconds=0)
        api.object(pid=42)
        with self.assertRaisesRegex(lifecycle.OwnershipError, "creation_after_admission_seal"):
            owner.pump_debug(owner._budget)
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        after = owner.terminal_evidence()
        self.assertEqual(len(before["admissions"]), 1)
        self.assertEqual(len(after["admissions"]), 2)
        names = [call[0] for call in api.calls]
        self.assertLess(names.index("kill_job"), names.index("kill_original")
                        if "kill_original" in names else len(names))
        self.assertFalse(after["terminal_proven"])

    def test_wrong_thread_and_failed_detach_cannot_close_debug_controls_by_claim(self):
        owner, api, _ = self.master()
        api.fail.add("detach")
        owner._budget = Budget(-1)
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        evidence = owner.terminal_evidence()
        self.assertFalse(evidence["closed"])
        self.assertGreater(evidence["debug_connections"], 0)
        self.assertIn("debug_detach_failed", evidence["errors"])

    def test_cleanup_reserve_is_not_spent_waiting_for_natural_settle(self):
        owner, api, _ = self.master()
        owner._budget = Budget(0.01)
        with self.assertRaises(TimeoutError):
            owner.seal_admission(owner._budget, settle_seconds=5)
        self.assertFalse(owner.admission_closed)

    def test_pump_uses_zero_wait_and_has_a_finite_event_slice(self):
        owner, api = fixture()
        api.object()
        owner.pump_debug(owner._budget)
        api.calls.clear()
        api.stream_forever = True
        count = owner.pump_debug(owner._budget)
        self.assertGreater(count, 0)
        self.assertLessEqual(count, debug.MAX_EVENTS_PER_PUMP)
        self.assertTrue(all(call[1] == 0 for call in api.calls if call[0] == "wait_event"))
        self.assertTrue(all(call[2] == 0 for call in api.calls if call[0] == "wait_original"))

    def test_expired_original_cannot_be_extended_by_a_new_caller_budget(self):
        owner, api = fixture()
        api.object()
        owner._budget = Budget(-1)
        with self.assertRaises(TimeoutError):
            owner.pump_debug(Budget(60))
        self.assertEqual(api.calls, [])
        self.assertIn("debug_original_deadline_expired", owner.terminal_evidence()["errors"])

    def test_expired_cleanup_releases_owned_handles_but_detach_is_never_terminal(self):
        owner, api = fixture()
        api.object()
        owner.pump_debug(owner._budget)
        owner._budget = Budget(-1)
        with self.assertRaises(lifecycle.OwnershipError):
            owner.close()
        evidence = owner.terminal_evidence()
        self.assertEqual(evidence["owned_handles_remaining"], 0)
        self.assertGreater(evidence["fallback_detaches"], 0)
        self.assertFalse(evidence["terminal_proven"])
        self.assertTrue(evidence["errors"])

    def test_native_wait_and_pipe_wait_cannot_block_the_debug_loop(self):
        owner, api = fixture()
        obj = api.object()
        owner.pump_debug(owner._budget)
        # A separate creation handle refers to the same original object.
        api.handles[9001] = obj
        process = debug._DebugProcess(owner, api, obj.pid, 9001, 9002, None, None, None)
        start = time.monotonic()
        with self.assertRaises(subprocess.TimeoutExpired):
            process.wait(timeout=0.01)
        self.assertLess(time.monotonic() - start, 0.5)
        self.assertTrue(all(call[2] == 0 for call in api.calls if call[0] == "wait_original"))
        api.exit(obj)
        self.assertEqual(process.wait(timeout=0.1), 0)
        owner.close()

    def test_wrong_thread_cannot_continue_debug_events(self):
        owner, api = fixture()
        api.object()
        errors = []
        def run():
            try:
                owner.pump_debug(owner._budget)
            except lifecycle.OwnershipError as exc:
                errors.append(str(exc))
        thread = threading.Thread(target=run)
        thread.start()
        thread.join(timeout=0.5)
        self.assertFalse(thread.is_alive())
        self.assertEqual(errors, ["debug_thread_changed"])
        self.assertFalse(any(call[0] == "continue" for call in api.calls))

    def test_only_debug_process_flag_is_accepted_by_base_spawn(self):
        owner = lifecycle.WindowsJob.__new__(lifecycle.WindowsJob)
        for flags in (2, 3, 0x01000000, -1, True, "1"):
            with self.subTest(flags=flags):
                with self.assertRaisesRegex(lifecycle.OwnershipError, "unsupported"):
                    owner.spawn([], Path("."), {}, Budget(), creation_flags=flags)

    def test_debug_event_layout_uses_fixed_windows_integer_widths_offline(self):
        self.assertEqual(C.sizeof(debug.U16), 2)
        self.assertEqual(C.sizeof(debug.U32), 4)
        self.assertEqual(C.sizeof(debug.I32), 4)
        self.assertEqual(C.sizeof(debug.FileTime), 8)
        self.assertEqual(C.sizeof(debug.DebugEvent), {4: 96, 8: 176}[C.sizeof(C.c_void_p)])
        # This checks host ctypes layout arithmetic, not either Windows ABI.


class PipeCreationBoundary(ScriptedBoundary):
    """Real anonymous host pipes, scripted process creation; still non-native."""

    def __init__(self):
        super().__init__()
        self.root = self.object(member=False)
        self.handles[9001] = self.root
        self.flags = None

    def InitializeProcThreadAttributeList(self, storage, count, flags, size):
        size._obj.value = 64
        return int(storage is not None)

    def UpdateProcThreadAttribute(self, *args):
        return 1

    def DeleteProcThreadAttributeList(self, *args):
        pass

    def CreateProcessW(self, app, line, process_sa, thread_sa, inherit, flags,
                       env, cwd, startup, output):
        self.calls.append(("create_process", flags))
        self.flags = flags
        output._obj.hProcess, output._obj.hThread = 9001, 9002
        output._obj.dwProcessId, output._obj.dwThreadId = self.root.pid, self.root.pid + 300
        if not flags & debug.DEBUG_PROCESS:
            self.events.clear()
        return 1

    def ResumeThread(self, handle):
        self.calls.append(("resume", handle))
        return 1


class SpawnFailureControls(unittest.TestCase):
    def setup_owner(self, debug_owner=True):
        owner, _ = fixture()
        api = PipeCreationBoundary()
        if not debug_owner:
            plain = lifecycle.WindowsJob.__new__(lifecycle.WindowsJob)
            for name in ("job", "processes", "process", "extra_handles", "verified_extra_handles",
                         "terminal_handles", "terminal_prepared", "admission_closed",
                         "terminal_pending", "terminal_total"):
                setattr(plain, name, getattr(owner, name))
            owner = plain
        else:
            owner._budget = None
        owner.api, owner.last_error = api, lambda: api.error
        if debug_owner:
            owner.nt = api
        return owner, api

    def spawn(self, owner):
        msvcrt = SimpleNamespace(get_osfhandle=lambda fd: fd)
        with mock.patch.dict(sys.modules, {"msvcrt": msvcrt}):
            return owner.spawn(["owned-control.exe", "-D"], Path("."), {}, Budget())

    def close_streams(self, owner):
        if owner.process is not None:
            for stream in (owner.process.stdin, owner.process.stdout, owner.process.stderr):
                if stream is not None:
                    stream.close()

    def test_default_spawn_flags_factory_and_resume_order_remain_unchanged(self):
        owner, api = self.setup_owner(debug_owner=False)
        try:
            process = self.spawn(owner)
            self.assertIs(type(process), lifecycle.WindowsProcess)
            self.assertEqual(api.flags, 0x00000004 | 0x00000400 | 0x00080000)
            names = [call[0] for call in api.calls]
            self.assertLess(names.index("assign"), names.index("resume"))
        finally:
            self.close_streams(owner)
            owner.close()

    def test_debug_spawn_uses_inherited_chain_flag_and_original_process_factory(self):
        owner, api = self.setup_owner()
        try:
            process = self.spawn(owner)
            self.assertIsInstance(process, debug._DebugProcess)
            self.assertEqual(api.flags, 0x00000004 | 0x00000400 | 0x00080000 | debug.DEBUG_PROCESS)
            self.assertFalse(api.flags & 2)
            self.assertTrue(owner.admissions[0]["is_master"])
            self.assertTrue(owner.admissions[0]["identity_bound_before_first_continue"])
            names = [call[0] for call in api.calls]
            self.assertLess(names.index("assign"), names.index("resume"))
            self.assertLess(names.index("command"), names.index("continue"))
        finally:
            self.close_streams(owner)
            owner.close()
        self.assertTrue(owner.terminal_evidence()["closed"])

    def test_preassignment_failure_wait_pumps_exit_instead_of_blocking_original_object(self):
        owner, api = self.setup_owner()
        api.fail.add("assign")
        start = time.monotonic()
        with self.assertRaises(lifecycle.OwnershipError):
            self.spawn(owner)
        self.assertLess(time.monotonic() - start, 0.5)
        self.assertTrue(api.root.signaled)
        self.assertNotIn("resume", [call[0] for call in api.calls])
        try:
            with self.assertRaises(lifecycle.OwnershipError):
                owner.close()
        finally:
            self.close_streams(owner)
        evidence = owner.terminal_evidence()
        self.assertTrue(evidence["closed"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertEqual(evidence["owned_handles_remaining"], 0)

    def test_interrupted_factory_retains_and_reaps_creation_handles_without_native_wait(self):
        owner, api = self.setup_owner()
        factory = owner._process_factory
        def interrupted(*args):
            factory(*args)  # interruption after its own handles were retained
            raise InterruptedError("offline creation-registration interruption")
        owner._process_factory = interrupted
        start = time.monotonic()
        with self.assertRaises(InterruptedError):
            self.spawn(owner)
        self.assertLess(time.monotonic() - start, 0.5)
        self.assertTrue(api.root.signaled)
        try:
            with self.assertRaises(lifecycle.OwnershipError):
                owner.close()
        finally:
            self.close_streams(owner)
        self.assertEqual(owner.terminal_evidence()["owned_handles_remaining"], 0)
        self.assertTrue(owner.terminal_evidence()["closed"])
        self.assertTrue(all(call[2] == 0 for call in api.calls if call[0] == "wait_original"))

    def test_thread_cannot_mix_two_debug_owners_events(self):
        owner, _ = self.setup_owner()
        other, _ = self.setup_owner()
        try:
            self.spawn(owner)
            with self.assertRaisesRegex(lifecycle.OwnershipError, "already_owns_root"):
                self.spawn(other)
        finally:
            self.close_streams(owner)
            owner.close()
            other.close()


class FinishOwnedControls(unittest.TestCase):
    def make_child(self, *, stdin=None):
        owner, api = fixture()
        obj = api.object()
        api.handles[9001] = obj  # separate CreateProcess handle, same original
        streams = (stdin if stdin is not None else io.BytesIO(),
                   io.BytesIO(b"offline control stdout\n"),
                   io.BytesIO(b"offline control stderr\n"))
        process = debug._DebugProcess(owner, api, obj.pid, 9001, 9002, *streams)
        owner.process = process
        owner.processes.append(process)
        owner.pump_debug(owner._budget)
        api.exit(obj)
        owner.pump_debug(owner._budget)
        child = lab.OwnedChild(["offline-control.exe", "-D"], owner._budget, owner=owner)
        child.process = process
        for stream, destination in ((process.stdout, child.stdout), (process.stderr, child.stderr)):
            pump = lifecycle.Pump(stream)
            child.pumps.append(pump)
            child.capture[pump] = destination.extend
        self.addCleanup(self.release, child)
        return child, api

    def release(self, child):
        try:
            child.owner.close()
        except lifecycle.OwnershipError:
            pass  # retry releases the injected failed handle; HOLD stays sticky
        for stream in (child.process.stdin, child.process.stdout, child.process.stderr):
            stream.close()

    def fail_debug_close(self, child, api):
        duplicate = child.owner._originals[0].handle
        api.close_failures[duplicate] = 1
        return duplicate

    def test_finish_owned_records_actual_debug_close_failure_and_closes_streams(self):
        child, api = self.make_child()
        duplicate = self.fail_debug_close(child, api)
        terminal, errors = lifecycle.finish_owned(
            child.owner, child.process, child.pumps, child.budget, child.capture)
        self.assertEqual(errors, {"owned_close_failed"})
        self.assertEqual(terminal["errors"], ["owned_close_failed"])
        self.assertTrue(terminal["direct_reaped"])
        self.assertTrue(terminal["pipes_drained"])
        self.assertEqual(terminal["active_processes"], 0)
        self.assertTrue(all(stream.closed for stream in (
            child.process.stdin, child.process.stdout, child.process.stderr)))
        evidence = child.owner.terminal_evidence()
        self.assertIn("owned_handle_close_failed", evidence["errors"])
        self.assertFalse(evidence["terminal_proven"])
        self.assertFalse(evidence["closed"])
        self.assertEqual(evidence["owned_handles_remaining"], 1)
        self.assertNotIn(duplicate, api.closed)
        self.assertEqual(bytes(child.stdout), b"offline control stdout\n")
        self.assertEqual(bytes(child.stderr), b"offline control stderr\n")

    def test_keep_child_saves_failed_debug_close_receipt_and_both_captures(self):
        child, api = self.make_child()
        self.fail_debug_close(child, api)
        with tempfile.TemporaryDirectory(prefix="keelshell-debug-control-") as temporary:
            output = Path(temporary) / "evidence"
            evidence = lab.Evidence(output)
            receipt = evidence.keep_child("offline-debug-close", child)
            self.assertFalse(lab.terminal_ok(receipt))
            self.assertIn("owned_close_failed", receipt["errors"])
            self.assertIn("terminal_unproven", receipt["errors"])
            self.assertEqual(receipt["terminal"]["errors"], ["owned_close_failed"])
            saved = json.loads((output / "offline-debug-close.receipt.json").read_text("utf-8"))
            self.assertEqual(saved, receipt)
            self.assertEqual((output / "offline-debug-close.stdout").read_bytes(), bytes(child.stdout))
            self.assertEqual((output / "offline-debug-close.stderr").read_bytes(), bytes(child.stderr))
            self.assertEqual(child.close(), receipt)
            self.assertEqual(len(evidence.children), 1)

    def test_failing_stream_close_does_not_skip_other_streams_or_hide_debug_error(self):
        class FailingClose(io.BytesIO):
            pending_failure = True

            def close(self):
                super().close()
                if self.pending_failure:
                    self.pending_failure = False
                    raise OSError("offline controlled stream close failure")

        child, api = self.make_child(stdin=FailingClose())
        self.fail_debug_close(child, api)
        terminal, errors = lifecycle.finish_owned(
            child.owner, child.process, child.pumps, child.budget, child.capture)
        self.assertEqual(errors, {"owned_close_failed", "owned_stream_close_failed"})
        self.assertEqual(terminal["errors"], sorted(errors))
        self.assertTrue(all(stream.closed for stream in (
            child.process.stdin, child.process.stdout, child.process.stderr)))
        self.assertFalse(child.owner.terminal_evidence()["terminal_proven"])

    def test_successful_finish_keeps_empty_errors_and_complete_stream_closure(self):
        child, _ = self.make_child()
        terminal, errors = lifecycle.finish_owned(
            child.owner, child.process, child.pumps, child.budget, child.capture)
        self.assertEqual(errors, set())
        self.assertEqual(terminal["errors"], [])
        self.assertTrue(all(stream.closed for stream in (
            child.process.stdin, child.process.stdout, child.process.stderr)))
        evidence = child.owner.terminal_evidence()
        self.assertTrue(evidence["closed"])
        self.assertEqual(evidence["owned_handles_remaining"], 0)
        self.assertEqual(evidence["errors"], [])


@unittest.skipUnless(os.name == "nt", "native Windows debug/ABI control UNRUN on this host")
class NativeWindowsControl(unittest.TestCase):
    def test_real_inherited_child_and_original_object_exit(self):
        owner = debug.WindowsDebugJob()
        budget = Budget(20)
        source = ("import subprocess,sys;"
                  "subprocess.run([sys.executable,'-B','-c','pass'],check=True)")
        process = None
        try:
            process = owner.spawn([sys.executable, "-B", "-c", source],
                                  Path(__file__).resolve().parent.parent, dict(os.environ), budget)
            self.assertEqual(process.wait(timeout=10), 0)
            before = owner.terminal_evidence(budget)
            self.assertEqual(before["errors"], [])
            self.assertEqual(len(before["admissions"]), 2)
            self.assertTrue(before["complete_original_object_census"])
            self.assertTrue(all(row["natural_terminal_before_cleanup"]
                                and row["identity_bound_before_first_continue"]
                                for row in before["admissions"]))
        finally:
            try:
                owner.close()
            finally:
                if process is not None:
                    for stream in (process.stdin, process.stdout, process.stderr):
                        if stream is not None:
                            stream.close()
        evidence = owner.terminal_evidence()
        self.assertEqual(evidence["state"], "PASS", json.dumps(evidence))
        self.assertTrue(evidence["closed"])
        self.assertEqual(evidence["owned_handles_remaining"], 0)


if __name__ == "__main__":
    unittest.main()
