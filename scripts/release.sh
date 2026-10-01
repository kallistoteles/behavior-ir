#!/usr/bin/env bash
# Cuts a Behavior release (contracts/release.md): builds dist/v<version>/, runs the release check
# and, if it passes, creates the annotated tag v<version>. Pushing the tag and uploading the
# artifacts are manual. --build-only builds dist/v<version>/ without checking or tagging.
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
if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
  die "tag v$version already exists; a release tag is never moved"
fi

out="dist/v$version"
scripts/release-build.sh "$version" "$out"
if [ "$build_only" -eq 1 ]; then
  echo "release: built $out (not checked, not tagged)" >&2
  exit 0
fi
scripts/release-check.sh "$out"
git tag -a "v$version" -m "Behavior $version"
echo "release: tagged v$version; push the tag and upload $out/ by hand" >&2
