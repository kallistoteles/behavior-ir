#!/usr/bin/env bash
# Tests of scripts/check-terms.sh (feature 011, FR-013): the model documents never call models
# plugins or extensions.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf 'Models lower to Behavior IR.\n' >"$tmp/clean.md"
printf 'A state machine plugin.\n' >"$tmp/planted.md"
printf 'Extensions add semantics.\n' >"$tmp/planted2.md"
"$here/check-terms.sh" "$tmp/clean.md" >/dev/null || { echo "FAIL clean"; exit 1; }
for f in planted planted2; do
  if "$here/check-terms.sh" "$tmp/$f.md" >"$tmp/out" 2>&1 || ! grep -q "$f.md" "$tmp/out"; then
    echo "FAIL $f: $(cat "$tmp/out")"; exit 1
  fi
done
echo "test_check_terms: OK"
