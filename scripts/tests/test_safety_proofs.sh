#!/usr/bin/env bash
# Fail-closed aggregation controls. Real proofs run in the mandatory named gate;
# these mocks exercise failures, never stand in for theorem evidence.
set -euo pipefail
cd "$(dirname "$0")/../.."
python - <<'PY'
import copy
import json
import tempfile
from pathlib import Path
from unittest.mock import patch
from models.examples.safety_models import run

roots=run.REQUIRED_ROOTS
passed={"outcome":"Certified under assumptions","obligations":{key:"Passed" for key in run.REQUIRED_OBLIGATIONS},
        "proof":{"audit":[{"declaration":r,"expected_type_checked":True,"axioms":[]} for r in roots]}}
assert run.required_assessment(passed)
for status in ["Unsupported","Inconclusive","Failed","Lowered"]:
    changed=copy.deepcopy(passed);changed["outcome"]=status
    assert not run.required_assessment(changed)
for key in run.REQUIRED_OBLIGATIONS:
    for value in ["NotRun","Failed",True,1,None]:
        changed=copy.deepcopy(passed);changed["obligations"][key]=value
        assert not run.required_assessment(changed)
for root in roots:
    changed=copy.deepcopy(passed);changed["proof"]["audit"]=[r for r in changed["proof"]["audit"] if r["declaration"]!=root]
    assert not run.required_assessment(changed)
for key,value in [("expected_type_checked",False),("expected_type_checked",1),("axioms",["sorryAx"])]:
    changed=copy.deepcopy(passed);changed["proof"]["audit"][0][key]=value
    assert not run.required_assessment(changed)
assert not run.required_report({"cases":{name:{"assessment":passed,"certified":True} for name in run.CASES[:-1]}})
for failure in [FileNotFoundError("missing cargo"),RuntimeError("early Rust failure")]:
    with tempfile.TemporaryDirectory() as td, patch.object(run,"rust_preflight",side_effect=failure):
        assert run.main(["--gate","--certify","--output",td])!=0
        doc=json.loads((Path(td)/"summary.json").read_text())
        assert set(doc["cases"])==set(run.CASES)
        assert all(r["status"]=="NotRun" for r in doc["cases"].values())
        assert doc["errors"]
        assert doc["passed"] is False
# Active overrides and a missing selected tool must leave a report, not skip proofs.
with tempfile.TemporaryDirectory() as td, patch.dict("os.environ",{"BEHAVIOR_LEAN_BINARY":"/missing"}):
    assert run.main(["--gate","--certify","--output",td])!=0
    assert (Path(td)/"summary.json").is_file()
with tempfile.TemporaryDirectory() as td, patch.object(run,"rust_preflight"), patch.object(run,"certify_safety",side_effect=FileNotFoundError("Lean missing or wrong")):
    assert run.main(["--gate","--certify","--output",td])!=0
    doc=json.loads((Path(td)/"summary.json").read_text())
    assert all(r["status"]=="Inconclusive" for r in doc["cases"].values())
print("test_safety_proofs: OK (failure aggregation only; real proofs are mandatory separately)")
PY

# Exercise the real command boundary as well as the aggregation functions.
# This controlled cargo process exits before any Rust tests; it is not proof evidence.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"
cat >"$tmp/bin/cargo" <<'CARGO'
#!/bin/sh
exit 42
CARGO
chmod +x "$tmp/bin/cargo"
if PATH="$tmp/bin:$PATH" scripts/check-safety-proofs.sh --output "$tmp/early-rust" >"$tmp/rust.log" 2>&1; then
  echo 'test_safety_proofs: accepted an early Rust failure' >&2; exit 1
fi
python - "$tmp/early-rust/summary.json" <<'PY'
import json,sys
from models.examples.safety_models.run import CASES
report=json.load(open(sys.argv[1]))
assert report['rust_preflight']=='Failed'
assert report['passed'] is False
assert set(report['cases'])==set(CASES)
assert all(row['status']=='NotRun' for row in report['cases'].values())
assert report['errors']
PY
if BEHAVIOR_LEAN_BINARY="$tmp/nonexistent" scripts/check-safety-proofs.sh --output "$tmp/rejected-override" >"$tmp/tools.log" 2>&1; then
  echo 'test_safety_proofs: accepted an executable override' >&2; exit 1
fi
python - "$tmp/rejected-override/summary.json" <<'PYCODE'
import json,sys
report=json.load(open(sys.argv[1]))
assert report['errors']
assert all(row['status']=='NotRun' for row in report['cases'].values())
PYCODE
echo 'test_safety_proofs: real command failure controls OK'
