#!/usr/bin/env bash
# Tests of scripts/fetch-core.sh (feature 011, FR-024, R3): the released conformance archive and
# CLI are installed only when their checksums and the CLI's engine version match the pin.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

# release <dir> <engine version the CLI reports>: a fake Core Release 0.10.2 in <dir>.
release() {
  local d="$1"
  mkdir -p "$d/src/tests/fixtures" "$d/src/schema" "$d/assets"
  echo '{"x":1}' >"$d/src/tests/fixtures/x.json"
  echo '{"y":1}' >"$d/src/schema/y.json"
  (cd "$d/src" && tar -czf "$d/assets/behavior-conformance-0.10.2.tar.gz" tests schema)
  printf '#!/bin/sh\necho %s\n' "'{\"engine\":\"$2\"}'" >"$d/assets/behavior-0.10.2-x86_64-linux-musl"
  chmod +x "$d/assets/behavior-0.10.2-x86_64-linux-musl"
}

# project <dir> <release dir>: an ecosystem checkout pinning that release.
project() {
  mkdir -p "$1/python/behavior"
  python3 - "$1" "$2/assets" <<'PY'
import hashlib, json, sys
root, assets = sys.argv[1:]
sha = lambda f: hashlib.sha256(open(f"{assets}/{f}", "rb").read()).hexdigest()
pin = {"format": "behavior.core_pin.v1", "version": "0.10.2", "tag": "v0.10.2",
       "commit": "a" * 40, "repository": "https://github.com/kallistoteles/behavior-ir-core",
       "assets": {"conformance": {"file": "behavior-conformance-0.10.2.tar.gz",
                                  "sha256": sha("behavior-conformance-0.10.2.tar.gz")},
                  "cli": {"file": "behavior-0.10.2-x86_64-linux-musl",
                          "sha256": sha("behavior-0.10.2-x86_64-linux-musl")}}}
open(f"{root}/core-release.json", "w").write(json.dumps(pin, sort_keys=True) + "\n")
PY
}

# expect <name> <exit> <pattern> <project> <release dir>
expect() {
  local rc=0
  BEHAVIOR_CORE_RELEASE_DIR="$5/assets" "$here/fetch-core.sh" --root "$4" >"$tmp/out" 2>&1 || rc=$?
  if [ "$rc" -ne "$2" ] || ! grep -qE "$3" "$tmp/out"; then
    echo "FAIL $1: exit $rc (want $2): $(cat "$tmp/out")"
    failures=$((failures + 1))
  fi
}

release "$tmp/r1" 0.10.2
project "$tmp/p1" "$tmp/r1"
expect installs 0 'BEHAVIOR_CORE_DIR=.*\.core/0\.10\.2' "$tmp/p1" "$tmp/r1"
[ -f "$tmp/p1/.core/0.10.2/tests/fixtures/x.json" ] || { echo "FAIL installs: fixtures"; failures=$((failures + 1)); }
[ -x "$tmp/p1/python/behavior/_bin/behavior" ] || { echo "FAIL installs: cli"; failures=$((failures + 1)); }

release "$tmp/r2" 0.10.2
project "$tmp/p2" "$tmp/r2"
echo corrupt >>"$tmp/r2/assets/behavior-conformance-0.10.2.tar.gz"
expect corrupted 1 'behavior-conformance-0\.10\.2\.tar\.gz.*expected [0-9a-f]{64}.*got [0-9a-f]{64}' "$tmp/p2" "$tmp/r2"

release "$tmp/r3" 0.10.9
project "$tmp/p3" "$tmp/r3"
expect wrong_engine 1 '0\.10\.9.*0\.10\.2' "$tmp/p3" "$tmp/r3"

if [ "$failures" -ne 0 ]; then echo "test_fetch_core: $failures failed"; exit 1; fi
echo "test_fetch_core: OK"
