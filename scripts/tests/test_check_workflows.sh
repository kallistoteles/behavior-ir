#!/usr/bin/env bash
# Tests of scripts/check-workflows.sh (feature 011, FR-030, FR-035): workflows only orchestrate,
# never read Spec Kit's or an agent's state, and pin every action by commit.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0
pin=0123456789abcdef0123456789abcdef01234567

# case <name> <expected exit> <workflow body>
case_() {
  local dir="$tmp/$1"
  mkdir -p "$dir/.github/workflows" "$dir/scripts"
  printf '%s\n' "$3" >"$dir/.github/workflows/ci.yml"
  printf '#!/bin/sh\necho ok\n' >"$dir/scripts/gates.sh"
  local rc=0
  "$here/check-workflows.sh" --root "$dir" >"$tmp/out" 2>&1 || rc=$?
  if [ "$rc" -ne "$2" ]; then
    echo "FAIL $1: exit $rc, expected $2: $(cat "$tmp/out")"
    failures=$((failures + 1))
  fi
}

case_ pinned 0 "steps:
  - uses: actions/checkout@$pin
  - run: nix develop -c scripts/gates.sh"
case_ pinned_with_comment 0 "steps:
  - uses: actions/checkout@$pin # v4.4.0"
case_ tag_pinned 1 "steps:
  - uses: actions/checkout@v4"
case_ spec_kit_state 1 "steps:
  - run: cat .specify/feature.json"
case_ agent_state 1 "steps:
  - run: ls .claude/skills"
case_ specs_dir 1 "steps:
  - run: cat specs/011-core-ecosystem-split/plan.md"

if [ "$failures" -ne 0 ]; then echo "test_check_workflows: $failures failed"; exit 1; fi
echo "test_check_workflows: OK"
