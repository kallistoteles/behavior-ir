#!/usr/bin/env bash
# The preflight of the core/ecosystem split (feature 011, FR-022). Every criterion must hold
# before the move; a failing one is fixed on its own first.
#
#   1. boundary:  the core has no dependency on, or reference to, bindings or models
#   2. surface:   the public contract is identifiable (behavior-engine, api/engine-surface.txt)
#   3. ownership: every file has exactly one owner (contracts/ownership.md)
#   4. alone:     the core-owned files build, test and pass the determinism check by themselves
#   5. consumer:  the binding consumes the core through behavior-engine only
set -uo pipefail
cd "$(dirname "$0")/.."

failed=0
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

criterion() {
  local name="$1"; shift
  if "$@" >"$tmp/$name.log" 2>&1; then
    echo "preflight: $name OK"
  else
    echo "preflight: $name FAILED"
    sed 's/^/  /' "$tmp/$name.log" | tail -n 20
    failed=1
  fi
}

# PREFLIGHT_WORK_DIR keeps the copy and its build between runs (faster). The copy always lands at
# the same path, because test binaries refer to their fixtures by the path they were built at.
core_alone() {
  local work="${PREFLIGHT_WORK_DIR:-$tmp}"
  local dir="$work/core"
  rm -rf "$dir"
  mkdir -p "$dir"
  python3 scripts/check-ownership.py --list core >"$tmp/core-files" || return 1
  tar -cf - -T "$tmp/core-files" | tar -xf - -C "$dir" || return 1
  # The copy is not a git repository; the workspace still names the binding, which the core's
  # own Cargo.toml drops at extraction (T026).
  sed -i '/"crates\/behavior-py",/d' "$dir/Cargo.toml"
  (
    cd "$dir" &&
      CARGO_TARGET_DIR="$work/target" cargo test -q --workspace &&
      CARGO_TARGET_DIR="$work/target" cargo build -q --workspace &&
      BEHAVIOR_BIN="$work/target/debug/behavior" scripts/determinism-check.sh --core
  )
}

criterion boundary sh -c 'python3 scripts/check-ownership.py --list core-only | scripts/check-boundary.sh --files-from -'
criterion surface scripts/check-public-surface.sh
criterion ownership python3 scripts/check-ownership.py
criterion alone core_alone
criterion consumer scripts/check-public-surface.sh --consumer

if [ "$failed" -ne 0 ]; then
  echo "preflight: FAILED" >&2
  exit 1
fi
echo "preflight: OK"
