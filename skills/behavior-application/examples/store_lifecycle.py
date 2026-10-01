"""A host application around a behavior model. The host supplies identities, time and storage;
every business decision is a behavior action; every change goes evaluate -> commit; history
replays."""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, InMemoryBackend, Input, Store, action, constraint, create, entity, field,
    nominal, replay_behavior, replay_data, requires, set_,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Account:
    balance = field(Money)


@constraint
def non_negative_balance(a: Account):
    return a.balance >= Money(Decimal("0"))


@action
def open_account(*, account_id: Input[Id[Account]]):
    create(Account, id=account_id, balance=Money(Decimal("0")))


@action
def deposit(account: Account, *, amount: Input[Money]):
    requires(amount > Money(Decimal("0")))
    set_(account.balance, account.balance + amount)


model = BehaviorModule(entities=[Account], constraints=[non_negative_balance],
                       actions=[open_account, deposit])

# The store starts from a genesis: the model's entity declarations plus a seed state.
store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, []))
start = store.current()
now = "2026-09-30T12:00:00Z"  # time comes from the host, never from the engine

opened = store.evaluate(model, "open_account", bindings={}, input={"account_id": "a1"},
                        commit_time=now)
assert opened.decision.result == "ALLOW" and opened.bundle is not None
store.commit(model, opened.bundle)

paid = store.evaluate(model, "deposit", bindings={"account": "a1"},
                      input={"amount": Decimal("25.00")}, commit_time=now)
assert paid.bundle is not None
store.commit(model, paid.bundle)

# A denied decision has no bundle: there is nothing to commit.
refused = store.evaluate(model, "deposit", bindings={"account": "a1"},
                         input={"amount": Decimal("0.00")}, commit_time=now)
assert refused.decision.result == "DENY" and refused.bundle is None

# Any past state stays readable; history replays without trusting stored results.
assert store.load("Account", "a1")["value"]["balance"] == "25.00"
assert store.current().position == 2
assert len(list(store.history())) == 2
assert replay_data(store).ok
assert replay_behavior(store, [model]).ok
assert start.position == 0
