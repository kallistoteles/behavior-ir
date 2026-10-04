#!/usr/bin/env bash
# Models compile downward; the documents that define them never call them plugins or extensions
# (feature 011, FR-013), words that suggest components changing runtime semantics.
#
#   scripts/check-terms.sh [file...]      # default: the models area
set -euo pipefail
cd "$(dirname "$0")/.."
if [ $# -eq 0 ]; then
  mapfile -t files < <(git ls-files --cached --others --exclude-standard -- models | grep -E '\.(md|py)$')
  set -- "${files[@]}"
fi
if grep -HniE '\b(plugin|extension)s?\b' "$@" >&2; then
  echo "check-terms: FAILED: name models by what they are (lowered to Behavior IR)" >&2
  exit 1
fi
echo "check-terms: OK"
