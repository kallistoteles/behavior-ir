"""The three verification outcomes on one small model.

- proven: holds for every valid state, input and context;
- counterexample: a failing case that the engine itself reproduced;
- inconclusive: not decided; blocking, and never a success.
"""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, Input, Profile, action, count, create, ensures, entity, evaluate, field,
    nominal, requires, select, set_, sum_, verify,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    credit_limit = field(Money)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)


def orders_of(customer: Customer):  # a Python helper that builds the same query each time
    return select(Order).where(lambda o: o.customer == customer.id)


@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):
    requires(amount >= Money(Decimal("0")))
    requires(sum_(orders_of(customer), lambda o: o.amount) + amount <= customer.credit_limit)
    create(Order, id=order_id, customer=customer.id, amount=amount)
    ensures(sum_(orders_of(customer), lambda o: o.amount) <= customer.credit_limit)


@action
def place_order_unchecked(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):
    create(Order, id=order_id, customer=customer.id, amount=amount)
    ensures(sum_(orders_of(customer), lambda o: o.amount) <= customer.credit_limit)


@action
def raise_limit(customer: Customer, *, limit: Input[Money]):
    over = select(Order).where(lambda o: o.amount > customer.credit_limit)
    requires(limit >= customer.credit_limit)
    requires(count(over) == 0)
    set_(customer.credit_limit, limit)
    ensures(count(over) == 0)


model = BehaviorModule(entities=[Customer, Order],
                       actions=[place_order, place_order_unchecked, raise_limit])
attestation = verify(model, Profile(checks=["postcondition"]))
outcome = {c["action"]["name"]: c["outcome"] for c in attestation.checks}
assert outcome == {"place_order": "proven", "place_order_unchecked": "counterexample",
                   "raise_limit": "inconclusive"}, outcome
assert not attestation.verified  # a counterexample or an inconclusive result blocks

# A counterexample carries the request that fails; plain evaluation reproduces it exactly.
finding = next(f for f in attestation.findings if f["kind"] == "postcondition")
cx = finding["counterexample"]
again = evaluate(model, cx["record"]["action"]["name"], state=cx["state"], input=cx["input"],
                 context=cx["context"], data_version="verification", facts=cx["facts"])
assert again.result == "DENY"
assert again.reasons[0]["code"] == "POSTCONDITION_FAILED"

# An inconclusive check says why, and is a blocking finding.
inconclusive = next(c for c in attestation.checks if c["outcome"] == "inconclusive")
assert inconclusive["reason"] == "counterexample_not_reproduced"
assert any(f["kind"] == "inconclusive" and f["severity"] == "blocking"
           for f in attestation.findings)
