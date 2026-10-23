#!/usr/bin/env bash
# Collection/exit propagation only. These fake tests are never proof evidence.
set -euo pipefail
cd "$(dirname "$0")/../.."
python - <<'PY'
from pathlib import Path
import os,re,subprocess,tempfile
source=Path('scripts/gates.sh').read_text()
block=re.search(r'gate "pytest" .*?(?=\ngate )',source,re.S).group(0)
fast='specs/511-safety-certification-scale/experiments/tests'
heavy=fast+'/test_scale_acceptance.py'
assert fast in block, 'mandatory fast scale controls are absent from existing pytest gate'
assert '--ignore='+heavy in block, 'real full-scale proof entrypoint must be excluded from fast collection'
from models.examples.safety_models.run import REQUIRED_ROOTS
for name in ['Safety.Candidate.traceChecked_sound','Safety.Candidate.compactPayload_flatten','Safety.Candidate.literalBytes_injective','Safety.Candidate.checked_from_parts']:
    assert name in REQUIRED_ROOTS, 'mandatory new audited root is absent from proof aggregation: '+name
with tempfile.TemporaryDirectory() as td:
    root=Path(td)
    (root/'python/tests').mkdir(parents=True);(root/'models').mkdir()
    (root/'python/tests/test_existing.py').write_text('def test_existing(): assert True\n')
    tests=root/fast;tests.mkdir(parents=True)
    (tests/'test_scale_acceptance.py').write_text('raise AssertionError("MUST NOT COLLECT FULL SCALE")\n')
    script='set -euo pipefail\ngate() { local name="$1"; shift; if ! "$@"; then exit 1; fi; }\n'+block+'\n'
    for passes in [True,False]:
        (tests/'test_fast.py').write_text(f'def test_fast(): assert {passes}\n')
        result=subprocess.run(['bash','-c',script],cwd=root,env={k:v for k,v in os.environ.items() if k not in ['PYTEST_ADDOPTS','PYTHONPATH']},text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        assert (result.returncode==0)==passes,result.stdout
        assert 'MUST NOT COLLECT FULL SCALE' not in result.stdout,result.stdout
        if passes:assert '2 passed' in result.stdout,result.stdout
print('test_safety_scale: collection and failure propagation OK; not proof evidence')
PY
