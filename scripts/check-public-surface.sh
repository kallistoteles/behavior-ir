#!/usr/bin/env bash
# The public core contract (feature 011, FR-006, FR-006b, FR-007).
#
#   scripts/check-public-surface.sh             # provider: the facade exports exactly
#                                               # api/engine-surface.txt, every item explicitly
#   scripts/check-public-surface.sh --consumer  # consumer: the binding uses behavior-engine only
#
# The facade (`crates/behavior-engine/src/lib.rs`) writes one re-export per line:
# `pub use path::Item;` or `pub use path::Item as Name;`, inside `pub mod name { … }` blocks for
# namespaces; `pub fn` items are exported too. A glob (`*`) or a group (`{…}`) re-export fails.
set -euo pipefail
cd "$(dirname "$0")/.."

mode=provider
case "${1:-}" in
  "") ;;
  --consumer) mode=consumer ;;
  *) echo "usage: scripts/check-public-surface.sh [--consumer]" >&2; exit 2 ;;
esac

if [ "$mode" = provider ]; then
  python3 - <<'PY'
import re, sys
from pathlib import Path

facade = Path("crates/behavior-engine/src/lib.rs")
listed = Path("api/engine-surface.txt")
for p in (facade, listed):
    if not p.is_file():
        sys.exit(f"check-public-surface: {p} does not exist")

errors, exported, stack = [], [], []
depth = 0
for n, raw in enumerate(facade.read_text().splitlines(), 1):
    line = raw.split("//")[0].strip()
    if not line:
        continue
    m = re.match(r"pub mod (\w+)\s*\{$", line)
    if m:
        stack.append((m.group(1), depth))
        depth += 1
        continue
    if line.startswith("pub use"):
        m = re.match(r"pub use ([\w:]+)(?: as (\w+))?;$", line)
        if "*" in line:
            errors.append(f"{facade}:{n}: wildcard re-export: {line}")
        elif "{" in line or not m:
            errors.append(f"{facade}:{n}: one item per `pub use` line: {line}")
        else:
            name = m.group(2) or m.group(1).rsplit("::", 1)[-1]
            exported.append("::".join([s for s, _ in stack] + [name]))
        continue
    m = re.match(r"pub fn (\w+)", line)
    if m:
        exported.append("::".join([s for s, _ in stack] + [m.group(1)]))
    depth += line.count("{") - line.count("}")
    while stack and depth <= stack[-1][1]:
        stack.pop()

want = [l for l in listed.read_text().splitlines() if l.strip()]
for l in want:
    if l.endswith("*"):
        errors.append(f"{listed}: wildcard entry: {l}")
if want != sorted(set(want)):
    errors.append(f"{listed}: entries must be sorted and unique")
have, want_set = set(exported), set(want)
if len(exported) != len(have):
    errors.append("the facade exports a name twice")
for x in sorted(have - want_set):
    errors.append(f"exported but not in {listed}: {x}")
for x in sorted(want_set - have):
    errors.append(f"listed in {listed} but not exported: {x}")
for e in errors:
    print(f"check-public-surface: {e}", file=sys.stderr)
if errors:
    sys.exit(1)
print(f"check-public-surface: OK ({len(have)} items)")
PY
else
  fail=0
  bad="$(python3 - <<'PY'
import tomllib
deps = tomllib.load(open("crates/behavior-py/Cargo.toml", "rb")).get("dependencies", {})
internal = {"behavior-core", "behavior-store", "behavior-verify", "behavior-cli"}
print("\n".join(sorted(internal & set(deps))))
PY
)"
  for d in $bad; do
    echo "check-public-surface: crates/behavior-py depends on internal core crate $d" >&2
    fail=1
  done
  if grep -rnIE '\bbehavior_(core|store|verify|cli)\b' crates/behavior-py python >"${TMPDIR:-/tmp}/surface-hits.$$" 2>/dev/null; then
    while IFS= read -r hit; do
      echo "check-public-surface: internal core crate used: $hit" >&2
    done <"${TMPDIR:-/tmp}/surface-hits.$$"
    fail=1
  fi
  rm -f "${TMPDIR:-/tmp}/surface-hits.$$"
  if [ "$fail" -ne 0 ]; then
    echo "check-public-surface: FAILED (consumer)" >&2
    exit 1
  fi
  echo "check-public-surface: OK (consumer: behavior-engine only)"
fi
