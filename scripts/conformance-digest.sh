#!/usr/bin/env bash
# Conformance digest (feature 011, SC-003, SC-009): the SHA-256 of every conformance fixture and
# schema file, and of every output the determinism check compares, as canonical JSON on stdout.
# Two digests that are equal prove that a move changed no fixture and no engine output.
#
#   scripts/conformance-digest.sh [--core-dir DIR] [--no-outputs]
#
# Keys are `file:<path>` (relative to DIR, default the repository) and
# `output:<section>:<label>` (the determinism check's sections, `core` and `ecosystem`).
set -euo pipefail
cd "$(dirname "$0")/.."

core_dir=.
outputs=1
while [ $# -gt 0 ]; do
  case "$1" in
    --core-dir) core_dir="${2:?--core-dir needs a directory}"; shift 2 ;;
    --no-outputs) outputs=0; shift ;;
    *) echo "usage: scripts/conformance-digest.sh [--core-dir DIR] [--no-outputs]" >&2; exit 2 ;;
  esac
done

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
: >"$tmp/outputs.tsv"
if [ "$outputs" -eq 1 ]; then
  if ! BEHAVIOR_DIGEST_DIR="$tmp" scripts/determinism-check.sh >/dev/null 2>"$tmp/check.err"; then
    echo "conformance-digest: the determinism check failed: $(tail -n 3 "$tmp/check.err")" >&2
    exit 1
  fi
fi

python3 - "$core_dir" "$tmp/outputs.tsv" <<'PY'
import hashlib, json, os, sys

root, outputs = sys.argv[1], sys.argv[2]
digest = {}
for top in ("tests/fixtures", "schema"):
    for dirpath, dirnames, filenames in os.walk(os.path.join(root, top)):
        dirnames[:] = sorted(d for d in dirnames if d != "__pycache__")
        for name in filenames:
            if name.endswith(".pyc"):
                continue
            path = os.path.join(dirpath, name)
            with open(path, "rb") as f:
                digest["file:" + os.path.relpath(path, root)] = hashlib.sha256(f.read()).hexdigest()
for line in open(outputs).read().splitlines():
    section, label, sha = line.split("\t")
    key = f"output:{section}:{label}"
    if key in digest:
        sys.exit(f"conformance-digest: duplicate output label {key}")
    digest[key] = sha
sys.stdout.write(json.dumps(digest, sort_keys=True, separators=(",", ":")) + "\n")
PY
