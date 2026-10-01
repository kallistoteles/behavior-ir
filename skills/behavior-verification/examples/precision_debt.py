"""Known precision debt: properties that hold but that this release cannot prove yet. They are
reported as inconclusive (blocking) and must never be "fixed" by weakening the rule."""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, Input, Profile, action, count, entity, ensures, field, min_, nominal,
    remove, requires, select, set_, verify,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    credit_limit = field(Money)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)


# 1. A changed capture: the query after the effect uses the new limit, which is a different set
#    than the one the precondition saw. It holds (raising the limit can only shrink the set), but
#    the verifier treats the new set as unrelated.
@action
def raise_limit(customer: Customer, *, limit: Input[Money]):
    over = select(Order).where(lambda o: o.amount > customer.credit_limit)
    requires(limit >= customer.credit_limit)
    requires(count(over) == 0)
    set_(customer.credit_limit, limit)
    ensures(count(over) == 0)


# 2. A minimum after a removal: the verifier does not know the next-smallest member.
@action
def remove_cheapest(order: Order, customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(order.customer == customer.id)
    requires(min_(orders, lambda o: o.amount).value_or(order.amount) == order.amount)
    remove(order)
    ensures(min_(orders, lambda o: o.amount).value_or(order.amount) >= order.amount)


model = BehaviorModule(entities=[Customer, Order], actions=[raise_limit, remove_cheapest])
attestation = verify(model, Profile(checks=["postcondition"]))
assert {c["action"]["name"]: c["outcome"] for c in attestation.checks} == {
    "raise_limit": "inconclusive", "remove_cheapest": "inconclusive"}
assert not attestation.verified
