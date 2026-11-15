"""Validate final declarations and lock against the same retained official replies."""
import gzip
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('batch_dependency_check_snapshot', ROOT / 'scripts/check_dependencies.py')
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
previous = json.loads((OUT / 'dependency-report-r3.json').read_text())


def fetch(url, _timeout):
    if not url.startswith(module.API_ROOT) or not module.NAME.fullmatch(url[len(module.API_ROOT):]):
        raise module.CheckFailure('non_official_url_rejected')
    name = url[len(module.API_ROOT):]
    return json.loads(gzip.decompress((OUT / 'registry-metadata-r3' / (name + '.json.gz')).read_bytes()))


report = module.run_check(ROOT, fetch=fetch, timeout=6, jobs=8)
report['metadata_route'] = 'retained official responses from dependency-report-r3, without another network query'
report['metadata_collected_at_utc'] = previous['checked_at_utc']
report['metadata_completed_at_utc'] = previous['completed_at_utc']
report['raw_public_responses'] = previous['raw_public_responses']
(OUT / 'dependency-report-r4.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'status': report['status'], 'requirements': len(report['rust']), 'raw_responses': len(report['raw_public_responses']), 'errors': report['errors']}))
raise SystemExit(0 if report['status'] == 'passed' else 1)
