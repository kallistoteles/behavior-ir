#!/usr/bin/env bash
# A published release equals a local build of its commit (feature 011, SC-011): downloads the
# release's assets, rebuilds them from the tag in a temporary worktree, and compares SHA256SUMS.
#
#   scripts/release-verify.sh v<version> [owner/repo]
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/check-core-pin.sh
tag="${1:?usage: scripts/release-verify.sh v<version> [owner/repo]}"
repo="${2:-kallistoteles/behavior-ir}"
tmp="$(mktemp -d)"
trap 'git worktree remove --force "$tmp/src" 2>/dev/null || true; rm -rf "$tmp"' EXIT

gh release download "$tag" -R "$repo" -D "$tmp/published"
git worktree add -q --detach "$tmp/src" "$tag"
(cd "$tmp/src" && scripts/release-build.sh "${tag#v}" "$tmp/local" >/dev/null)
if cmp -s "$tmp/published/SHA256SUMS" "$tmp/local/SHA256SUMS"; then
  echo "release-verify: identical ($tag)"
else
  echo "release-verify: DIFFERENT ($tag)" >&2
  diff "$tmp/published/SHA256SUMS" "$tmp/local/SHA256SUMS" >&2 || true
  exit 1
fi
