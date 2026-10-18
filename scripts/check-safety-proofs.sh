#!/usr/bin/env bash
# Build-time proof gate only. Core Store/replay never invoke this script or Lean.
set -euo pipefail
cd "$(dirname "$0")/.."
case "$#" in
  0) output=target/safety-models/proof-gate ;;
  2) [ "$1" = --output ] || { echo 'usage: check-safety-proofs.sh [--output PATH]' >&2; exit 2; }; output="$2" ;;
  *) echo 'usage: check-safety-proofs.sh [--output PATH]' >&2; exit 2 ;;
esac
# The existing reference runner owns reports, early failures, complete cases and
# trusted Rust certification. Supplied JSON flags are never a certificate input.
exec python -m models.examples.safety_models.run --gate --certify --output "$output"
