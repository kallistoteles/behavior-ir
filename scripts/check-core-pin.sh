#!/usr/bin/env bash
# The ecosystem builds against exactly the Core Release it declares (feature 011, FR-017,
# FR-024, data-model.md "Ecosystem pin"). Fails, naming both values, unless:
#   1. the `rev` of behavior-engine in Cargo.toml and its source in Cargo.lock are the commit of
#      core-release.json, from its repository;
#   2. no path or [patch] override of the core is active. A development override is allowed
#      only with BEHAVIOR_DEV_CORE_PATH set and never in CI;
#   3. cargo resolves behavior-engine from that git revision (skipped with
#      CHECK_CORE_PIN_NO_METADATA=1, for the script's own tests).
#
#   scripts/check-core-pin.sh [--root DIR]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "${1:-}" = "--root" ]; then root="$(cd "${2:?--root needs a directory}" && pwd)"; fi
cd "$root"

python3 - <<'PY'
import json, os, pathlib, sys, tomllib

errors = []
pin = json.load(open("core-release.json"))
repo, commit = pin["repository"], pin["commit"]
dep = tomllib.load(open("Cargo.toml", "rb"))["workspace"]["dependencies"]["behavior-engine"]
if dep.get("git") != repo:
    errors.append(f"Cargo.toml takes behavior-engine from {dep.get('git') or dep}, "
                  f"core-release.json declares {repo}")
if dep.get("rev") != commit:
    errors.append(f"Cargo.toml pins behavior-engine at {dep.get('rev')}, "
                  f"core-release.json declares {commit}")
want = f"git+{repo}?rev={commit}#{commit}"
lock = [p for p in tomllib.load(open("Cargo.lock", "rb"))["package"]
        if p["name"] == "behavior-engine"]
if [p.get("source") for p in lock] != [want]:
    errors.append(f"Cargo.lock resolves behavior-engine from "
                  f"{[p.get('source') for p in lock]}, expected {want}")

dev = os.environ.get("BEHAVIOR_DEV_CORE_PATH") and not os.environ.get("CI")
for cfg in (pathlib.Path(".cargo/config.toml"), pathlib.Path(".cargo/config")):
    if not cfg.is_file():
        continue
    data = tomllib.load(open(cfg, "rb"))
    overrides = [f"{cfg}: [patch.\"{src}\"] {name} = {spec}"
                 for src, deps in data.get("patch", {}).items() for name, spec in deps.items()]
    overrides += [f"{cfg}: paths = {p}" for p in data.get("paths", [])]
    for o in overrides:
        if dev:
            print(f"check-core-pin: development override active: {o}", file=sys.stderr)
        else:
            errors.append(f"core override active (allowed only with BEHAVIOR_DEV_CORE_PATH, "
                          f"never in CI): {o}")

for e in errors:
    print(f"check-core-pin: {e}", file=sys.stderr)
sys.exit(1 if errors else 0)
PY

if [ -z "${CHECK_CORE_PIN_NO_METADATA:-}" ] && ! { [ -n "${BEHAVIOR_DEV_CORE_PATH:-}" ] && [ -z "${CI:-}" ]; }; then
  source="$(cargo metadata -q --format-version 1 | python3 -c '
import json, sys
print(next((p["source"] or "a path") for p in json.load(sys.stdin)["packages"] if p["name"] == "behavior-engine"))')"
  want="$(python3 -c 'import json; p = json.load(open("core-release.json")); r, c = p["repository"], p["commit"]; print(f"git+{r}?rev={c}#{c}")')"
  if [ "$source" != "$want" ]; then
    echo "check-core-pin: cargo resolves behavior-engine from $source, expected $want" >&2
    exit 1
  fi
fi
echo "check-core-pin: OK ($(python3 -c 'import json; p = json.load(open("core-release.json")); print(p["tag"], p["commit"])'))"
