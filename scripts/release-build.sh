#!/usr/bin/env bash
# Builds the artifacts of a Behavior release into <out> (contracts/release.md): the manylinux
# wheel of the Python binding, SHA256SUMS and release-manifest.json. It neither checks nor tags;
# scripts/release.sh and scripts/release-check.sh call it.
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?usage: scripts/release-build.sh <version> <out>}"
out="${2:?usage: scripts/release-build.sh <version> <out>}"

rm -rf "$out"
mkdir -p "$out"
maturin build --release --zig --compatibility manylinux_2_28 --out "$out" >&2
wheel="$(cd "$out" && ls ./*.whl)"
(cd "$out" && sha256sum ./*.whl | sed 's# \./# #' >SHA256SUMS)

info="$(cargo run -q -p behavior-cli -- engine-info)"
commit="$(git rev-parse HEAD)"
python3 - "$out" "$version" "$commit" "$info" "${wheel#./}" <<'PY'
import json, sys
out, version, commit, info, wheel = sys.argv[1:]
versions = json.loads(info)
if versions["engine"] != version:
    sys.exit(f"release-build: engine version {versions['engine']} is not {version}")
sums = dict(reversed(line.split()) for line in open(f"{out}/SHA256SUMS").read().splitlines())
# The Python binding's version is the release version in Python's form (PEP 440), as maturin
# writes it into the wheel: the package's own converter, loaded without importing the package.
import runpy
python_version = runpy.run_path("python/behavior/_versions.py")["python_version"](version)
manifest = {
    "format": "behavior.release_manifest.v1",
    "release": version,
    "tag": f"v{version}",
    "commit": commit,
    "versions": versions,
    "bindings": {"python": {"version": python_version, "requires_python": ">=3.13"}},
    "platforms": ["manylinux_2_28_x86_64"],
    "artifacts": [{"file": wheel, "sha256": sums[wheel]}],
    "solver": {"name": "z3", "version": "4.16.0"},
}
with open(f"{out}/release-manifest.json", "w") as f:
    f.write(json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n")
PY
echo "release-build: $out" >&2
