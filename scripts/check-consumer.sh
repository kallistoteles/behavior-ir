#!/usr/bin/env bash
# The external consumer of `behavior-engine` (feature 011, FR-006d): builds and tests
# `consumer/`, a crate outside the workspace whose only core dependency is the public facade.
#
#   scripts/check-consumer.sh            # against this checkout (a path dependency)
#   scripts/check-consumer.sh --rev SHA  # against a pushed revision of behavior-ir-core
set -euo pipefail
cd "$(dirname "$0")/.."

repo="https://github.com/kallistoteles/behavior-ir-core"
if [ "${1:-}" = "--rev" ]; then
  rev="${2:?--rev needs a commit}"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  cp -r consumer "$tmp/consumer"
  mkdir -p "$tmp/tests" && cp -r tests/fixtures "$tmp/tests/fixtures"
  sed -i "s#^behavior-engine = .*#behavior-engine = { git = \"$repo\", rev = \"$rev\" }#" \
    "$tmp/consumer/Cargo.toml"
  manifest="$tmp/consumer/Cargo.toml"
else
  manifest=consumer/Cargo.toml
fi
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}/consumer" \
  cargo test -q --manifest-path "$manifest"
echo "check-consumer: OK" >&2
