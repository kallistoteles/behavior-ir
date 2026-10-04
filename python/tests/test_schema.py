"""Every valid and DSL-emitted wire file conforms to the published JSON Schema."""

from __future__ import annotations

import json

import jsonschema
import pytest

from .conftest import CORE_DIR, FIXTURES

SCHEMAS = {
    v: json.loads((CORE_DIR / "schema" / f"wire-ir-{v}.schema.json").read_text())
    for v in ("0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7")
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


def test_documents_use_their_minimal_version() -> None:
    # 0.5 only for documents that use an entity lifecycle form (feature 006).
    for path in FILES:
        version = json.loads(path.read_text())["ir_version"]
        want = {"accounts.json": "0.5", "orders.json": "0.6"}.get(path.name, "0.4")
        assert version == want, path.name


MIGRATION_SCHEMA = json.loads((CORE_DIR / "schema" / "migration-ir-0.1.schema.json").read_text())
MIGRATIONS = sorted(
    p for p in (FIXTURES / "migration").glob("*/*.json")
    if p.parent.name in ("valid", "invalid") and not p.name.endswith(".expected.json")
)


def test_the_migration_schema_is_valid() -> None:
    jsonschema.Draft202012Validator.check_schema(MIGRATION_SCHEMA)


def test_there_are_valid_migration_fixtures() -> None:
    assert MIGRATIONS, "tests/fixtures/migration/valid/ has no migration documents"


@pytest.mark.parametrize("path", MIGRATIONS, ids=lambda p: p.name)
def test_migration_file_conforms(path) -> None:  # type: ignore[no-untyped-def]
    doc = json.loads(path.read_text())
    jsonschema.validate(doc, MIGRATION_SCHEMA, cls=jsonschema.Draft202012Validator)


def test_migration_schema_rejects_module_only_and_unknown_forms() -> None:
    doc = {"migration_ir": "0.1", "name": "m", "source": "sha256:" + "0" * 64,
           "target": "sha256:" + "1" * 64,
           "transforms": [{"entity": "E", "loc": {"file": "m.py", "line": 1},
                           "fields": {"x": {"op": "frobnicate", "args": [],
                                            "loc": {"file": "m.py", "line": 1}}}}]}
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(doc, MIGRATION_SCHEMA, cls=jsonschema.Draft202012Validator)


READ_FILES = sorted((FIXTURES / "reads" / "modules").glob("*.json")) + sorted(
    (FIXTURES / "reads" / "valid").glob("*.json")
)


def test_there_are_read_fixtures() -> None:
    assert any(p.parent.name == "valid" for p in READ_FILES), "no ad-hoc read documents"
    assert any(p.name == "lab.json" for p in READ_FILES), "no module with declared reads"


@pytest.mark.parametrize("path", READ_FILES, ids=lambda p: f"{p.parent.name}/{p.name}")
def test_read_fixture_conforms(path) -> None:  # type: ignore[no-untyped-def]
    # Modules with reads and read documents (feature 010) conform to their version's schema.
    doc = json.loads(path.read_text())
    jsonschema.validate(doc, SCHEMAS[doc["ir_version"]], cls=jsonschema.Draft202012Validator)


def test_reads_need_wire_ir_0_7() -> None:
    doc = json.loads((FIXTURES / "reads" / "modules" / "lab.json").read_text())
    doc["ir_version"] = "0.6"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(doc, SCHEMAS["0.6"], cls=jsonschema.Draft202012Validator)


def test_read_schema_rejects_malformed_bodies() -> None:
    doc = json.loads((FIXTURES / "reads" / "valid" / "customer_count.json").read_text())
    both = json.loads(json.dumps(doc))
    both["read"]["body"]["project"] = {"over": both["read"]["body"]["value"], "param": "x",
                                       "items": []}
    two_keys = json.loads((FIXTURES / "reads" / "valid" / "orders_over.json").read_text())
    two_keys["read"]["body"]["project"]["items"][0]["derived"] = "x"
    bad_role = json.loads((FIXTURES / "reads" / "valid" / "orders_over.json").read_text())
    bad_role["read"]["params"][0]["role"] = "read"
    for bad in (both, two_keys, bad_role):
        with pytest.raises(jsonschema.ValidationError):
            jsonschema.validate(bad, SCHEMAS["0.7"], cls=jsonschema.Draft202012Validator)
