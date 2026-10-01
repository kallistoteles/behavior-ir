"""Concurrency: a bundle commits only onto the state it was evaluated against. When the store has
moved on, the host re-evaluates on the current state; it never forces or retries a stale bundle."""

from decimal import Decimal

from behavior import (
    BehaviorModule, InMemoryBackend, Input, StateConflict, Store, action, entity, field, nominal,
    requires, set_,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Account:
    balance = field(Money)


@action
def withdraw(account: Account, *, amount: Input[Money]):
    requires(amount <= account.balance)
    set_(account.balance, account.balance - amount)


model = BehaviorModule(entities=[Account], actions=[withdraw])
seed = [{"entity": "Account", "value": {"id": "a1", "balance": "100.00"}}]
store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))
now = "2026-09-30T12:00:00Z"


def withdraw_(amount: str):  # type: ignore[no-untyped-def]
    return store.evaluate(model, "withdraw", bindings={"account": "a1"},
                          input={"amount": Decimal(amount)}, commit_time=now)


first, second = withdraw_("60.00"), withdraw_("50.00")  # both against the same state
assert first.bundle is not None and second.bundle is not None
store.commit(model, first.bundle)
try:
    store.commit(model, second.bundle)
except StateConflict as conflict:
    assert conflict.current["position"] == 1
else:
    raise AssertionError("a stale bundle must not commit")

# Re-evaluate on the current state: now the rule decides with the real balance.
retried = withdraw_("50.00")
assert retried.decision.result == "DENY"
assert store.load("Account", "a1")["value"]["balance"] == "40.00"

# Committing the same transition twice (a retry after a lost acknowledgement) is recognized and
# never applied twice.
assert store.commit(model, first.bundle).already
assert store.current().position == 1
