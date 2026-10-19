#!/usr/bin/env bash
# Feature 510: required proof tools come from the existing locked Nix package.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() { echo "test_safety_toolchain: $*" >&2; exit 1; }

for system in x86_64-linux aarch64-linux; do
  version="$(nix eval --raw "$root#packages.$system.safety-proof-toolchain.version")"
  [ "$version" = 4.30.0 ] || fail "unexpected $system Lean version: $version"
done

system="$(nix eval --impure --raw --expr builtins.currentSystem)"
selected="$(nix eval --raw "$root#packages.$system.safety-proof-toolchain.outPath")"
derivation="$(nix eval --raw "$root#packages.$system.safety-proof-toolchain.drvPath")"
case "$selected" in /nix/store/*-lean4-4.30.0) ;; *) fail "not the locked Lean output" ;; esac
case "$derivation" in /nix/store/*-lean4-4.30.0.drv) ;; *) fail "not the locked Lean derivation" ;; esac

mkdir -p "$tmp/fake-bin"
cat >"$tmp/fake-bin/lean" <<'FAKE'
#!/bin/sh
echo 'Lean 99.0.0 (untrusted PATH override)'
exit 99
FAKE
cp "$tmp/fake-bin/lean" "$tmp/fake-bin/leanchecker"
chmod +x "$tmp/fake-bin/lean" "$tmp/fake-bin/leanchecker"

# A caller-controlled PATH must not select either proof executable.
nix develop "$root" --command env \
  PATH="$tmp/fake-bin:$PATH" EXPECTED_TOOLCHAIN="$selected" EXPECTED_DERIVATION="$derivation" \
  bash -c '
    set -euo pipefail
    test "$BEHAVIOR_SAFETY_TOOLCHAIN" = "$EXPECTED_TOOLCHAIN"
    test "$BEHAVIOR_SAFETY_TOOLCHAIN_DRV" = "$EXPECTED_DERIVATION"
    test -x "$BEHAVIOR_SAFETY_TOOLCHAIN/bin/leanchecker"
    "$BEHAVIOR_SAFETY_TOOLCHAIN/bin/lean" --version | grep -E "version 4[.]30[.]0([ ,)]|$)"
  '

for variable in BEHAVIOR_LEAN_BINARY BEHAVIOR_LEANCHECKER_BINARY; do
  if env "$variable=$tmp/fake-bin/lean" nix develop "$root" --command true >"$tmp/override.log" 2>&1; then
    fail "$variable was accepted"
  fi
  if ! grep -q 'Safety toolchain overrides are unsupported' "$tmp/override.log"; then
    fail "$variable failed for an unrelated reason"
  fi
done

echo 'test_safety_toolchain: OK'
