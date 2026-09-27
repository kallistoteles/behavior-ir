#!/usr/bin/env bash
# Determinism gate (constitution: Development Workflow and Quality Gates).
# Runs every deterministic operation twice and compares the outputs byte for byte.
set -euo pipefail

cd "$(dirname "$0")/.."
BIN="${BEHAVIOR_BIN:-target/debug/behavior}"
if [ ! -x "$BIN" ]; then
  echo "determinism-check: $BIN not found; run 'cargo build --workspace' first" >&2
  exit 1
fi

fail=0
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

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
}

# Admission of every valid wire fixture.
for f in tests/fixtures/wire/valid/*.json; do
  run_twice "admit $f" "$BIN" admit "$f"
  rc=0; "$BIN" admit "$f" >/dev/null || rc=$?
  if [ "$rc" -ne 0 ]; then echo "ADMISSION FAILED: $f" >&2; fail=1; fi
done

# Evaluation and replay of every request fixture (US2).
if [ -f tests/fixtures/requests/expectations.json ]; then
  while IFS=' ' read -r name wire; do
    w="tests/fixtures/wire/valid/$wire.json"
    r="tests/fixtures/requests/$name.json"
    run_twice "eval $name" "$BIN" eval "$w" "$r"
    "$BIN" eval "$w" "$r" >"$tmp/record.json" 2>/dev/null || true
    rc=0; "$BIN" replay "$w" "$tmp/record.json" >/dev/null 2>&1 || rc=$?
    if [ "$rc" -ne 0 ]; then echo "REPLAY MISMATCH: $name" >&2; fail=1; fi
  done < <(python3 -c 'import json,sys; [print(k, v["wire"]) for k, v in sorted(json.load(open(sys.argv[1])).items())]' tests/fixtures/requests/expectations.json)
fi

# Evaluation and replay of the feature 002, 003, 004 and 006 requests (constraints, fixed-scale
# values, exact ratios, evaluation facts).
for pair in "002:constraints" "003:fixed_scale" "004:exact_closure" "006:accounts"; do
  dir="tests/fixtures/requests/${pair%%:*}"
  w="tests/fixtures/wire/valid/${pair##*:}.json"
  for r in "$dir"/*.json; do
    case "$r" in */expectations.json) continue ;; esac
    run_twice "eval $r" "$BIN" eval "$w" "$r"
    "$BIN" eval "$w" "$r" >"$tmp/record.json" 2>/dev/null || true
    rc=0; "$BIN" replay "$w" "$tmp/record.json" >/dev/null 2>&1 || rc=$?
    if [ "$rc" -ne 0 ]; then echo "REPLAY MISMATCH: $r" >&2; fail=1; fi
  done
done

# Verification attestations (feature 002): every verify fixture and every valid wire file,
# without a cache so the solver runs both times.
# Persistence (feature 005): a fixed store history, its records and replay reports (SC-005).
cargo run -q -p behavior-store --example history >"$tmp/history_a" 2>/dev/null || fail=1
cargo run -q -p behavior-store --example history >"$tmp/history_b" 2>/dev/null || fail=1
if ! cmp -s "$tmp/history_a" "$tmp/history_b" || [ ! -s "$tmp/history_a" ]; then
  echo "NOT DETERMINISTIC: store history" >&2
  fail=1
fi
if [ "$(tail -n 2 "$tmp/history_a" | grep -c '"ok":true')" -ne 2 ]; then
  echo "STORE REPLAY FAILED" >&2
  fail=1
fi
# Entity lifecycle (feature 006): a fixed history with creations, removals and reference changes,
# and its data, behavior and reference replay reports.
cargo run -q -p behavior-store --example lifecycle_history >"$tmp/lifecycle_a" 2>/dev/null || fail=1
cargo run -q -p behavior-store --example lifecycle_history >"$tmp/lifecycle_b" 2>/dev/null || fail=1
if ! cmp -s "$tmp/lifecycle_a" "$tmp/lifecycle_b" || [ ! -s "$tmp/lifecycle_a" ]; then
  echo "NOT DETERMINISTIC: lifecycle history" >&2
  fail=1
fi
if [ "$(tail -n 3 "$tmp/lifecycle_a" | grep -c '"ok":true')" -ne 3 ]; then
  echo "LIFECYCLE REPLAY FAILED" >&2
  fail=1
fi

if command -v "${BEHAVIOR_Z3:-z3}" >/dev/null 2>&1; then
  for f in tests/fixtures/verify/*.json tests/fixtures/wire/valid/*.json; do
    case "$f" in *.expected.json) continue ;; esac
    run_twice "verify $f" "$BIN" verify "$f"
  done

  # A governance decision: a forced-inconclusive finding (resource limit 1, not a hard model)
  # waived with the test key.
  g=tests/fixtures/governance
  w=tests/fixtures/verify/purchase_remaining.json
  "$BIN" verify "$w" --profile "$g/forced_inconclusive_profile.json" >"$tmp/att.json" || true
  "$BIN" eval "$w" "$g/approve_request.json" >"$tmp/rec.json" || true
  python3 - "$tmp" <<'PY'
import json, sys
tmp = sys.argv[1]
a = json.load(open(f"{tmp}/att.json"))
f = a["findings"][0]
json.dump({"behavior_version": a["behavior_version"], "finding_hash": f["hash"],
           "profile_hash": a["profile"]["hash"], "verifier_version": a["verifier_version"],
           "rationale": "determinism check"}, open(f"{tmp}/waiver.json", "w"))
open(f"{tmp}/seed", "w").write(json.load(open("tests/fixtures/governance/keys.json"))["A"]["seed"])
PY
  "$BIN" sign-waiver "$tmp/waiver.json" --seed "$tmp/seed" >"$tmp/signed.json"
  run_twice "authorize (waived inconclusive)" "$BIN" authorize "$w" "$tmp/rec.json" \
    --policy "$g/verified_or_waived.json" --attestation "$tmp/att.json" \
    --waiver "$tmp/waiver.json" --signature "$tmp/signed.json" --now 2026-09-25T12:00:00Z
  rc=0; "$BIN" authorize "$w" "$tmp/rec.json" --policy "$g/verified_or_waived.json" \
    --attestation "$tmp/att.json" --waiver "$tmp/waiver.json" --signature "$tmp/signed.json" \
    --now 2026-09-25T12:00:00Z >/dev/null || rc=$?
  if [ "$rc" -ne 0 ]; then echo "AUTHORIZE: expected allow for the waived case" >&2; fail=1; fi
else
  echo "determinism-check: z3 not found; skipping verification checks" >&2
fi

# The Python examples print records; two runs must be byte-identical.
if python3 -c "import behavior._engine" 2>/dev/null; then
  run_twice "examples.invoice.run" python3 -m examples.invoice.run
  run_twice "examples.project_margin.run" python3 -m examples.project_margin.run
  run_twice "examples.accounts.run" python3 -m examples.accounts.run
fi

if [ "$fail" -ne 0 ]; then
  echo "determinism-check: FAILED" >&2
  exit 1
fi
echo "determinism-check: OK"
