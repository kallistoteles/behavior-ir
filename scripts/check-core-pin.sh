#!/usr/bin/env bash
# The ecosystem builds against exactly the Core Release it declares (feature 011, FR-017,
# FR-024, data-model.md "Ecosystem pin"). Fails, naming both values, unless:
#   1. the `rev` of behavior-engine in Cargo.toml and its source in Cargo.lock are the commit of
#      core-release.json, from its repository;
#   2. no path or [patch] override is active in the supported dependency graph;
#   3. cargo resolves the engine and its internal core crates from that git revision without
#      changing the lockfile (including overrides inherited from outside the checkout).
# Private experiments must disable their overrides before running this check. No bypass exists.
#
#   scripts/check-core-pin.sh [--root DIR]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "${1:-}" = "--root" ]; then root="$(cd "${2:?--root needs a directory}" && pwd)"; fi
cd "$root"

python3 - <<'PY'
import json, pathlib, sys, tomllib

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
if "path" in dep:
    errors.append(f"Cargo.toml declares a forbidden core path dependency: {dep['path']}")
want = f"git+{repo}?rev={commit}#{commit}"
lock = [p for p in tomllib.load(open("Cargo.lock", "rb"))["package"]
        if p["name"] == "behavior-engine"]
if [p.get("source") for p in lock] != [want]:
    errors.append(f"Cargo.lock resolves behavior-engine from "
                  f"{[p.get('source') for p in lock]}, expected {want}")

for cfg in (pathlib.Path("Cargo.toml"), pathlib.Path(".cargo/config.toml"), pathlib.Path(".cargo/config")):
    if not cfg.is_file():
        continue
    data = tomllib.load(open(cfg, "rb"))
    overrides = [f"{cfg}: [patch.\"{src}\"] {name} = {spec}"
                 for src, deps in data.get("patch", {}).items() for name, spec in deps.items()]
    overrides += [f"{cfg}: paths = {p}" for p in data.get("paths", [])]
    overrides += [f"{cfg}: [replace] {name} = {spec}"
                  for name, spec in data.get("replace", {}).items()]
    for o in overrides:
        errors.append(f"core override active; disable it before required gates or release work: {o}")

for e in errors:
    print(f"check-core-pin: {e}", file=sys.stderr)
sys.exit(1 if errors else 0)
PY

want="$(python3 -c 'import json; p = json.load(open("core-release.json")); r, c = p["repository"], p["commit"]; print(f"git+{r}?rev={c}#{c}")')"
cargo metadata --locked -q --format-version 1 | python3 -c '
import json, sys
expected = sys.argv[1]
names = {"behavior-engine", "behavior-core", "behavior-store", "behavior-verify", "behavior-cli"}
core = [p for p in json.load(sys.stdin)["packages"] if p["name"] in names]
errors = []
if sum(p["name"] == "behavior-engine" for p in core) != 1:
    errors.append("expected exactly one resolved behavior-engine")
for p in sorted(core, key=lambda p: p["name"]):
    name = p["name"]
    source = p.get("source") or "a path"
    if source != expected:
        errors.append(f"cargo resolves {name} from {source}, expected {expected}")
for error in errors:
    print(f"check-core-pin: {error}", file=sys.stderr)
sys.exit(1 if errors else 0)
' "$want"
echo "check-core-pin: OK ($(python3 -c 'import json; p = json.load(open("core-release.json")); print(p["tag"], p["commit"])'))"
