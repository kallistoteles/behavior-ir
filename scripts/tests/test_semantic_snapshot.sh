#!/usr/bin/env bash
# Feature 508: scripts/semantic-snapshot.py --compare reports every difference, including a change
# of JSON type that Python equality hides (0 == False, 1 == True, 1 == 1.0).
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0
fail() { echo "test_semantic_snapshot: $1" >&2; failures=$((failures + 1)); }
cmp_() { "$here/scripts/semantic-snapshot.py" --compare "$tmp/a.json" "$tmp/b.json" >/dev/null 2>&1; }

printf '{"x":{"position":0,"ok":true,"n":1,"s":["a"]}}\n' >"$tmp/a.json"
cp "$tmp/a.json" "$tmp/b.json"
cmp_ || fail "identical snapshots must compare equal"
for changed in '{"x":{"position":false,"ok":true,"n":1,"s":["a"]}}' \
               '{"x":{"position":0,"ok":1,"n":1,"s":["a"]}}' \
               '{"x":{"position":0,"ok":true,"n":1.0,"s":["a"]}}' \
               '{"x":{"position":0,"ok":true,"n":1,"s":["a","b"]}}' \
               '{"x":{"position":0,"ok":true,"n":1}}' \
               '{"x":{"position":"0","ok":true,"n":1,"s":["a"]}}'; do
  printf '%s\n' "$changed" >"$tmp/b.json"
  cmp_ && fail "a changed snapshot must differ: $changed"
done
[ -x "$here/scripts/semantic-snapshot.py" ] || fail "scripts/semantic-snapshot.py must be executable"

if [ "$failures" -ne 0 ]; then echo "test_semantic_snapshot: $failures failure(s)" >&2; exit 1; fi
echo "test_semantic_snapshot: OK"
