"""Exact arithmetic: fixed-scale money is exact, ratios are exact values, and rounding happens
only in an explicit `rescale` with a named rounding mode."""

from decimal import Decimal

from behavior import (
    BehaviorModule, BehaviorTypeError, Exact, Rounding, action, derived, entity, evaluate, field,
    nominal, requires, rescale, set_,
)

Money = nominal("Money", Decimal, ops={"add", "order", "ratio", "scale"}, scale=2)


@entity
class Share:
    amount = field(Money)
    budget = field(Money)
    part = field(Money)


@derived
def portion(share: Share) -> Exact[Decimal]:
    return share.amount / share.budget  # an exact ratio such as 1/3: no rounding here


@action
def allocate(share: Share):
    requires(portion(share) <= Decimal("0.5"))
    set_(share.part, rescale(portion(share) * share.budget, Money, Rounding.HALF_EVEN))


model = BehaviorModule(entities=[Share], derived=[portion], actions=[allocate])
decision = evaluate(model, "allocate", data_version="1", state={"share": {
    "id": "s1", "amount": Decimal("10.00"), "budget": Decimal("30.00"), "part": Decimal("0")}})
assert decision.result == "ALLOW", decision.reasons
assert decision.changes[0].new == "10.00"


# Storing an exact value into a fixed-scale field without `rescale` is refused where it is
# written: the fix is to say how to round.
@action
def store_ratio(share: Share):
    set_(share.part, share.amount / share.budget)


try:
    BehaviorModule(entities=[Share], actions=[store_ratio])
except BehaviorTypeError as e:
    assert e.code == "LOSSY_CONVERSION", e.code
else:
    raise AssertionError("an implicit rounding must be refused")
