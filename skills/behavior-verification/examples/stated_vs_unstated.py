"""Classifying a failed proof. The same postcondition, with and without the guarantee it relies on:

- the guarantee is not stated: the verifier finds a real, runtime-confirmed counterexample
  (orders with negative amounts can exist), and the fix is to state it in the model;
- the guarantee is stated (an entity constraint): the property holds, but this release cannot use
  constraints on unknown members of a set, so the result is inconclusive. That is precision
  debt: keep the constraint and report the blocking inconclusive; never delete the rule.
"""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, Input, Profile, action, constraint, create, ensures, entity, field,
    nominal, requires, select, sum_, verify,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    credit_limit = field(Money)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)


@constraint
def non_negative_amount(o: Order):
    return o.amount >= Money(Decimal("0"))


@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):
    requires(amount >= Money(Decimal("0")))
    create(Order, id=order_id, customer=customer.id, amount=amount)
    ensures(sum_(select(Order).where(lambda o: o.customer == customer.id),
                 lambda o: o.amount) >= amount)


def outcome(constraints: list) -> str:
    model = BehaviorModule(entities=[Customer, Order], constraints=constraints,
                           actions=[place_order])
    (check,) = verify(model, Profile(checks=["postcondition"])).checks
    return str(check["outcome"])


assert outcome([]) == "counterexample"  # unstated: state the guarantee
assert outcome([non_negative_amount]) == "inconclusive"  # stated: precision debt
