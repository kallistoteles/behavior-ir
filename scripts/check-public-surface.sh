#!/usr/bin/env bash
# The ecosystem uses only the public core contract (feature 011, FR-007, US2 scenario 2):
#   - the binding's only core dependency is `behavior-engine`, never an internal core crate;
#   - no source names an internal core crate (`behavior_core`, `behavior_store`,
#     `behavior_verify`, `behavior_cli`);
#   - no core-owned file lives here (contracts/ownership.md): the core is consumed from its
#     release, never copied.
#
#   scripts/check-public-surface.sh [--consumer]   (the flag is accepted for symmetry with the core)
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-}" in "" | --consumer) ;; *) echo "usage: scripts/check-public-surface.sh" >&2; exit 2 ;; esac

fail=0
for d in $(python3 - <<'PY'
import tomllib
deps = tomllib.load(open("crates/behavior-py/Cargo.toml", "rb")).get("dependencies", {})
internal = {"behavior-core", "behavior-store", "behavior-verify", "behavior-cli"}
print("\n".join(sorted(internal & set(deps))))
PY
); do
  echo "check-public-surface: crates/behavior-py depends on internal core crate $d" >&2
  fail=1
done
hits="$(grep -rnIE '\bbehavior_(core|store|verify|cli)\b' crates python examples skills release || true)"
if [ -n "$hits" ]; then
  while IFS= read -r hit; do echo "check-public-surface: internal core crate used: $hit" >&2; done <<<"$hits"
  fail=1
fi
copied="$(python3 scripts/check-ownership.py --list core-only)"
if [ -n "$copied" ]; then
  while IFS= read -r f; do echo "check-public-surface: core-owned file in the ecosystem: $f" >&2; done <<<"$copied"
  fail=1
fi
if [ "$fail" -ne 0 ]; then
  echo "check-public-surface: FAILED" >&2
  exit 1
fi
echo "check-public-surface: OK (behavior-engine only; no core file copied)"
