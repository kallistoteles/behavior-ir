"""Evaluation through the Python API (quickstart.md §2)."""

from __future__ import annotations

import json
from decimal import Decimal

import pytest

from behavior import BehaviorInvalid, BehaviorModule, evaluate, replay

from .fixtures import cycles, invoice_model

M = invoice_model.model


def invoice(**over: object) -> dict[str, object]:
    base: dict[str, object] = {"id": "1042", "amount": Decimal("43200"), "status": "pending",
                               "approved_by": None}
    base.update(over)
    return base


def actor(**over: object) -> dict[str, object]:
    base: dict[str, object] = {"id": "anna", "role": "manager", "approval_limit": 50000}
    base.update(over)
    return base


def approve(inv: dict[str, object], act: dict[str, object]):  # type: ignore[no-untyped-def]
    return evaluate(M, "approve_invoice", state={"invoice": inv}, context={"actor": act},
                    input={}, data_version="18342")


def test_allowed() -> None:
    d = approve(invoice(), actor())
    assert d.result == "ALLOW"
    assert [(c.field, c.old, c.new) for c in d.changes] == [
        ("status", "pending", "approved"), ("approved_by", None, "anna")]


def test_above_limit() -> None:
    d = approve(invoice(amount=Decimal("60000")), actor())
    assert d.result == "DENY"
    assert d.changes == []
    step = d.trace[3]
    assert step.outcome is False
    assert step.reads == {"invoice.amount": "60000.00", "actor.approval_limit": "50000.00"}


def test_wrong_role_skips_later_preconditions() -> None:
    d = approve(invoice(), actor(role="clerk"))
    assert d.result == "DENY"
    assert [s.outcome for s in d.trace[1:4]] == [True, False, "skipped"]


def test_exact_boundary() -> None:
    assert approve(invoice(amount=Decimal("50000.00")), actor()).result == "ALLOW"


def test_invalid_starting_state() -> None:
    d = approve(invoice(amount=Decimal("-1")), actor())
    assert d.result == "INVALID_STATE"
    assert [s.phase for s in d.trace] == ["invariant_pre"]


def test_enum_members_are_accepted_as_values() -> None:
    d = approve(invoice(status=invoice_model.InvoiceStatus.PENDING), actor())
    assert d.result == "ALLOW"


def test_float_is_rejected_before_the_engine() -> None:
    with pytest.raises(TypeError):
        approve(invoice(amount=43200.0), actor())


def test_records_are_deterministic_and_replay() -> None:
    a = approve(invoice(), actor())
    b = approve(invoice(), actor())
    assert a.record_json == b.record_json
    assert replay(M, a.record_json).matches
    tampered = json.loads(a.record_json)
    tampered["trace"][3]["reads"]["invoice.amount"] = "1"
    r = replay(M, json.dumps(tampered))
    assert not r.matches and r.diff is not None


def test_unadmitted_module_raises() -> None:
    with pytest.raises(BehaviorInvalid):
        evaluate(cycles.model, "anything", state={}, context={}, input={}, data_version="1")


def test_module_type_hint() -> None:
    assert isinstance(M, BehaviorModule)
