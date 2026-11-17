"""Pre-instruction admission and original-object termination for an owned SSH lab.

Only native Windows can construct this owner. Offline fault-injection controls
exercise the evidence rules, not Win32 ABI, OpenSSH, or native acceptance.
CreateProcess's own handles and DuplicateHandle results belong to us; debug
event process/thread handles belong to the debug API. Only event hFile handles
are explicitly closed. See Microsoft's Debugging Events / WaitForDebugEvent /
ContinueDebugEvent contracts; EXIT alone cannot prove kernel shutdown.

ProcessCommandLineInformation (60) is an internal NT information class. Its
absence, a malformed buffer, or unavailable pre-continue process parameters is
a hard evidence failure. We never substitute argv or a later process with the
same PID, or call a post-continue reading a pre-instruction identity binding.
"""

import ctypes as C
from dataclasses import dataclass
import math
import os
import subprocess
import threading
import time
from types import MappingProxyType

from windows_native_crash import (
    CLEANUP_RESERVE, MAX_JOB_PROCESSES, JobExtendedLimits, OwnershipError,
    WindowsJob, WindowsProcess,
)


U16, U32, I32 = C.c_uint16, C.c_uint32, C.c_int32
HANDLE, PTR, ULONG_PTR = C.c_void_p, C.c_void_p, C.c_size_t
DEBUG_PROCESS = 0x00000001
EXCEPTION, CREATE_THREAD, CREATE_PROCESS, EXIT_THREAD, EXIT_PROCESS = range(1, 6)
LOAD_DLL, UNLOAD_DLL, DEBUG_STRING, RIP = range(6, 10)
DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED = 0x00010002, 0x80010001
BREAKPOINT = 0x80000003
ERROR_SEM_TIMEOUT = 121
WAIT_OBJECT_0, WAIT_TIMEOUT = 0, 258
MAX_COMMAND_BYTES = 65536
MAX_EVENTS_PER_PUMP = 64
MAX_REFRESH_PER_PUMP = 64
PUMP_SECONDS = 0.01
_DEBUG_THREAD = threading.local()


class FileTime(C.Structure):
    _fields_ = [("low", U32), ("high", U32)]


class UnicodeString(C.Structure):
    _fields_ = [("Length", U16), ("MaximumLength", U16), ("Buffer", PTR)]


class ExceptionRecord(C.Structure):
    _fields_ = [("code", U32), ("flags", U32), ("record", PTR),
                ("address", PTR), ("parameter_count", U32),
                ("parameters", ULONG_PTR * 15)]


class ExceptionInfo(C.Structure):
    _fields_ = [("record", ExceptionRecord), ("first_chance", U32)]


class CreateProcessInfo(C.Structure):
    _fields_ = [("hFile", HANDLE), ("hProcess", HANDLE), ("hThread", HANDLE),
                ("base", PTR), ("debug_offset", U32), ("debug_size", U32),
                ("tls", PTR), ("start", PTR), ("image_name", PTR),
                ("unicode", U16)]


class CreateThreadInfo(C.Structure):
    _fields_ = [("hThread", HANDLE), ("tls", PTR), ("start", PTR)]


class ExitInfo(C.Structure):
    _fields_ = [("code", U32)]


class LoadDllInfo(C.Structure):
    _fields_ = [("hFile", HANDLE), ("base", PTR), ("debug_offset", U32),
                ("debug_size", U32), ("image_name", PTR), ("unicode", U16)]


class DebugStringInfo(C.Structure):
    _fields_ = [("data", PTR), ("unicode", U16), ("length", U16)]


class DebugUnion(C.Union):
    _fields_ = [("exception", ExceptionInfo), ("create_process", CreateProcessInfo),
                ("create_thread", CreateThreadInfo), ("exit_process", ExitInfo),
                ("exit_thread", ExitInfo), ("load_dll", LoadDllInfo),
                ("unload_dll", PTR), ("debug_string", DebugStringInfo),
                ("rip", U32 * 2)]


class DebugEvent(C.Structure):
    _fields_ = [("code", U32), ("pid", U32), ("tid", U32), ("u", DebugUnion)]


@dataclass
class _Original:
    pid: int
    handle: int
    image: str | None = None
    creation_time: int | None = None
    command_line: str | None = None
    member: bool = False
    bound: bool = False
    create_continued: bool = False
    initial_breakpoint: bool = False
    exit_event: int | None = None
    exit_continued: bool = False
    signaled: bool = False
    exit_code: int | None = None
    natural: bool = False

    def snapshot(self):
        return {
            "pid": self.pid, "image": self.image, "creation_time": self.creation_time,
            "command_line": self.command_line,
            "retained_before_first_instruction": bool(self.handle),
            "identity_bound_before_first_continue": self.bound,
            "exit_event_received": self.exit_event is not None,
            "exit_event_code": self.exit_event, "exit_event_continued": self.exit_continued,
            "original_signaled": self.signaled, "exit_code": self.exit_code,
            "natural_terminal_before_cleanup": self.natural,
        }


@dataclass
class _Pending:
    event: DebugEvent
    processed: bool = False
    status: int = DBG_CONTINUE
    failed: bool = False
    termination_requested: bool = False
    original: _Original | None = None


class _SharedBudget:
    def __init__(self, original, caller):
        self.original, self.caller = original, caller

    def remaining(self, reserve=0):
        return min(self.original.remaining(reserve=reserve),
                   self.caller.remaining(reserve=reserve))


class _DebugProcess(WindowsProcess):
    """Keep failure-path reaping on the creating thread's debug event loop."""

    def __init__(self, owner, *args):
        super().__init__(*args)
        self.owner = owner
        owner._own(self._handle, "creation_process")
        owner._own(self.thread_handle, "creation_thread")

    def poll(self):
        self.owner._pump(self.owner._budget, strict=not self.owner._terminating)
        return super().poll()

    def wait(self, timeout):
        if not math.isfinite(timeout) or timeout < 0:
            raise ValueError("debug wait requires a finite nonnegative timeout")
        end = time.monotonic() + timeout
        while True:
            code = self.poll()
            if code is not None:
                return code
            allowance = min(end - time.monotonic(), self.owner._budget.remaining())
            if allowance <= 0:
                raise subprocess.TimeoutExpired("owned-debug-process", timeout)
            time.sleep(min(0.001, allowance))

    def resume(self):
        previous = self.api.ResumeThread(self.thread_handle)
        if self.owner._close_handle(self.thread_handle):
            self.thread_handle = None
        if previous != 1:
            self.owner._fail("primary_suspend_count_invalid")
        self.owner._raise_errors()
        self.owned_started = True

    def kill(self):
        self.owner._terminating = True
        self.owner._admission_phase_closed = True
        if WindowsProcess.poll(self) is None:
            if not self.api.TerminateProcess(self._handle, 125):
                self.owner._fail("direct_termination_failed")
            else:
                self.owner._killed_pids.add(self.pid)
        self.owner._pump(self.owner._budget, strict=False)

    def close_handles(self):
        for field in ("thread_handle", "_handle"):
            if self.owner._close_handle(getattr(self, field)):
                setattr(self, field, None)


class WindowsDebugJob(WindowsJob):
    """Own every inherited debug creation before its first user instruction.

    ``spawn`` and all event pumping/cleanup must run on the same thread.
    ``pump_debug(budget)`` performs at most 64 zero-wait polls and 10ms of work
    between bounded native calls. The original budget includes cleanup.
    ``admissions`` is a tuple of read-only mapping snapshots; use
    ``terminal_evidence()`` for JSON. A live master normally leaves terminal
    state HOLD; inspect complete census and each non-master's natural terminal
    before stopping the server. Cleanup cannot manufacture a natural terminal.
    """

    def __init__(self):
        if os.name != "nt":
            raise OwnershipError("native_windows_debug_job_required")
        self._initialize_debug_state(native=True)
        super().__init__()
        try:
            self._bind_debug_api()
        except BaseException:
            self.close()
            raise

    def _initialize_debug_state(self, *, native=False):
        self._native = native
        self._thread = None
        self._budget = None
        self._originals = []
        self._master = None
        self._by_pid = {}
        self._identities = set()
        self._owned = {}
        self._connections = set()
        self._debug_borrowed = {}
        self._killed_pids = set()
        self._pending = None
        self._errors = set()
        self._exceptions = []
        self._terminating = False
        self._admission_phase_closed = False
        self._pumping = False
        self._closed = False
        self._last_total = None
        self._last_active = None
        self._census_complete = False
        self._events = 0
        self._refresh_cursor = 0
        self._continued = 0
        self._detached = 0
        self._evidence = {}

    def _bind_debug_api(self):
        pointer = C.sizeof(PTR)
        if pointer not in (4, 8) or C.sizeof(DebugEvent) != {4: 96, 8: 176}[pointer]:
            raise OwnershipError("unsupported_debug_event_abi")
        self.nt = C.WinDLL("ntdll", use_last_error=True)
        declarations = [
            ("WaitForDebugEvent", [C.POINTER(DebugEvent), U32], I32),
            ("ContinueDebugEvent", [U32, U32, U32], I32),
            ("DebugSetProcessKillOnExit", [I32], I32),
            ("DebugActiveProcessStop", [U32], I32),
            ("GetCurrentProcess", [], HANDLE),
            ("DuplicateHandle", [HANDLE, HANDLE, HANDLE, C.POINTER(HANDLE), U32, I32, U32], I32),
            ("GetProcessId", [HANDLE], U32),
            ("IsProcessInJob", [HANDLE, HANDLE, C.POINTER(I32)], I32),
            ("QueryFullProcessImageNameW", [HANDLE, U32, C.POINTER(U16), C.POINTER(U32)], I32),
            ("GetProcessTimes", [HANDLE] + [C.POINTER(FileTime)] * 4, I32),
            ("GetExitCodeProcess", [HANDLE, C.POINTER(U32)], I32),
        ]
        for name, arguments, result in declarations:
            function = getattr(self.api, name)
            function.argtypes, function.restype = arguments, result
        query = self.nt.NtQueryInformationProcess
        query.argtypes = [HANDLE, U32, PTR, U32, C.POINTER(U32)]
        query.restype = I32

    def _fail(self, reason):
        self._errors.add(reason)
        self._census_complete = False

    def _raise_errors(self):
        if self._errors:
            raise OwnershipError("debug_job_hold: " + ", ".join(sorted(self._errors)))

    def _require_thread(self):
        if self._thread is not None and threading.get_ident() != self._thread:
            self._fail("debug_thread_changed")
            raise OwnershipError("debug_thread_changed")

    def _own(self, handle, role):
        if not handle or handle in self._owned:
            raise OwnershipError("invalid_or_duplicate_owned_handle")
        self._owned[handle] = role

    def _close_handle(self, handle):
        if not handle:
            return True
        if handle not in self._owned:
            self._fail("attempted_unowned_handle_close")
            return False
        if not self.api.CloseHandle(handle):
            self._fail("owned_handle_close_failed")
            return False
        del self._owned[handle]
        return True

    def _process_factory(self, *args):
        return _DebugProcess(self, *args)

    def _cleanup_unregistered_creation(self, info, budget):
        """Recover an interruption before the base registered its local process."""
        self._fail("debug_creation_registration_interrupted")
        process = _DebugProcess.__new__(_DebugProcess)
        WindowsProcess.__init__(process, self.api, info.dwProcessId,
                                info.hProcess, info.hThread, None, None, None)
        process.owner = self
        for handle, role in ((info.hProcess, "creation_process"),
                             (info.hThread, "creation_thread")):
            if handle and handle not in self._owned:
                self._own(handle, role)
        self.process = process
        self.processes.append(process)
        try:
            process.kill()
            process.wait(timeout=budget.remaining())
        except (OwnershipError, OSError, TimeoutError, subprocess.SubprocessError):
            self._fail("debug_unregistered_creation_reap_failed")

    def spawn(self, command, cwd, env, deadline, merge_error=False):
        """Create suspended in the Job with inherited DEBUG_PROCESS events."""
        if self._budget is not None or self._closed:
            raise OwnershipError("debug_owner_accepts_one_root")
        existing = getattr(_DEBUG_THREAD, "owner", None)
        if existing is not None and existing is not self:
            raise OwnershipError("debug_thread_already_owns_root")
        self._thread = threading.get_ident()
        self._budget = deadline
        deadline.remaining(reserve=CLEANUP_RESERVE)
        _DEBUG_THREAD.owner = self
        try:
            process = super().spawn(command, cwd, env, deadline, merge_error,
                                    creation_flags=DEBUG_PROCESS)
            if not self.api.DebugSetProcessKillOnExit(True):
                self._fail("debug_kill_on_exit_failed")
                self._raise_errors()
            self.pump_debug(deadline)
            return process
        except BaseException:
            # The base factory makes even pre-assignment failure waits pump.
            # Caller still owns close() and the original, unrenewed deadline.
            self._terminating = True
            self._fail("debug_spawn_failed")
            if self.process is not None:
                self.process.kill()
            raise

    @property
    def admissions(self):
        """Read-only snapshots; handles and mutable internal state stay private."""
        return tuple(MappingProxyType(self._snapshot(row)) for row in self._originals)

    def _snapshot(self, row):
        return dict(row.snapshot(), is_master=row is self._master)

    def _check_limits(self, budget):
        budget.remaining()
        if not self.job:
            raise OwnershipError("debug_job_handle_closed")
        limits = JobExtendedLimits()
        if not self.api.QueryInformationJobObject(
                self.job, 9, C.byref(limits), C.sizeof(limits), None):
            raise OwnershipError("debug_job_limits_unavailable")
        flags = limits.BasicLimitInformation.LimitFlags
        if not flags & 0x2000 or flags & (0x0800 | 0x1000):
            raise OwnershipError("debug_job_breakaway_or_kill_policy_invalid")
        return limits

    def _confirm_member(self, handle, budget):
        budget.remaining()
        present = I32()
        if not self.api.IsProcessInJob(handle, self.job, C.byref(present)):
            raise OwnershipError("debug_membership_query_failed")
        if not present.value:
            if not self.api.AssignProcessToJobObject(self.job, handle):
                raise OwnershipError("debug_assignment_failed")
            if not self.api.IsProcessInJob(handle, self.job, C.byref(present)) or not present.value:
                raise OwnershipError("debug_assignment_unconfirmed")

    def _image_and_time(self, handle, budget):
        budget.remaining()
        name = (U16 * 32768)()
        length = U32(len(name))
        if not self.api.QueryFullProcessImageNameW(handle, 0, name, C.byref(length)):
            raise OwnershipError("debug_image_query_failed")
        if not 0 < length.value < len(name):
            raise OwnershipError("debug_image_length_invalid")
        image = C.string_at(C.addressof(name), length.value * 2).decode("utf-16-le")
        if "\0" in image:
            raise OwnershipError("debug_image_contains_nul")
        times = [FileTime() for _ in range(4)]
        budget.remaining()
        if not self.api.GetProcessTimes(handle, *(C.byref(value) for value in times)):
            raise OwnershipError("debug_creation_time_query_failed")
        creation = (times[0].high << 32) | times[0].low
        if not creation:
            raise OwnershipError("debug_creation_time_invalid")
        return image, creation

    def _command_line(self, handle, budget):
        query = self.nt.NtQueryInformationProcess
        required = U32()
        budget.remaining()
        status = query(handle, 60, None, 0, C.byref(required)) & 0xffffffff
        if status not in (0xC0000004, 0xC0000023):
            raise OwnershipError("debug_command_line_size_unavailable")
        header = C.sizeof(UnicodeString)
        if not header < required.value <= header + MAX_COMMAND_BYTES:
            raise OwnershipError("debug_command_line_budget_invalid")
        storage = C.create_string_buffer(required.value)
        returned = U32()
        budget.remaining()
        if query(handle, 60, storage, len(storage), C.byref(returned)) != 0:
            raise OwnershipError("debug_command_line_query_failed")
        if not header <= returned.value <= len(storage):
            raise OwnershipError("debug_command_line_return_length_invalid")
        value = UnicodeString.from_buffer(storage)
        start, end = C.addressof(storage), C.addressof(storage) + returned.value
        if (not value.Buffer or not value.Length or value.Length % 2
                or value.MaximumLength % 2 or value.MaximumLength < value.Length
                or value.Buffer < start + header or value.Buffer + value.MaximumLength > end):
            raise OwnershipError("debug_command_line_buffer_invalid")
        line = C.string_at(value.Buffer, value.Length).decode("utf-16-le")
        if "\0" in line:
            raise OwnershipError("debug_command_line_contains_nul")
        return line

    def _admit(self, pending, budget):
        event = pending.event
        self._connections.add(event.pid)
        if len(self._originals) >= MAX_JOB_PROCESSES:
            raise OwnershipError("debug_admission_budget_exceeded")
        info = event.u.create_process
        self._debug_borrowed[event.pid] = info.hProcess
        if not info.hProcess or not info.hThread:
            raise OwnershipError("debug_create_handles_missing")
        current = self.api.GetCurrentProcess()
        duplicate = HANDLE()
        budget.remaining()
        if not self.api.DuplicateHandle(current, info.hProcess, current,
                                        C.byref(duplicate), 0, False, 2):
            raise OwnershipError("debug_duplicate_failed")
        self._own(duplicate.value, "debug_original")
        row = _Original(event.pid, duplicate.value)
        self._originals.append(row)
        pending.original = row
        if self.process is not None and event.pid == self.process.pid and self._master is None:
            self._master = row
        if self.api.GetProcessId(row.handle) != event.pid:
            raise OwnershipError("debug_event_pid_mismatch")
        previous = self._by_pid.get(event.pid)
        if previous is not None:
            self._refresh_original(previous, budget)
        if previous is not None and not (previous.exit_continued and previous.signaled):
            raise OwnershipError("debug_pid_reused_before_original_terminal")
        self._check_limits(budget)
        self._confirm_member(row.handle, budget)
        row.member = True
        row.image, row.creation_time = self._image_and_time(row.handle, budget)
        identity = (row.pid, row.creation_time)
        if identity in self._identities:
            raise OwnershipError("debug_duplicate_process_identity")
        row.command_line = self._command_line(row.handle, budget)
        row.bound = True
        self._identities.add(identity)
        self._by_pid[row.pid] = row
        self.terminal_handles[row.pid] = row.handle
        if self._admission_phase_closed:
            raise OwnershipError("debug_creation_after_admission_seal")

    def _exception_status(self, row, info):
        code, first = info.record.code, info.first_chance
        if first not in (0, 1):
            raise OwnershipError("debug_exception_chance_invalid")
        initial = code == BREAKPOINT and first == 1 and not row.initial_breakpoint
        if initial:
            row.initial_breakpoint = True
        status = DBG_CONTINUE if initial else DBG_EXCEPTION_NOT_HANDLED
        if len(self._exceptions) >= MAX_JOB_PROCESSES:
            raise OwnershipError("debug_exception_budget_exceeded")
        self._exceptions.append({
            "pid": row.pid, "creation_time": row.creation_time, "code": code,
            "first_chance": bool(first), "initial_breakpoint": initial,
            "continue_status": status,
        })
        return status

    def _event_file(self, event):
        if event.code == CREATE_PROCESS:
            return event.u.create_process.hFile
        if event.code == LOAD_DLL:
            return event.u.load_dll.hFile
        return None

    def _process_event(self, pending, budget):
        event = pending.event
        file = self._event_file(event)
        if file:
            self._own(file, "debug_event_file")
        try:
            if not event.pid or not event.tid:
                raise OwnershipError("debug_event_identifiers_invalid")
            if event.code == CREATE_PROCESS:
                self._admit(pending, budget)
            else:
                self._connections.add(event.pid)
                row = self._by_pid.get(event.pid)
                if self._terminating:
                    row = next((item for item in reversed(self._originals)
                                if item.pid == event.pid), row)
                pending.original = row
                if row is None or not row.bound or not row.create_continued or row.exit_continued:
                    if (self._terminating and event.code == EXIT_PROCESS
                            and event.pid in self._killed_pids):
                        pending.failed = True
                        if row is not None:
                            row.exit_event = event.u.exit_process.code
                        return
                    raise OwnershipError("debug_event_without_live_admission")
                if event.code == EXCEPTION:
                    pending.status = self._exception_status(row, event.u.exception)
                elif event.code == EXIT_PROCESS:
                    if row.exit_event is not None:
                        raise OwnershipError("debug_duplicate_exit_event")
                    row.exit_event = event.u.exit_process.code
                elif event.code not in (CREATE_THREAD, EXIT_THREAD, LOAD_DLL, UNLOAD_DLL, DEBUG_STRING):
                    raise OwnershipError("debug_unknown_or_rip_event")
        finally:
            if file and not self._close_handle(file):
                pending.failed = True

    def _terminate_pending(self, pending):
        pid = pending.event.pid
        if pending.termination_requested or pid in self._killed_pids:
            return True
        handle = pending.original.handle if pending.original else None
        if not handle:
            # Event handles remain borrowed until EXIT is continued. They may
            # unblock failure cleanup, but can never become admission evidence.
            handle = self._debug_borrowed.get(pid)
        if not handle or not self.api.TerminateProcess(handle, 125):
            self._fail("debug_pending_termination_unproven")
            return False
        pending.termination_requested = True
        self._killed_pids.add(pid)
        return True

    def _continue_pending(self, budget):
        pending = self._pending
        row = pending.original
        budget.remaining()
        if pending.failed:
            if not self._terminating or not self._terminate_pending(pending):
                return False
        elif row is None or not row.bound or not row.member:
            raise OwnershipError("debug_continue_without_bound_original")
        if not self.api.ContinueDebugEvent(pending.event.pid, pending.event.tid, pending.status):
            self._fail("debug_continue_failed")
            return False
        self._continued += 1
        if row is not None:
            if pending.event.code == CREATE_PROCESS:
                row.create_continued = True
            elif pending.event.code == EXIT_PROCESS:
                row.exit_continued = True
        if pending.event.code == EXIT_PROCESS:
            self._connections.discard(pending.event.pid)
            self._debug_borrowed.pop(pending.event.pid, None)
            self._killed_pids.discard(pending.event.pid)
        self._pending = None
        # EXIT is only an event. The original object may become signaled later.
        if row is not None and pending.event.code == EXIT_PROCESS:
            self._refresh_original(row, budget)
        return True

    def pump_debug(self, budget):
        """Drain a finite slice without a native blocking event wait."""
        return self._pump(budget, strict=True)

    def _pump(self, budget, *, strict):
        self._require_thread()
        if self._pumping or self._closed or budget is None:
            return 0
        if self._budget is not None:
            budget = _SharedBudget(self._budget, budget)
        self._pumping = True
        end, count = time.monotonic() + PUMP_SECONDS, 0
        try:
            while count < MAX_EVENTS_PER_PUMP and time.monotonic() < end:
                budget.remaining()
                if self._pending is None:
                    event = DebugEvent()
                    if not self.api.WaitForDebugEvent(C.byref(event), 0):
                        if self.last_error() != ERROR_SEM_TIMEOUT:
                            self._fail("debug_event_wait_failed")
                        break
                    self._events += 1
                    status = DBG_EXCEPTION_NOT_HANDLED if event.code == EXCEPTION else DBG_CONTINUE
                    self._pending = _Pending(event, status=status)
                pending = self._pending
                if not pending.processed:
                    pending.processed = True
                    try:
                        self._process_event(pending, budget)
                    except (OwnershipError, OSError, UnicodeError) as exc:
                        pending.failed = True
                        self._fail(str(exc) if isinstance(exc, OwnershipError)
                                   else "debug_identity_read_failed")
                if not self._continue_pending(budget):
                    break
                count += 1
            self._refresh_originals(budget, end=end, limit=MAX_REFRESH_PER_PUMP)
        except TimeoutError:
            self._fail("debug_original_deadline_expired")
            if strict:
                raise
        finally:
            self._pumping = False
        if strict:
            self._raise_errors()
        return count

    def _refresh_original(self, row, budget):
        if row.exit_code is not None:
            return
        budget.remaining()
        status = self.api.WaitForSingleObject(row.handle, 0)
        if status == WAIT_TIMEOUT:
            return
        if status != WAIT_OBJECT_0:
            self._fail("debug_original_wait_failed")
            return
        row.signaled = True
        code = U32()
        if not self.api.GetExitCodeProcess(row.handle, C.byref(code)):
            self._fail("debug_original_exit_query_failed")
            return
        row.exit_code = code.value
        if row.exit_continued and code.value == row.exit_event:
            row.natural = not self._terminating
        elif row.exit_continued:
            self._fail("debug_exit_code_disagrees_with_original")

    def _refresh_originals(self, budget, *, end=None, limit=None):
        count = len(self._originals)
        for _ in range(min(count, limit if limit is not None else count)):
            if end is not None and time.monotonic() >= end:
                break
            self._refresh_cursor %= count
            row = self._originals[self._refresh_cursor]
            self._refresh_cursor += 1
            self._refresh_original(row, budget)

    def _census(self, budget):
        before = self.accounting(budget)
        pids = self.process_ids(budget)
        after = self.accounting(budget)
        self._last_total, self._last_active = after.TotalProcesses, after.ActiveProcesses
        complete = (
            before.TotalProcesses == after.TotalProcesses == len(self._originals)
            and all(row.bound and row.member for row in self._originals)
            and all(pid in self._by_pid for pid in pids)
            and before.ActiveProcesses == after.ActiveProcesses == len(pids)
        )
        if self.terminal_prepared and after.TotalProcesses != self.terminal_total:
            complete = False
        self._census_complete = complete
        if not complete:
            self._fail("debug_admissions_job_census_incomplete")
        return complete

    def retain_current_members(self, deadline):
        """Never reopen a PID: only original CREATE-event duplicates are usable."""
        deadline = _SharedBudget(self._budget, deadline)
        self.pump_debug(deadline)
        self._census(deadline)
        self._raise_errors()

    def prepare_terminal(self, deadline):
        """Freeze cumulative admission before requesting any forced cleanup."""
        if self.terminal_prepared:
            return
        deadline = _SharedBudget(self._budget, deadline)
        self.retain_current_members(deadline)
        total = self._last_total
        limits = self._check_limits(deadline)
        self._admission_phase_closed = True
        limits.BasicLimitInformation.LimitFlags |= 0x0008
        limits.BasicLimitInformation.ActiveProcessLimit = 1
        installed = bool(self.api.SetInformationJobObject(
            self.job, 9, C.byref(limits), C.sizeof(limits)))
        if not installed:
            self._fail("debug_cleanup_admission_barrier_failed")
        checked = self._check_limits(deadline).BasicLimitInformation
        self.admission_closed = (installed and bool(checked.LimitFlags & 0x0008)
                                 and checked.ActiveProcessLimit == 1)
        if not self.admission_closed:
            self._fail("debug_cleanup_admission_barrier_unconfirmed")
        self.retain_current_members(deadline)
        if total != self._last_total:
            self._fail("debug_admission_changed_during_cleanup_barrier")
        self._raise_errors()
        self.terminal_total = total
        self.terminal_prepared = True

    def seal_admission(self, budget, *, settle_seconds=1.0):
        """Settle natural non-master exits, then close admission with a live master.

        This explicit phase is for a parent that has already completed and
        disconnected its clients. It cannot prove absence of unrelated external
        broker processes. No cleanup-induced exit counts as natural.
        """
        if not math.isfinite(settle_seconds) or not 0 <= settle_seconds <= 5:
            raise ValueError("natural settle must be finite and at most five seconds")
        budget = _SharedBudget(self._budget, budget)
        try:
            end = time.monotonic() + min(settle_seconds, budget.remaining(reserve=CLEANUP_RESERVE))
        except TimeoutError:
            self._fail("debug_natural_seal_deadline_expired")
            raise
        while True:
            self.pump_debug(budget)
            self._census(budget)
            self._raise_errors()
            if self._master is None or self._master.signaled or self._terminating:
                self._fail("debug_live_master_unproven_at_seal")
                self._raise_errors()
            if all(row.natural for row in self._originals if row is not self._master):
                if self._last_active == 1 and self.process_ids(budget) == [self._master.pid]:
                    break
            allowance = min(end - time.monotonic(), budget.remaining(reserve=CLEANUP_RESERVE))
            if allowance <= 0:
                self._fail("debug_non_master_natural_settle_expired")
                self._raise_errors()
            time.sleep(min(0.001, allowance))
        self.prepare_terminal(budget)
        if self._last_active != 1 or not all(
                row.natural for row in self._originals if row is not self._master):
            self._fail("debug_natural_seal_snapshot_changed")
            self._raise_errors()
        return self.terminal_evidence(budget)

    def terminate(self):
        """Request termination and continue exit events within the same budget."""
        self._require_thread()
        self._admission_phase_closed = True
        self._pump(self._budget, strict=False)
        self._terminating = True
        if self.job and not self.api.TerminateJobObject(self.job, 125):
            self._fail("debug_job_termination_failed")
            self._terminate_originals()
        else:
            self._killed_pids.update(row.pid for row in self._originals
                                     if row.member and not row.exit_continued)
        if self._pending and self._pending.failed:
            self._terminate_pending(self._pending)
        self._pump(self._budget, strict=False)

    force = terminate

    def _terminate_originals(self):
        handles = [row.handle for row in self._originals if not row.signaled]
        if self.process is not None and self._master is None and self.process._handle:
            handles.append(self.process._handle)
        for handle in dict.fromkeys(handles):
            if not self.api.TerminateProcess(handle, 125):
                self._fail("debug_original_termination_failed")
                continue
            pid = self.api.GetProcessId(handle)
            self._killed_pids.add(pid)

    def active(self, deadline):
        deadline = _SharedBudget(self._budget, deadline)
        self._pump(deadline, strict=False)
        self._census(deadline)
        self._refresh_originals(deadline)
        pending = sum(not row.signaled or row.exit_code is None or not row.exit_continued
                      for row in self._originals)
        self.terminal_pending = pending
        self._raise_errors()
        return max(self._last_active, pending, len(self._connections), int(self._pending is not None))

    def original_wait_handles(self):
        """Stable handles, including earlier original objects after PID reuse."""
        roots = [process._handle for process in self.processes if process._handle]
        return list(dict.fromkeys(roots + [row.handle for row in self._originals]))

    def wait_retained(self, deadline):
        """Failure cleanup continues debug events instead of blocking on EXIT."""
        deadline = _SharedBudget(self._budget, deadline)
        while True:
            self._pump(deadline, strict=False)
            self._refresh_originals(deadline)
            if (self._pending is not None and self._pending.failed
                    and not self._terminate_pending(self._pending)):
                raise OwnershipError("debug_failure_event_cannot_be_safely_continued")
            if not self._connections and self._pending is None and all(
                    row.signaled and row.exit_code is not None for row in self._originals):
                return
            time.sleep(min(0.001, deadline.remaining()))

    def _close_debug_connections(self):
        # Detach is only a last resource-release attempt after a kill request.
        # It is always HOLD, never an alternative terminal or natural proof.
        for pid in tuple(self._connections):
            self._fail("debug_detach_required_after_cleanup_failure")
            row = self._by_pid.get(pid)
            handle = row.handle if row is not None else None
            if self._pending is not None and self._pending.event.pid == pid:
                pending = self._pending
                if pending.original is not None:
                    handle = pending.original.handle
                elif pending.event.code == CREATE_PROCESS:
                    handle = pending.event.u.create_process.hProcess
            # Never act on a PID that might now identify an unrelated process.
            if not handle or self.api.WaitForSingleObject(handle, 0) != WAIT_TIMEOUT:
                self._fail("debug_detach_original_identity_unproven")
                continue
            if self.api.DebugActiveProcessStop(pid):
                self._detached += 1
                self._connections.discard(pid)
            else:
                self._fail("debug_detach_failed")

    def close(self):
        """Pump shutdown, then close owned handles; unresolved cleanup raises."""
        if self._closed:
            return
        try:
            self._require_thread()
            if self._budget is not None:
                self.terminate()
                self.wait_retained(self._budget)
                if self.job:
                    self._census(self._budget)
        except (OwnershipError, OSError, TimeoutError):
            self._fail("debug_cleanup_terminal_unproven")
        finally:
            # Last kill-on-close guarantee before any failure-path detach.
            if getattr(self, "job", None):
                if self.job not in self._owned:
                    self._own(self.job, "job")
                if self._close_handle(self.job):
                    self.job = None
            if self._thread is None or threading.get_ident() == self._thread:
                self._close_debug_connections()
            self._update_evidence()
            # Event process/thread handles were never registered as owned.
            for handle in tuple(self._owned):
                self._close_handle(handle)
            if self.job not in self._owned:
                self.job = None
            for process in getattr(self, "processes", ()):
                process.thread_handle = process._handle = None
            self.terminal_handles.clear()
            self._closed = not self._owned and not self._connections
            if self._closed and getattr(_DEBUG_THREAD, "owner", None) is self:
                del _DEBUG_THREAD.owner
            self._update_evidence()
        self._raise_errors()

    def _update_evidence(self):
        rows = [self._snapshot(row) for row in self._originals]
        terminal = (bool(rows) and self._census_complete and self._last_active == 0
                    and self._pending is None and not self._connections
                    and all(row.signaled and row.exit_continued and row.exit_code is not None
                            and row.exit_code == row.exit_event for row in self._originals))
        self._evidence = {
            "evidence_kind": "NATIVE_WINDOWS" if self._native else "OFFLINE_CONTROL_NOT_NATIVE",
            "state": "PASS" if self._native and terminal and not self._errors else "HOLD",
            "terminal_proven": bool(terminal and not self._errors),
            "admissions": rows, "job_total_processes": self._last_total,
            "job_active_processes": self._last_active,
            "complete_original_object_census": self._census_complete and not self._errors,
            "admission_closed": self.admission_closed,
            "admission_phase_closed": self._admission_phase_closed,
            "identities_bound": all(row.bound for row in self._originals) and bool(rows),
            "pending_process_objects": sum(not row.signaled for row in self._originals),
            "debug_events_received": self._events, "debug_events_continued": self._continued,
            "pending_debug_event": self._pending is not None,
            "debug_connections": len(self._connections), "fallback_detaches": self._detached,
            "owned_handles_remaining": len(self._owned), "closed": self._closed,
            "exceptions": [dict(row) for row in self._exceptions], "errors": sorted(self._errors),
        }
        return self._evidence

    def terminal_evidence(self, budget=None):
        """Refresh the full census; unknown/missed events remain explicit HOLD."""
        if not self._closed and self._budget is not None:
            budget = _SharedBudget(self._budget, budget or self._budget)
            try:
                self._pump(budget, strict=False)
                if self.job:
                    self._census(budget)
                self._refresh_originals(budget)
            except (OwnershipError, OSError, TimeoutError):
                self._fail("debug_terminal_evidence_unavailable")
        return self._update_evidence()
