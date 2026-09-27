"""Exact arithmetic closure in the DSL (feature 004): exact ratios, `Exact[Decimal]`, and stores
that need a proof of representability or an explicit rescale."""

from __future__ import annotations

import json
from decimal import Decimal

import pytest

from behavior import (
    BehaviorInvalid, BehaviorModule, BehaviorTypeError, Exact, Rounding, action, derived, entity,
    evaluate, field, nominal, requires, rescale, set_,
)

Money = nominal("Money", Decimal, ops={"add", "order", "ratio", "scale"}, scale=2)
Price = nominal("Price", Decimal, ops={"add", "order", "ratio", "scale"})


@entity
class Share:
    amount = field(Money)
    budget = field(Money)
    part = field(Money)


@entity
class Order:
    gross = field(Price)
    discount = field(Price)
    net = field(Price)


@derived
def portion(share: Share) -> Exact[Decimal]:
    return share.amount / share.budget


@action
def scaled(share: Share):  # type: ignore[no-untyped-def]
    requires(portion(share) <= Decimal("0.5"))
    set_(share.part, rescale(portion(share) * share.budget, Money, Rounding.HALF_EVEN))


def test_ratio_is_an_exact_dimensionless_value() -> None:
    m = BehaviorModule(entities=[Share], derived=[portion], actions=[scaled])
    wire = json.loads(m.to_wire_json())
    assert wire["derived"][0]["type"] == {"t": "exact"}
    d = evaluate(m, "scaled", state={"share": {"id": "s1", "amount": Decimal("10.00"),
                                               "budget": Decimal("30.00"),
                                               "part": Decimal("0")}}, data_version="1")
    assert d.result == "ALLOW"
    assert d.changes[0].new == "10.00"
    record = json.loads(d.record_json)
    assert record["record_version"] == "0.4"
    assert record["derived"][0]["value"] == "1/3"


def test_exact_of_an_unscaled_nominal() -> None:
    @derived
    def total(order: Order) -> Exact[Price]:
        return order.gross - order.discount

    m = BehaviorModule(entities=[Order], derived=[total])
    assert json.loads(m.to_wire_json())["derived"][0]["type"] == {"t": "exact", "name": "Price"}


def test_exact_needs_a_decimal_type() -> None:
    with pytest.raises(Exception, match="Decimal or a decimal nominal"):
        Exact[int]


def test_storing_a_ratio_needs_rescale() -> None:
    @action
    def store_ratio(share: Share):  # type: ignore[no-untyped-def]
        set_(share.part, share.amount / share.budget)

    with pytest.raises(BehaviorTypeError, match="rescale"):
        BehaviorModule(entities=[Share], actions=[store_ratio]).engine


def test_storing_an_unscaled_decimal_sum_needs_a_scale_or_rescale() -> None:
    # Typed as an exact store, then refuted by admission: two 28-digit prices may need 29
    # significant digits (admission reads types and literals, not constraints).
    @action
    def apply(order: Order):  # type: ignore[no-untyped-def]
        set_(order.net, order.gross - order.discount)

    with pytest.raises(BehaviorInvalid, match="LOSSY_CONVERSION.*rescale"):
        BehaviorModule(entities=[Order], actions=[apply]).engine
