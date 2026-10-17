#!/usr/bin/env bash
# The end-to-end conformance suites: feature 506 (contract conformance-report.md) and feature 507
# (WorkQueue, research R9), on one shared report machinery (models/conformance/report.py).
#
#   scripts/conformance.sh <506|507> [quick|full|complete]
#                                     (default mode: full when BEHAVIOR_FULL_PROPERTIES=1)
#
# 1. the feature's Rust halves, each recording its result for the suite to read;
# 2. the feature's pytest suite, which writes target/conformance/<feature>-report.json;
# 3. the gate: quick fails on any `failed` check or obligation; full (the integration gate CI
#    runs) on any status other than passed/blocked; complete (full conformance) on anything not
#    passed.
#
# Each feature owns only its own files in target/conformance; running one never removes the
# other's report.
set -euo pipefail
cd "$(dirname "$0")/.."
feature="${1:-}"
case "$feature" in
  506|507) ;;
  *) echo "conformance: usage: $0 <506|507> [quick|full|complete]" >&2; exit 2 ;;
esac
mode="${2:-$([ "${BEHAVIOR_FULL_PROPERTIES:-}" = 1 ] && echo full || echo quick)}"
case "$mode" in quick|full|complete) ;; *) echo "conformance: unknown mode $mode" >&2; exit 2 ;; esac
# The pinned Core Release only (constitution, Bindings and Packaging): no CLI override.
if [ -n "${BEHAVIOR_CLI_BINARY:-}" ]; then
  echo "conformance: BEHAVIOR_CLI_BINARY is set; the suite runs only the pinned bundled CLI" >&2
  exit 2
fi
# T047's reviewed 35-second budget applies to the complete 507 quick run with the prerequisites
# already built (quickstart). Keep timing out of the deterministic conformance JSON.
quick_started=""
if [ "$feature" = 507 ] && [ "$mode" = quick ]; then
  quick_started="$(python -c 'import time; print(time.monotonic())')"
fi
export BEHAVIOR_CONFORMANCE_DIR="$PWD/target/conformance"
mkdir -p "$BEHAVIOR_CONFORMANCE_DIR"
if [ "$mode" = quick ]; then unset BEHAVIOR_FULL_PROPERTIES; else export BEHAVIOR_FULL_PROPERTIES=1; fi
out="$BEHAVIOR_CONFORMANCE_DIR"

rust_rc=0
case "$feature" in
  506)
    suite=models/examples/purchase_approval
    module=models.examples.purchase_approval.report
    rm -f "$out/506-report.json" "$out"/c16a-rust.* "$out"/c12-rust.*
    # A failing or uncompilable Rust half must still leave a report: record it and go on, so
    # C16a is reported failed (with the log) and every other scenario still runs.
    if ! cargo test -q -p behavior-models --test e2e_reference >"$out/c16a-rust.log" 2>&1; then
      rust_rc=1
      cp "$out/c16a-rust.log" "$out/c16a-rust.failed"
      echo "conformance: the Rust half of C16a failed (see c16a-rust.log)" >&2
    fi
    log="$out/c12-rust.log"
    if cargo test -q -p behavior-models --lib child_state >"$log" 2>&1; then passed=true; else passed=false; fi
    tests="$(sed -n 's/^test result: [a-zA-Z]*\. \([0-9]*\) passed.*/\1/p' "$log" | head -1)"
    printf '{"passed":%s,"tests":%s}\n' "$passed" "${tests:-0}" >"$out/c12-rust.json"
    ;;
  507)
    suite=models/examples/work_queue
    module=models.examples.work_queue.report
    rm -f "$out/507-report.json" "$out"/w20-rust.*
    # W20: the independent Rust authoring of the reference queue writes its identities.
    if ! cargo test -q -p behavior-models --test work_queue_reference >"$out/w20-rust.log" 2>&1; then
      rust_rc=1
      cp "$out/w20-rust.log" "$out/w20-rust.failed"
      echo "conformance: the Rust half of W20 failed (see w20-rust.log)" >&2
    fi
    ;;
esac

pytest_rc=0
python -m pytest -q -p no:cacheprovider "$suite" || pytest_rc=$?
report="$out/$feature-report.json"
if [ ! -f "$report" ]; then
  echo "conformance: no report was written (pytest exit $pytest_rc)" >&2
  exit 1
fi
gate_rc=0
python -m "$module" "$report" "$mode" || gate_rc=$?
runtime_rc=0
if [ -n "$quick_started" ]; then
  python - "$quick_started" <<'PY' || runtime_rc=$?
import sys
import time

elapsed = time.monotonic() - float(sys.argv[1])
print(f"conformance 507 quick runtime: {elapsed:.2f}s / 35s")
if elapsed > 35:
    print("conformance: FAILED 507 quick runtime budget (35s)", file=sys.stderr)
    sys.exit(1)
PY
fi
# pytest's own exit status counts too: a failure outside a scenario test (a collection error,
# an unmarked test) is not in the report, but it must still fail the run.
if [ "$rust_rc" -ne 0 ] || [ "$pytest_rc" -ne 0 ] || [ "$gate_rc" -ne 0 ] || [ "$runtime_rc" -ne 0 ]; then
  [ "$pytest_rc" -ne 0 ] && echo "conformance: pytest exited $pytest_rc" >&2
  exit 1
fi
