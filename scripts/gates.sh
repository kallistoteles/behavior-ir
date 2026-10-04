#!/usr/bin/env bash
# The quality gates of the constitution, in one list (feature 011, FR-034): Spec Kit's implement
# phase, scripts/release-check.sh and CI all run exactly this. Stops at the first failing gate
# and names it.
set -euo pipefail
cd "$(dirname "$0")/.."

gate() {
  local name="$1"; shift
  echo "gates: $name" >&2
  if ! "$@"; then
    echo "gates: FAILED $name" >&2
    exit 1
  fi
}

gate "cargo fmt" cargo fmt --all --check
gate "clippy" cargo clippy -q --workspace --all-targets -- -D warnings
gate "cargo test" cargo test -q --workspace
gate "cargo build" cargo build -q --workspace
gate "stage cli" scripts/stage-cli.sh
gate "maturin develop" maturin develop -q
gate "pytest" python -m pytest -q python/tests
gate "mypy" mypy
gate "determinism check" scripts/determinism-check.sh
gate "boundary" sh -c 'python3 scripts/check-ownership.py --list core-only | scripts/check-boundary.sh --files-from -'
gate "public surface" scripts/check-public-surface.sh
gate "public surface (consumer)" scripts/check-public-surface.sh --consumer
echo "gates: OK" >&2
