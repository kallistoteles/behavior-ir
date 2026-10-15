#!/usr/bin/env bash
# Tests of scripts/check-terms.sh: the model documents never call models plugins or extensions
# (feature 011, FR-013), and the approval tests never call a refused attempt rejected (feature
# 503, T051).
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf 'Models lower to Behavior IR.\n' >"$tmp/clean.md"
printf 'A state machine plugin.\n' >"$tmp/planted.md"
printf 'Extensions add semantics.\n' >"$tmp/planted2.md"
# Approval: outcome and choice uses of REJECT/REJECTED are fine.
cat >"$tmp/test_approval_clean.py" <<'PY'
def test_veto_first_reject_is_rejected(): assert outcome == "REJECTED"
def test_decision_after_rejected_is_denied_terminal(): assert d.result == "DENY"
PY
printf 'def test_second_approve_is_rejected(): assert d.result == "DENY"\n' \
  >"$tmp/test_approval_attempt.py"
printf '// the stale commit was rejected\n' >"$tmp/approval_comment.rs"
printf '    assert d.result == "DENY"  # approve by a1 again -> rejected\n' \
  >"$tmp/test_approval_arrow.py"
for f in clean.md test_approval_clean.py; do
  "$here/check-terms.sh" "$tmp/$f" >/dev/null || { echo "FAIL clean $f"; exit 1; }
done
for f in planted.md planted2.md test_approval_attempt.py approval_comment.rs test_approval_arrow.py; do
  if "$here/check-terms.sh" "$tmp/$f" >"$tmp/out" 2>&1 || ! grep -q "$f" "$tmp/out"; then
    echo "FAIL $f: $(cat "$tmp/out")"; exit 1
  fi
done
echo "test_check_terms: OK"
