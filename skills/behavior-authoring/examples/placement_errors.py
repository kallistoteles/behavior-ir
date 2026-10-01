"""What admission refuses, and the fix for each. The engine reports errors where the author
wrote them; a refused model never runs."""

from decimal import Decimal

from behavior import (
    BehaviorDefinitionError, BehaviorModule, BehaviorTypeError, Id, action, admit, constraint,
    count, entity, exists, field, invariant, nominal, requires, select,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    name = field(str)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)


# QUERY_NOT_ALLOWED: a query inside an entity constraint (or a per-entity invariant). Queries
# describe the whole state, so they belong in module invariants and action conditions.
@constraint
def few_orders(o: Order):
    return count(select(Order)) < 100


refused = BehaviorModule(entities=[Customer, Order], constraints=[few_orders])
assert [e.code for e in admit(refused).errors] == ["QUERY_NOT_ALLOWED"]


@invariant
def at_most_100_orders():
    return count(select(Order)) < 100


fixed = BehaviorModule(entities=[Customer, Order], invariants=[at_most_100_orders])
assert admit(fixed).ok


# NON_LOCAL_PREDICATE: a query filter may read only the candidate, literals and the enclosing
# parameters, never another entity's existence or another query. This one is refused while the
# filter is built, as a BehaviorTypeError with the code.
@invariant
def orders_of_existing_customers():
    return count(select(Order).where(lambda o: exists(o.customer))) >= 0


try:
    BehaviorModule(entities=[Customer, Order], invariants=[orders_of_existing_customers])
except BehaviorTypeError as e:
    assert e.code == "NON_LOCAL_PREDICATE", e.code
else:
    raise AssertionError("a non-local filter must be refused")


# TYPE_MISMATCH: set algebra combines queries over one entity type only.
@action
def mixed(customer: Customer):
    requires(count(select(Order).union(select(Customer))) == 0)


try:
    BehaviorModule(entities=[Customer, Order], actions=[mixed])
except BehaviorTypeError as e:
    assert e.code == "TYPE_MISMATCH", e.code
else:
    raise AssertionError("mixed set algebra must be refused")


# A query is a set described by behavior, not a Python collection: no loops, len() or truth tests.
@action
def iterate(customer: Customer):
    for _ in select(Order):
        pass


try:
    BehaviorModule(entities=[Customer, Order], actions=[iterate])
except BehaviorDefinitionError as e:
    assert "not a Python collection" in str(e)
else:
    raise AssertionError("iterating a query must be refused")
