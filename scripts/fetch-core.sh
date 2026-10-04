#!/usr/bin/env bash
# Installs the pinned Core Release (feature 011, FR-024, research R3): downloads its conformance
# archive and its `behavior` CLI, refuses any asset whose SHA-256 differs from core-release.json,
# unpacks the schemas and fixtures into .core/<version>/, installs the CLI where the Python
# package bundles it (python/behavior/_bin/behavior), and checks that the CLI reports the
# declared core version. No core checkout is involved.
#
#   scripts/fetch-core.sh [--root DIR]
#
# Prints `export BEHAVIOR_CORE_DIR=…` on stdout. BEHAVIOR_CORE_RELEASE_DIR takes the assets from
# a local directory instead of the GitHub Release (tests, offline work).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "${1:-}" = "--root" ]; then root="$(cd "${2:?--root needs a directory}" && pwd)"; fi
cd "$root"

read -r version tag repository conformance conformance_sha cli cli_sha < <(python3 -c '
import json
p = json.load(open("core-release.json"))
a = p["assets"]
print(p["version"], p["tag"], p["repository"], a["conformance"]["file"], a["conformance"]["sha256"],
      a["cli"]["file"], a["cli"]["sha256"])')

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fetch() {
  if [ -n "${BEHAVIOR_CORE_RELEASE_DIR:-}" ]; then
    cp "$BEHAVIOR_CORE_RELEASE_DIR/$1" "$tmp/$1"
  else
    curl -fsSL --retry 3 -o "$tmp/$1" "$repository/releases/download/$tag/$1"
  fi
  local got
  got="$(sha256sum "$tmp/$1" | cut -d' ' -f1)"
  if [ "$got" != "$2" ]; then
    echo "fetch-core: $1: expected $2, got $got" >&2
    exit 1
  fi
}
fetch "$conformance" "$conformance_sha"
fetch "$cli" "$cli_sha"
chmod 0755 "$tmp/$cli"

engine="$("$tmp/$cli" engine-info | python3 -c 'import json, sys; print(json.load(sys.stdin)["engine"])')"
if [ "$engine" != "$version" ]; then
  echo "fetch-core: the CLI of $tag reports engine $engine, core-release.json declares $version" >&2
  exit 1
fi

dest="$root/.core/$version"
rm -rf "$dest.tmp" && mkdir -p "$dest.tmp"
tar -xzf "$tmp/$conformance" -C "$dest.tmp"
rm -rf "$dest" && mv "$dest.tmp" "$dest"
mkdir -p python/behavior/_bin
install -m 0755 "$tmp/$cli" python/behavior/_bin/behavior.tmp
mv python/behavior/_bin/behavior.tmp python/behavior/_bin/behavior
echo "fetch-core: core $tag installed (fixtures in .core/$version, CLI in python/behavior/_bin)" >&2
echo "export BEHAVIOR_CORE_DIR=$dest"
