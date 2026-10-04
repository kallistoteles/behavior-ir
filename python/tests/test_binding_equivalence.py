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


def _fixture_model(name: str) -> Any:
    return importlib.import_module(f"python.tests.fixtures.{name}").model


# Every equivalence pair (feature 011, SC-004): a module written through the binding and the
# core's wire fixture it must reproduce, identity for identity.
PAIRS: list[tuple[str, Any, Path]] = [
    *[(d, lambda d=d: model_of(d), BINDINGS / f"{d}.json") for d in DOMAINS],
    ("lab (reads)", lambda: _fixture_model("lab_model"), FIXTURES / "reads/modules/lab.json"),
    ("cultures_v1", lambda: _fixture_model("cultures_v1"),
     FIXTURES / "migration/modules/cultures_v1.json"),
    ("cultures_v2", lambda: _fixture_model("cultures_v2"),
     FIXTURES / "migration/modules/cultures_v2.json"),
]
# Migration pairs: the binding's migration and the core's migration document.
MIGRATION_PAIRS: list[tuple[str, Any, Path, Path, Path]] = [
    ("cultures_v1_to_v2",
     lambda: importlib.import_module("python.tests.fixtures.cultures_migration").migration,
     FIXTURES / "migration/modules/cultures_v1.json",
     FIXTURES / "migration/modules/cultures_v2.json",
     FIXTURES / "migration/valid/cultures_v1_to_v2.json"),
]


def item_drift(fixture: str, ours: dict[str, str], theirs: dict[str, str]) -> str | None:
    """The first item whose identity differs between the binding's module and the core's
    fixture, named with both hashes; None when every item is identical."""
    for item in sorted(set(ours) | set(theirs)):
        if ours.get(item) != theirs.get(item):
            return (f"{fixture}: {item} differs: binding {ours.get(item, 'missing')}, "
                    f"core fixture {theirs.get(item, 'missing')}")
    return None


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


# --- feature 011: one semantic truth across authoring paths (US3, SC-004) ------------------------

def _hashes(path: Path) -> dict[str, str]:
    out = cli("hashes", str(path))
    assert out.returncode == 0, out.stderr
    value: dict[str, str] = json.loads(out.stdout)
    return value


def test_drift_names_the_fixture_and_item(tmp_path: Path) -> None:
    """A binding change that alters the IR it emits fails naming the fixture and the item."""
    wire = json.loads(model_of("invoice").to_wire_json())
    action = next(a for a in wire["actions"] if a["preconditions"])
    action["preconditions"].append(action["preconditions"][0])
    drifted = tmp_path / "drifted.json"
    drifted.write_text(json.dumps(wire))
    message = item_drift("invoice", _hashes(drifted), _hashes(BINDINGS / "invoice.json"))
    assert message is not None
    assert "invoice" in message and f"action:{action['name']}" in message
    assert message.count("sha256:") == 2, message
    assert item_drift("invoice", _hashes(BINDINGS / "invoice.json"),
                      _hashes(BINDINGS / "invoice.json")) is None


def test_every_wire_form_has_a_pair() -> None:
    """SC-004: at least one equivalence pair per wire IR form."""
    def walk(v: Any) -> Any:
        if isinstance(v, dict):
            yield v
            for c in v.values():
                yield from walk(c)
        elif isinstance(v, list):
            for c in v:
                yield from walk(c)

    docs = [json.loads(path.read_text()) for _, _, path in PAIRS]
    nodes = [n for d in docs for n in walk(d)]
    forms = {
        "entities and types": any(d["entities"] and d["enums"] for d in docs),
        "lifecycle": any("create" in n for n in nodes) and any("remove" in n for n in nodes),
        "queries": any(n.get("op") == "select" for n in nodes),
        "module invariants": any(d["invariants"] for d in docs),
        "exact arithmetic": any(n.get("scale") is not None for d in docs for n in d["nominals"]),
        "reads": any(d.get("reads") for d in docs),
        "migrations": bool(MIGRATION_PAIRS),
    }
    assert all(forms.values()), [f for f, ok in forms.items() if not ok]




@pytest.mark.parametrize(("name", "module", "path"), PAIRS, ids=[p[0] for p in PAIRS])
def test_every_pair_has_the_same_identities(name: str, module: Any, path: Path,
                                            tmp_path: Path) -> None:
    dsl = tmp_path / "dsl.json"
    dsl.write_text(module().to_wire_json())
    assert cli("version", str(dsl)).stdout == cli("version", str(path)).stdout, name
    drift = item_drift(name, _hashes(dsl), _hashes(path))
    assert drift is None, drift


@pytest.mark.parametrize(("name", "migration", "source", "target", "doc"), MIGRATION_PAIRS,
                         ids=[p[0] for p in MIGRATION_PAIRS])
def test_every_migration_pair_admits_identically(name: str, migration: Any, source: Path,
                                                 target: Path, doc: Path,
                                                 tmp_path: Path) -> None:
    ours = tmp_path / "migration.json"
    ours.write_text(migration().to_json())
    a = cli("migration", "admit", str(source), str(target), str(ours))
    b = cli("migration", "admit", str(source), str(target), str(doc))
    assert a.returncode == b.returncode == 0, (a.stderr, b.stderr)
    assert without_locations(json.loads(a.stdout)) == without_locations(json.loads(b.stdout)), name
