#!/usr/bin/env python3
"""Owned CDB application-suite probe; publishes bounded text, never memory dumps.

The Windows process handle, opened before initial ``g``, supplies the original
test exit status. CDB's own exit status and text are diagnostic evidence only.
"""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import secrets
import signal
import subprocess
import sys
import threading
import time

"""Bounded owned preparation trees; Windows APIs are UNRUN until native controls.

Windows children are created suspended, assigned to a kill-on-close Job, then
resumed using the retained primary-thread handle. No command can start before
its Job assignment. The POSIX process-group backend is for real offline controls,
not Windows emulation. Jobs cover ordinary CreateProcess descendants, not broker
or WMI-created unrelated processes; this is lifecycle ownership, not a sandbox.
"""
import ctypes
from contextlib import contextmanager
from ctypes import wintypes
import os
import queue
import signal
import subprocess
import threading
import time

CAPTURE_LIMIT = 8 * 1024 * 1024
CLEANUP_RESERVE = 5


class OwnershipError(Exception):
    """A process could not be owned or reach its required terminal state."""


class Pump:
    """Drain one pipe without unbounded communicate or unbounded memory."""

    def __init__(self, stream):
        self.stream = stream
        self.chunks = queue.Queue(maxsize=512)
        self.done = threading.Event()
        self.overflow = threading.Event()
        self.failed = threading.Event()
        self.thread = threading.Thread(target=self.read, daemon=True)
        self.thread.start()

    def read(self):
        try:
            while data := self.stream.read1(4096):
                try:
                    self.chunks.put_nowait(data)
                except queue.Full:
                    self.overflow.set()
        except OSError:
            self.failed.set()
        finally:
            self.done.set()

    def drain(self, capture, deadline):
        # A live producer cannot turn a drain into an unbounded loop.
        for _ in range(512):
            deadline.remaining()
            try:
                capture(self.chunks.get_nowait())
            except queue.Empty:
                return

    def complete(self):
        return self.done.is_set() and self.chunks.empty()


class PosixOwner:
    """Actual isolated POSIX groups for offline descendant lifecycle controls."""

    def __init__(self):
        self.process = None
        self.closed = False

    def spawn(self, command, cwd, env, deadline, merge_error=False):
        deadline.remaining(reserve=CLEANUP_RESERVE)
        self.process = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE,
                                        stderr=subprocess.STDOUT if merge_error else subprocess.PIPE,
                                        start_new_session=True)
        return self.process

    def terminate(self):
        if self.process is not None:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass

    def force(self):
        if self.process is not None:
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def active(self, deadline):
        if self.process is None:
            return 0
        # A signalled group can contain terminal zombies until their OS parent
        # reaps them. They are reported separately by controls and are not live
        # owned work. ps is bounded; only pid/pgid/state are requested.
        result = subprocess.run(["ps", "-axo", "pid=,pgid=,stat="],
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                timeout=min(0.5, deadline.remaining()), text=True, check=True)
        return sum(1 for row in result.stdout.splitlines()
                   if len(fields := row.split()) == 3
                   and fields[1] == str(self.process.pid) and not fields[2].startswith("Z"))

    def close(self):
        self.closed = True


class WindowsProcess:
    """Small process/pipe interface retaining both native creation handles."""

    def __init__(self, api, pid, handle, thread, stdin, stdout, stderr):
        self.api = api
        self.pid = pid
        self._handle = handle
        self.thread_handle = thread
        self.stdin, self.stdout, self.stderr = stdin, stdout, stderr
        self.returncode = None
        self.owned_started = False

    def poll(self):
        if self.returncode is not None:
            return self.returncode
        status = self.api.WaitForSingleObject(self._handle, 0)
        if status == 258:
            return None
        if status != 0:
            raise OwnershipError("process wait failed")
        code = wintypes.DWORD()
        if not self.api.GetExitCodeProcess(self._handle, ctypes.byref(code)):
            raise OwnershipError("process exit query failed")
        self.returncode = code.value
        return self.returncode

    def wait(self, timeout):
        status = self.api.WaitForSingleObject(self._handle, max(0, int(timeout * 1000)))
        if status == 258:
            raise subprocess.TimeoutExpired("owned-native-process", timeout)
        if status != 0:
            raise OwnershipError("process wait failed")
        return self.poll()

    def kill(self):
        if self.poll() is None and not self.api.TerminateProcess(self._handle, 125):
            raise OwnershipError("process termination failed")

    def resume(self):
        if not self.thread_handle:
            raise OwnershipError("primary thread handle missing")
        previous = self.api.ResumeThread(self.thread_handle)
        self.api.CloseHandle(self.thread_handle)
        self.thread_handle = None
        if previous != 1:
            raise OwnershipError("primary suspend count was not exactly one")
        self.owned_started = True

    def close_handles(self):
        for handle in [self.thread_handle, self._handle]:
            if handle:
                self.api.CloseHandle(handle)
        self.thread_handle = self._handle = None


class WindowsJob:
    """Own a command before execution and its ordinary CreateProcess descendants."""

    def __init__(self):
        self.api = ctypes.WinDLL("kernel32", use_last_error=True)
        api = self.api
        for name, args, result in [
            ("CreateJobObjectW", [ctypes.c_void_p, wintypes.LPCWSTR], wintypes.HANDLE),
            ("SetInformationJobObject", [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD], wintypes.BOOL),
            ("AssignProcessToJobObject", [wintypes.HANDLE, wintypes.HANDLE], wintypes.BOOL),
            ("IsProcessInJob", [wintypes.HANDLE, wintypes.HANDLE, ctypes.POINTER(wintypes.BOOL)], wintypes.BOOL),
            ("OpenProcess", [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
            ("WaitForSingleObject", [wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            ("GetExitCodeProcess", [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL),
            ("TerminateProcess", [wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
            ("TerminateJobObject", [wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
            ("QueryInformationJobObject", [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p], wintypes.BOOL),
            ("CloseHandle", [wintypes.HANDLE], wintypes.BOOL),
            ("ResumeThread", [wintypes.HANDLE], wintypes.DWORD),
            ("InitializeProcThreadAttributeList", [ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, ctypes.POINTER(ctypes.c_size_t)], wintypes.BOOL),
            ("UpdateProcThreadAttribute", [ctypes.c_void_p, wintypes.DWORD, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p], wintypes.BOOL),
            ("DeleteProcThreadAttributeList", [ctypes.c_void_p], None),
            ("CreateProcessW", [wintypes.LPCWSTR, wintypes.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, wintypes.BOOL, wintypes.DWORD, ctypes.c_void_p, wintypes.LPCWSTR, ctypes.c_void_p, ctypes.c_void_p], wintypes.BOOL),
        ]:
            function = getattr(api, name)
            function.argtypes, function.restype = args, result
        class BasicLimits(ctypes.Structure):
            _fields_ = [("PerProcessUserTimeLimit", ctypes.c_longlong), ("PerJobUserTimeLimit", ctypes.c_longlong),
                        ("LimitFlags", wintypes.DWORD), ("MinimumWorkingSetSize", ctypes.c_size_t),
                        ("MaximumWorkingSetSize", ctypes.c_size_t), ("ActiveProcessLimit", wintypes.DWORD),
                        ("Affinity", ctypes.c_size_t), ("PriorityClass", wintypes.DWORD), ("SchedulingClass", wintypes.DWORD)]
        class ExtendedLimits(ctypes.Structure):
            _fields_ = [("BasicLimitInformation", BasicLimits), ("IoInfo", ctypes.c_ulonglong * 6),
                        ("ProcessMemoryLimit", ctypes.c_size_t), ("JobMemoryLimit", ctypes.c_size_t),
                        ("PeakProcessMemoryUsed", ctypes.c_size_t), ("PeakJobMemoryUsed", ctypes.c_size_t)]
        self.job = api.CreateJobObjectW(None, None)
        self.processes = []
        self.process = None
        self.extra_handles = []
        if not self.job:
            raise OwnershipError("CreateJobObject failed")
        limits = ExtendedLimits()
        limits.BasicLimitInformation.LimitFlags = 0x2000  # KILL_ON_JOB_CLOSE, no breakaway
        if not api.SetInformationJobObject(self.job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            self.close()
            raise OwnershipError("SetInformationJobObject failed")

    def assign(self, handle):
        present = wintypes.BOOL()
        if not self.api.IsProcessInJob(handle, self.job, ctypes.byref(present)):
            raise OwnershipError("IsProcessInJob failed")
        if not present and not self.api.AssignProcessToJobObject(self.job, handle):
            raise OwnershipError("AssignProcessToJobObject failed")

    def spawn(self, command, cwd, env, deadline, merge_error=False):
        """Create suspended with an explicit inheritable pipe-handle allowlist."""
        import msvcrt
        deadline.remaining(reserve=CLEANUP_RESERVE)
        api = self.api
        class Startup(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("lpReserved", wintypes.LPWSTR),
                        ("lpDesktop", wintypes.LPWSTR), ("lpTitle", wintypes.LPWSTR),
                        ("dwX", wintypes.DWORD), ("dwY", wintypes.DWORD),
                        ("dwXSize", wintypes.DWORD), ("dwYSize", wintypes.DWORD),
                        ("dwXCountChars", wintypes.DWORD), ("dwYCountChars", wintypes.DWORD),
                        ("dwFillAttribute", wintypes.DWORD), ("dwFlags", wintypes.DWORD),
                        ("wShowWindow", wintypes.WORD), ("cbReserved2", wintypes.WORD),
                        ("lpReserved2", ctypes.c_void_p), ("hStdInput", wintypes.HANDLE),
                        ("hStdOutput", wintypes.HANDLE), ("hStdError", wintypes.HANDLE)]
        class StartupEx(ctypes.Structure):
            _fields_ = [("StartupInfo", Startup), ("lpAttributeList", ctypes.c_void_p)]
        class ProcessInfo(ctypes.Structure):
            _fields_ = [("hProcess", wintypes.HANDLE), ("hThread", wintypes.HANDLE),
                        ("dwProcessId", wintypes.DWORD), ("dwThreadId", wintypes.DWORD)]
        stdin_read, stdin_write = os.pipe()
        stdout_read, stdout_write = os.pipe()
        stderr_read, stderr_write = (None, None) if merge_error else os.pipe()
        child_fds = [stdin_read, stdout_write] + ([] if merge_error else [stderr_write])
        fds = set(child_fds + [stdin_write, stdout_read] + ([] if merge_error else [stderr_read]))
        attribute = None
        attribute_initialized = False
        process = None
        information = ProcessInfo()
        created = False
        opened = []
        try:
            for fd in child_fds:
                os.set_inheritable(fd, True)
            handles = (wintypes.HANDLE * len(child_fds))(*(msvcrt.get_osfhandle(fd) for fd in child_fds))
            size = ctypes.c_size_t()
            api.InitializeProcThreadAttributeList(None, 1, 0, ctypes.byref(size))
            if not size.value:
                raise OwnershipError("attribute list size unavailable")
            storage = ctypes.create_string_buffer(size.value)
            attribute = ctypes.cast(storage, ctypes.c_void_p)
            if not api.InitializeProcThreadAttributeList(attribute, 1, 0, ctypes.byref(size)):
                raise OwnershipError("attribute list initialization failed")
            attribute_initialized = True
            if not api.UpdateProcThreadAttribute(attribute, 0, 0x20002, ctypes.byref(handles), ctypes.sizeof(handles), None, None):
                raise OwnershipError("explicit pipe handle list failed")
            startup = StartupEx()
            startup.StartupInfo.cb = ctypes.sizeof(startup)
            startup.StartupInfo.dwFlags = 0x100  # STARTF_USESTDHANDLES
            startup.StartupInfo.hStdInput = msvcrt.get_osfhandle(stdin_read)
            startup.StartupInfo.hStdOutput = msvcrt.get_osfhandle(stdout_write)
            startup.StartupInfo.hStdError = msvcrt.get_osfhandle(stdout_write if merge_error else stderr_write)
            startup.lpAttributeList = attribute
            line = ctypes.create_unicode_buffer(subprocess.list2cmdline(command))
            block = ctypes.create_unicode_buffer("\0".join(f"{key}={value}" for key, value in sorted(env.items(), key=lambda item: item[0].upper())) + "\0\0")
            flags = 0x00000004 | 0x00000400 | 0x00080000  # SUSPENDED / UNICODE_ENV / EXTENDED_STARTUPINFO
            if not api.CreateProcessW(None, line, None, None, True, flags, block, str(cwd), ctypes.byref(startup), ctypes.byref(information)):
                raise OwnershipError("CreateProcessW failed")
            created = True
            # Retain the native handles before wrapping pipes or assigning.
            process = WindowsProcess(api, information.dwProcessId, information.hProcess,
                                     information.hThread, None, None, None)
            self.process = process
            self.processes.append(process)
            for fd in child_fds:
                os.close(fd)
                fds.remove(fd)
            process.stdin = os.fdopen(stdin_write, "wb")
            opened.append(process.stdin)
            fds.remove(stdin_write)
            process.stdout = os.fdopen(stdout_read, "rb")
            opened.append(process.stdout)
            fds.remove(stdout_read)
            if stderr_read is not None:
                process.stderr = os.fdopen(stderr_read, "rb")
                opened.append(process.stderr)
                fds.remove(stderr_read)
            # The primary thread has never executed command work, so assignment
            # failure cannot leave a command-created descendant outside the Job.
            self.assign(process._handle)
            deadline.remaining(reserve=CLEANUP_RESERVE)
            process.resume()
            return process
        except BaseException:
            # A pre-assignment target is still suspended and cannot have run any
            # command work. Its retained handle supplies bounded direct cleanup.
            if process is not None:
                process.kill()
                process.wait(timeout=deadline.remaining())
            for stream in opened:
                stream.close()
            raise
        finally:
            # Even an interruption between CreateProcessW returning and Python
            # root registration must release the still-suspended native root.
            if process is None and (created or information.hProcess):
                api.TerminateProcess(information.hProcess, 125)
                try:
                    allowance = deadline.remaining()
                except TimeoutError:
                    allowance = 0
                api.WaitForSingleObject(information.hProcess, max(0, int(allowance * 1000)))
                api.CloseHandle(information.hThread)
                api.CloseHandle(information.hProcess)
            if attribute_initialized:
                api.DeleteProcThreadAttributeList(attribute)
            for fd in fds:
                os.close(fd)

    def terminate(self):
        if not self.api.TerminateJobObject(self.job, 125):
            raise OwnershipError("TerminateJobObject failed")

    force = terminate

    def active(self, deadline):
        deadline.remaining()
        values = (ctypes.c_ulonglong * 6)()
        if not self.api.QueryInformationJobObject(self.job, 1, ctypes.byref(values), ctypes.sizeof(values), None):
            raise OwnershipError("Job accounting query failed")
        return ctypes.cast(values, ctypes.POINTER(wintypes.DWORD))[10]

    def close(self):
        if self.job:
            self.api.CloseHandle(self.job)  # last kill-on-close guarantee
            self.job = None
        for process in self.processes:
            process.close_handles()
        for handle in self.extra_handles:
            self.api.CloseHandle(handle)
        self.extra_handles.clear()


def owner_for_host():
    return WindowsJob() if os.name == "nt" else PosixOwner()


@contextmanager
def deferred_interrupts():
    """An interruption requests cleanup; repeated SIGINT cannot bypass cleanup."""
    previous = None
    if threading.current_thread() is threading.main_thread():
        previous = signal.signal(signal.SIGINT, signal.SIG_IGN)
    try:
        yield
    finally:
        if previous is not None:
            signal.signal(signal.SIGINT, previous)


def cleanup(owner, process, pumps, deadline, capture):
    """All grace, force, draining and reap remain inside the original deadline."""
    errors = set()
    owner.terminate()
    grace_end = min(time.monotonic() + 0.4, deadline.end)
    while process.poll() is None and time.monotonic() < grace_end:
        for pump in pumps:
            pump.drain(capture[pump], deadline)
        time.sleep(min(0.01, deadline.remaining()))
    if owner.active(deadline):
        owner.force()
    if process.poll() is None:
        process.kill()  # also covers an initial debugger-assignment failure
    process.wait(timeout=deadline.remaining())
    while True:
        for pump in pumps:
            pump.drain(capture[pump], deadline)
        active = owner.active(deadline)
        if not active and all(pump.complete() for pump in pumps):
            break
        time.sleep(min(0.01, deadline.remaining()))
    for pump in pumps:
        pump.thread.join(timeout=deadline.remaining())
        if pump.thread.is_alive() or pump.failed.is_set() or pump.overflow.is_set():
            errors.add("owned_pipe_terminal_failed")
    return {"direct_reaped": process.returncode is not None, "active_processes": active,
            "pipes_drained": all(pump.complete() for pump in pumps), "errors": sorted(errors)}


def finish_owned(owner, process, pumps, deadline, capture):
    """Close the Job even if an API/pipe/reap check fails; never extend budget."""
    errors = set()
    terminal = None
    with deferred_interrupts():
        try:
            terminal = cleanup(owner, process, pumps, deadline, capture)
            errors.update(terminal["errors"])
        except (OSError, OwnershipError, TimeoutError, subprocess.SubprocessError):
            errors.add("owned_cleanup_failed")
            try:
                owner.force()
            except (OSError, OwnershipError, subprocess.SubprocessError):
                errors.add("owned_fallback_force_failed")
            try:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=deadline.remaining())
            except (OSError, OwnershipError, TimeoutError, subprocess.SubprocessError):
                errors.add("owned_fallback_reap_failed")
        finally:
            owner.close()  # Windows final-handle close kills ordinary descendants.
            for stream in [process.stdin, process.stdout, process.stderr]:
                if stream and not any(pump.stream is stream and pump.thread.is_alive() for pump in pumps):
                    stream.close()
    return terminal, errors


def run_owned(command, cwd, env, deadline, owner=None, tick=None):
    """Capture preparation privately and establish an owned terminal before return.

    tick is a control hook used to deliver an actual SIGINT in isolated tests.
    It is absent from application workflow calls.
    """
    owner = owner if owner is not None else owner_for_host()
    process = None
    pumps = []
    output = bytearray()
    error_output = bytearray()
    errors = set()
    observed = None
    timed_out = interrupted = False
    terminal = None
    def retain(destination, data):
        available = CAPTURE_LIMIT - len(destination)
        destination.extend(data[:max(0, available)])
        if len(data) > available:
            errors.add("private_capture_budget_exceeded")
    capture = {}
    try:
        process = owner.spawn(command, cwd, env, deadline)
        for stream, destination in [(process.stdout, output), (process.stderr, error_output)]:
            pump = Pump(stream)
            pumps.append(pump)
            capture[pump] = lambda data, destination=destination: retain(destination, data)
        while True:
            if (code := process.poll()) is not None:
                observed = code
            deadline.remaining(reserve=CLEANUP_RESERVE)
            if tick:
                tick(process)
            for pump in pumps:
                pump.drain(capture[pump], deadline)
            if observed is not None and all(pump.complete() for pump in pumps):
                break
            time.sleep(min(0.01, deadline.remaining(reserve=CLEANUP_RESERVE)))
    except TimeoutError:
        timed_out = True
    except KeyboardInterrupt:
        interrupted = True
    except (OSError, OwnershipError, subprocess.SubprocessError):
        errors.add("owned_preparation_failed")
    finally:
        with deferred_interrupts():
            # spawn may be interrupted after native creation and before returning.
            # Each backend retains its root as soon as it exists.
            process = process if process is not None else owner.process
            if process is not None:
                try:
                    if getattr(process, "owned_started", True) and (code := process.poll()) is not None:
                        observed = code
                except (OSError, OwnershipError, subprocess.SubprocessError):
                    errors.add("owned_exit_query_failed")
                terminal, cleanup_errors = finish_owned(owner, process, pumps, deadline, capture)
                errors.update(cleanup_errors)
            else:
                owner.close()
    # An actual failed command is retained even if later pipe/cleanup diagnostics
    # failed. A forced cleanup125 is never relabelled as an original result.
    code = observed if observed not in (None, 0) else (130 if interrupted else 124 if timed_out else 125 if errors or observed is None else 0)
    receipt = {"observed_exit": observed, "controller_exit": code, "timed_out": timed_out,
               "interrupted": interrupted, "terminal": terminal, "errors": sorted(errors)}
    return subprocess.CompletedProcess(command, code, output.decode("utf-8", errors="replace"),
                                       error_output.decode("utf-8", errors="replace")), receipt


SOURCE_SHA = "a717400bf791bad43a293db50a02a64fb58bcfc7"
TOOLCHAIN = "1.98.1"
TEST_COUNT = 706
JOB_SECONDS = 90 * 60
DIAGNOSTIC_ERROR = 125
TIMEOUT = 124
MAX_TEXT = 2 * 1024 * 1024
MAX_LINE = 16 * 1024


class ProbeError(Exception):
    """The diagnostic cannot establish its required evidence."""


class Deadline:
    """One absolute budget covers preparation, execution, and cleanup."""

    def __init__(self, epoch, clock=time.monotonic, wall=time.time):
        remaining = epoch - wall()
        if not 0 < remaining <= JOB_SECONDS:
            raise ProbeError("deadline is expired or exceeds the original 90-minute budget")
        self.end = clock() + remaining
        self.clock = clock

    def remaining(self, reserve=0):
        value = self.end - self.clock() - reserve
        if value <= 0:
            raise TimeoutError("original job budget exhausted")
        return value


def result_code(target, diagnostic_ok, timed_out=False):
    """An observed failure always wins over a secondary diagnostic failure."""
    if target is not None and target != 0:
        return target
    if timed_out:
        return TIMEOUT
    return 0 if target == 0 and diagnostic_ok else DIAGNOSTIC_ERROR


def signed_exit(code):
    """CPython sys.exit accepts signed C ints; preserve the Windows DWORD bits."""
    return code if code < 0x80000000 else code - 0x100000000


def test_names(text):
    names = re.findall(r"^([A-Za-z0-9_:]+): test$", text, re.MULTILINE)
    if len(names) != TEST_COUNT or len(set(names)) != TEST_COUNT:
        raise ProbeError("application list must contain exactly 706 distinct tests")
    return frozenset(names)


def application_artifact(text):
    artifacts = []
    for line in text.splitlines():
        item = json.loads(line)
        if (item.get("reason") == "compiler-artifact"
                and item.get("target", {}).get("name") == "keelshell-app"
                and item.get("profile", {}).get("test") is True
                and item.get("executable")):
            artifacts.append(item["executable"])
    if len(artifacts) != 1:
        raise ProbeError("build must identify one application test executable")
    return Path(artifacts[0]).resolve()


def debugger_script(nonce):
    """Observe both chances for c0000409; all continuations remain unhandled."""
    def diagnostic(chance):
        return (f".echo KEEL_{nonce}_NATIVE_BEGIN_{chance}; .lastevent; .exr -1; "
                ".ecxr; lm a @$ip; kn 0x40; ~* kn 0x40; ~#s; "
                f".echo KEEL_{nonce}_NATIVE_END_{chance}; gn")
    second = diagnostic("SECOND")
    events = "* asrt av dm dz c000008e eh gp ii iov ip isc lsq sbo sov wkd aph 3c ch clr".split()
    commands = [".outmask- 0x80", "sxi out"]
    commands += [f'sxd -c2 "{second}" {event}' for event in events]
    # -c is evaluated on first chance even with second-chance break status.
    # Fail-fast bypasses handlers and can produce only second chance; an ordinary
    # raised c0000409 can expose both. The observed .lastevent must match the hook.
    commands += [f'sxd -c "{diagnostic("FIRST")}" -c2 "{second}" 0xc0000409',
                 f'.printf "KEEL_{nonce}_PID:%d\\n", @$tpid']
    return "\n".join(commands) + "\n"


MODULE_ENUM = {name: name.upper() for name in (
    "ntdll", "kernelbase", "kernel32", "ucrtbase", "vcruntime140", "vcruntime140_1",
    "msvcp140", "d3d11", "d3d12", "dxgi", "ws2_32", "win32u", "user32", "gdi32", "gdi32full")}
HEX64 = r"(?:[0-9a-fA-F]{1,16}|[0-9a-fA-F]{1,8}`[0-9a-fA-F]{1,8})"
HEX32 = r"[0-9a-fA-F]{1,8}"
LIMITATIONS = ["SHARED_STREAM_NOT_SOURCE_AUTHENTICATED", "TRUSTED_OWNED_TEST_TARGET_REQUIRED",
               "NUMERIC_STACK_FRAMES_WITHOUT_SYMBOL_TEXT", "FIXED_MODULE_ENUM_ONLY",
               "RAW_TARGET_DEBUGGER_TEXT_PRIVATE_NOT_UPLOADED"]


class TextEvidence:
    """Project strict numeric/fixed fields; a nonce never proves output origin.

    The raw shared stream is bounded and private. Arbitrary text, symbols,
    descriptions, register strings and module paths are never upload fields.
    Unknown or ambiguous lines invalidate an event; they are not relabelled as
    debugger evidence. Numeric projections are not authentication against an
    actively malicious target encoding data as numbers.
    """

    def __init__(self, nonce, names, app_module=None, raw_path=None):
        self.nonce, self.names = nonce, names
        self.allowed_modules = dict(MODULE_ENUM)
        if app_module is not None and re.fullmatch(r"keelshell_app-[0-9a-f]+|python(?:[0-9]+)?", app_module, re.I):
            self.allowed_modules[app_module.lower()] = "OWNED_EXECUTABLE"
        self.pid = None
        self.harness = []
        self.events = []
        self.current = None
        self.stack = None
        self.blocks = 0
        self.errors = set()
        self.total = 0
        self.pending = b""
        self.raw = None
        if raw_path is not None:
            self.raw = os.fdopen(os.open(raw_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb")

    @property
    def in_native(self):
        return self.current is not None

    def feed(self, data):
        available = max(0, MAX_TEXT - self.total)
        if self.raw:
            self.raw.write(data[:available])
        self.total += len(data)
        if self.total > MAX_TEXT:
            self.errors.add("text_budget_exceeded")
            return
        self.pending += data
        while b"\n" in self.pending:
            line, self.pending = self.pending.split(b"\n", 1)
            self.line(line.decode("utf-8", errors="replace").rstrip("\r"))
        if len(self.pending) > MAX_LINE:
            self.errors.add("line_budget_exceeded")
            self.pending = b""

    def invalid(self, kind):
        self.errors.add(kind)
        if self.current is not None:
            self.current["valid"] = False

    def unique(self, key, value):
        core_order = ["event_pid", "event_tid", "observed_chance", "exception_address",
                      "exception_code", "exception_flags", "parameter_count", "module_header",
                      "fault_module", "module_start", "module_end"]
        if key in core_order:
            position = self.current.get("field_position", 0)
            if position >= len(core_order) or core_order[position] != key:
                self.invalid("native_field_order")
            else:
                self.current["field_position"] = position + 1
            if key == "module_header" and set(self.current["parameters"]) != set(range(self.current.get("parameter_count", 0))):
                self.invalid("native_parameter_indices_missing")
        if key in self.current:
            self.invalid("duplicate_native_field")
        else:
            self.current[key] = value

    def line(self, line):
        if len(line.encode("utf-8")) > MAX_LINE:
            self.invalid("line_budget_exceeded")
            return
        marker = re.fullmatch(f"KEEL_{self.nonce}_NATIVE_(BEGIN|END)_(FIRST|SECOND)", line)
        if marker and marker[1] == "BEGIN":
            if self.current is not None:
                self.invalid("nested_native_block")
            self.current = {"chance": marker[2], "valid": True, "parameters": {}, "stacks": []}
            self.stack = None
            return
        if marker and marker[1] == "END":
            if self.current is None or self.current["chance"] != marker[2]:
                self.invalid("unmatched_native_end")
            else:
                self.blocks += 1
                self.commit_event()
            self.current = None
            self.stack = None
            return
        if self.current is not None:
            self.native_line(line)
            return
        if match := re.fullmatch(f"KEEL_{self.nonce}_PID:([0-9]{{1,10}})", line):
            value = int(match[1])
            if self.pid is not None or not 0 < value <= 0xffffffff:
                self.invalid("invalid_or_duplicate_pid")
            else:
                self.pid = value
        elif match := re.fullmatch(r"test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored(?:,.*)?|test has been running for over 60 seconds)", line):
            if match[1] in self.names:
                self.harness.append(f"test {match[1]} ... {match[2].split(',')[0]}")
        elif re.fullmatch(r"running [0-9]+ tests", line):
            self.harness.append(line)
        elif re.fullmatch(r"test result: (ok|FAILED)\. [0-9]+ passed; [0-9]+ failed; [0-9]+ ignored; [0-9]+ measured; [0-9]+ filtered out; finished in [0-9.]+s", line):
            self.harness.append(line)

    def native_line(self, line):
        if not line.strip():
            return
        # The fixed output schema never exports these strings. They additionally
        # invalidate an event even if embedded in otherwise plausible CDB text.
        if re.search(r"authorization|bearer|token\s*=|password\s*=", line, re.I):
            self.invalid("unexpected_native_text")
            return
        if match := re.fullmatch(r"Last event:\s*([0-9a-fA-F]{1,8})\.([0-9a-fA-F]{1,8}):.*\((first|second) chance\)", line):
            self.unique("event_pid", int(match[1], 16))
            self.unique("event_tid", int(match[2], 16))
            self.unique("observed_chance", match[3].upper())
            return
        if match := re.fullmatch(rf"\s*(ExceptionAddress|ExceptionCode|ExceptionFlags):\s*({HEX64})(?:\s+\([^\r\n]*\))?", line):
            key = {"ExceptionAddress": "exception_address", "ExceptionCode": "exception_code", "ExceptionFlags": "exception_flags"}[match[1]]
            value = int(match[2].replace('`', ''), 16)
            if value > (0xffffffffffffffff if key == "exception_address" else 0xffffffff):
                self.invalid("native_integer_width")
            self.unique(key, value)
            return
        if match := re.fullmatch(r"\s*NumberParameters:\s*([0-9]{1,2})", line):
            count = int(match[1])
            if count > 15:
                self.invalid("native_parameter_count")
            self.unique("parameter_count", count)
            return
        if match := re.fullmatch(rf"\s*Parameter\[([0-9]{{1,2}})\]:\s*({HEX64})", line):
            index, value = int(match[1]), int(match[2].replace('`', ''), 16)
            if (index >= 15 or index != len(self.current["parameters"])
                    or "parameter_count" not in self.current or "module_header" in self.current):
                self.invalid("invalid_or_duplicate_parameter")
            else:
                self.current["parameters"][index] = value
            return
        if re.fullmatch(r"\s*start\s+end\s+module name", line, re.I):
            self.unique("module_header", True)
            return
        if self.current.get("module_header") and "fault_module" not in self.current:
            if match := re.fullmatch(rf"\s*({HEX64})\s+({HEX64})\s+([A-Za-z0-9_-]+)(?:\s+\([^\r\n]*\))?", line):
                enum = self.allowed_modules.get(match[3].lower())
                if enum is None:
                    self.invalid("module_not_in_fixed_enum")
                else:
                    self.unique("fault_module", enum)
                    self.unique("module_start", int(match[1].replace('`', ''), 16))
                    self.unique("module_end", int(match[2].replace('`', ''), 16))
                return
        if match := re.fullmatch(rf"\s*[.#]?\s*([0-9]{{1,3}})\s+Id:\s*({HEX32})\.({HEX32})\s+Suspend:\s*[0-9]+\s+Teb:?\s*({HEX64})\s+(?:Unfrozen|Frozen)", line, re.I):
            if len(self.current["stacks"]) >= 512:
                self.invalid("native_thread_budget")
                return
            if "module_end" not in self.current or int(match[2], 16) != self.current.get("event_pid"):
                self.invalid("native_thread_order_or_pid")
            self.stack = {"debugger_thread_index": int(match[1]), "pid": int(match[2], 16),
                          "tid": int(match[3], 16), "frames": []}
            self.current["stacks"].append(self.stack)
            return
        if re.fullmatch(r"\s*#?\s*Child-SP\s+RetAddr\s+Call Site", line):
            if "module_end" not in self.current:
                self.invalid("native_stack_order")
            if self.stack is None or self.stack["frames"]:
                self.stack = {"pid": self.current.get("event_pid"), "tid": self.current.get("event_tid"), "frames": []}
                self.current["stacks"].append(self.stack)
            return
        if match := re.fullmatch(rf"\s*([0-9a-fA-F]{{1,2}})\s+({HEX64})\s+({HEX64})\s+[^\r\n]+", line):
            if self.stack is None:
                self.invalid("native_frame_without_thread")
                return
            ordinal = int(match[1], 16)
            if ordinal != len(self.stack["frames"]) or ordinal >= 64:
                self.invalid("native_frame_order_or_budget")
            else:
                self.stack["frames"].append({"ordinal": ordinal,
                    "stack_pointer": int(match[2].replace('`', ''), 16),
                    "return_address": int(match[3].replace('`', ''), 16)})
            return
        # Known private-only CDB context output: numeric registers, CPU flags,
        # module!symbol labels and disassembly. No part is stored in upload data.
        if re.fullmatch(r"\s*(?:(?:[a-z][a-z0-9]{1,7})=[0-9a-fA-F`]+\s*)+", line):
            return
        if re.fullmatch(r"\s*(?:[a-zA-Z]{2,3}\s+){2,}[a-zA-Z]{2,3}\s*", line):
            return
        if re.fullmatch(r"\s*iopl=[0-3](?:\s+(?:nv|ov|up|dn|ei|di|pl|ng|zr|nz|ac|na|pe|po|cy|nc))+\s*", line):
            return
        if re.fullmatch(r"\s*Debugger time:\s*(?:Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s+(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+[0-9:. ()+-]+(?:UTC[0-9:. ()+-]*)?", line, re.I):
            return
        if re.fullmatch(r"[A-Za-z0-9_-]+![A-Za-z0-9_:<>$,.+-]+:", line):
            return
        if re.fullmatch(rf"\s*{HEX64}\s+[0-9a-fA-F]+\s+[^\r\n]+", line):
            return
        if re.fullmatch(r"\s*Subcode:\s*0x[0-9a-fA-F]{1,8}\s+[A-Z0-9_]+", line):
            return
        self.invalid("unexpected_native_text")

    def commit_event(self):
        event = self.current
        required = {"chance", "event_pid", "event_tid", "observed_chance", "exception_address", "exception_code",
                    "exception_flags", "parameter_count", "fault_module", "module_start", "module_end"}
        if not required <= event.keys() or event.get("observed_chance") != event["chance"]:
            self.invalid("native_required_fields_or_chance_missing")
        if self.pid is not None and event.get("event_pid") != self.pid:
            self.invalid("native_pid_does_not_match_target")
        if event.get("module_start", 0) >= event.get("module_end", 0):
            self.invalid("native_module_range_invalid")
        if set(event["parameters"]) != set(range(event.get("parameter_count", 0))):
            self.invalid("native_parameter_indices_missing")
        if not any(stack["frames"] for stack in event["stacks"]):
            self.invalid("native_stack_missing")
        if event.get("valid"):
            projected = {key: event[key] for key in sorted(required) if key != "observed_chance"}
            projected["parameters"] = [event["parameters"][index] for index in range(event["parameter_count"])]
            projected["stacks"] = event["stacks"]
            self.events.append(projected)

    def finish(self):
        if self.pending:
            self.line(self.pending.decode("utf-8", errors="replace").rstrip("\r"))
        if self.current is not None:
            self.invalid("incomplete_native_block")
        if self.raw:
            self.raw.close()
            self.raw = None

    def exception_complete(self):
        return bool(self.events) and not self.errors and not self.in_native

    def upload_events(self):
        """No native projection is published after an ambiguous/invalid stream."""
        return self.events if not self.errors and not self.in_native else []

    def exception_details(self):
        events = self.upload_events()
        return {"exception_codes": [f"0x{event['exception_code']:08x}" for event in events],
                "parameter_0": [event["parameters"][0] for event in events if event["parameters"]],
                "fault_module": events[-1]["fault_module"] if events else None,
                "native_frame_count": sum(len(stack["frames"]) for event in events for stack in event["stacks"]),
                "chances": [event["chance"] for event in events]}


class WindowsOwner(WindowsJob):
    """CDB and its target share the Job assigned before CDB's initial resume."""

    def __init__(self):
        super().__init__()
        self.target = None

    def debugger(self, process):
        self.assign(process._handle)

    def open_target(self, pid):
        self.target = self.api.OpenProcess(0x00100000 | 0x1000 | 0x0100 | 0x0001, False, pid)
        if not self.target:
            raise ProbeError("OpenProcess target failed before g")
        self.extra_handles.append(self.target)
        self.assign(self.target)

    def exit_code(self):
        if not self.target:
            return None
        wait = self.api.WaitForSingleObject(self.target, 0)
        if wait == 258:
            return None
        if wait != 0:
            raise ProbeError("WaitForSingleObject target failed")
        code = wintypes.DWORD()
        if not self.api.GetExitCodeProcess(self.target, ctypes.byref(code)):
            raise ProbeError("GetExitCodeProcess failed")
        return code.value


def run_debugger(command, cwd, env, deadline, evidence, owner, require_suite_summary=True):
    """Run once; original target status is independent of CDB/controller status."""
    process = None
    pumps = []
    capture = {}
    target_exit = None
    started = timed_out = interrupted = False
    errors = set()
    startup_end = time.monotonic() + 30
    terminal = None
    try:
        process = owner.spawn(command, cwd, env, deadline, merge_error=True)
        pump = Pump(process.stdout)
        pumps.append(pump)
        capture[pump] = evidence.feed
        owner.debugger(process)
        while True:
            deadline.remaining(reserve=CLEANUP_RESERVE)
            if not started and time.monotonic() >= startup_end:
                raise ProbeError("CDB initial PID was not observed within startup budget")
            pump.drain(evidence.feed, deadline)
            if evidence.pid is not None and not started:
                owner.open_target(evidence.pid)
                process.stdin.write(b"g\n")
                process.stdin.flush()
                started = True
            if (observed := owner.exit_code()) is not None:
                target_exit = observed
            if process.poll() is not None and pump.complete():
                break
            time.sleep(min(0.01, deadline.remaining(reserve=CLEANUP_RESERVE)))
    except TimeoutError:
        timed_out = True
        errors.add("original_job_budget_exhausted")
    except KeyboardInterrupt:
        interrupted = True
        errors.add("controller_interrupted")
    except (OSError, ProbeError, OwnershipError, subprocess.SubprocessError) as error:
        errors.add(type(error).__name__)
    finally:
        with deferred_interrupts():
            # Genuine target status is sampled before any forced Job cleanup. The
            # exit125 caused by cleanup must never become the original test result.
            process = process if process is not None else owner.process
            try:
                if (observed := owner.exit_code()) is not None:
                    target_exit = observed
            except (OSError, ProbeError, OwnershipError):
                errors.add("original_exit_query_failed")
            if process is not None:
                terminal, cleanup_errors = finish_owned(owner, process, pumps, deadline, capture)
                errors.update(cleanup_errors)
            else:
                owner.close()
            evidence.finish()
    errors.update(evidence.errors)
    if target_exit is None:
        errors.add("original_exit_unobserved")
    debugger_exit = None if process is None else process.returncode
    if debugger_exit is None:
        errors.add("debugger_exit_unobserved")
    elif debugger_exit != 0 and (target_exit is None or debugger_exit & 0xffffffff != target_exit):
        errors.add("debugger_nonzero_exit")
    summaries = [re.fullmatch(r"test result: ok\. ([0-9]+) passed; 0 failed; ([0-9]+) ignored; 0 measured; 0 filtered out; finished in [0-9.]+s", line) for line in evidence.harness]
    complete_suite = any(match and int(match[1]) + int(match[2]) == TEST_COUNT for match in summaries)
    if target_exit == 0 and require_suite_summary and not complete_suite:
        errors.add("completed_suite_summary_missing")
    if target_exit is not None and target_exit >= 0x80000000 and not evidence.exception_complete():
        errors.add("native_exception_context_incomplete")
    if target_exit is not None and target_exit >= 0x80000000 and f"0x{target_exit:08x}" not in evidence.exception_details()["exception_codes"]:
        errors.add("native_exception_code_does_not_match_exit")
    code = result_code(target_exit, not errors, timed_out)
    if interrupted and target_exit in (None, 0):
        code = 130
    return {"original_test_exit": target_exit,
            "original_test_exit_hex": None if target_exit is None else f"0x{target_exit:08x}",
            "debugger_exit": debugger_exit, "test_started": started,
            "owned_cleanup_complete": terminal is not None and not terminal["errors"],
            "owned_terminal": terminal,
            "timed_out": timed_out, "interrupted": interrupted,
            "diagnostic_errors": sorted(errors), "controller_exit": code}


def bounded(command, source, env, deadline):
    """Every preparation command is owned before work and reaches a terminal."""
    result, receipt = run_owned(command, source, env, deadline)
    if result.returncode:
        # stdout/stderr remain private. CalledProcessError preserves the actual
        # preparation failure and leaves all later application stages unreachable.
        error = subprocess.CalledProcessError(result.returncode, command)
        error.owned_receipt = receipt
        raise error
    return result


def cdb_command(cdb, binary, init, arguments):
    """Keep debugger heap, telemetry, shell and network symbol behavior explicit."""
    return [str(cdb), "-G", "-hd", "-nosqm", "-noshell", "-sins", "-netsyms", "no",
            "-y", str(binary.parent), "-cf", str(init), str(binary), *arguments]


def native_controls(cdb, output, env, deadline):
    """Owned Windows controls establish handle/exception/cleanup mechanics first.

    RaiseFailFastException is an actual Win32 fatal exception with parameter7. It
    does not claim to reproduce __fastfail or the application crash mechanism.
    """
    child = output / "owned-native-control.py"
    child.write_text("import ctypes,sys\n"
                     "from ctypes import wintypes\n"
                     "if sys.argv[1] == 'first':\n"
                     "    call=ctypes.WinDLL('kernel32').RaiseException\n"
                     "    call.argtypes=[wintypes.DWORD,wintypes.DWORD,wintypes.DWORD,ctypes.POINTER(ctypes.c_size_t)]\n"
                     "    call.restype=None\n"
                     "    try: call(0xC0000409,0,1,(ctypes.c_size_t * 1)(9))\n"
                     "    except OSError: sys.exit(0)\n"
                     "    sys.exit(98)\n"
                     "if sys.argv[1] == 'exception':\n"
                     "    class Record(ctypes.Structure):\n"
                     "        _fields_=[('code',wintypes.DWORD),('flags',wintypes.DWORD),('record',ctypes.c_void_p),('address',ctypes.c_void_p),('count',wintypes.DWORD),('parameters',ctypes.c_size_t * 15)]\n"
                     "    call=ctypes.WinDLL('kernel32').RaiseFailFastException\n"
                     "    call.argtypes=[ctypes.POINTER(Record),ctypes.c_void_p,wintypes.DWORD]\n"
                     "    call.restype=None\n"
                     "    record=Record();record.code=0xC0000409;record.flags=1\n"
                     "    record.address=ctypes.cast(call,ctypes.c_void_p).value\n"
                     "    record.count=1;record.parameters[0]=7\n"
                     "    call(ctypes.byref(record),None,0)\n"
                     "    sys.exit(99)\n"
                     "sys.exit(int(sys.argv[1]))\n", encoding="ascii")
    controls = []
    for argument, expected in [("0", 0), ("7", 7), ("259", 259), ("first", 0), ("exception", 0xC0000409)]:
        nonce = secrets.token_hex(16)
        init = output / f"control-{argument}-cdb-init.txt"
        init.write_text(debugger_script(nonce), encoding="ascii")
        evidence = TextEvidence(nonce, set(), app_module=Path(sys.executable).stem, raw_path=output / "private-temp" / f"control-{argument}-shared-raw.bin")
        command = cdb_command(cdb, Path(sys.executable), init, [str(child), argument])
        receipt = run_debugger(command, output, env, deadline, evidence, WindowsOwner(), require_suite_summary=False)
        ok = (receipt["original_test_exit"] == expected and receipt["controller_exit"] == expected
              and not receipt["diagnostic_errors"] and receipt["owned_cleanup_complete"]
              and (argument != "exception" or (evidence.exception_details()["parameter_0"] == [7] and evidence.exception_details()["chances"] == ["SECOND"]))
              and (argument != "first" or (evidence.exception_details()["parameter_0"] == [9] and evidence.exception_details()["chances"] == ["FIRST"])))
        receipt.update({"control": argument, "expected_exit": expected, "control_passed": ok,
                        "exception_events": evidence.upload_events(), "limitations": LIMITATIONS})
        controls.append(receipt)
        (output / "native-controls.json").write_text(json.dumps(controls, indent=2) + "\n", encoding="utf-8")
        if not ok:
            raise ProbeError("owned native controller proof failed before application tests")
    return controls


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--cdb", type=Path, required=True)
    parser.add_argument("--cdb-sha256", required=True)
    parser.add_argument("--deadline-unix", type=float, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    if os.name != "nt":
        raise ProbeError("this probe requires actual x64 Windows")
    if "RUST_MIN_STACK" in os.environ:
        raise ProbeError("RUST_MIN_STACK must remain unset")
    source, cdb, output = options.source.resolve(), options.cdb.resolve(), options.output.resolve()
    deadline = Deadline(options.deadline_unix)
    output.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ)
    # Cargo/test inherits the original runner environment; only the private temp
    # root used by scripts/check.py and explicit no-color output are added.
    scratch = output / "private-temp"
    scratch.mkdir()
    env["TMPDIR"] = str(scratch)
    env["CARGO_TERM_COLOR"] = "never"
    if hashlib.sha256(cdb.read_bytes()).hexdigest() != options.cdb_sha256.lower():
        raise ProbeError("CDB hash differs from verified Microsoft binary")
    if bounded(["git", "rev-parse", "HEAD"], source, env, deadline).stdout.strip() != SOURCE_SHA:
        raise ProbeError("diagnostic source differs from a717400")
    if bounded(["git", "status", "--porcelain", "--untracked-files=no"], source, env, deadline).stdout:
        raise ProbeError("diagnostic source has tracked changes")
    bounded(["rustup", "show"], source, env, deadline)
    rust = bounded(["rustc", "-Vv"], source, env, deadline).stdout
    if f"release: {TOOLCHAIN}\n" not in rust or "host: x86_64-pc-windows-msvc\n" not in rust:
        raise ProbeError("exact Rust1.98.1 x64-msvc is required")
    controls = native_controls(cdb, output, env, deadline)
    build = bounded(["cargo", "test", "-p", "keelshell-app", "--bin", "keelshell-app", "--no-run", "--locked", "--message-format=json"], source, env, deadline)
    binary = application_artifact(build.stdout)
    names = test_names(bounded([str(binary), "--list"], source, env, deadline).stdout)
    nonce = secrets.token_hex(16)
    init = output / "cdb-init.txt"
    init.write_text(debugger_script(nonce), encoding="ascii")
    evidence = TextEvidence(nonce, names, app_module=binary.stem, raw_path=scratch / "app-shared-raw.bin")
    command = cdb_command(cdb, binary, init, ["--test-threads=4", "--no-capture"])
    receipt = run_debugger(command, source, env, deadline, evidence, WindowsOwner())
    receipt.update({"source_sha": SOURCE_SHA, "rust_release": TOOLCHAIN, "test_count": len(names),
                    "test_threads": 4, "no_capture": True, "cdb_sha256": options.cdb_sha256.lower(),
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "native_blocks": evidence.blocks, "native_controls_passed": len(controls),
                    "exception_context": evidence.exception_details(),
                    "runner_image": {"os": os.environ.get("ImageOS"), "version": os.environ.get("ImageVersion")},
                    "controller_created_memory_dump": False})
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    (output / "harness.txt").write_text("\n".join(evidence.harness) + "\n", encoding="utf-8")
    (output / "native.json").write_text(json.dumps({"events": evidence.upload_events(), "limitations": LIMITATIONS}, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt), flush=True)
    return receipt["controller_exit"]


if __name__ == "__main__":
    try:
        code = main()
    except KeyboardInterrupt:
        print(json.dumps({"diagnostic_failed": True, "kind": "KeyboardInterrupt"}), flush=True)
        code = 130
    except subprocess.CalledProcessError as error:
        # Preserve the actual failed preparation command status too; do not echo
        # stderr, command arguments, token-bearing environment or source text.
        print(json.dumps({"preparation_failed": True, "exit": error.returncode, "owned_terminal": getattr(error, "owned_receipt", None)}), flush=True)
        code = error.returncode
    except (OSError, ProbeError, OwnershipError, TimeoutError, subprocess.TimeoutExpired) as error:
        print(json.dumps({"diagnostic_failed": True, "kind": type(error).__name__}), flush=True)
        code = TIMEOUT if isinstance(error, (TimeoutError, subprocess.TimeoutExpired)) else DIAGNOSTIC_ERROR
    except Exception as error:
        # Unexpected parser/API failures still cannot echo private command output
        # or environment through a traceback in the public workflow log.
        print(json.dumps({"diagnostic_failed": True, "kind": type(error).__name__}), flush=True)
        code = DIAGNOSTIC_ERROR
    sys.exit(signed_exit(code))
