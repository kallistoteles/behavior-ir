"""Every valid and DSL-emitted wire file conforms to the published JSON Schema."""

from __future__ import annotations

import json

import jsonschema
import pytest

from .conftest import FIXTURES, REPO_ROOT

SCHEMA = json.loads((REPO_ROOT / "schema" / "wire-ir-0.1.schema.json").read_text())
FILES = sorted((FIXTURES / "wire" / "valid").glob("*.json")) + sorted(
    (FIXTURES / "wire" / "python").glob("*.json")
)


def test_schema_is_valid() -> None:
    jsonschema.Draft202012Validator.check_schema(SCHEMA)


@pytest.mark.parametrize("path", FILES, ids=lambda p: f"{p.parent.name}/{p.name}")
def test_wire_file_conforms(path) -> None:  # type: ignore[no-untyped-def]
    jsonschema.validate(json.loads(path.read_text()), SCHEMA, cls=jsonschema.Draft202012Validator)


def test_schema_rejects_unknown_keys() -> None:
    doc = json.loads((FIXTURES / "wire" / "valid" / "invoice.json").read_text())
    doc["unexpected"] = True
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(doc, SCHEMA, cls=jsonschema.Draft202012Validator)
