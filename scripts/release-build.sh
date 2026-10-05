#!/usr/bin/env bash
# Builds the artifacts of a Behavior release into <out> (contracts/release.md, feature 011): the
# manylinux wheel of the Python binding, which bundles the pinned Core Release's `behavior` CLI,
# SHA256SUMS and release-manifest.json (with the exact core it bundles). The build is
# reproducible on the pinned toolchain (nix develop). It neither checks nor tags;
# scripts/release.sh and scripts/release-check.sh call it.
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/check-core-pin.sh
version="${1:?usage: scripts/release-build.sh <version> <out>}"
out="${2:?usage: scripts/release-build.sh <version> <out>}"

SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)"
export SOURCE_DATE_EPOCH CARGO_INCREMENTAL=0
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
export RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$cargo_home=/cargo"
export CARGO_TARGET_DIR="$PWD/target/release-build"

rm -rf "$out"
mkdir -p "$out"
# The verified core: its CLI goes into the wheel (python/behavior/_bin/behavior).
scripts/fetch-core.sh >/dev/null
maturin build --release --locked --zig --compatibility manylinux_2_28 --out "$out" >&2
wheel="$(cd "$out" && ls ./*.whl)"
(cd "$out" && sha256sum ./*.whl | sed 's# \./# #' >SHA256SUMS)

info="$(python/behavior/_bin/behavior engine-info)"
commit="$(git rev-parse HEAD)"
python3 - "$out" "$version" "$commit" "$info" "${wheel#./}" <<'PY'
import json, sys
out, version, commit, info, wheel = sys.argv[1:]
versions = json.loads(info)
pin = json.load(open("core-release.json"))
if versions["engine"] != pin["version"]:
    sys.exit(f"release-build: the bundled core reports engine {versions['engine']}, "
             f"core-release.json declares {pin['version']}")
import tomllib
declared = tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"]
if declared != version:
    sys.exit(f"release-build: Cargo.toml declares release {declared}, not {version}")
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
    "core": {k: pin[k] for k in ("version", "tag", "commit", "repository")},
    "bindings": {"python": {"version": python_version, "requires_python": ">=3.13"}},
    "platforms": ["manylinux_2_28_x86_64"],
    "artifacts": [{"file": wheel, "sha256": sums[wheel]}],
    "solver": {"name": "z3", "version": "4.16.0"},
}
with open(f"{out}/release-manifest.json", "w") as f:
    f.write(json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n")
PY
echo "release-build: $out" >&2
