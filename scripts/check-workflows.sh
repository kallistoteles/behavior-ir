#!/usr/bin/env bash
# The workflows only orchestrate (feature 011, FR-030, FR-035): they never read Spec Kit's or an
# agent's state or the specifications, the scripts they run never read that state either, and
# every action is pinned by a full commit SHA.
#
#   scripts/check-workflows.sh [--root DIR]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "${1:-}" = "--root" ]; then root="$(cd "${2:?--root needs a directory}" && pwd)"; fi
cd "$root"

fail=0
shopt -s nullglob
for w in .github/workflows/*.yml .github/workflows/*.yaml; do
  if grep -nE '\.specify|\.claude|feature\.json|(^|[^[:alnum:]_/.-])specs/' "$w" >/tmp/cw.$$; then
    sed "s#^#check-workflows: $w:#" /tmp/cw.$$ >&2; fail=1
  fi
  while IFS= read -r line; do
    ref="${line#*uses:}"; ref="${ref%%#*}"; ref="$(echo "$ref" | tr -d " '\"")"
    case "$ref" in
      ./*) ;;
      *@*) [[ "${ref##*@}" =~ ^[0-9a-f]{40}$ ]] ||
             { echo "check-workflows: $w: $ref is not pinned by a commit SHA" >&2; fail=1; } ;;
      *) echo "check-workflows: $w: $ref has no version" >&2; fail=1 ;;
    esac
  done < <(grep -E '^\s*-?\s*uses:' "$w" || true)
  for s in $(grep -oE 'scripts/[A-Za-z0-9_./-]+' "$w" | sort -u); do
    [ -f "$s" ] || continue
    if grep -nE '\.specify|\.claude/|feature\.json' "$s" >/tmp/cw.$$; then
      sed "s#^#check-workflows: $s (run by $w):#" /tmp/cw.$$ >&2; fail=1
    fi
  done
done
rm -f /tmp/cw.$$
if [ "$fail" -ne 0 ]; then echo "check-workflows: FAILED" >&2; exit 1; fi
echo "check-workflows: OK"
