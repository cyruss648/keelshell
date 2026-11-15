"""Darwin wire parsing and owned child lifecycle, without invoking host metrics."""
import contextlib
import ctypes
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import types
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/keelshell-session/src/monitor/darwin.py"
collector = types.ModuleType("darwin_collector_under_test")
exec(compile(SOURCE.read_bytes(), str(SOURCE), "exec"), collector.__dict__)
HEADER = "Name Mtu Network Address Ipkts Ierrs Ibytes Opkts Oerrs Obytes Coll\n"
LINK = "en0 1500 <Link#4> aa:bb:cc:dd:ee:ff 100 0 123456 200 0 654321 0\n"
LOOP = "lo0 16384 <Link#1> 20 0 2000 20 0 2000 0\n"

class NetstatTests(unittest.TestCase):
    def parse(self, text):
        return collector.parse_netstat(text.encode("ascii"))

    def test_link_only_avoids_ipv4_ipv6_double_count(self):
        rows = self.parse(HEADER + LINK + "en0 1500 192.0.2 192.0.2.1 100 - 123456 200 - 654321 -\n" + "en0 1500 fe80::%en0 fe80::1%en0 100 - 123456 200 - 654321 -\n" + LOOP)
        self.assertEqual(rows, [("en0", 4, 123456, 654321), ("lo0", 1, 2000, 2000)])

    def test_unpadded_link_address_is_valid_bsd_output(self):
        self.assertEqual(self.parse(HEADER + LINK.replace("aa:bb:cc:dd:ee:ff", "a:b:c:d:e:f"))[0][2:], (123456,654321))

    def test_inactive_name_suffix_is_not_part_of_kernel_name(self):
        self.assertEqual(self.parse(HEADER + LINK.replace("en0 ", "en0* "))[0][0], "en0")

    def test_different_link_ids_may_share_truncated_display_prefix(self):
        first=LINK.replace("en0 ", "longinterf ")
        second=first.replace("<Link#4>", "<Link#5>")
        self.assertEqual(len(self.parse(HEADER+first+second)),2)

    def test_duplicate_link_index_rejects_duplicate_totals(self):
        with self.assertRaises(collector.CollectionFailure):
            self.parse(HEADER + LINK + LINK.replace("en0 ","en1 "))

    def test_link_counter_resets_remain_exact_integer_counters(self):
        self.assertEqual(self.parse(HEADER + LINK.replace("123456", "1").replace("654321", "2"))[0][2:], (1,2))

    def test_link_fields_reject_negative_nonfinite_or_overflow(self):
        for bad in ("-1", "+1", "NaN", "inf", "18446744073709551616"):
            with self.subTest(bad=bad), self.assertRaises(collector.CollectionFailure):
                self.parse(HEADER + LINK.replace("123456", bad))

    def test_header_and_link_shape_are_required(self):
        for text in ("",LINK,HEADER,HEADER.replace("Ibytes","Bytes")+LINK,HEADER+LINK.replace("<Link#4>","<Link#0>"),HEADER+LINK.replace("<Link#4>","<Link#x>"),HEADER+LINK.rstrip()+" 8\n",HEADER+"en0 1500 <Link#4> 1 2\n"):
            with self.subTest(text=text[:40]),self.assertRaises(collector.CollectionFailure):
                self.parse(text)

    def test_interface_limit_and_input_limit(self):
        text=HEADER+"".join(LINK.replace("en0 ","en%d " % i).replace("<Link#4>","<Link#%d>" % (i+1)) for i in range(256))
        self.assertEqual(len(self.parse(text)),256)
        with self.assertRaises(collector.CollectionFailure):
            self.parse(text+LINK.replace("en0 ","en256 ").replace("<Link#4>","<Link#257>"))
        with self.assertRaises(collector.CollectionFailure):
            collector.parse_netstat(b"x"*(collector.MAX_CHILD_OUTPUT+1))

    def test_full_u64_is_not_rounded(self):
        self.assertEqual(self.parse(HEADER+LINK.replace("123456","18446744073709551615"))[0][2],(1<<64)-1)

@unittest.skipUnless(os.name == "posix", "Collector child lifecycle requires POSIX")
class ChildTests(unittest.TestCase):
    def run_child(self, code, **kwargs):
        return collector.run_bounded([sys.executable,"-I","-S","-B","-c",code],**kwargs)

    def test_fixed_command_output_is_captured(self):
        self.assertEqual(self.run_child("print('bounded')"),("ok",b"bounded\n"))

    def test_failed_child_diagnostic_and_stdout_are_discarded(self):
        self.assertEqual(self.run_child("import sys;print('discard');sys.stderr.write('private diagnostic');sys.exit(3)"),("failed",b""))

    def test_absent_executable_is_typed_missing(self):
        with mock.patch.object(collector.subprocess,"Popen",side_effect=FileNotFoundError):
            self.assertEqual(collector.run_bounded(["/fixed/not-present"]),("missing",b""))

    def test_spawn_permission_denied_is_typed(self):
        with mock.patch.object(collector.subprocess,"Popen",side_effect=PermissionError):
            self.assertEqual(collector.run_bounded(["/fixed/not-permitted"]),("permission_denied",b""))

    def test_spawn_os_error_is_unknown_failure(self):
        with mock.patch.object(collector.subprocess,"Popen",side_effect=OSError):
            self.assertEqual(collector.run_bounded(["/fixed/fails"]),("failed",b""))

    def owned(self, code, **kwargs):
        actual=subprocess.Popen
        children=[]
        def capture(*args,**popen_kwargs):
            child=actual(*args,**popen_kwargs)
            children.append(child)
            return child
        started=time.monotonic()
        with mock.patch.object(collector.subprocess,"Popen",side_effect=capture):
            result=self.run_child(code,**kwargs)
        self.assertLess(time.monotonic()-started,2.0)
        self.assertTrue(children and all(child.poll() is not None for child in children))
        return result

    def test_timeout_kills_and_reaps_only_owned_child(self):
        self.assertEqual(self.owned("import time;time.sleep(5)",seconds=0.08),("timeout",b""))

    def test_output_cap_kills_and_reaps_while_child_is_writing(self):
        self.assertEqual(self.owned("import os,time;os.write(1,b'x'*65536);time.sleep(5)",limit=1024),("invalid_output",b""))

    def test_closed_stdout_does_not_allow_unbounded_wait(self):
        self.assertEqual(self.owned("import os,time;os.close(1);time.sleep(5)",seconds=0.08),("timeout",b""))

    def test_stderr_has_no_pipe_and_cannot_fill_collector_memory(self):
        self.assertEqual(self.owned("import os;os.write(2,b'x'*1048576);print('ok')"),("ok",b"ok\n"))

    def test_stdin_and_environment_do_not_inherit_user_context(self):
        code="import os,sys;print(sys.stdin.buffer.read()==b'',os.getenv('HOME') is None,os.getcwd())"
        self.assertEqual(self.run_child(code),("ok",b"True True /\n"))

    def test_global_alarm_interrupt_also_reaps_owned_child(self):
        previous=signal.signal(signal.SIGALRM,collector.deadline)
        actual=subprocess.Popen
        children=[]
        def capture(*args,**kwargs):
            child=actual(*args,**kwargs)
            children.append(child)
            return child
        try:
            with mock.patch.object(collector.subprocess,"Popen",side_effect=capture):
                signal.setitimer(signal.ITIMER_REAL,0.08)
                with self.assertRaises(collector.DeadlineExceeded):
                    self.run_child("import time;time.sleep(5)")
        finally:
            signal.setitimer(signal.ITIMER_REAL,0)
            signal.signal(signal.SIGALRM,previous)
        self.assertTrue(children and all(child.poll() is not None for child in children))

class CapacityAndWireTests(unittest.TestCase):
    def test_skip_never_invokes_df(self):
        with mock.patch.object(collector,"run_bounded",side_effect=AssertionError("df must not run")):
            self.assertEqual(collector.collect_capacity(True),("skipped",b""))

    def test_non_skipped_df_is_exactly_fixed_posix_kib_command(self):
        with mock.patch.object(collector,"run_bounded",return_value=("ok",b"fixture")) as call:
            self.assertEqual(collector.collect_capacity(False),("ok",b"fixture"))
        call.assert_called_once_with(["/bin/df","-Pk"])

    def test_optional_failure_has_no_payload(self):
        for status in ("missing","permission_denied","timeout","failed","invalid_output"):
            with mock.patch.object(collector,"run_bounded",return_value=(status,b"")):
                self.assertEqual(collector.collect_capacity(False),(status,b""))

    def test_invalid_utf8_or_controlled_df_payload_is_optional_invalid_output(self):
        for raw in (b"\xff",b"bad\0df",b"bad\rdf"):
            with mock.patch.object(collector,"run_bounded",return_value=("ok",raw)):
                self.assertEqual(collector.collect_capacity(False),("invalid_output",b""))

    def test_render_matches_frozen_real_wire_fixture(self):
        frame=(ROOT/"crates/keelshell-session/src/monitor/darwin/fixtures/schema1-skipped.txt").read_bytes()
        head=frame.split(b"\n@@KS:df@@\n",1)[0].decode().splitlines()[1:]
        fields=[tuple(line.split("\t",1)) for line in head if not line.startswith(("df_status\t","df_bytes\t"))]
        self.assertEqual(collector.render(fields,"skipped",b""),frame)

    def test_render_counts_utf8_bytes_and_refuses_oversized_output(self):
        out=collector.render([],"ok","测试".encode())
        self.assertIn(b"df_bytes\t6\n",out)
        with self.assertRaises(collector.CollectionFailure):
            collector.render([],"ok",b"x"*collector.MAX_OUTPUT)

    @unittest.skipUnless(hasattr(signal, "SIGALRM") and hasattr(signal, "alarm"), "Entry alarm requires POSIX")
    def test_platform_failure_emits_no_untrusted_diagnostic(self):
        with mock.patch.object(collector.sys,"platform","linux"),mock.patch.object(collector.sys,"stdout",new=io.StringIO()) as output:
            self.assertEqual(collector.main(),64)
            self.assertEqual(output.getvalue(),"")

    @unittest.skipUnless(hasattr(signal, "SIGALRM") and hasattr(signal, "alarm"), "Entry alarm requires POSIX")
    def test_core_failure_emits_no_untrusted_diagnostic(self):
        with mock.patch.object(collector.sys,"platform","darwin"),mock.patch.object(collector.sys,"argv",["-","1"]),mock.patch.object(collector,"library",side_effect=RuntimeError("private diagnostic")),mock.patch.object(collector.sys,"stdout",new=io.StringIO()) as output:
            self.assertEqual(collector.main(),65)
            self.assertEqual(output.getvalue(),"")

    @unittest.skipUnless(hasattr(signal, "SIGALRM") and hasattr(signal, "alarm"), "Entry alarm requires POSIX")
    def test_missing_required_netstat_uses_missing_command_exit(self):
        with mock.patch.object(collector.sys,"platform","darwin"),mock.patch.object(collector.sys,"argv",["-","1"]),mock.patch.object(collector,"library",return_value=None),mock.patch.object(collector,"collect_core",side_effect=FileNotFoundError):
            self.assertEqual(collector.main(),66)

if __name__=="__main__":
    unittest.main(verbosity=2)
