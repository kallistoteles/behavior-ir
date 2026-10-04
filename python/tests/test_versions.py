"""Feature 008: the binding reports its own and the engine's versions (research R2)."""

from __future__ import annotations

import json
import subprocess
import sys

import pytest

import behavior


def _engine_info() -> dict[str, object]:
    out = subprocess.run(
        [sys.executable, "-m", "behavior._cli", "engine-info"], check=True, capture_output=True,
        text=True,
    ).stdout
    value: dict[str, object] = json.loads(out)
    return value


def test_the_binding_version_is_the_release_version() -> None:
    assert behavior.__version__ == "0.10.4"


def test_versions_are_the_engine_info_plus_the_core_and_the_binding() -> None:
    expected = _engine_info()
    pin = _pin()
    expected["core"] = {"version": pin["version"], "commit": pin["commit"]}
    expected["binding"] = {"python": "0.10.4"}
    assert behavior.versions() == expected


def test_a_binding_refuses_a_different_engine() -> None:
    with pytest.raises(ImportError, match=r"0\.8\.0.*0\.9\.0"):
        behavior._check_versions("0.8.0", "0.9.0")
    behavior._check_versions("0.8.0", "0.8.0")


@pytest.mark.parametrize(("cargo", "python"), [
    ("0.8.0", "0.8.0"),
    ("0.9.0-rc.1", "0.9.0rc1"),
    ("0.9.0-alpha.2", "0.9.0a2"),
    ("0.9.0-beta.3", "0.9.0b3"),
    ("1.0.0+build.5", "1.0.0+build.5"),
    ("0.9.0-alpha", "0.9.0a0"),
    ("0.9.0-rc1", "0.9.0rc1"),
    ("0.9.0-RC.1", "0.9.0rc1"),
    ("0.9.0-beta.2", "0.9.0b2"),
    ("0.9.0-rc.1+build.7", "0.9.0rc1+build.7"),
])
def test_engine_versions_compare_in_python_form(cargo: str, python: str) -> None:
    """The wheel's version is the Cargo version normalized to Python's form (PEP 440), so a
    pre-release binding must accept its own engine (code review, feature 008)."""
    assert behavior._python_version(cargo) == python
    behavior._check_versions(python, cargo)
    with pytest.raises(ImportError):
        behavior._check_versions(python, "0.7.0")


def test_unsupported_pre_release_labels_are_refused() -> None:
    """A version Python packaging cannot represent is refused by name, never passed through."""
    from behavior._versions import python_version

    for unsupported in ["0.9.0-snapshot", "0.9.0-dev.4", "0.9.0-rc.x"]:
        with pytest.raises(ValueError, match=unsupported.replace(".", r"\.")):
            python_version(unsupported)


# --- feature 011: the binding names the exact core it was built against -------------------------

def _pin() -> dict[str, object]:
    from pathlib import Path
    root = Path(__file__).resolve().parents[2]
    value: dict[str, object] = json.loads((root / "core-release.json").read_text())
    return value


def test_a_skewed_core_is_refused() -> None:
    """US2 scenario 3: an extension built against another core than the one this release
    declares refuses to import, naming both versions."""
    with pytest.raises(ImportError, match=r"core 0\.10\.9.*declares core 0\.10\.2"):
        behavior._check_core(declared="0.10.2", engine="0.10.9")
    behavior._check_core(declared="0.10.2", engine="0.10.2")


def test_versions_report_the_core() -> None:
    pin = _pin()
    assert behavior.versions()["core"] == {"version": pin["version"], "commit": pin["commit"]}
    assert behavior.versions()["engine"] == pin["version"]


def test_binding_and_core_versions_may_differ() -> None:
    """FR-017: the ecosystem has its own release version; only the core must match exactly."""
    behavior._check_versions("0.11.0", "0.11.0")
    behavior._check_core(declared="0.10.2", engine="0.10.2")
    assert behavior.versions()["binding"] == {"python": behavior.__version__}
