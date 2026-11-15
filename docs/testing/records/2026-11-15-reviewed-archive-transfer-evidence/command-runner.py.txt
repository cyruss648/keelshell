import datetime, hashlib, json, os, signal, subprocess, sys, time
from pathlib import Path
root = Path(__file__).resolve().parents[2]
out = Path(__file__).resolve().parent
label, duration, *command = sys.argv[1:]
if not command or not label.replace('-', '').isalnum():
    raise SystemExit('invalid command label')
log = out / 'logs' / (label + '.log')
receipt = out / 'logs' / (label + '.json')
if log.exists() or receipt.exists():
    raise SystemExit('refusing to replace evidence')
env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'work/recovery-batch-integration-20261115/target'), CARGO_BUILD_JOBS='4', CARGO_NET_OFFLINE='true', TMPDIR=str(out / 'tmp') + '/', PYTHONDONTWRITEBYTECODE='1')
start = datetime.datetime.now(datetime.timezone.utc).isoformat()
then = time.monotonic()
timed_out = False
with log.open('xb') as stream:
    proc = subprocess.Popen(command, cwd=root, env=env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
    print('START', label, proc.pid, flush=True)
    try:
        code = proc.wait(timeout=float(duration))
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(proc.pid, signal.SIGTERM)
        try:
            code = proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            code = proc.wait()
record = dict(command=command, cwd=str(root), label=label, pid=proc.pid, started_utc=start, finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), seconds=round(time.monotonic()-then,3), exit=code, timed_out=timed_out, log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(), target=env['CARGO_TARGET_DIR'])
with receipt.open('x') as stream:
    json.dump(record, stream, indent=2)
    stream.write('\n')
print(json.dumps(record), flush=True)
print('\n'.join(log.read_text(errors='replace').splitlines()[-18:]), flush=True)
raise SystemExit(code if not timed_out else 124)
