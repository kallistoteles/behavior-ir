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


def universe(*invoices: dict[str, object]) -> dict[str, object]:
    """The complete parent snapshot (Core 017): every declared entity type, empty ones too."""
    return {"universe": [{"entity": "Invoice", "members": list(invoices)},
                         {"entity": "User", "members": []}]}


def approve(inv: dict[str, object], act: dict[str, object]):  # type: ignore[no-untyped-def]
    return evaluate(M, "approve_invoice", state={"invoice": inv}, context={"actor": act},
                    input={}, data_version="18342", facts=universe(inv))


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
    # Core 017 (verifier 0.9.0): the complete parent snapshot is validated before evaluation, so
    # an invalid parent is an invalid input naming the violated invariant (was INVALID_STATE).
    d = approve(invoice(amount=Decimal("-1")), actor())
    assert d.result == "INVALID_INPUT"
    assert d.trace == []
    assert d.diagnostics["reasons"] == [{
        "code": "INVALID_STATE_SNAPSHOT",
        "message": "facts: Invoice#1042: invariant `non_negative_amount` is violated"}]


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
        evaluate(cycles.model, "anything", state={}, context={}, input={}, data_version="1",
                 facts={"universe": []})


def test_module_type_hint() -> None:
    assert isinstance(M, BehaviorModule)


def test_the_complete_parent_snapshot_is_required() -> None:
    """Core 017 (feature 504, FR-010): direct evaluation takes the complete parent snapshot from
    the host; the binding never completes it."""
    with pytest.raises(TypeError):
        evaluate(M, "approve_invoice", state={"invoice": invoice()},  # type: ignore[call-arg]
                 context={"actor": actor()}, input={}, data_version="18342")
    partial = {"universe": [{"entity": "Invoice", "members": [invoice()]}]}
    d = evaluate(M, "approve_invoice", state={"invoice": invoice()}, context={"actor": actor()},
                 input={}, data_version="18342", facts=partial)
    assert d.result == "INVALID_INPUT"
    assert [r["code"] for r in d.reasons] == ["INVALID_STATE_SNAPSHOT"]


def test_bound_state_must_agree_with_the_snapshot() -> None:
    d = evaluate(M, "approve_invoice", state={"invoice": invoice()}, context={"actor": actor()},
                 input={}, data_version="18342", facts=universe(invoice(status="approved")))
    assert d.result == "INVALID_INPUT"
