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
    assert behavior.__version__ == "0.8.0"


def test_versions_are_the_engine_info_plus_the_binding() -> None:
    expected = _engine_info()
    expected["binding"] = {"python": "0.8.0"}
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
