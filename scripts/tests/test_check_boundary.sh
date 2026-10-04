#!/usr/bin/env bash
# Tests of scripts/check-boundary.sh (feature 011, FR-005): core files never refer to the
# ecosystem, and the core's own `examples/` and `skills/` paths are not mistaken for it.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

# case <name> <expected exit> <file path> <content>
case_() {
  local name="$1" want="$2" path="$3" content="$4" dir="$tmp/$1"
  mkdir -p "$dir/$(dirname "$path")"
  printf '%s\n' "$content" >"$dir/$path"
  local rc=0
  printf '%s\n' "$path" | "$here/check-boundary.sh" --root "$dir" --files-from - --no-metadata \
    >"$tmp/out" 2>&1 || rc=$?
  if [ "$rc" -ne "$want" ]; then
    echo "FAIL $name: exit $rc, expected $want: $(cat "$tmp/out")"
    failures=$((failures + 1))
  elif [ "$want" -ne 0 ] && ! grep -q "$path" "$tmp/out"; then
    echo "FAIL $name: the offending file is not named: $(cat "$tmp/out")"
    failures=$((failures + 1))
  fi
}

case_ store_example 0 crates/behavior-store/examples/history.rs '// cargo run -q -p behavior-store --example history'
case_ example_run_line 0 scripts/check.sh 'cargo run -q -p behavior-store --example lifecycle_history'
case_ engine_skill 0 README.md 'see .claude/skills/behavior-engine-development/SKILL.md'
case_ python_package 1 docs/x.md 'the binding lives in python/behavior/module.py'
case_ native_module 1 crates/a/src/lib.rs 'import behavior._engine'
case_ binding_crate 1 Cargo.toml 'members = ["crates/behavior-py"]'
case_ consumer_skill 1 docs/y.md 'read skills/behavior-authoring/SKILL.md'
case_ example_module 1 scripts/z.sh 'python3 -m examples.invoice.run'
case_ example_dir 1 docs/z.md 'see examples/lab_reads/model.py'
case_ allow_listed 0 specs/011-core-ecosystem-split/plan.md 'python/behavior stays in the ecosystem'

if [ "$failures" -ne 0 ]; then echo "test_check_boundary: $failures failed"; exit 1; fi
echo "test_check_boundary: OK"
