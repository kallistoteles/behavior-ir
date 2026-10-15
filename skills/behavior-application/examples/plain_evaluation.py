"""Evaluation without a store: the host supplies the complete parent snapshot, every declared
entity type. Nothing is inferred from the bound entities. The decision record is the audit
artifact, and it replays on its own."""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, action, count, entity, evaluate, field, nominal, remove, replay, requires,
    select,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    name = field(str)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)


@action
def close_customer(customer: Customer):
    requires(count(select(Order).where(lambda o: o.customer == customer.id)) == 0)
    remove(customer)


model = BehaviorModule(entities=[Customer, Order], actions=[close_customer])
state = {"customer": {"id": "c1", "name": "Ada"}}

# The snapshot must list every declared entity type; the bound customer is not a universe.
partial = {"universe": [{"entity": "Customer", "members": [state["customer"]]}]}
missing = evaluate(model, "close_customer", state=state, data_version="1", facts=partial)
assert missing.result == "INVALID_INPUT"
assert missing.reasons[0]["code"] == "INVALID_STATE_SNAPSHOT"

universe = {"universe": [
    {"entity": "Customer", "members": [state["customer"]]},
    {"entity": "Order", "members": [{"id": "o1", "customer": "c2", "amount": "5.00"}]},
]}
decision = evaluate(model, "close_customer", state=state, data_version="snapshot-17",
                    facts=universe)
assert decision.result == "ALLOW"

# The record keeps exactly what was observed and replays without any store.
assert decision.facts["queries"][0]["members"] == []
assert replay(model, decision.record_json).matches
