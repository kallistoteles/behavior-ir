"""Cargo (SemVer) versions in the form Python packaging gives them (PEP 440).

This is the one place the conversion lives. The package uses it to compare its own version (the
wheel's metadata, PEP 440) with the engine's (Cargo form). `scripts/release-build.sh` loads this
file directly, without importing the package, to write the binding version into the release
manifest. It has no dependencies.
"""

from __future__ import annotations

import re

# Only the labels whose PEP 440 form is certain; any other is refused by name, so a release with
# such a version fails at build time instead of producing a wheel that cannot import.
_LABELS = {"alpha": "a", "a": "a", "beta": "b", "b": "b", "rc": "rc", "c": "rc"}
_PRE = re.compile(r"(?P<label>[A-Za-z]+)[.\-_]?(?P<number>[0-9]*)")


def python_version(cargo: str) -> str:
    """`0.9.0-rc.1` -> `0.9.0rc1`, `0.9.0-alpha` -> `0.9.0a0`, `0.9.0-rc1` -> `0.9.0rc1`; build
    metadata (`+…`) is kept as a local version. Raises ValueError for a pre-release that Python
    packaging cannot represent."""
    version, plus, local = cargo.partition("+")
    release, dash, pre = version.partition("-")
    if dash:
        m = _PRE.fullmatch(pre)
        label = m and _LABELS.get(m.group("label").lower())
        if not m or label is None:
            raise ValueError(f"version {cargo}: pre-release `{pre}` has no Python equivalent")
        release += f"{label}{int(m.group('number') or 0)}"
    return release + (plus + local if plus else "")
