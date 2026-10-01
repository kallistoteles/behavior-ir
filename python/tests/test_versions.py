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
