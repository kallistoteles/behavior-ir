#!/usr/bin/env bash
# Determinism gate of the ecosystem (constitution: Development Workflow and Quality Gates).
# Runs every deterministic operation twice and compares the outputs byte for byte:
#   - the bundled core CLI over the pinned release's wire fixtures (section `core`: the engine's
#     own outputs, labelled as the core's determinism check labels them);
#   - the release smoke scenario and the Python examples (section `ecosystem`).
# The core's full check runs in behavior-ir-core (feature 011).
#
#   scripts/determinism-check.sh [--ecosystem]
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-}" in "" | --ecosystem) ;; *) echo "usage: scripts/determinism-check.sh [--ecosystem]" >&2; exit 2 ;; esac

core_version="$(python3 -c 'import json; print(json.load(open("core-release.json"))["version"])')"
CORE_DIR="${BEHAVIOR_CORE_DIR:-.core/$core_version}"
BIN=python/behavior/_bin/behavior
if [ ! -x "$BIN" ] || [ ! -d "$CORE_DIR/tests/fixtures" ]; then
  echo "determinism-check: the pinned core is not installed; run scripts/fetch-core.sh first" >&2
  exit 1
fi

fail=0
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# digest <label> <file>: with BEHAVIOR_DIGEST_DIR set, records the output's SHA-256 under its
# section and label (scripts/conformance-digest.sh).
digest() {
  if [ -n "${BEHAVIOR_DIGEST_DIR:-}" ]; then
    printf '%s\t%s\t%s\n' "$section" "$1" "$(sha256sum <"$2" | cut -d' ' -f1)" \
      >>"$BEHAVIOR_DIGEST_DIR/outputs.tsv"
  fi
}

# run_twice <label> <command...>: exit codes and stdout must match between runs.
run_twice() {
  local label="$1"; shift
  local rc1=0 rc2=0
  "$@" >"$tmp/a" 2>/dev/null || rc1=$?
  "$@" >"$tmp/b" 2>/dev/null || rc2=$?
  if [ "$rc1" -ne "$rc2" ] || ! cmp -s "$tmp/a" "$tmp/b"; then
    echo "NOT DETERMINISTIC: $label" >&2
    fail=1
  fi
  digest "$label" "$tmp/a"
}

# The bundled core CLI admits every valid wire fixture of the pinned release.
section=core
for f in "$CORE_DIR"/tests/fixtures/wire/valid/*.json; do
  rel="tests/fixtures/wire/valid/$(basename "$f")"
  run_twice "admit $rel" "$BIN" admit "$f"
  rc=0; "$BIN" admit "$f" >/dev/null || rc=$?
  if [ "$rc" -ne 0 ]; then echo "ADMISSION FAILED: $rel" >&2; fail=1; fi
done

# The Python examples print records; two runs must be byte-identical.
section=ecosystem
if python3 -c "import behavior._engine" 2>/dev/null; then
  # The consumer smoke scenario of a release (feature 008), run from outside the repository.
  cp release/smoke.py "$tmp/smoke.py"
  run_twice "release/smoke.py" python3 "$tmp/smoke.py"
  # Deterministic is not enough: the scenario must also succeed (a failed check exits 1 the same
  # way every time).
  if ! python3 "$tmp/smoke.py" >/dev/null 2>"$tmp/smoke.err" || ! grep -q 'smoke: OK' "$tmp/smoke.err"; then
    echo "SMOKE SCENARIO FAILED: $(tail -n 3 "$tmp/smoke.err")" >&2
    fail=1
  fi
  run_twice "examples.invoice.run" python3 -m examples.invoice.run
  run_twice "examples.project_margin.run" python3 -m examples.project_margin.run
  run_twice "examples.accounts.run" python3 -m examples.accounts.run
  run_twice "examples.orders.run" python3 -m examples.orders.run
  run_twice "examples.lab_reads.run" python3 -m examples.lab_reads.run
else
  echo "determinism-check: the Python engine is not built; run maturin develop" >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo "determinism-check: FAILED" >&2
  exit 1
fi
echo "determinism-check: OK"
