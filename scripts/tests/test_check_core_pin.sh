#!/usr/bin/env bash
# Tests of scripts/check-core-pin.sh (feature 011, FR-017, FR-024, US2 scenarios 1 and 3): the
# ecosystem builds against exactly the declared Core Release, never a path or patch override.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0
mkdir -p "$tmp/bin"
cat >"$tmp/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
printf '%s\n' "$*" >"$CARGO_CALL"
printf '{"packages":[{"name":"behavior-engine","source":%s},{"name":"behavior-core","source":%s}]}\n' \
  "$CARGO_SOURCE" "$CARGO_CORE_SOURCE"
CARGO
chmod +x "$tmp/bin/cargo"
repo=https://github.com/kallistoteles/behavior-ir-core
pin=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
other=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb

# setup <dir> <toml rev> <lock rev>
setup() {
  mkdir -p "$1"
  printf '{"commit":"%s","format":"behavior.core_pin.v1","repository":"%s","tag":"v0.10.2","version":"0.10.2"}\n' \
    "$pin" "$repo" >"$1/core-release.json"
  printf '[workspace]\nmembers = []\n\n[workspace.dependencies]\nbehavior-engine = { git = "%s", rev = "%s" }\n' \
    "$repo" "$2" >"$1/Cargo.toml"
  printf '[[package]]\nname = "behavior-engine"\nversion = "0.10.2"\nsource = "git+%s?rev=%s#%s"\n' \
    "$repo" "$3" "$3" >"$1/Cargo.lock"
}

# expect <name> <exit> <pattern> <dir> [env...]
expect() {
  local name="$1" want="$2" pattern="$3" dir="$4"; shift 4
  local rc=0
  env -u CI -u BEHAVIOR_DEV_CORE_PATH -u CHECK_CORE_PIN_NO_METADATA \
    PATH="$tmp/bin:$PATH" CARGO_CALL="$tmp/cargo-call" \
    CARGO_SOURCE="\"git+$repo?rev=$pin#$pin\"" \
    CARGO_CORE_SOURCE="\"git+$repo?rev=$pin#$pin\"" "$@" \
    "$here/check-core-pin.sh" --root "$dir" >"$tmp/out" 2>&1 || rc=$?
  if [ "$rc" -ne "$want" ] || ! grep -qE "$pattern" "$tmp/out"; then
    echo "FAIL $name: exit $rc (want $want): $(cat "$tmp/out")"
    failures=$((failures + 1))
  fi
}

setup "$tmp/ok" "$pin" "$pin"
expect matching 0 'OK' "$tmp/ok"
if ! grep -q -- '--locked' "$tmp/cargo-call"; then
  echo 'FAIL metadata must not update committed dependency metadata'
  failures=$((failures + 1))
fi
expect resolved_path 1 'cargo resolves.*a path' "$tmp/ok" CARGO_SOURCE=null
expect transitive_core_path 1 'behavior-core.*a path' "$tmp/ok" CARGO_CORE_SOURCE=null
expect transitive_core_revision 1 "behavior-core.*$other.*$pin" "$tmp/ok" \
  CARGO_CORE_SOURCE="\"git+$repo?rev=$other#$other\""
expect metadata_bypass_refused 1 'cargo resolves.*a path' "$tmp/ok" \
  CARGO_SOURCE=null CHECK_CORE_PIN_NO_METADATA=1
expect dev_metadata_bypass_refused 1 'cargo resolves.*a path' "$tmp/ok" \
  CARGO_SOURCE=null BEHAVIOR_DEV_CORE_PATH=1
setup "$tmp/toml" "$other" "$pin"
expect toml_rev 1 "Cargo.toml.*$other.*$pin|$pin.*$other" "$tmp/toml"
setup "$tmp/lock" "$pin" "$other"
expect lock_rev 1 "Cargo.lock.*$other" "$tmp/lock"
setup "$tmp/patch" "$pin" "$pin"
mkdir -p "$tmp/patch/.cargo"
printf '[patch."%s"]\nbehavior-engine = { path = "../behavior-ir-core/crates/behavior-engine" }\n' \
  "$repo" >"$tmp/patch/.cargo/config.toml"
expect patch_override 1 'override.*behavior-ir-core/crates/behavior-engine' "$tmp/patch"
expect dev_override_refused 1 'override' "$tmp/patch" BEHAVIOR_DEV_CORE_PATH=1
expect dev_override_never_in_ci 1 'override' "$tmp/patch" BEHAVIOR_DEV_CORE_PATH=1 CI=true
setup "$tmp/manifest-patch" "$pin" "$pin"
printf '\n[patch."%s"]\nbehavior-engine = { path = "../local-core" }\n' "$repo" \
  >>"$tmp/manifest-patch/Cargo.toml"
expect tracked_patch 1 'Cargo.toml.*override|override.*Cargo.toml' "$tmp/manifest-patch"
setup "$tmp/manifest-path" "$pin" "$pin"
sed -i 's/rev = /path = "..\/local-core", rev = /' "$tmp/manifest-path/Cargo.toml"
expect tracked_path 1 'path' "$tmp/manifest-path"

if [ "$failures" -ne 0 ]; then echo "test_check_core_pin: $failures failed"; exit 1; fi
echo "test_check_core_pin: OK"
