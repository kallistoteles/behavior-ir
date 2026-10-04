#!/usr/bin/env bash
# Checks and builds a Behavior release from its tag (contracts/release.md, feature 011): the
# annotated tag v<version> must exist on the checked-out commit and pass scripts/check-tag.sh;
# then dist/v<version>/ is built, the release check runs, and the release notes name the exact
# core the package bundles. It neither creates tags nor publishes; the ecosystem-release workflow
# publishes a pushed tag. --build-only builds dist/v<version>/ without any check.
set -euo pipefail
cd "$(dirname "$0")/.."
build_only=0
if [ "${1:-}" = "--build-only" ]; then
  build_only=1
  shift
fi
version="${1:?usage: scripts/release.sh [--build-only] <version>}"
die() {
  echo "release: $*" >&2
  exit 1
}

workspace="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"])')"
[ "$version" = "$workspace" ] || die "version $version differs from the workspace version $workspace"
[ -z "$(git status --porcelain)" ] || die "the working tree is not clean; commit or stash first"

out="dist/v$version"
if [ "$build_only" -eq 1 ]; then
  scripts/release-build.sh "$version" "$out"
  echo "release: built $out (not checked)" >&2
  exit 0
fi
scripts/check-tag.sh "v$version"
[ "$(git rev-parse "v$version^{commit}")" = "$(git rev-parse HEAD)" ] ||
  die "v$version is not the checked-out commit"
scripts/release-build.sh "$version" "$out"
scripts/release-check.sh "$out"
python3 - "$out" <<'PY'
import json, sys
out = sys.argv[1]
m = json.load(open(f"{out}/release-manifest.json"))
c, v = m["core"], m["versions"]
lines = [
    f"Behavior {m['release']} (commit {m['commit']}).",
    "",
    f"Bundles **Behavior Core {c['version']}** ({c['tag']}, commit {c['commit']}, {c['repository']}):",
    f"engine {v['engine']}, verifier {v['verifier']}, wire IR {', '.join(v['wire_ir'])}.",
    "",
    "Install with `pip install --require-hashes` using the wheel and its SHA256SUMS entry.",
]
open(f"{out}/NOTES.md", "w").write("\n".join(lines) + "\n")
PY
echo "release: $out" >&2
