"""Feature 008 (FR-003a, FR-003b, SC-009): a module authored through the Python binding is the same
module as its canonical wire form. The wire form is written directly by
`tests/fixtures/wire/build_fixtures.py`, never through the DSL. Both paths meet at the same
behavior version, the same item hashes and the same decisions. Every future binding must pass
the same comparison against the same fixtures.
"""

from __future__ import annotations

import importlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

from behavior import evaluate

from .conftest import FIXTURES

BINDINGS = FIXTURES / "bindings"
DOMAINS = ["invoice", "project_margin", "accounts", "ledger", "orders"]


def cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run([sys.executable, "-m", "behavior._cli", *args], capture_output=True,
                          text=True)


def without_locations(value: Any) -> Any:
    """A record without source locations: they are metadata of where a module was written, not
    part of its semantics or identity."""
    if isinstance(value, dict):
        return {k: without_locations(v) for k, v in value.items() if k != "loc"}
    if isinstance(value, list):
        return [without_locations(v) for v in value]
    return value


def model_of(domain: str) -> Any:
    return importlib.import_module(f"examples.{domain}.behavior").model


@pytest.mark.parametrize("domain", DOMAINS)
def test_the_binding_builds_the_canonical_module(domain: str, tmp_path: Path) -> None:
    dsl = tmp_path / "dsl.json"
    dsl.write_text(model_of(domain).to_wire_json())
    canonical = str(BINDINGS / f"{domain}.json")
    assert cli("version", str(dsl)).stdout == cli("version", canonical).stdout
    ours, theirs = cli("hashes", str(dsl)), cli("hashes", canonical)
    assert ours.returncode == theirs.returncode == 0
    assert json.loads(ours.stdout) == json.loads(theirs.stdout)


@pytest.mark.parametrize("domain", DOMAINS)
def test_the_binding_decides_as_the_canonical_module(domain: str, tmp_path: Path) -> None:
    requests = json.loads((BINDINGS / f"{domain}.request.json").read_text())
    assert set(requests) == {"allowed", "refused"}
    results = set()
    for name, request in requests.items():
        path = tmp_path / f"{name}.json"
        path.write_text(json.dumps(request))
        wire_record = cli("eval", str(BINDINGS / f"{domain}.json"), str(path)).stdout
        decision = evaluate(
            model_of(domain), request["action"], state=request["state"],
            input=request.get("input"), context=request.get("context"),
            data_version=request["data_version"], facts=request.get("facts"),
            git_revision=request.get("git_revision"),
        )
        assert without_locations(json.loads(decision.record_json)) == without_locations(
            json.loads(wire_record)), name
        results.add(decision.result == "ALLOW")
    assert results == {True, False}, "one allowed and one refused decision"
