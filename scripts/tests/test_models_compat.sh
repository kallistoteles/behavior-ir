#!/usr/bin/env bash
# Tests of scripts/check-models-compat.py (feature 501, SC-009, lowering contract §7): an
# established version tuple's hashes are immutable. Editing an expected hash never passes; a
# changed lowering passes only under a new tuple; only tuples of an unpinned Core may go.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

tuple() { # tuple <lowerer> <core version> <cases json>
  printf '{"core_release":{"commit":"c%s","version":"%s"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":1,"cases":%s}' \
    "$2" "$2" "$1" "$3"
}
manifest() { # manifest <tuple>...
  local IFS=,
  printf '{"format":"behavior.model_compat.v1","tuples":[%s]}\n' "$*"
}
computed() { # computed <lowerer> <cases json>
  printf '{"tuple":{"core_release":{"commit":"c1.0","version":"1.0"},"lowerer_version":%s,"model_kind":"state_machine","model_semantic_version":1},"cases":%s}\n' \
    "$1" "$2"
}
A1='{"a":{"behavior_hash":"sha256:b1","models":{"m":"sha256:m1"}}}'
A2='{"a":{"behavior_hash":"sha256:b2","models":{"m":"sha256:m2"}}}'
AB='{"a":{"behavior_hash":"sha256:b1","models":{"m":"sha256:m1"}},"b":{"behavior_hash":"sha256:b3","models":{"n":"sha256:n3"}}}'

# repo <name> <base manifest>: a repository whose `main` holds the base manifest, on a branch.
repo() {
  local r="$tmp/$1"
  mkdir -p "$r/models/fixtures"
  git -C "$r" init -q -b main
  printf '{"commit":"c1.0","version":"1.0"}\n' >"$r/core-release.json"
  printf '%s' "$2" >"$r/models/fixtures/compat.json"
  git -C "$r" add -A
  git -C "$r" -c user.name=t -c user.email=t@t commit -qm base
  git -C "$r" checkout -qb feature
  echo "$r"
}

# expect <PASS|FAIL> <name> <repo> <branch manifest> <computed>
expect() {
  local r="$3"
  printf '%s' "$4" >"$r/models/fixtures/compat.json"
  printf '%s' "$5" >"$tmp/computed.json"
  if python3 "$here/check-models-compat.py" --root "$r" --computed "$tmp/computed.json" \
      >/dev/null 2>"$tmp/err"; then got=PASS; else got=FAIL; fi
  if [ -n "${VERBOSE:-}" ]; then echo "$2: $got $(cat "$tmp/err")" >&2; fi
  if [ "$got" != "$1" ]; then
    echo "test_models_compat: $2: expected $1, got $got: $(cat "$tmp/err")" >&2
    failures=$((failures + 1))
  fi
}

base="$(manifest "$(tuple 1 1.0 "$A1")")"
expect PASS "unchanged" "$(repo unchanged "$base")" "$base" "$(computed 1 "$A1")"
expect FAIL "edited expected hash, same tuple" "$(repo edited "$base")" \
  "$(manifest "$(tuple 1 1.0 "$A2")")" "$(computed 1 "$A2")"
expect FAIL "output changed, manifest not edited" "$(repo drift "$base")" "$base" "$(computed 1 "$A2")"
expect PASS "lowerer bump with a new tuple" "$(repo bump "$base")" \
  "$(manifest "$(tuple 1 1.0 "$A1")" "$(tuple 2 1.0 "$A2")")" "$(computed 2 "$A2")"

base_ab="$(manifest "$(tuple 1 1.0 "$AB")")"
expect FAIL "removed case of an established tuple" "$(repo removed "$base_ab")" "$base" "$(computed 1 "$A1")"

old="$(manifest "$(tuple 1 0.9 "$A2")" "$(tuple 1 1.0 "$A1")")"
expect PASS "tuple of an unpinned core removed" "$(repo unpinned "$old")" "$base" "$(computed 1 "$A1")"
expect FAIL "tuple of the pinned core removed" "$(repo pinned "$base")" \
  "$(manifest "$(tuple 2 1.0 "$A2")")" "$(computed 2 "$A2")"

# No merge base: the gate fails, it never passes silently.
nobase="$tmp/nobase"
mkdir -p "$nobase/models/fixtures"
git -C "$nobase" init -q -b trunk
printf '{"commit":"c1.0","version":"1.0"}\n' >"$nobase/core-release.json"
expect FAIL "no merge base" "$nobase" "$base" "$(computed 1 "$A1")"

if [ "$failures" -ne 0 ]; then
  echo "test_models_compat: $failures failure(s)" >&2
  exit 1
fi
echo "test_models_compat: OK"
