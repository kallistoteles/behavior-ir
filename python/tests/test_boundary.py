"""The PyO3 boundary carries native values, not JSON (research R12)."""

from __future__ import annotations

import json
import pathlib
from decimal import Decimal

import pytest

from behavior import _engine

from .fixtures import invoice_model

PACKAGE = pathlib.Path(__file__).resolve().parents[1] / "behavior"


def test_behavior_package_does_not_use_json() -> None:
    offenders = [
        p.name for p in PACKAGE.glob("*.py")
        if "import json" in p.read_text() or "json." in p.read_text()
    ]
    assert offenders == []


def test_node_types_are_engine_types() -> None:
    b = _engine.Builder()
    b.declare_entity("U", [("tag", _engine.Type.option(_engine.Type.string()), "/m.py", 1)],
                     "/m.py", 1)
    b.push_scope("derived", [("u", None, _engine.Type.entity("U"))], "/m.py", 2)
    node = b.field("u", "tag", "/m.py", 3)
    assert node.type == _engine.Type.option(_engine.Type.string())
    assert node.type.is_option()
    assert node.type.inner() == _engine.Type.string()


def test_engine_errors_carry_code_and_message() -> None:
    b = _engine.Builder()
    b.declare_entity("U", [("n", _engine.Type.int(), "/m.py", 1)], "/m.py", 1)
    b.push_scope("derived", [("u", None, _engine.Type.entity("U"))], "/m.py", 2)
    with pytest.raises(_engine.EngineError) as info:
        b.field("u", "missing", "/m.py", 3)
    code, message = info.value.args
    assert code == "UNKNOWN_FIELD" and "missing" in message


def test_module_evaluates_native_values() -> None:
    engine = invoice_model.model.engine
    record = engine.evaluate(
        "approve_invoice",
        {"invoice": {"id": "1042", "amount": Decimal("4.32E+4"), "approved_by": None,
                     "status": invoice_model.InvoiceStatus.PENDING}},
        {},
        {"actor": {"id": "anna", "role": "manager", "approval_limit": 50000}},
        "18342",
    )
    assert record.result == "ALLOW"
    assert isinstance(record.data, dict)
    assert record.data["state"]["invoice"]["amount"] == "43200.00"
    assert json.loads(record.json) == record.data


def test_floats_are_rejected_at_the_boundary() -> None:
    engine = invoice_model.model.engine
    with pytest.raises(TypeError):
        engine.evaluate("approve_invoice", {"invoice": {"amount": 1.5}}, {}, {}, "1")
