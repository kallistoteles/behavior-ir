"""A host with entity lifecycle: python -m examples.accounts.run"""

from decimal import Decimal

from behavior import CommitRefused, InMemoryBackend, Store, replay_behavior, replay_data
from examples.accounts.behavior import model

now = "2026-09-27T12:00:00Z"
seed = [{"entity": "Customer", "value": {"id": "c1", "name": "Ada"}}]
store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))


def run(action: str, bindings: dict[str, str], **input: object):  # type: ignore[no-untyped-def]
    ev = store.evaluate(model, action, bindings=bindings, input=input, commit_time=now)
    if ev.bundle is None:
        print(f"{action}: {ev.decision.result}", [r["code"] for r in ev.decision.reasons])
        return None
    result = store.commit(model, ev.bundle).result_state
    print(f"{action}: committed at position {result.position}")
    return result


# The host supplies identities; the engine never generates them.
run("open_account", {"owner": "c1"}, account_id="a42", initial=Decimal("0.00"))
run("open_account", {"owner": "c1"}, account_id="a42", initial=Decimal("5.00"))  # used identity
run("remove_customer", {"customer": "c1"})  # still referenced by a42
before_close = store.current()
run("close_account", {"account": "a42"})
run("open_account", {"owner": "c1"}, account_id="a42", initial=Decimal("0.00"))  # removed: still used
run("remove_customer", {"customer": "c1"})
try:
    run("open_account", {"owner": "c1"}, account_id="a43", initial=Decimal("0.00"))
except CommitRefused as e:
    print("open_account:", e.code, "(c1 no longer exists)")
print("a42 before closing:", store.load("Account", "a42", at=before_close)["value"])
print("replay:", replay_data(store).ok, replay_behavior(store, [model]).ok)
