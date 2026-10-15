#!/usr/bin/env bash
# Terms the model documents and tests must keep straight.
#
# 1. Models compile downward; the documents that define them never call them plugins or
#    extensions (feature 011, FR-013), words that suggest components changing runtime semantics.
# 2. Approval (feature 503, T051): an attempt that is not permitted is DENIED. REJECT is a
#    recorded choice and REJECTED a derived outcome, so an attempt is never "rejected". In the
#    approval tests, a refused attempt called rejected fails: an attempt word followed by
#    `is/was/gets/got rejected` (`second_approve_is_rejected`, "the retry was rejected"), or an
#    arrow to `rejected` on a line about a denial.
#
#   scripts/check-terms.sh [file...]      # default: the models area and the approval tests
set -euo pipefail
cd "$(dirname "$0")/.."
if [ $# -eq 0 ]; then
  mapfile -t models < <(git ls-files --cached --others --exclude-standard -- models | grep -E '\.(md|py)$')
  mapfile -t approval < <(git ls-files --cached --others --exclude-standard -- \
    'python/tests/test_approval*.py' 'crates/behavior-models/tests/approval_*.rs')
else
  models=("$@")
  approval=()
  for f in "$@"; do
    case "$(basename "$f")" in test_approval*.py | approval_*.rs) approval+=("$f") ;; esac
  done
fi
fail=0
if [ ${#models[@]} -gt 0 ] && grep -HniE '\b(plugin|extension)s?\b' "${models[@]}" >&2; then
  echo "check-terms: FAILED: name models by what they are (lowered to Behavior IR)" >&2
  fail=1
fi
attempt='(attempt|second|retry|stale|decision|call|commit|approve)[a-z_]*[ _](is|was|gets|got)[ _]rejected\b'
arrow='(->|→|=>) *rejected\b.*(DENY|denied|refused)|(DENY|denied|refused).*(->|→|=>) *rejected\b'
if [ ${#approval[@]} -gt 0 ] && grep -HnE "$attempt|$arrow" "${approval[@]}" >&2; then
  echo "check-terms: FAILED: a refused attempt is DENIED; REJECT/REJECTED name the choice and outcome" >&2
  fail=1
fi
[ "$fail" -eq 0 ] || exit 1
echo "check-terms: OK"
