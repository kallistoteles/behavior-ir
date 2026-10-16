#!/usr/bin/env bash
# Feature 506: the end-to-end behaviour conformance suite (contract conformance-report.md).
#
#   scripts/conformance.sh [quick|full|complete]   (default: full when BEHAVIOR_FULL_PROPERTIES=1)
#
# 1. the Rust halves: the independent Rust authoring of the reference workflow (C16a) and the
#    model authority's child-state tests (C12), each recording its result;
# 2. the pytest suite, which writes target/conformance/506-report.json;
# 3. the gate: quick fails on any `failed` check; full (the integration gate CI runs) on any
#    status other than passed/blocked; complete (full conformance) on anything not passed.
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-$([ "${BEHAVIOR_FULL_PROPERTIES:-}" = 1 ] && echo full || echo quick)}"
case "$mode" in quick|full|complete) ;; *) echo "conformance: unknown mode $mode" >&2; exit 2 ;; esac
# The pinned Core Release only (constitution, Bindings and Packaging): no CLI override.
if [ -n "${BEHAVIOR_CLI_BINARY:-}" ]; then
  echo "conformance: BEHAVIOR_CLI_BINARY is set; the suite runs only the pinned bundled CLI" >&2
  exit 2
fi
export BEHAVIOR_CONFORMANCE_DIR="$PWD/target/conformance"
rm -rf "$BEHAVIOR_CONFORMANCE_DIR"
mkdir -p "$BEHAVIOR_CONFORMANCE_DIR"
if [ "$mode" = quick ]; then unset BEHAVIOR_FULL_PROPERTIES; else export BEHAVIOR_FULL_PROPERTIES=1; fi

# A failing or uncompilable Rust half must still leave a report: record it and go on, so C16a is
# reported failed (with the log) and every other scenario still runs.
rust_rc=0
if ! cargo test -q -p behavior-models --test e2e_reference \
    >"$BEHAVIOR_CONFORMANCE_DIR/c16a-rust.log" 2>&1; then
  rust_rc=1
  cp "$BEHAVIOR_CONFORMANCE_DIR/c16a-rust.log" "$BEHAVIOR_CONFORMANCE_DIR/c16a-rust.failed"
  echo "conformance: the Rust half of C16a failed (see c16a-rust.log)" >&2
fi

log="$BEHAVIOR_CONFORMANCE_DIR/c12-rust.log"
if cargo test -q -p behavior-models --lib child_state >"$log" 2>&1; then passed=true; else passed=false; fi
tests="$(sed -n 's/^test result: [a-zA-Z]*\. \([0-9]*\) passed.*/\1/p' "$log" | head -1)"
printf '{"passed":%s,"tests":%s}\n' "$passed" "${tests:-0}" >"$BEHAVIOR_CONFORMANCE_DIR/c12-rust.json"

pytest_rc=0
python -m pytest -q -p no:cacheprovider models/examples/purchase_approval || pytest_rc=$?
report="$BEHAVIOR_CONFORMANCE_DIR/506-report.json"
if [ ! -f "$report" ]; then
  echo "conformance: no report was written (pytest exit $pytest_rc)" >&2
  exit 1
fi
gate_rc=0
python -m models.examples.purchase_approval.report "$report" "$mode" || gate_rc=$?
# pytest's own exit status counts too: a failure outside a scenario test (a collection error,
# an unmarked test) is not in the report, but it must still fail the run.
if [ "$rust_rc" -ne 0 ] || [ "$pytest_rc" -ne 0 ] || [ "$gate_rc" -ne 0 ]; then
  [ "$pytest_rc" -ne 0 ] && echo "conformance: pytest exited $pytest_rc" >&2
  exit 1
fi
