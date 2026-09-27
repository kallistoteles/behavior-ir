"""A host using the persistence contract: python -m examples.ledger.run"""

from decimal import Decimal

from behavior import InMemoryBackend, StateConflict, Store, replay_behavior, replay_data
from examples.ledger.behavior import model

accounts = [("a1", "100.00"), ("a2", "5.00")]
seed = [{"entity": "Account", "value": {"id": i, "active": True, "balance": Decimal(b)}} for i, b in accounts]
now = "2026-09-27T12:00:00Z"


def transfer(store: Store, frm: str, to: str, amount: str):  # type: ignore[no-untyped-def]
    return store.evaluate(model, "transfer", bindings={"from_": frm, "to": to},
                          input={"amount": Decimal(amount)}, commit_time=now)


# --- the glue a host needs (SC-006) ---------------------------------------------------------
store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))
print("genesis", store.current())
first, second = transfer(store, "a1", "a2", "20.00"), transfer(store, "a2", "a1", "1.00")
print("committed", store.commit(model, first.bundle).result_state)
try:
    store.commit(model, second.bundle)  # evaluated against the old state
except StateConflict as e:
    print("conflict at position", e.current["position"], "- re-evaluating")
    print("committed", store.commit(model, transfer(store, "a2", "a1", "1.00").bundle).result_state)
print("denied:", transfer(store, "a2", "a1", "500.00").decision.result)
print("replay:", replay_data(store).ok, replay_behavior(store, [model]).ok)
# ----------------------------------------------------------------------------------------------
print("a1 =", store.load("Account", "a1")["value"]["balance"])
