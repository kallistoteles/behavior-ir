"""A host deciding over sets: python -m examples.orders.run"""

from decimal import Decimal

from behavior import InMemoryBackend, Store, replay_behavior, replay_data, verify
from examples.orders.behavior import model

now = "2026-09-27T12:00:00Z"
seed = [
    {"entity": "Customer", "value": {"id": "c1", "name": "Ada", "credit_limit": "100.00",
                                     "region": "north"}},
    {"entity": "Order", "value": {"id": "o1", "customer": "c1", "amount": "10.00",
                                  "status": "closed", "region": "north"}},
    {"entity": "Employee", "value": {"id": "e1", "personnel_number": "N1"}},
]


def fresh() -> Store:
    return Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))


def run(store: Store, action: str, bindings: dict[str, str], **input: object):  # type: ignore[no-untyped-def]
    ev = store.evaluate(model, action, bindings=bindings, input=input, commit_time=now)
    queries = ev.decision.facts.get("queries", [])
    members = [[m["id"] for m in q["members"]] for q in queries]
    if ev.bundle is None:
        print(f"{action}: {ev.decision.result}", [r["code"] for r in ev.decision.reasons], members)
        return None
    store.commit(model, ev.bundle)
    print(f"{action}: ALLOW, query members {members}")
    return ev


# 1. Only closed orders: c1 can be closed; the open-orders instance has no members.
run(fresh(), "close_customer", {"customer": "c1"})
# 2. After an order is placed, closing c1 is denied; the members are exactly that order.
store = fresh()
run(store, "place_order", {"customer": "c1"}, order_id="o2", amount=Decimal("40.00"))
run(store, "close_customer", {"customer": "c1"})
# 3. Exact sums: a second order over the credit limit is refused.
run(store, "place_order", {"customer": "c1"}, order_id="o3", amount=Decimal("60.01"))
run(store, "check_orders", {"customer": "c1"})
# 4. Personnel numbers stay unique: `hire` checks it, and the module invariant refuses a
#    renumbering onto a taken number on the resulting state.
run(store, "hire", {}, employee_id="e2", number="N1")
run(store, "hire", {}, employee_id="e2", number="N2")
run(store, "renumber", {"employee": "e2"}, number="N1")
print("replay:", replay_data(store).ok, replay_behavior(store, [model]).ok)
attestation = verify(model)
print("verification:", sorted({(c["action"]["name"], c["outcome"]) for c in attestation.checks
                              if c["action"] is not None}))
