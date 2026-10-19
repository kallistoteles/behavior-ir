#!/usr/bin/env bash
# Tests of the wire guard in scripts/check-public-surface.sh (feature 502, T001, research R4):
# `into_wire` is allowed only in crates/behavior-models/src/request_flow/create.rs and (feature
# 503, T001, research R8) crates/behavior-models/src/approval/lower.rs.
# Feature 510 allows public serialization only in the private literal-order hint
# helper; serde serialization and private Core imports remain forbidden there.
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
# A copy of the working tree (the guard also lists files with git, see check-ownership.py).
(cd "$here" && git ls-files -z --cached --others --exclude-standard) \
  | (cd "$here" && tar --null -T - -cf - 2>/dev/null) | (mkdir "$tmp/tree" && tar -C "$tmp/tree" -xf -)
(cd "$tmp/tree" && git init -q && git add -A)
src="$tmp/tree/crates/behavior-models/src"
run() { (cd "$tmp/tree" && git add -A && scripts/check-public-surface.sh) >"$tmp/out" 2>&1; }

run || { echo "FAIL clean tree: $(cat "$tmp/out")"; exit 1; }

# expect <name> <file> <line> <PASS|FAIL>
expect() {
  mkdir -p "$(dirname "$src/$2")"
  printf '%s\n' "$3" >"$src/$2"
  if run; then got=PASS; else got=FAIL; fi
  rm -f "$src/$2"
  if [ "$got" != "$4" ] || { [ "$4" = FAIL ] && ! grep -q "$2" "$tmp/out"; }; then
    echo "FAIL $1: expected $4, got $got: $(cat "$tmp/out")"; exit 1
  fi
}
expect "into_wire elsewhere" probe_wire.rs 'fn f(n: Node) -> WExpr { n.into_wire() }' FAIL
cp "$src/request_flow/create.rs" "$tmp/create.rs.orig"
printf '\nfn probe(n: Node) -> WExpr { n.into_wire() }\n' >>"$src/request_flow/create.rs"
run || { echo "FAIL into_wire in create.rs rejected: $(cat "$tmp/out")"; exit 1; }
cp "$tmp/create.rs.orig" "$src/request_flow/create.rs"
mkdir -p "$src/approval"
touch "$src/approval/lower.rs"
cp "$src/approval/lower.rs" "$tmp/lower.rs.orig"
printf '\nfn probe(n: Node) -> WExpr { n.into_wire() }\n' >>"$src/approval/lower.rs"
run || { echo "FAIL into_wire in approval/lower.rs rejected: $(cat "$tmp/out")"; exit 1; }
cp "$tmp/lower.rs.orig" "$src/approval/lower.rs"
expect "into_wire in approval/resolve.rs" approval/probe_resolve.rs 'fn f(n: Node) -> WExpr { n.into_wire() }' FAIL
expect "to_wire_json" probe_json.rs 'fn f(m: &Module) -> String { m.to_wire_json() }' FAIL
expect "serde_json::to_string" probe_serde.rs 'fn f(v: &Value) -> String { serde_json::to_string(v).unwrap() }' FAIL
expect "serde_json::to_vec" probe_vec.rs 'fn f(v: &Value) -> Vec<u8> { serde_json::to_vec(v).unwrap() }' FAIL
# The sole public serializer exception discovers untrusted string-array hints.
helper="$src/safety/membership_orders.rs"
if [ -f "$helper" ]; then cp "$helper" "$tmp/membership_orders.orig"; fi
printf '%s\n' 'fn probe(m: &Module) { behavior_engine::serialize::to_wire_json(m); }' >>"$helper"
run || { echo "FAIL named discovery helper rejected: $(cat "$tmp/out")"; exit 1; }
if [ -f "$tmp/membership_orders.orig" ]; then cp "$tmp/membership_orders.orig" "$helper"; else rm "$helper"; fi
expect "serializer next to helper" safety/probe_orders.rs 'fn f(m: &Module) { behavior_engine::serialize::to_wire_json(m); }' FAIL
expect "private import in helper neighbor" safety/probe_private.rs 'use behavior_core::semantic::ExprKind;' FAIL
# The exception never permits serde serialization or private Core imports in the helper.
cp "$helper" "$tmp/helper.saved" 2>/dev/null || touch "$tmp/helper.saved"
printf '%s\n' 'fn f(v: &Value) { serde_json::to_string(v); }' >>"$helper"
if run; then echo "FAIL helper serde serialization accepted"; exit 1; fi
cp "$tmp/helper.saved" "$helper"
printf '%s\n' 'use behavior_core::semantic::ExprKind;' >>"$helper"
if run; then echo "FAIL helper private import accepted"; exit 1; fi
cp "$tmp/helper.saved" "$helper"
echo "test_check_public_surface: OK"
