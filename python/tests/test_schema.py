"""Every valid and DSL-emitted wire file conforms to the published JSON Schema."""

from __future__ import annotations

import json

import jsonschema
import pytest

from .conftest import FIXTURES, REPO_ROOT

SCHEMAS = {
    v: json.loads((REPO_ROOT / "schema" / f"wire-ir-{v}.schema.json").read_text())
    for v in ("0.1", "0.2", "0.3", "0.4", "0.5")
}
SCHEMA = SCHEMAS["0.4"]
FILES = sorted((FIXTURES / "wire" / "valid").glob("*.json")) + sorted(
    (FIXTURES / "wire" / "python").glob("*.json")
)


def test_schemas_are_valid() -> None:
    for schema in SCHEMAS.values():
        jsonschema.Draft202012Validator.check_schema(schema)


@pytest.mark.parametrize("path", FILES, ids=lambda p: f"{p.parent.name}/{p.name}")
def test_wire_file_conforms(path) -> None:  # type: ignore[no-untyped-def]
    doc = json.loads(path.read_text())
    jsonschema.validate(doc, SCHEMAS[doc["ir_version"]], cls=jsonschema.Draft202012Validator)


def test_schema_rejects_unknown_keys() -> None:
    doc = json.loads((FIXTURES / "wire" / "valid" / "invoice.json").read_text())
    doc["unexpected"] = True
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(doc, SCHEMA, cls=jsonschema.Draft202012Validator)


def test_only_0_4_and_0_5_documents_remain() -> None:
    # 0.5 only for documents that use an entity lifecycle form (feature 006).
    for path in FILES:
        version = json.loads(path.read_text())["ir_version"]
        assert version == ("0.5" if path.name == "accounts.json" else "0.4"), path.name
