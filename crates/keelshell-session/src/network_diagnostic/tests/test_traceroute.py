"""Offline controlled processes, never a real network/tool acceptance claim."""
import base64
import copy
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
import types
import unittest
from unittest import mock

SOURCE = Path(__file__).resolve().parents[1]
adapter = types.ModuleType("keelshell_traceroute_fixture")
exec(compile((SOURCE / "icmp.py").read_text(), str(SOURCE / "icmp.py"), "exec"), adapter.__dict__)
exec(compile((SOURCE / "traceroute.py").read_text(), str(SOURCE / "traceroute.py"), "exec"), adapter.__dict__)
REQUEST = {"kind": "traceroute", "host": "127.0.0.1", "port": 33434, "path": "", "https": True,
           "traceroute": {"protocol": "udp", "family": "ipv4", "max_hops": 2,
                          "probes_per_hop": 2, "wait_ms": 500}}
# Exact bytes observed from the parent's owned upstream 2.1.5 binary on stderr.
UPSTREAM_VERSION_STDERR = (
    b"Modern traceroute for Linux, version 2.1.5\n"
    b"Copyright (c) 2016  Dmitry Butskoy,   License: GPL v2 or any later\n"
)


def output(address="127.0.0.1", hops=2, rows=None):
    return (f"traceroute to {address} ({address}), {hops} hops max, 60 byte packets\n"
            + (f" 1  {address}  0.010 ms  0.020 ms\n" if rows is None else rows))


class ParserTests(unittest.TestCase):
    def test_repeated_addresses_changed_addresses_stars_and_icmp_annotations(self):
        rows = " 1  192.0.2.1  0.010 ms *\n 2  127.0.0.1  0.020 ms  0.030 ms\n"
        hops = adapter.traceroute_parse(output(rows=rows).encode(), "127.0.0.1", REQUEST["traceroute"])
        self.assertEqual(hops[0]["probes"][1], {"address": None, "rtt_us": None, "annotation": None})
        self.assertEqual(hops[1]["probes"][1]["address"], "127.0.0.1")
        self.assertEqual(hops[1]["probes"][1]["rtt_us"], 30)
        rows = " 1  ::2  0.010 ms !H ::1  0.020 ms !<1>\n"
        hops = adapter.traceroute_parse(output("::1", rows=rows).encode(), "::1", REQUEST["traceroute"])
        self.assertEqual([p["annotation"] for p in hops[0]["probes"]], ["!H", "!<1>"])

    def test_unknown_extra_missing_or_inconsistent_tool_output_fails_closed(self):
        valid = output()
        for text in (
            valid + "private output\n", "private\n" + valid, valid[:-1],
            valid.replace("60 byte", "80 byte"), valid.replace(" 1 ", " 2 "),
            valid.replace("  0.020 ms", ""), valid.replace("0.010", "NaN"),
            valid.replace("0.010", "501.001"), valid.replace("ms", "ms !PRIVATE", 1),
            valid.replace("0.010", "\x1b[31m0.010"), valid.replace("0.010", "-0.010"),
            valid.replace("127.0.0.1  0.010", "::1  0.010"),
        ):
            with self.subTest(text=text):
                with self.assertRaises((ValueError, UnicodeError)):
                    adapter.traceroute_parse(text.encode(), "127.0.0.1", REQUEST["traceroute"])

    def test_partial_capture_keeps_only_complete_rows(self):
        data = output(rows=" 1  * *\n 2  127.0.0.1  0.010").encode()
        hops = adapter.traceroute_parse(data, "127.0.0.1", REQUEST["traceroute"], partial=True)
        self.assertEqual([hop["ttl"] for hop in hops], [1])
        self.assertTrue(all(p["rtt_us"] is None for p in hops[0]["probes"]))


class AdapterTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="keelshell-owned-traceroute-")
        self.root = Path(self.temp.name)
        self.tool = self.root / "traceroute"
        self.seen = []
        self.actual_runner = adapter.icmp_run_owned

    def tearDown(self):
        self.temp.cleanup()

    def write_tool(self, text=None, code=0, err="", version=None, body=None,
                   version_stdout=b"", version_code=0):
        self.tool.write_text(
            f"#!{sys.executable}\nimport os,pathlib,sys,time\n"
            "if sys.argv[1:]==['--version']:\n"
            f" sys.stdout.buffer.write({version_stdout!r});"
            f"sys.stderr.buffer.write({UPSTREAM_VERSION_STDERR if version is None else version!r});"
            f"sys.exit({version_code})\n"
            + (body if body is not None else
               f"sys.stdout.write({output() if text is None else text!r});"
               f"sys.stderr.write({err!r});sys.exit({code})\n"))
        self.tool.chmod(0o700)

    def run_adapter(self, request=None, stopped=lambda: False, dns=None):
        actual_stat = os.stat
        def stat_owned(path, *args, **kwargs):
            if path in ("/usr/bin/traceroute", "/bin/traceroute"):
                path = self.tool
            return actual_stat(path, *args, **kwargs)
        def run(argv, *args, **kwargs):
            self.seen.append(list(argv))
            if argv[0] in ("/usr/bin/traceroute", "/bin/traceroute"):
                argv = [str(self.tool), *argv[1:]]
            elif dns is not None:
                argv = [sys.executable, "-I", "-S", "-B", "-c", dns]
            return self.actual_runner(argv, *args, **kwargs)
        with mock.patch.object(adapter.sys, "platform", "linux"), \
                mock.patch.object(adapter.os, "stat", side_effect=stat_owned), \
                mock.patch.object(adapter, "icmp_run_owned", side_effect=run):
            return adapter.traceroute_diagnostic(
                REQUEST if request is None else request, time.monotonic(), stopped)

    def test_exact_upstream_stderr_version_runs_resolver_and_probe(self):
        self.write_tool(version=UPSTREAM_VERSION_STDERR)
        report = self.run_adapter()
        self.assertEqual(report["status"], "success")
        self.assertEqual(report["traceroute"]["reason"], "destination_responded")
        self.assertEqual(report["traceroute"]["implementation"], "linux_traceroute_2_1_5")
        self.assertEqual(self.seen[0], ["/usr/bin/traceroute", "--version"])
        self.assertEqual(len(self.seen), 3)
        self.assertEqual(self.seen[1][0], sys.executable)
        self.assertEqual(self.seen[2][-2:], ["127.0.0.1", "60"])

    def test_version_admission_rejects_stdout_both_split_extra_nonzero_and_other_versions(self):
        first_line, second_line = UPSTREAM_VERSION_STDERR.splitlines(keepends=True)
        cases = (
            ("stdout_only", UPSTREAM_VERSION_STDERR, b"", 0),
            ("both_streams", UPSTREAM_VERSION_STDERR, UPSTREAM_VERSION_STDERR, 0),
            ("split_streams", first_line, second_line, 0),
            ("stdout_extra_byte", b"\n", UPSTREAM_VERSION_STDERR, 0),
            ("stderr_leading_byte", b"", b"\n" + UPSTREAM_VERSION_STDERR, 0),
            ("stderr_extra_byte", b"", UPSTREAM_VERSION_STDERR + b"\n", 0),
            ("stderr_missing_final_byte", b"", UPSTREAM_VERSION_STDERR[:-1], 0),
            ("nonzero_exit", b"", UPSTREAM_VERSION_STDERR, 1),
            ("other_version", b"", UPSTREAM_VERSION_STDERR.replace(b"2.1.5", b"2.1.6"), 0),
            ("other_implementation", b"", b"BusyBox v1.36\n", 0),
        )
        for label, stdout, stderr, code in cases:
            with self.subTest(case=label):
                self.write_tool(version=stderr, version_stdout=stdout, version_code=code)
                self.seen.clear()
                report = self.run_adapter()
                self.assertEqual(report["status"], "unsupported_environment")
                self.assertEqual(report["traceroute"]["reason"], "unsupported_implementation")
                self.assertIsNone(report["traceroute"]["implementation"])
                self.assertIsNone(report["traceroute"]["address"])
                self.assertFalse(report["traceroute"]["hops"])
                self.assertEqual(self.seen, [["/usr/bin/traceroute", "--version"]])

    def test_explicit_options_dispatch_numeric_address_with_fixed_argv(self):
        self.write_tool()
        report = self.run_adapter()
        self.assertEqual(report["status"], "success")
        self.assertEqual(self.seen[-1], ["/usr/bin/traceroute", "-4", "-n", "-M", "default",
            "-f", "1", "-m", "2", "-q", "2", "-N", "1", "-w", "0.500", "-z", "0",
            "--", "127.0.0.1", "60"])
        self.assertEqual(report["traceroute"]["implementation"], "linux_traceroute_2_1_5")

    def test_ipv6_icmp_is_explicit_and_does_not_fallback(self):
        request = copy.deepcopy(REQUEST)
        request["host"] = "::1"
        request["traceroute"].update(protocol="icmp", family="ipv6")
        self.write_tool(text=output("::1"))
        report = self.run_adapter(request, dns="print('[\"::1\"]')")
        self.assertEqual(report["status"], "success")
        self.assertEqual(self.seen[-1][1:6], ["-6", "-n", "-M", "icmp", "-f"])
        self.assertEqual(self.seen[-1][-2:], ["::1", "60"])

    def test_unsafe_request_never_starts_any_process(self):
        self.write_tool()
        for host in ("-I", "x;id", "$(id)", "fe80::1%eth0", "a\n", "127.1"):
            request = copy.deepcopy(REQUEST)
            request["host"] = host
            self.seen.clear()
            self.assertEqual(self.run_adapter(request)["status"], "traceroute_failure")
            self.assertFalse(self.seen)
        for key, value in (("max_hops", 17), ("probes_per_hop", 4), ("wait_ms", "500;id"),
                           ("wait_ms", 1001), ("max_hops", True), ("protocol", "--help")):
            request = copy.deepcopy(REQUEST)
            request["traceroute"][key] = value
            self.seen.clear()
            self.assertEqual(self.run_adapter(request)["status"], "traceroute_failure")
            self.assertFalse(self.seen)

    def test_missing_utility_and_unknown_version_are_honest_without_probe(self):
        with mock.patch.object(adapter.sys, "platform", "linux"), \
                mock.patch.object(adapter.os, "stat", side_effect=FileNotFoundError), \
                mock.patch.object(adapter, "icmp_run_owned") as run:
            report = adapter.traceroute_diagnostic(REQUEST, time.monotonic(), lambda: False)
        self.assertEqual(report["traceroute"]["reason"], "utility_missing")
        run.assert_not_called()
        for version in (b"BusyBox v1.36\n", adapter.TRACE_VERSION.replace(b"2.1.5", b"2.1.6"),
                        adapter.TRACE_VERSION + b"private\n"):
            self.write_tool(version=version)
            self.seen.clear()
            report = self.run_adapter()
            self.assertEqual(report["traceroute"]["reason"], "unsupported_implementation")
            self.assertEqual(len(self.seen), 1)

    def test_hop_limit_and_unreachable_never_become_success(self):
        for rows, status in (
            (" 1  * *\n 2  * *\n", "traceroute_hop_limit"),
            (" 1  127.0.0.1  0.010 ms !X *\n", "traceroute_unreachable"),
        ):
            self.write_tool(text=output(rows=rows))
            self.assertEqual(self.run_adapter()["status"], status)

    def test_resolution_missing_selected_family_never_runs_tool_probe(self):
        self.write_tool()
        for dns in ("import sys;sys.exit(3)", "print('[\"::1\"]')"):
            self.seen.clear()
            report = self.run_adapter(dns=dns)
            self.assertIn(report["status"], ("dns_failure", "traceroute_failure"))
            self.assertEqual(len(self.seen), 2)
            self.assertFalse(report["traceroute"]["hops"])

    def test_permission_options_nonzero_missing_and_flood_have_fixed_reasons(self):
        for text, code, err, reason in (
            ("", 1, "\nsocket: Permission denied\n", "permission_denied"),
            ("", 1, "You do not have enough privileges to use this traceroute method.\n"
             "socket: Operation not permitted\n", "permission_denied"),
            ("", 2, "unknown option PRIVATE", "unsupported_options"),
            ("", 1, "private error", "process_failed"),
            ("private output", 0, "", "invalid_output"),
            (output(), 0, "private error", "invalid_output"),
            ("x" * 40000, 0, "", "output_limit"),
            ("", 1, "x" * 40000, "output_limit"),
        ):
            self.write_tool(text=text, code=code, err=err)
            report = self.run_adapter()
            self.assertEqual(report["traceroute"]["reason"], reason)
            self.assertNotIn("private", json.dumps(report).lower())

    def test_cancel_reaps_tool_and_keeps_only_complete_rows(self):
        pidfile = self.root / "pid"
        partial = output(rows=" 1  * *\n 2  *")
        self.write_tool(body=f"pathlib.Path({str(pidfile)!r}).write_text(str(os.getpid()))\n"
                        f"sys.stdout.write({partial!r});sys.stdout.flush()\n"
                        "time.sleep(30)\n")
        ready_at = [None]
        def stopped():
            if ready_at[0] is None and pidfile.exists():
                ready_at[0] = time.monotonic()
            return ready_at[0] is not None and time.monotonic() - ready_at[0] >= .1
        report = self.run_adapter(stopped=stopped)
        self.assertEqual(report["status"], "interrupted")
        self.assertEqual([row["ttl"] for row in report["traceroute"]["hops"]], [1])
        pid = int(pidfile.read_text())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)
        with self.assertRaises(ChildProcessError):
            os.waitpid(pid, os.WNOHANG)

    def test_real_supervisor_deadline_actually_waits_owned_fake_tool(self):
        pidfile = self.root / "pid"
        self.write_tool(body=f"pathlib.Path({str(pidfile)!r}).write_text(str(os.getpid()))\n"
                        "time.sleep(30)\n")
        # Replace discovery and Linux platform only in this offline driver;
        # the full production supervisor and owned stage runner remain real.
        source = (SOURCE / "icmp.py").read_text() + "\n" + (SOURCE / "traceroute.py").read_text()
        source += ("\nsys.platform='linux'\n"
                   f"TRACE_FIXTURE={str(self.tool)!r}\n"
                   "_production_stat=os.stat\n"
                   "os.stat=lambda path: _production_stat(TRACE_FIXTURE) if path in "
                   "('/usr/bin/traceroute','/bin/traceroute') else _production_stat(path)\n"
                   "_production_run=icmp_run_owned\n"
                   "def icmp_run_owned(argv,*args,**kwargs):\n"
                   " if argv[0] in ('/usr/bin/traceroute','/bin/traceroute'):\n"
                   "  argv=[TRACE_FIXTURE]+argv[1:]\n"
                   " return _production_run(argv,*args,**kwargs)\n")
        source += (SOURCE / "probe.py").read_text()
        encoded = base64.b64encode(json.dumps(REQUEST).encode()).decode()
        child = subprocess.Popen([sys.executable, "-I", "-S", "-B", "-", encoded,
                                  base64.b64encode(b"pass").decode()],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        started = time.monotonic()
        try:
            out, err = child.communicate(source.encode(), timeout=10)
        finally:
            if child.poll() is None:
                child.kill()
                child.communicate(timeout=1)
        self.assertEqual(child.returncode, 0)
        self.assertFalse(err)
        self.assertLess(time.monotonic() - started, 9)
        self.assertEqual(out.count(b"KEELSHELL_DIAGNOSTIC_V1\n"), 1)
        report = json.loads(out.split(b"\n", 1)[1])
        self.assertEqual(report["status"], "timeout")
        self.assertFalse(report["traceroute"]["hops"])
        pid = int(pidfile.read_text())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)


if __name__ == "__main__":
    unittest.main()
