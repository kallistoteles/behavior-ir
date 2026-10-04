"""Reading a store: questions are reads, not actions. A read changes nothing, binds only what it
asks about, reads any past position, and its record is evidence that replays. Agents call
declared reads through `read_intent` and see only the declared result."""

from behavior import (
    BehaviorModule, Id, InMemoryBackend, Input, IntentRejected, Store, action, create, derived,
    entity, field, project, read, replay_read, select, sum_,
)


@entity
class Customer:
    name = field(str)
    credit_limit = field(int)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(int)
    open = field(bool)


@derived
def exposure(c: Customer):
    return sum_(select(Order).where(lambda o: (o.customer == c.id) & o.open), lambda o: o.amount)


@derived
def in_good_standing(c: Customer):
    return exposure(c) <= c.credit_limit


@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[int]):
    create(Order, id=order_id, customer=customer.id, amount=amount, open=True)


@read
def open_total(customer: Customer):
    return exposure(customer)


@read
def open_orders():
    return project(select(Order).where(lambda o: o.open), lambda o: [o.customer, o.amount])


@read
def customer_summary(customer: Customer):
    return project(customer, lambda c: [c.name, in_good_standing(c)])


model = BehaviorModule(entities=[Customer, Order], derived=[exposure, in_good_standing],
                       actions=[place_order], reads=[open_total, open_orders, customer_summary])
seed = [{"entity": "Customer", "value": {"id": "k1", "name": "Ada", "credit_limit": 100}}]
store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))
now = "2026-10-03T12:00:00Z"


def place(order_id: str, amount: int) -> None:
    e = store.evaluate(model, "place_order", bindings={"customer": "k1"},
                       input={"order_id": order_id, "amount": amount}, commit_time=now)
    assert e.bundle is not None
    store.commit(model, e.bundle)


place("o1", 30)
before = store.current()
place("o2", 90)

# A question is a read: no action, no commit, no placeholder entity to bind.
total = store.read(model, "open_total", bindings={"customer": "k1"})
assert total.value == 120 and store.current().position == 2
rows = store.read(model, "open_orders")
assert [r["id"] for r in rows.value] == ["o1", "o2"]  # identity order

# The past reads exactly as it was.
assert store.read(model, "open_total", bindings={"customer": "k1"}, at=before).value == 30

# The record is evidence: it replays against the store and on its own.
assert store.replay_read(model, total.record).matches
assert replay_read(model, total.record).matches

# An agent asks through the capability boundary and sees only what the read declares.
x = store.read_intent(model, {"capability": "customer_summary", "targets": {"customer": "k1"}})
assert x.response.value == {"id": "k1", "name": "Ada", "in_good_standing": False}
assert "credit_limit" not in x.response.to_json()  # what the read observed stays in x.record
try:
    store.read_intent(model, {"capability": "place_order", "targets": {"customer": "k1"}})
except IntentRejected as e:
    assert e.errors[0]["code"] == "UNKNOWN_CAPABILITY"  # an action is not a read capability
else:
    raise AssertionError("expected a rejection")
print("reading_state: OK")
