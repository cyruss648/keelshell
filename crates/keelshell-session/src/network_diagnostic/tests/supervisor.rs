#[cfg(test)]
mod tests {
    use super::super::SCRIPT;

    #[test]
    fn repeated_shutdown_during_blocked_worker_confirms_actual_reap() {
        let driver = r#"
import base64,errno,json,os,pathlib,signal,subprocess,sys,tempfile,time
with tempfile.TemporaryDirectory(prefix='keelshell-supervisor-') as root:
    pidfile=pathlib.Path(root)/'worker.pid'
    worker="import os,pathlib,sys,time; pathlib.Path(sys.argv[1]).write_text(str(os.getpid())); time.sleep(30)"
    encoded=base64.b64encode(worker.encode()).decode('ascii')
    child=subprocess.Popen([sys.executable,'-I','-S','-B','-',str(pidfile),encoded],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        child.stdin.write(sys.argv[1].encode()); child.stdin.close(); child.stdin=None
        end=time.monotonic()+3
        while not pidfile.exists() and time.monotonic()<end: time.sleep(.01)
        assert pidfile.exists(), 'worker never started'
        pid=int(pidfile.read_text())
        for number in (signal.SIGHUP,signal.SIGTERM,signal.SIGINT):
            child.send_signal(number); time.sleep(.01)
        out,err=child.communicate(timeout=10)
        assert child.returncode==0 and not err, (child.returncode,err)
        assert out.startswith(b'KEELSHELL_DIAGNOSTIC_V1\n'), out
        result=json.loads(out.split(b'\n',1)[1])
        assert result['status']=='interrupted', result
        assert 7900<=result['timing']['total_ms']<=9000, result
        try: os.kill(pid,0)
        except OSError as e: assert e.errno==errno.ESRCH
        else: raise AssertionError('owned worker survived confirmed shutdown')
        print(json.dumps({'supervisor_actual_exit':child.returncode,'worker_absent':True,'status':result['status']}))
    finally:
        if child.poll() is None:
            child.kill(); child.communicate(timeout=2)
"#;
        let output = std::process::Command::new("python3")
            .args(["-I", "-S", "-B", "-c", driver, SCRIPT])
            .output()
            .unwrap_or_else(|error| panic!("Python 3 supervisor lifecycle prerequisite: {error}"));
        assert!(
            output.status.success(),
            "actual supervisor shutdown: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("\"worker_absent\": true"));
    }

    #[test]
    fn denied_worker_kill_reports_unknown_and_owned_fixture_actually_reaps() {
        let driver = r#"
import base64,io,json,os,signal,sys
source=sys.argv[1]
sys.argv=['owned-supervisor',base64.b64encode(b'{}').decode(),base64.b64encode(b'import time; time.sleep(30)').decode()]
class Sink:
    def __init__(self): self.buffer=io.BytesIO()
sink=Sink(); original_stdout=sys.stdout; original_killpg=os.killpg
def denied(*_): raise PermissionError('owned injected signal denial')
os.killpg=denied; sys.stdout=sink; actual_reap=None
try:
    exec(compile(source,'fixed-remote-supervisor','exec'),globals())
    packet=sink.buffer.getvalue()
    assert packet.startswith(b'KEELSHELL_DIAGNOSTIC_V1\n')
    report=json.loads(packet.split(b'\n',1)[1])
    was_live=child.poll() is None
finally:
    os.killpg=original_killpg; sys.stdout=original_stdout
    if 'child' in globals() and child is not None:
        if child.poll() is None: original_killpg(child.pid,signal.SIGKILL)
        child.communicate(timeout=2); actual_reap=child.returncode
assert was_live and actual_reap is not None, 'owned denial fixture must be contained'
print(json.dumps({'status':report['status'],'worker_was_live':was_live,'fixture_actual_wait_exit':actual_reap}))
assert report['status']=='cleanup_unknown', report
"#;
        let output = std::process::Command::new("python3")
            .args(["-I", "-S", "-B", "-c", driver, SCRIPT])
            .output()
            .unwrap_or_else(|error| panic!("owned signal-denial prerequisite: {error}"));
        println!(
            "owned worker-denial lifecycle {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            output.status.success(),
            "denied signal outcome: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("\"fixture_actual_wait_exit\": -9")
        );
    }
}
