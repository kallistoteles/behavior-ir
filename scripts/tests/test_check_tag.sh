#!/usr/bin/env bash
# Tests of scripts/check-tag.sh (feature 011, FR-028, FR-029): a release starts only from an
# annotated tag whose version is the declared release version, on a commit of main whose
# required checks are green. Each refusal names the values.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

repo="$tmp/repo"
mkdir -p "$repo/scripts" "$tmp/bin"
cp "$here/check-tag.sh" "$repo/scripts/" 2>/dev/null || true
cd "$repo"
git init -q -b main
git config user.email t@example.com
git config user.name test
printf '[workspace.package]\nversion = "0.10.1"\n' >Cargo.toml
git add -A && git commit -q -m one
git update-ref refs/remotes/origin/main HEAD
git tag -a v0.10.1 -m release
git tag v0.10.1-light
git tag -a v0.10.2 -m wrong
git checkout -q -b side
sed -i 's/0.10.1/0.10.1-side2/' Cargo.toml && git commit -q -am side-version && git tag -a v0.10.1-side2 -m side
git checkout -q main

# A stub `gh` that answers the check-runs query from $GH_STUB.
cat >"$tmp/bin/gh" <<'GH'
#!/usr/bin/env bash
cat "$GH_STUB"
GH
chmod +x "$tmp/bin/gh"
printf '{"check_runs":[{"name":"gates","conclusion":"success"},{"name":"consumer","conclusion":"success"}]}' >"$tmp/green.json"
printf '{"check_runs":[{"name":"gates","conclusion":"failure"}]}' >"$tmp/red.json"

# expect <name> <exit> <grep pattern> <tag> [env...]
expect() {
  local name="$1" want="$2" pattern="$3" tag="$4"; shift 4
  local rc=0
  env -u GITHUB_TOKEN -u GH_TOKEN -u CHECK_TAG_REQUIRED -u GITHUB_REPOSITORY "$@" PATH="$tmp/bin:$PATH" scripts/check-tag.sh "$tag" >"$tmp/out" 2>&1 || rc=$?
  if [ "$rc" -ne "$want" ] || ! grep -qE "$pattern" "$tmp/out"; then
    echo "FAIL $name: exit $rc (want $want), output: $(cat "$tmp/out")"
    failures=$((failures + 1))
  fi
}

expect valid_no_token 0 'CI status not verified \(no token\)' v0.10.1
expect light_tag 1 'v0.10.1-light is not annotated' v0.10.1-light
expect wrong_version 1 '0\.10\.2.*0\.10\.1' v0.10.2
expect not_on_main 1 'not on main' v0.10.1-side2
expect green_ci 0 'OK' v0.10.1 GH_TOKEN=x GH_STUB="$tmp/green.json" GITHUB_REPOSITORY=o/r
expect red_ci 1 'gates.*failure' v0.10.1 GH_TOKEN=x GH_STUB="$tmp/red.json" GITHUB_REPOSITORY=o/r
expect missing_check 1 'consumer.*missing' v0.10.1 GH_TOKEN=x GH_STUB="$tmp/red.json" GITHUB_REPOSITORY=o/r CHECK_TAG_REQUIRED="gates consumer"

if [ "$failures" -ne 0 ]; then echo "test_check_tag: $failures failed"; exit 1; fi
echo "test_check_tag: OK"
