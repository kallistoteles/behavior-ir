#!/usr/bin/env bash
# Self-tests of the shared conformance report (feature 506 contract conformance-report.md §5;
# feature 507 research R10): every outcome maps to its status, nothing that did not pass is
# reported as passed, the report is deterministic, and an obligation is never better than its
# weakest evidence dependency.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0
fail() { echo "test_conformance_report: $1" >&2; failures=$((failures + 1)); }

mkdir -p "$tmp/proj"
cat >"$tmp/proj/conftest.py" <<'PY'
from models.examples.purchase_approval.report import *  # noqa: F401,F403
PY
cat >"$tmp/proj/test_outcomes.py" <<'PY'
import pytest
from models.examples.purchase_approval.report import (
    Blocked, Inconclusive, Unsupported, Unverifiable)

@pytest.mark.scenario("S-pass", level=1)
def test_pass(): pass

@pytest.mark.scenario("S-fail", level=1)
def test_fail(): assert False, "expected failure"

@pytest.mark.scenario("S-error", level=1)
def test_error(): raise RuntimeError("boom")

@pytest.mark.scenario("S-skip", level=1)
def test_skip(): pytest.skip("not allowed")

@pytest.mark.scenario("S-inc", level=4)
def test_inconclusive(): raise Inconclusive("solver unknown", "core")

@pytest.mark.scenario("S-unv", level=4)
def test_unverifiable(): raise Unverifiable("context missing", "ecosystem")

@pytest.mark.scenario("S-uns", level=4)
def test_unsupported(): raise Unsupported("quick mode", "environment")

@pytest.mark.scenario("S-blk", level=4)
def test_blocked(): raise Blocked("upstream", "behavior-ir-core#31")

@pytest.mark.scenario("S-noowner", level=4)
def test_no_owner(): raise Unsupported("no owner", "")

@pytest.mark.scenario("S-ready", level=2)
def test_readiness_blocks(): pass

@pytest.mark.scenario("S-run", level=4)
def test_readiness_with_run_keeps_its_own_status(): pass
PY

run() { # run <out dir> [extra env...]
  ( cd "$tmp/proj" && env PYTHONPATH="$root" BEHAVIOR_CONFORMANCE_DIR="$1" \
      BEHAVIOR_CONFORMANCE_DECLARED="S-pass,S-fail,S-error,S-skip,S-inc,S-unv,S-uns,S-blk,S-noowner,S-ready,S-run,S-missing" \
      BEHAVIOR_CONFORMANCE_READINESS='[{"issue":"behavior-ir-core#99","affects":["S-ready"],"reason":"open"},{"issue":"behavior-ir-core#98","affects":["S-run"],"run":true,"reason":"open"}]' \
      "${@:2}" python -m pytest -q -p no:cacheprovider . >/dev/null 2>&1 ) || true
}
run "$tmp/r1"
run "$tmp/r2"
report="$tmp/r1/506-report.json"
[ -f "$report" ] || { echo "test_conformance_report: no report written" >&2; exit 1; }

python3 - "$report" <<'PY' || failures=$((failures + 1))
import json, sys
r = json.load(open(sys.argv[1]))
s = {e["id"]: e for e in r["checks"]}
want = {"S-pass": "passed", "S-fail": "failed", "S-error": "failed", "S-skip": "failed",
        "S-inc": "inconclusive", "S-unv": "unverifiable", "S-uns": "unsupported",
        "S-blk": "blocked", "S-noowner": "failed", "S-ready": "blocked", "S-missing": "failed",
        "S-run": "passed"}
bad = [f"{k}: {s.get(k, {}).get('status')} != {v}" for k, v in want.items()
       if s.get(k, {}).get("status") != v]
assert s["S-missing"]["reason"] == "not run", s["S-missing"]
assert all(e["owner"] for e in r["checks"] if e["status"] not in ("passed", "failed"))
assert r["summary"]["passed"] == 2, r["summary"]
assert r["complete"] is False
if bad:
    print("test_conformance_report:", bad, file=sys.stderr); sys.exit(1)
PY
cmp -s "$tmp/r1/506-report.json" "$tmp/r2/506-report.json" || fail "reports differ between runs"
python3 -c 'import json,sys; sys.exit("obligations" in json.load(open(sys.argv[1])))' "$report" \
  || fail "a report without obligations must have no obligations key"

# Obligations (feature 507, research R10): the weakest dependency decides; assumptions never do.
run "$tmp/r3" env BEHAVIOR_CONFORMANCE_OBLIGATIONS='{
  "VO-A": {"dependencies": [{"class": "test", "check": "S-pass"}]},
  "VO-B": {"dependencies": [{"class": "test", "check": "S-pass"}, {"class": "structural", "check": "S-fail"}]},
  "VO-C": {"dependencies": [{"class": "smt", "check": "S-nowhere"}]},
  "VO-D": {"dependencies": [{"class": "contract", "check": "S-pass"}], "assumptions": ["trusted time"]},
  "VO-E": {"dependencies": [{"class": "test", "check": "S-pass"}, {"class": "smt", "check": "S-blk"}]},
  "VO-F": {"dependencies": [{"class": "folklore", "check": "S-pass"}]},
  "VO-G": {"dependencies": [], "assumptions": ["only an assumption"]}}'
python3 - "$tmp/r3/506-report.json" <<'PY' || failures=$((failures + 1))
import json, sys
o = json.load(open(sys.argv[1]))["obligations"]
want = {"VO-A": "passed", "VO-B": "failed", "VO-C": "failed", "VO-D": "passed",
        "VO-E": "blocked", "VO-F": "failed", "VO-G": "failed"}
bad = [f"{k}: {o[k]['status']} != {v}" for k, v in want.items() if o[k]["status"] != v]
assert o["VO-C"]["dependencies"][0]["reason"] == "not run", o["VO-C"]
assert o["VO-D"]["assumptions"] == ["trusted time"], o["VO-D"]
if bad:
    print("test_conformance_report:", bad, file=sys.stderr); sys.exit(1)
PY
printf '{"summary":{},"checks":[{"id":"C01","status":"passed","reason":null}],"obligations":{"VO-X":{"status":"failed"}}}' >"$tmp/o.json"
out="$( cd "$root" && python -m models.examples.purchase_approval.report "$tmp/o.json" quick 2>&1 )" \
  && fail "quick mode must fail with a failed obligation"
grep -q '^  VO-X: failed$' <<<"$out" || fail "the gate must name the failed obligation: $out"

# Gate rules: quick passes with only an unsupported C19; full fails with it.
# The WorkQueue induction arguments depend on the forbidden-writer negative control too.
python3 - <<'PY' || failures=$((failures + 1))
from models.examples.work_queue.report import OBLIGATIONS
from models.conformance.report import obligation_entries
checks = {d["check"]: {"status": "passed", "reason": None}
          for o in OBLIGATIONS.values() for d in o["dependencies"]}
for check, affected in (("NC-writer", ("VO-003", "VO-005", "VO-007", "VO-011")),
                        ("NC-generation", ("VO-005",)), ("NC-expiry", ("VO-005",)),
                        ("SA-OWN", ("VO-007", "VO-011"))):
    original = checks[check]
    checks[check] = {"status": "failed", "reason": "injected premise regression"}
    out = obligation_entries(checks)
    for vo in affected:
        assert out[vo]["status"] == "failed", (check, vo, out[vo])
    checks[check] = original
PY

printf '{"summary":{},"checks":[{"id":"C19","status":"unsupported","reason":"quick mode"},{"id":"C01","status":"passed","reason":null},{"id":"Cb","status":"blocked","reason":"x"}]}' >"$tmp/g.json"
( cd "$root" && python -m models.examples.purchase_approval.report "$tmp/g.json" quick >/dev/null ) \
  || fail "quick mode must pass with an unsupported C19"
( cd "$root" && python -m models.examples.purchase_approval.report "$tmp/g.json" full >/dev/null ) \
  && fail "full mode must fail with an unsupported C19"

# A blocked check passes the integration gate (full) but never full conformance (complete).
printf '{"summary":{},"complete":false,"checks":[{"id":"C19","status":"blocked","reason":"core#35","owner":"behavior-ir-core#35"},{"id":"C01","status":"passed","reason":null}]}' >"$tmp/b.json"
( cd "$root" && python -m models.examples.purchase_approval.report "$tmp/b.json" full >/dev/null ) \
  || fail "full mode must pass with a blocked check"
( cd "$root" && python -m models.examples.purchase_approval.report "$tmp/b.json" complete >/dev/null ) \
  && fail "complete mode must fail with a blocked check"

if [ "$failures" -ne 0 ]; then echo "test_conformance_report: $failures failure(s)" >&2; exit 1; fi
echo "test_conformance_report: OK"
