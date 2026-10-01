#!/usr/bin/env bash
# The release check (feature 008, research R9). Given a built dist directory (or building one into
# a temporary directory), it proves the release installs and behaves as documented outside this
# repository, without the engine toolchain. Any failure stops the check and names the step.
#
#   scripts/release-check.sh [--skip-gates] [dist-dir]
set -euo pipefail
cd "$(dirname "$0")/.."
skip_gates=0
if [ "${1:-}" = "--skip-gates" ]; then
  skip_gates=1
  shift
fi
dist="${1:-}"

current="setup"
step() {
  current="$1"
  echo "release-check: step $1" >&2
}
fail() {
  echo "release-check: FAILED step $current: $*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
version="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"])')"

step "1 gates"
if [ "$skip_gates" -eq 0 ]; then
  cargo fmt --all --check || fail "cargo fmt"
  cargo clippy --workspace --all-targets -- -D warnings || fail "clippy"
  cargo test -q --workspace || fail "cargo test"
  maturin develop -q || fail "maturin develop"
  BEHAVIOR_RELEASE_TESTS=1 python -m pytest -q python/tests || fail "pytest"
  mypy || fail "mypy"
  cargo build -q --workspace || fail "cargo build"
  scripts/determinism-check.sh || fail "determinism check"
else
  echo "release-check: gates skipped" >&2
fi

if [ -z "$dist" ]; then
  dist="$tmp/dist"
  scripts/release-build.sh "$version" "$dist" || fail "build"
fi
dist="$(cd "$dist" && pwd)"
wheel="$(ls "$dist"/*.whl 2>/dev/null | head -n 1)"
[ -n "$wheel" ] || fail "no wheel in $dist"

step "2 wheel compliance"
auditwheel show "$wheel" >"$tmp/auditwheel.txt" 2>&1 || fail "auditwheel: $(cat "$tmp/auditwheel.txt")"
grep -q 'manylinux_2_28_x86_64' "$tmp/auditwheel.txt" || fail "not manylinux_2_28: $(cat "$tmp/auditwheel.txt")"

started=$SECONDS
step "3 clean environment"
consumer="$tmp/consumer"
mkdir -p "$consumer"
python3.13 -m venv "$consumer/.venv" || fail "venv"
py="$consumer/.venv/bin/python"
clean_path="$consumer/.venv/bin:/usr/bin:/bin"
if env PATH="$clean_path" sh -c 'command -v cargo || command -v rustc' >/dev/null; then
  fail "the clean environment can see a Rust toolchain"
fi
cp release/smoke.py "$consumer/smoke.py"

step "4 install"
sum="$(awk -v w="$(basename "$wheel")" '$2 == w {print $1}' "$dist/SHA256SUMS")"
[ -n "$sum" ] || fail "no checksum for $(basename "$wheel") in SHA256SUMS"
echo "behavior @ file://$wheel --hash=sha256:$sum" >"$consumer/requirements.txt"
(cd "$consumer" && env PATH="$clean_path" "$py" -m pip install -q --no-index --require-hashes \
  -r requirements.txt) || fail "pip install --require-hashes"

step "5 smoke scenario"
(cd "$consumer" && env PATH="$clean_path" "$py" smoke.py >"$tmp/smoke_installed.txt") ||
  fail "release/smoke.py in the clean environment"

step "6 missing solver"
(cd "$consumer" && env PATH="$clean_path" BEHAVIOR_Z3=/nonexistent/z3 "$py" smoke.py \
  --expect-missing-solver >/dev/null) || fail "release/smoke.py --expect-missing-solver"

step "7 versions"
(cd "$consumer" && env PATH="$clean_path" "$py" - "$dist/release-manifest.json" <<'PY') || fail "behavior.versions() differs from the release manifest"
import json, sys
import behavior
manifest = json.load(open(sys.argv[1]))
expected = dict(manifest["versions"], binding={"python": manifest["bindings"]["python"]["version"]})
if behavior.versions() != expected:
    sys.exit(f"{behavior.versions()} != {expected}")
PY

step "8 byte identity with the repository"
# The repository build, in its own environment (never the developer's venv).
repo_venv="$tmp/repo-venv"
python3.13 -m venv --system-site-packages "$repo_venv" || fail "venv"
VIRTUAL_ENV="$repo_venv" maturin develop -q >"$tmp/develop.log" 2>&1 || fail "maturin develop: $(tail -n 3 "$tmp/develop.log")"
repo_py="$repo_venv/bin/python"
(cd "$consumer" && env PATH="$repo_venv/bin:$PATH" "$repo_py" smoke.py >"$tmp/smoke_repo.txt") ||
  fail "release/smoke.py in the repository"
cmp -s "$tmp/smoke_installed.txt" "$tmp/smoke_repo.txt" ||
  fail "installed and in-repo outputs differ: $(diff "$tmp/smoke_installed.txt" "$tmp/smoke_repo.txt" | head -n 5)"
elapsed=$((SECONDS - started))
[ "$elapsed" -le 600 ] || fail "steps 3-8 took ${elapsed}s (limit 600s, SC-001)"

step "9 skill examples"
shopt -s nullglob
for example in skills/*/examples/*.py; do
  work="$tmp/example"
  rm -rf "$work"
  mkdir -p "$work"
  cp "$example" "$work/"
  (cd "$work" && env PATH="$clean_path" "$py" "$(basename "$example")" >"$tmp/example.log" 2>&1) ||
    fail "skill examples: $example: $(tail -n 3 "$tmp/example.log")"
done
shopt -u nullglob

step "10 drift and equivalence"
tests=()
for t in test_skills.py test_public_api.py test_binding_equivalence.py; do
  [ -f "python/tests/$t" ] && tests+=("python/tests/$t")
done
env PATH="$repo_venv/bin:$PATH" "$repo_py" -m pytest -q -p no:cacheprovider "${tests[@]}" ||
  fail "skills, public API or binding equivalence"

echo "release-check: OK ($(basename "$wheel"))" >&2
