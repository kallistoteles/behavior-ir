#!/usr/bin/env bash
# Stages the core's `behavior` command-line tool into the Python package
# (`python/behavior/_bin/behavior`), where the console script runs it (feature 011, FR-006c).
#
#   scripts/stage-cli.sh            # a debug build, for development
#   scripts/stage-cli.sh --release  # the manylinux_2_28 build that goes into the wheel
#
# Until the core is a separate repository it is built from this workspace; afterwards
# scripts/fetch-core.sh installs the released binary instead.
set -euo pipefail
cd "$(dirname "$0")/.."
dest=python/behavior/_bin/behavior
mkdir -p "$(dirname "$dest")"
if [ "${1:-}" = "--release" ]; then
  target=x86_64-unknown-linux-gnu
  cargo zigbuild -q --release --target "$target.2.28" -p behavior-cli
  src="target/$target/release/behavior"
else
  cargo build -q -p behavior-cli
  src=target/debug/behavior
fi
install -m 0755 "$src" "$dest.tmp"
mv "$dest.tmp" "$dest"
echo "stage-cli: $dest" >&2
