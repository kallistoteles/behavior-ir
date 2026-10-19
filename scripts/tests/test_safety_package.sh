#!/usr/bin/env bash
# Public installed-package boundary; full distribution creation stays in release-check.sh.
set -euo pipefail
cd "$(dirname "$0")/../.."
root="$PWD"
if [ ! -f release/safety_smoke.py ]; then
  echo 'test_safety_package: missing public Safety smoke' >&2
  exit 1
fi
python_bin="$(command -v python3)"
package_files="$(cargo package -p behavior-models --allow-dirty --list 2>/dev/null)"
for source in Data FieldOwnership LifecycleData LifecycleDependency LifecycleCheck LifecycleSequence Check Audit; do
  if ! grep -qx "proofs/safety/$source.lean" <<<"$package_files"; then
    echo "test_safety_package: missing embedded source $source" >&2
    exit 1
  fi
done
if ! grep -qx 'proofs/toolchain.json' <<<"$package_files"; then
  echo 'test_safety_package: missing pinned tool identity' >&2
  exit 1
fi
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/no-tools"
maturin sdist -o "$tmp/sdist" >/dev/null 2>&1
"$python_bin" - "$tmp/sdist" <<'PYTEST'
from pathlib import Path
import sys, tarfile
archive, = Path(sys.argv[1]).glob("*.tar.gz")
with tarfile.open(archive) as stream:
    names = stream.getnames()
for suffix in ("/Cargo.lock", "/rust-toolchain.toml", "/crates/behavior-models/proofs/toolchain.json"):
    assert any(name.endswith(suffix) for name in names), f"missing packaged dependency identity resource {suffix}"
PYTEST
cd "$tmp"
PATH="$tmp/no-tools" "$python_bin" "$root/release/safety_smoke.py" --ordinary
if "$python_bin" "$root/release/safety_smoke.py" --certify --toolchain "$tmp/not-the-pin" --output "$tmp/refused"; then
  echo 'test_safety_package: wrong tools accepted' >&2
  exit 1
fi
echo 'test_safety_package: OK'
