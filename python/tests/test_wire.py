"""Wire IR emitted by the DSL is canonical and matches the reviewed golden files."""

from __future__ import annotations

import json

from .conftest import FIXTURES
from .fixtures import invoice_model, margin_model

GOLDEN = FIXTURES / "wire" / "python"


def test_invoice_matches_golden() -> None:
    assert invoice_model.model.to_wire_json() == (GOLDEN / "invoice.json").read_text()


def test_margin_matches_golden() -> None:
    assert margin_model.model.to_wire_json() == (GOLDEN / "project_margin.json").read_text()


def test_emission_is_canonical_and_stable() -> None:
    text = invoice_model.build_model().to_wire_json()
    assert text == invoice_model.build_model().to_wire_json()
    value = json.loads(text)
    assert json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) == text
    assert not text.endswith("\n")


def test_locations_are_relative_to_the_module_root() -> None:
    value = json.loads(invoice_model.model.to_wire_json())
    files = set()

    def walk(v: object) -> None:
        if isinstance(v, dict):
            if "loc" in v:
                files.add(v["loc"]["file"])
            for child in v.values():
                walk(child)
        elif isinstance(v, list):
            for child in v:
                walk(child)

    walk(value)
    assert files == {"invoice_model.py"}
