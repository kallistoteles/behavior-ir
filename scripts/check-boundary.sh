#!/usr/bin/env bash
# The core never depends on, refers to or knows about a binding, a model or the ecosystem
# (feature 011, FR-005). Fails, naming each offender, if a core crate depends on the Python
# binding or a core file names an ecosystem path.
#
#   scripts/check-boundary.sh [--root DIR] [--files-from FILE|-] [--no-metadata]
#
# Without --files-from, every tracked file under DIR is a core file (the core repository).
# Before the split, the preflight passes the core-only files of the ownership manifest.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
files_from=""
metadata=1
while [ $# -gt 0 ]; do
  case "$1" in
    --root) root="$(cd "${2:?--root needs a directory}" && pwd)"; shift 2 ;;
    --files-from) files_from="${2:?--files-from needs a file or -}"; shift 2 ;;
    --no-metadata) metadata=0; shift ;;
    *) echo "usage: scripts/check-boundary.sh [--root DIR] [--files-from FILE|-] [--no-metadata]" >&2
       exit 2 ;;
  esac
done

fail=0
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# 1. No core crate depends on the binding.
if [ "$metadata" -eq 1 ]; then
  (cd "$root" && cargo metadata -q --format-version 1 --no-deps) >"$tmp/metadata.json"
  python3 - "$tmp/metadata.json" <<'PY' || fail=1
import json, sys
core = {"behavior-core", "behavior-verify", "behavior-store", "behavior-cli", "behavior-engine"}
bad = [f"{p['name']} depends on {d['name']}"
       for p in json.load(open(sys.argv[1]))["packages"] if p["name"] in core
       for d in p["dependencies"] if d["name"] == "behavior-py"]
for b in bad:
    print(f"check-boundary: {b}", file=sys.stderr)
sys.exit(1 if bad else 0)
PY
fi

# 2. No core file names an ecosystem path. The core's own `examples/` (store examples) and
# `skills/` (the engine skill) are not ecosystem paths; only the named ones are.
names='invoice|project_margin|accounts|ledger|orders|lab_reads|schema_evolution|tryout'
pattern="behavior-py|behavior\\._engine|python/behavior|skills/behavior-(authoring|application|verification)"
pattern+="|examples\\.($names)\\b|(^|[^/[:alnum:]_])examples/($names)/"

if [ -n "$files_from" ]; then
  if [ "$files_from" = "-" ]; then cat >"$tmp/files"; else cp "$files_from" "$tmp/files"; fi
else
  (cd "$root" && git ls-files) >"$tmp/files"
fi
# The allow-list: specifications (historical records of the single repository; rewriting them
# would falsify history), proposals, and this check with its tests.
grep -vE '^(specs/|docs/proposals/|scripts/check-boundary\.sh$|scripts/tests/test_check_boundary\.sh$)' \
  "$tmp/files" >"$tmp/checked" || true

while IFS= read -r f; do
  [ -f "$root/$f" ] || continue
  if grep -nIE "$pattern" "$root/$f" >"$tmp/hits" 2>/dev/null; then
    while IFS= read -r hit; do
      echo "check-boundary: $f:$hit" >&2
    done <"$tmp/hits"
    fail=1
  fi
done <"$tmp/checked"

if [ "$fail" -ne 0 ]; then
  echo "check-boundary: FAILED" >&2
  exit 1
fi
echo "check-boundary: OK"
