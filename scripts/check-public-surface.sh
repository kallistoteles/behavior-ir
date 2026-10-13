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
# nodes in the shared builder; nothing serializes or reads wire expressions. The one exception
# (feature 502, research R4): RequestFlow creation lowering passes Core nodes to the builder's
# creation input type with `into_wire`, in exactly one file. Nor does the model crate serialize
# to text or bytes with serde (`to_string`, `to_vec`, `to_writer`): identity hashes go through
# Core's canonical hashing. `serde_json::to_value` of a `WType` is allowed, because canonical
# documents hold parameter types as `"type": <WType>` (data model §7).
wire="$( (grep -rnE '\bto_wire_json\b|\bserde_json::to_(string|string_pretty|vec|vec_pretty|writer)\b' \
            crates/behavior-models/src;
          grep -rnE '\binto_wire\b' crates/behavior-models/src \
            | grep -v '^crates/behavior-models/src/request_flow/create.rs:') || true)"
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
