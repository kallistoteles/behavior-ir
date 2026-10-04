#!/usr/bin/env bash
# The gate before any release is built (feature 011, FR-028, FR-029). Refuses, naming the values,
# unless the tag is annotated, its version is the declared release version
# (`[workspace.package] version` in Cargo.toml), and its commit is on main. With a GitHub token
# (`GH_TOKEN` or `GITHUB_TOKEN`) it also requires the commit's checks to be green: the checks
# named in CHECK_TAG_REQUIRED, or every reported check when that is unset.
#
#   scripts/check-tag.sh v<version>
set -euo pipefail
cd "$(dirname "$0")/.."

tag="${1:?usage: scripts/check-tag.sh v<version>}"
tag="${tag#refs/tags/}"
refuse() { echo "check-tag: $*" >&2; exit 1; }

kind="$(git cat-file -t "refs/tags/$tag" 2>/dev/null || true)"
[ -n "$kind" ] || refuse "no tag $tag"
[ "$kind" = tag ] || refuse "tag $tag is not annotated (a release tag must be: git tag -a)"

declared="$(git show "$tag^{commit}:Cargo.toml" | python3 -c 'import sys, tomllib; print(tomllib.loads(sys.stdin.read())["workspace"]["package"]["version"])')"
[ "${tag#v}" = "$declared" ] ||
  refuse "tag $tag names version ${tag#v}, but its Cargo.toml declares $declared"

commit="$(git rev-parse "$tag^{commit}")"
git rev-parse -q --verify origin/main >/dev/null || refuse "origin/main is unknown; fetch it first"
git merge-base --is-ancestor "$commit" origin/main ||
  refuse "tag $tag is on commit $commit, which is not on main"

if [ -n "${GH_TOKEN:-${GITHUB_TOKEN:-}}" ]; then
  repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is needed to query the checks}"
  gh api "repos/$repo/commits/$commit/check-runs" | python3 -c '
import json, os, sys
runs = {r["name"]: r.get("conclusion") for r in json.load(sys.stdin).get("check_runs", [])}
required = os.environ.get("CHECK_TAG_REQUIRED", "").split() or sorted(runs)
problems = [f"{n}: {runs[n]}" if n in runs else f"{n}: missing" for n in required
            if runs.get(n) != "success"]
if not required:
    problems = ["no checks reported"]
if problems:
    sys.exit("check-tag: checks not green on " + sys.argv[1] + ": " + ", ".join(problems))
' "$commit"
else
  echo "check-tag: CI status not verified (no token)" >&2
fi
echo "check-tag: OK ($tag, $commit)"
