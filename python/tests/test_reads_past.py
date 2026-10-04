"""Reading the past from Python (feature 010, US3): `store.read(..., at=)` gives what that state
determines; a module of another schema is refused."""

from __future__ import annotations

import pytest

from behavior import CommitRefused, InMemoryBackend, Store, count, read, select
from examples.lab_reads.model import model
from examples.orders import behavior as orders

T0 = "2026-10-03T12:00:00Z"


def _store() -> Store:
    seed = [{"entity": "Customer", "value": {"id": "k1", "name": "Ada", "credit_limit": 100}}]
    return Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed=seed))


def _place(store: Store, order_id: str, amount: int) -> None:
    e = store.evaluate(model, "place_order", bindings={"customer": "k1"},
                       input={"order_id": order_id, "amount": amount}, commit_time=T0)
    assert e.bundle is not None
    store.commit(model, e.bundle)


def test_a_past_position_reads_as_it_was() -> None:
    store = _store()
    _place(store, "o1", 10)
    _place(store, "o2", 20)
    then = store.read(model, "open_total", bindings={"customer": "k1"})
    position = store.current().position
    _place(store, "o3", 70)
    now = store.read(model, "open_total", bindings={"customer": "k1"})
    assert (then.value, now.value) == (30, 100)
    again = store.read(model, "open_total", bindings={"customer": "k1"},
                       at=store.state_at(position))
    assert again.record.to_json() == then.record.to_json()


def test_a_module_of_another_schema_is_refused() -> None:
    @read
    def customers():
        return count(select(orders.Customer))

    with pytest.raises(CommitRefused) as e:
        _store().read(orders.model, customers)
    assert e.value.code == "SCHEMA_MISMATCH"
