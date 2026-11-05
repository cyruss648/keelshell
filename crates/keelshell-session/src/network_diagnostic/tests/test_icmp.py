"""Offline adapter/process tests. Every executable is a local fake; no packets."""
import base64
import errno
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

SOURCE = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("keelshell_icmp", SOURCE / "icmp.py")
icmp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(icmp)
VERSION = (
    "ping from iputils 20240117\n"
    "libcap: yes, IDN: no, NLS: yes, error.h: yes, getrandom(): yes, __fpending(): yes\n"
)
REQUEST = {"kind": "icmp", "host": "never-resolved.invalid", "port": 443,
           "path": "", "https": True}
DNS_WORKER = (
    "import json; print('KEELSHELL_DIAGNOSTIC_V1');"
    " print(json.dumps({'status':'success','addresses':"
    "[{'family':'IPv4','address':'127.0.0.1'}],"
    "'limited':False,'timing':{'resolve_ms':1}}))"
)


def summary(received=4, sent=4, address="127.0.0.1", extras=""):
    loss = (sent - received) * 100 / sent
    header = (f"PING {address} ({address}) 56(84) bytes of data."
              if ":" not in address else f"PING {address} ({address}) 56 data bytes")
    text = (f"{header}\n\n--- {address} ping statistics ---\n"
            f"{sent} packets transmitted, {received} received{extras}, "
            f"{loss:g}% packet loss, time 3000ms\n")
    if received:
        text += "rtt min/avg/max/mdev = 0.010/0.020/0.030/0.008 ms\n"
    else:
        text += "\n"  # finish() emits the final line even without timing.
    return text


class ParserTests(unittest.TestCase):
    def test_complete_partial_loss_and_ipv6_have_exact_counts(self):
        for address in ("127.0.0.1", "::1"):
            for received in (0, 1, 2, 3, 4):
                counts, rtt = icmp.icmp_parse_summary(summary(received, address=address).encode(), address)
                self.assertEqual((counts["transmitted"], counts["received"]), (4, received))
                self.assertEqual(rtt is not None, received > 0)
                if rtt:
                    self.assertEqual(rtt["avg_us"], 20)
        self.assertEqual(icmp.icmp_parse_summary(
            summary(0)[:-1].encode() + b"pipe 4\n", "127.0.0.1")[0]["received"], 0)

    def test_unknown_or_contradictory_output_is_rejected(self):
        valid = summary()
        for text in (
            valid + "arbitrary private stderr\n",
            "arbitrary prefix\n" + valid,
            valid.replace("56(84)", "64(92)"),
            valid.replace("4 received", "5 received"),
            valid.replace("0% packet loss", "25% packet loss"),
            valid.replace("0.010/0.020/0.030", "0.040/0.020/0.030"),
            valid.replace("0.010", "NaN"),
            valid.replace("0.010", "0.01"),
            valid.replace("PING ", "PING \x1b[31m"),
            valid.replace("rtt min", "round-trip min"),
            valid.replace("127.0.0.1", "192.0.2.1"),
        ):
            with self.subTest(text=text):
                with self.assertRaises((ValueError, UnicodeError)):
                    icmp.icmp_parse_summary(text.encode(), "127.0.0.1")

    def test_permission_mapping_is_fixed_and_never_reflects_text(self):
        self.assertTrue(icmp.icmp_permission(b"ping: socket: Operation not permitted\n", "/bin/ping"))
        self.assertFalse(icmp.icmp_permission(b"private token: Permission denied\n", "/bin/ping"))
        self.assertFalse(icmp.icmp_permission(b"ping: socket: Operation not permitted SECRET\n", "/bin/ping"))


class AdapterTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="keelshell-fake-ping-")
        self.root = Path(self.temp.name)
        self.ping = self.root / "ping"
        self.seen = []
        self.real_runner = icmp.icmp_run_owned

    def tearDown(self):
        self.temp.cleanup()

    def write_ping(self, out=None, code=0, err="", version=VERSION, body=None):
        source = (
            f"#!{sys.executable}\n"
            "import json,os,pathlib,sys,time\n"
            "if sys.argv[1:]==['-V']:\n"
            f" sys.stdout.write({version!r}); sys.exit(0)\n"
        )
        if body is not None:
            source += body
        else:
            source += f"sys.stdout.write({(summary() if out is None else out)!r})\n"
            source += f"sys.stderr.write({err!r})\nsys.exit({code})\n"
        self.ping.write_text(source)
        self.ping.chmod(0o700)

    def run_adapter(self, stopped=lambda: False):
        # Only fixed production paths are discovered; route them to this owned
        # fake executable. The actual DNS stage is a constant print-only worker.
        info = self.ping.stat() if self.ping.exists() else None

        def run(argv, *args, **kwargs):
            self.seen.append(list(argv))
            if argv[0] in ("/usr/bin/ping", "/bin/ping"):
                argv = [str(self.ping), *argv[1:]]
            return self.real_runner(argv, *args, **kwargs)

        with mock.patch.object(icmp.sys, "platform", "linux"), \
                mock.patch.object(icmp.os, "stat", return_value=info), \
                mock.patch.object(icmp, "icmp_run_owned", side_effect=run):
            return icmp.icmp_diagnostic(REQUEST, DNS_WORKER, time.monotonic(), stopped)

    def test_preview_has_no_execution_hook_and_supported_dispatch_has_fixed_argv(self):
        self.write_ping()
        report = self.run_adapter()
        self.assertEqual(report["status"], "success")
        self.assertEqual(self.seen[-1], ["/usr/bin/ping", "-4", "-n", "-q", "-c", "4", "-i", "1", "-s", "56", "-W", "1", "--", "127.0.0.1"])
        self.assertNotIn("-w", self.seen[-1])
        self.assertEqual(report["icmp"]["implementation"], "iputils_20240117")

    def test_missing_utility_never_runs_any_stage(self):
        with mock.patch.object(icmp.sys, "platform", "linux"), \
                mock.patch.object(icmp.os, "stat", side_effect=FileNotFoundError), \
                mock.patch.object(icmp, "icmp_run_owned") as run:
            report = icmp.icmp_diagnostic(REQUEST, DNS_WORKER, time.monotonic(), lambda: False)
        self.assertEqual(report["status"], "utility_unavailable")
        self.assertEqual(report["icmp"]["reason"], "utility_missing")
        run.assert_not_called()

    def test_unsupported_os_and_unknown_version_do_not_send_ping(self):
        with mock.patch.object(icmp.sys, "platform", "darwin"), \
                mock.patch.object(icmp, "icmp_run_owned") as run:
            report = icmp.icmp_diagnostic(REQUEST, DNS_WORKER, time.monotonic(), lambda: False)
        self.assertEqual(report["icmp"]["reason"], "unsupported_platform")
        run.assert_not_called()
        for version in (VERSION.replace("20240117", "20240905"),
                        VERSION.replace("20240117", "20240117-custom"),
                        "BusyBox v1.36\n", VERSION + "unexpected\n"):
            self.seen.clear()
            self.write_ping(version=version)
            report = self.run_adapter()
            self.assertEqual(report["icmp"]["reason"], "unsupported_implementation")
            self.assertEqual(len(self.seen), 1)
            self.assertIsNone(report["icmp"]["counts"])

    def test_no_permission_from_exec_or_exact_tool_errno_is_typed(self):
        self.write_ping()
        self.ping.chmod(0o600)
        self.assertEqual(self.run_adapter()["status"], "permission_denied")
        self.write_ping(out="", code=2, err="ping: socket: Operation not permitted\n")
        report = self.run_adapter()
        self.assertEqual(report["status"], "permission_denied")
        self.assertIsNone(report["icmp"]["counts"])

    def test_full_partial_zero_replies_and_exit_disagreement_are_distinct(self):
        for received, code, status, reason in (
            (4, 0, "success", "completed"),
            (2, 0, "icmp_partial", "packet_loss"),
            (0, 1, "icmp_no_reply", "no_echo_reply"),
            (2, 1, "icmp_failure", "invalid_output"),
            (0, 0, "icmp_failure", "invalid_output"),
        ):
            with self.subTest(received=received, code=code):
                self.write_ping(out=summary(received), code=code)
                report = self.run_adapter()
                self.assertEqual((report["status"], report["icmp"]["reason"]), (status, reason))

    def test_quiet_summary_cannot_distinguish_local_send_errors_from_unanswered_sends(self):
        # Offline replay of the admitted upstream's hard_local_error contract,
        # not syscall injection or genuine iputils/ICMP acceptance. send_probe
        # returns 0 on success and -1 on the modeled EPERM failures. With an
        # empty error queue, quiet mode can suppress local errors while both
        # branches advance ntransmitted and leave nerrors at zero.
        tracefile = self.root / "modeled-send-history.json"
        for received in (0, 2):
            with self.subTest(unique_echo_replies=received):
                modeled_histories = (
                    [0] * received + [-1] * (4 - received),
                    [0] * 4,
                )
                reports = []
                successful_probe_counts = []
                for history in modeled_histories:
                    quiet_output = summary(received, sent=len(history))
                    self.write_ping(body=(
                        f"send_probe_returns={history!r}\n"
                        "trace={'send_probe_returns':send_probe_returns,"
                        "'successful_probe_calls':send_probe_returns.count(0),"
                        "'hard_local_errors':send_probe_returns.count(-1),"
                        "'modeled_errno':'EPERM' if -1 in send_probe_returns else None,"
                        "'error_queue_entries':0,"
                        "'ntransmitted':len(send_probe_returns),'nerrors':0}\n"
                        f"pathlib.Path({str(tracefile)!r}).write_text(json.dumps(trace))\n"
                        f"sys.stdout.write({quiet_output!r})\nsys.exit({0 if received else 1})\n"
                    ))
                    report = self.run_adapter()
                    trace = json.loads(tracefile.read_text())
                    successful_probe_counts.append(trace["successful_probe_calls"])
                    self.assertEqual(trace["ntransmitted"], 4)
                    self.assertEqual(trace["error_queue_entries"], 0)
                    self.assertEqual(report["status"], "icmp_partial" if received else "icmp_no_reply")
                    self.assertEqual(report["icmp"]["reason"], "packet_loss" if received else "no_echo_reply")
                    self.assertEqual(report["icmp"]["counts"]["transmitted"], 4)
                    self.assertEqual(report["icmp"]["counts"]["received"], received)
                    self.assertEqual(report["icmp"]["counts"]["errors"], 0)
                    # A hidden EPERM is not observable permission evidence.
                    # Neither the local failure nor a network-loss cause can
                    # be reconstructed from the identical tool output.
                    reports.append(report["icmp"])
                self.assertEqual(successful_probe_counts, [received, 4])
                self.assertEqual(reports[0], reports[1])

    def test_utility_error_stderr_and_output_budget_never_become_display_text(self):
        for out, code, err, reason in (
            (summary(), 2, "private secret", "process_failed"),
            (summary(), 0, "private secret", "invalid_output"),
            ("private secret", 0, "", "invalid_output"),
            ("x" * 40000, 0, "", "output_limit"),
            ("", 2, "x" * 40000, "output_limit"),
        ):
            self.write_ping(out=out, code=code, err=err)
            report = self.run_adapter()
            self.assertEqual(report["icmp"]["reason"], reason)
            self.assertNotIn("private secret", json.dumps(report))
            self.assertIsNone(report["icmp"]["counts"])

    def test_network_errors_are_counted_without_claiming_all_loss_or_success(self):
        self.write_ping(out=summary(2, extras=", +2 errors"))
        report = self.run_adapter()
        self.assertEqual((report["status"], report["icmp"]["reason"]), ("icmp_failure", "reported_errors"))
        self.assertEqual(report["icmp"]["counts"]["errors"], 2)

    def test_cancel_before_dispatch_starts_no_process(self):
        self.write_ping()
        report = self.run_adapter(stopped=lambda: True)
        self.assertEqual(report["status"], "interrupted")
        self.assertEqual(len(self.seen), 1)  # runner admission, no Popen
        self.assertIsNone(report["icmp"]["counts"])


class OwnedProcessTests(unittest.TestCase):
    def supervisor_fixture(self, cancel):
        with tempfile.TemporaryDirectory(prefix="keelshell-icmp-supervisor-") as root:
            fake = Path(root) / "ping"
            pidfile = Path(root) / "pid"
            fake.write_text(
                f"#!{sys.executable}\nimport pathlib,os,sys,time\n"
                f"if sys.argv[1:]==['-V']:\n print({VERSION!r},end='');sys.exit(0)\n"
                f"pathlib.Path({str(pidfile)!r}).write_text(str(os.getpid()))\ntime.sleep(30)\n")
            fake.chmod(0o700)
            source = (SOURCE / "icmp.py").read_text()
            source += (
                "\nsys.platform='linux'\n"
                f"_fake={str(fake)!r}\n"
                "_real_stat=os.stat\n"
                "def _stat(path,*a,**kw):\n"
                " return _real_stat(_fake if path in ('/usr/bin/ping','/bin/ping') else path,*a,**kw)\n"
                "os.stat=_stat\n"
                "_real_popen=subprocess.Popen\n"
                "def _popen(argv,*a,**kw):\n"
                " if argv[0] in ('/usr/bin/ping','/bin/ping'): argv=[_fake,*argv[1:]]\n"
                " return _real_popen(argv,*a,**kw)\n"
                "subprocess.Popen=_popen\n")
            source += (SOURCE / "probe.py").read_text()
            request = base64.b64encode(json.dumps(REQUEST).encode()).decode()
            worker = base64.b64encode(DNS_WORKER.encode()).decode()
            child = subprocess.Popen(
                [sys.executable, "-I", "-S", "-B", "-", request, worker],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            started = time.monotonic()
            try:
                child.stdin.write(source.encode())
                child.stdin.close()
                child.stdin = None
                until = started + 2
                while not pidfile.exists() and time.monotonic() < until:
                    time.sleep(.01)
                self.assertTrue(pidfile.exists())
                if cancel:
                    for number in (signal.SIGHUP, signal.SIGTERM, signal.SIGINT):
                        child.send_signal(number)
                out, err = child.communicate(timeout=9)
                self.assertEqual(child.returncode, 0)
                self.assertFalse(err)
                self.assertEqual(out.count(b"KEELSHELL_DIAGNOSTIC_V1\n"), 1)
                report = json.loads(out.split(b"\n", 1)[1])
                self.assertEqual(report["status"], "interrupted" if cancel else "timeout")
                self.assertIsNone(report["icmp"]["counts"])
                self.assertLess(time.monotonic() - started, 8.5)
                with self.assertRaises(ProcessLookupError):
                    os.kill(int(pidfile.read_text()), 0)
            finally:
                if child.poll() is None:
                    child.kill()
                    child.communicate(timeout=1)
                if pidfile.exists():
                    try:
                        os.killpg(int(pidfile.read_text()), signal.SIGKILL)
                    except ProcessLookupError:
                        pass

    def test_full_supervisor_repeated_shutdown_reaps_fake_ping_before_one_frame(self):
        self.supervisor_fixture(cancel=True)

    def test_full_supervisor_eight_second_budget_reaps_fake_ping_without_invented_loss(self):
        self.supervisor_fixture(cancel=False)

    def run_stall(self, cancel=False, close_pipes=False):
        with tempfile.TemporaryDirectory(prefix="keelshell-icmp-owned-") as root:
            pidfile = Path(root) / "pid"
            script = ("import os,pathlib,sys,time;"
                      "pathlib.Path(sys.argv[1]).write_text(str(os.getpid()));"
                      + ("os.close(1);os.close(2);" if close_pipes else "")
                      + "time.sleep(30)")
            event = threading.Event()
            timer = threading.Timer(.15, event.set)
            timer.start()
            started = time.monotonic()
            state, code, out, err = icmp.icmp_run_owned(
                [sys.executable, "-I", "-S", "-B", "-c", script, str(pidfile)],
                started + (2 if cancel else .25), [32768],
                event.is_set if cancel else lambda: False)
            timer.join(timeout=1)
            self.assertEqual(state, "interrupted" if cancel else "deadline")
            self.assertIsNotNone(code)
            self.assertEqual((out, err), (b"", b""))
            self.assertLess(time.monotonic() - started, 1.5)
            pid = int(pidfile.read_text())
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)
            with self.assertRaises(ChildProcessError):
                os.waitpid(pid, os.WNOHANG)

    def test_deadline_kills_and_actually_reaps_direct_child(self):
        self.run_stall()

    def test_cancel_kills_and_actually_reaps_direct_child(self):
        self.run_stall(cancel=True)

    def test_closed_output_does_not_bypass_deadline_or_actual_wait(self):
        self.run_stall(close_pipes=True)

    def test_output_flood_is_stopped_while_process_is_live(self):
        state, code, out, err = icmp.icmp_run_owned(
            [sys.executable, "-I", "-S", "-B", "-c",
             "import os,time;os.write(2,b'x'*40000);time.sleep(30)"],
            time.monotonic() + 1, [32768], lambda: False)
        self.assertEqual(state, "output_limit")
        self.assertIsNotNone(code)
        self.assertLessEqual(len(out) + len(err), 32768)

    def test_denied_kill_preserves_unknown_then_fixture_itself_reaps(self):
        with tempfile.TemporaryDirectory(prefix="keelshell-icmp-denied-") as root:
            pidfile = Path(root) / "pid"
            script = ("import os,pathlib,sys,time;"
                      "pathlib.Path(sys.argv[1]).write_text(str(os.getpid()));time.sleep(30)")
            children = []
            actual_popen = subprocess.Popen

            def own(*args, **kwargs):
                child = actual_popen(*args, **kwargs)
                children.append(child)
                return child

            try:
                with mock.patch.object(icmp.os, "killpg", side_effect=PermissionError), \
                        mock.patch.object(icmp.subprocess, "Popen", side_effect=own):
                    state, code, _, _ = icmp.icmp_run_owned(
                        [sys.executable, "-I", "-S", "-B", "-c", script, str(pidfile)],
                        time.monotonic() + .2, [32768], lambda: False)
                self.assertEqual(state, "cleanup_unknown")
                self.assertIsNone(code)
            finally:
                if pidfile.exists():
                    pid = int(pidfile.read_text())
                    os.killpg(pid, signal.SIGKILL)
                    self.assertEqual(children[0].wait(timeout=1), -signal.SIGKILL)
                    with self.assertRaises(ChildProcessError):
                        os.waitpid(pid, os.WNOHANG)

    def test_real_supervisor_emits_one_structured_platform_refusal_without_packets(self):
        # Execute the production concatenated supervisor on macOS. On Linux,
        # override only the adapter platform before the production main begins.
        source = (SOURCE / "icmp.py").read_text()
        source += "\nsys.platform = 'offline-unsupported-test'\n"
        source += (SOURCE / "probe.py").read_text()
        request = base64.b64encode(json.dumps(REQUEST).encode()).decode()
        worker = base64.b64encode(b"raise AssertionError('must not run')").decode()
        child = subprocess.Popen(
            [sys.executable, "-I", "-S", "-B", "-", request, worker],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            out, err = child.communicate(source.encode(), timeout=2)
        finally:
            if child.poll() is None:
                child.kill()
                child.communicate(timeout=1)
        self.assertEqual(child.returncode, 0)
        self.assertFalse(err)
        self.assertEqual(out.count(b"KEELSHELL_DIAGNOSTIC_V1\n"), 1)
        report = json.loads(out.split(b"\n", 1)[1])
        self.assertEqual(report["icmp"]["reason"], "unsupported_platform")


if __name__ == "__main__":
    unittest.main()
