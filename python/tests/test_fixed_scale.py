"""Fixed-scale decimals through the Python DSL (feature 003)."""

from __future__ import annotations

import json
from decimal import Decimal

import pytest

from behavior import (
    BehaviorDefinitionError, BehaviorModule, action, entity, evaluate, field, nominal, set_,
)

from .conftest import FIXTURES

Money = nominal("Money", Decimal, ops={"add", "order", "ratio", "scale"}, scale=2)


@entity
class Invoice:
    amount = field(Money)
    fee = field(Money)


@entity
class Budget:
    limit = field(Money)
    spent = field(Money)


@action
def add_spent(invoice: Invoice, budget: Budget):  # type: ignore[no-untyped-def]
    set_(budget.spent, budget.spent + invoice.amount)


model = BehaviorModule(entities=[Invoice, Budget], actions=[add_spent])


def state(amount: Decimal) -> dict[str, object]:
    return {
        "invoice": {"id": "i1", "amount": amount, "fee": Decimal("0")},
        "budget": {"id": "b1", "limit": Decimal("0"), "spent": Decimal("0.1")},
    }


def test_scale_is_declared_in_the_wire_ir() -> None:
    wire = json.loads(model.to_wire_json())
    assert wire["ir_version"] == "0.4"
    (money,) = wire["nominals"]
    assert money["scale"] == 2
    fixture = json.loads((FIXTURES / "wire" / "valid" / "fixed_scale.json").read_text())
    assert {k: v for k, v in money.items() if k != "loc"} == {
        k: v for k, v in fixture["nominals"][0].items() if k != "loc"
    }


def test_scale_needs_decimal_and_range() -> None:
    with pytest.raises(BehaviorDefinitionError):
        nominal("Count", int, scale=2)
    with pytest.raises(BehaviorDefinitionError):
        nominal("Tiny", Decimal, scale=29)


def test_off_grid_input_is_rejected_and_values_keep_their_scale() -> None:
    bad = evaluate(model, "add_spent", state=state(Decimal("100.505")), data_version="1")
    assert bad.result == "INVALID_INPUT"
    assert bad.reasons[0]["code"] == "OFF_GRID"
    ok = evaluate(model, "add_spent", state=state(Decimal("100.5")), data_version="1")
    assert ok.result == "ALLOW"
    (change,) = ok.changes
    # Decisions carry the record's canonical text: exactly two decimals.
    assert (change.old, change.new) == ("0.10", "100.60")


# --- US2: exact quantities and explicit rescale ------------------------------------------------

from behavior import BehaviorInvalid, BehaviorTypeError, Exact, Rounding, derived, rescale  # noqa: E402


def test_rounding_has_exactly_six_modes() -> None:
    assert [r.value for r in Rounding] == ["half_even", "half_up", "down", "up", "floor", "ceiling"]


def test_rescale_needs_an_explicit_mode() -> None:
    with pytest.raises(TypeError):
        rescale(Decimal("1"), Money)  # type: ignore[call-arg]


def _fee_model(value_of):  # type: ignore[no-untyped-def]
    @action
    def charge(invoice: Invoice):  # type: ignore[no-untyped-def]
        set_(invoice.fee, value_of(invoice))

    return BehaviorModule(entities=[Invoice, Budget], actions=[charge])


def test_rescale_matches_the_fixture_and_is_traced() -> None:
    m = _fee_model(lambda inv: rescale(inv.amount * Decimal("0.25"), Money, Rounding.HALF_EVEN))
    wire = json.loads(m.to_wire_json())
    fixture = json.loads((FIXTURES / "wire" / "valid" / "fixed_scale.json").read_text())
    ours = wire["actions"][0]["effects"][0]["value"]
    theirs = next(a for a in fixture["actions"] if a["name"] == "charge")["effects"][0]["value"]

    def no_loc(v):  # type: ignore[no-untyped-def]
        if isinstance(v, dict):
            return {k: no_loc(x) for k, x in v.items() if k != "loc"}
        if isinstance(v, list):
            return [no_loc(x) for x in v]
        return v

    assert no_loc(ours) == no_loc(theirs)
    d = evaluate(m, "charge", state={"invoice": {"id": "i1", "amount": Decimal("10.01"), "fee": Decimal("0")}},
                 data_version="1")
    assert d.result == "ALLOW"
    assert d.changes[0].new == "2.50"
    effect = next(s for s in json.loads(d.record_json)["trace"] if s["phase"] == "effect")
    assert effect["rescales"][0]["exact"] == "2.5025"


def test_storing_an_exact_quantity_needs_rescale() -> None:
    # Feature 004: an exact store is typed, then admission proves (or refutes) that it stays on
    # the grid: 0.25 may add two decimal places, so it needs an explicit rescale.
    with pytest.raises(BehaviorInvalid, match="LOSSY_CONVERSION.*rescale"):
        _fee_model(lambda inv: inv.amount * Decimal("0.25")).engine
    # Doubling stays on the grid: stored without rescale.
    assert _fee_model(lambda inv: inv.amount * Decimal("2.0")).engine is not None


def test_declared_derived_types() -> None:
    @derived
    def theoretical_fee(invoice: Invoice) -> Exact[Money]:
        return invoice.amount * Decimal("0.25")

    @derived
    def wrong_fee(invoice: Invoice) -> Money:
        return invoice.amount * Decimal("0.25")

    @action
    def use(invoice: Invoice):  # type: ignore[no-untyped-def]
        set_(invoice.fee, rescale(theoretical_fee(invoice), Money, Rounding.FLOOR))

    ok = BehaviorModule(entities=[Invoice, Budget], derived=[theoretical_fee], actions=[use])
    assert json.loads(ok.to_wire_json())["derived"][0]["type"] == {"t": "exact", "name": "Money"}

    @action
    def use_wrong(invoice: Invoice):  # type: ignore[no-untyped-def]
        set_(invoice.fee, rescale(wrong_fee(invoice), Money, Rounding.FLOOR))

    with pytest.raises(BehaviorTypeError, match="rescale"):
        BehaviorModule(entities=[Invoice, Budget], derived=[wrong_fee], actions=[use_wrong]).engine
