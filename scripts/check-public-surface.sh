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
# Every ecosystem crate: the binding and the model authority (feature 501).
for hit in $(python3 - <<'PY'
import tomllib
internal = {"behavior-core", "behavior-store", "behavior-verify", "behavior-cli"}
for crate in ("behavior-py", "behavior-models"):
    deps = tomllib.load(open(f"crates/{crate}/Cargo.toml", "rb")).get("dependencies", {})
    for d in sorted(internal & set(deps)):
        print(f"{crate}:{d}")
PY
); do
  echo "check-public-surface: crates/${hit%%:*} depends on internal core crate ${hit#*:}" >&2
  fail=1
done
# Models never use Wire IR as their interface (feature 501, research R2): fragments stay Core
# nodes in the shared builder; nothing serializes or reads wire expressions.
wire="$(grep -rnE '\b(into_wire|to_wire_json)\b' crates/behavior-models/src || true)"
if [ -n "$wire" ]; then
  while IFS= read -r hit; do echo "check-public-surface: wire IR used as model interface: $hit" >&2; done <<<"$wire"
  fail=1
fi
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
