#!/usr/bin/env bash
# Self-tests of the feature-506 conformance report (contract conformance-report.md §5): every
# outcome maps to its status, nothing that did not pass is reported as passed, and the report is
# deterministic.
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
PY

run() { # run <out dir> [extra env...]
  ( cd "$tmp/proj" && env PYTHONPATH="$root" BEHAVIOR_CONFORMANCE_DIR="$1" \
      BEHAVIOR_CONFORMANCE_DECLARED="S-pass,S-fail,S-error,S-skip,S-inc,S-unv,S-uns,S-blk,S-noowner,S-ready,S-missing" \
      BEHAVIOR_CONFORMANCE_READINESS='[{"issue":"behavior-ir-core#99","affects":["S-ready"],"reason":"open"}]' \
      python -m pytest -q -p no:cacheprovider . >/dev/null 2>&1 ) || true
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
        "S-blk": "blocked", "S-noowner": "failed", "S-ready": "blocked", "S-missing": "failed"}
bad = [f"{k}: {s.get(k, {}).get('status')} != {v}" for k, v in want.items()
       if s.get(k, {}).get("status") != v]
assert s["S-missing"]["reason"] == "not run", s["S-missing"]
assert all(e["owner"] for e in r["checks"] if e["status"] not in ("passed", "failed"))
assert r["summary"]["passed"] == 1, r["summary"]
assert r["complete"] is False
if bad:
    print("test_conformance_report:", bad, file=sys.stderr); sys.exit(1)
PY
cmp -s "$tmp/r1/506-report.json" "$tmp/r2/506-report.json" || fail "reports differ between runs"

# Gate rules: quick passes with only an unsupported C19; full fails with it.
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
