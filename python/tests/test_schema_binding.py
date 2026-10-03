"""Exact store-schema binding from Python (feature 009, FR-001–FR-005): a module's SchemaHash, a
store's schema at every position, and `SCHEMA_MISMATCH` before any decision."""

from __future__ import annotations

from decimal import Decimal

import pytest

from behavior import (
    BehaviorModule, CommitRefused, InMemoryBackend, SchemaRef, Store, action, entity, field,
    set_,
)
from examples.ledger.behavior import Account, available, freeze, model, non_negative_balance, transfer

T0 = "2026-09-27T12:00:00Z"
SEED = [{"entity": "Account", "value": {"id": "a1", "active": True, "balance": Decimal("1.00")}}]


@action
def reactivate(account: Account):
    set_(account.active, True)


@entity
class Note:
    text = field(str)


@action
def touch_note(note: Note):
    set_(note.text, note.text)


def _new_store() -> Store:
    return Store.create(InMemoryBackend(), model, Store.genesis_for(model, SEED))


def test_behavior_only_changes_keep_the_schema_hash() -> None:
    more = BehaviorModule(
        entities=[Account], constraints=[non_negative_balance], derived=[available],
        actions=[transfer, freeze, reactivate],
    )
    assert more.behavior_version != model.behavior_version
    assert more.schema_hash == model.schema_hash
    assert model.schema_hash.startswith("sha256:")


def test_a_new_entity_type_changes_the_schema_hash() -> None:
    wider = BehaviorModule(entities=[Account, Note], actions=[freeze, touch_note])
    assert wider.schema_hash != model.schema_hash


def test_a_store_reports_its_schema_at_every_position() -> None:
    store = _new_store()
    ev = store.evaluate(model, "freeze", bindings={"account": "a1"}, commit_time=T0)
    assert ev.bundle is not None
    store.commit(model, ev.bundle)
    for position in (0, 1):
        s = store.schema_at(store.state_at(position))
        assert isinstance(s, SchemaRef)
        assert s.hash == model.schema_hash
        assert s.since == 0
        assert s.migration_record == store.store_id
        assert set(s.declarations) == {"Account"}
    assert store.schema_history() == [store.schema_at(store.current())]


def test_a_module_of_another_schema_is_refused_before_any_decision() -> None:
    store = _new_store()
    wider = BehaviorModule(entities=[Account, Note], actions=[freeze, touch_note])
    with pytest.raises(CommitRefused) as e:
        store.evaluate(wider, "freeze", bindings={"account": "a1"}, commit_time=T0)
    assert e.value.code == "SCHEMA_MISMATCH"
    assert model.schema_hash in str(e.value) and wider.schema_hash in str(e.value)
    assert "Note" in str(e.value)


def test_a_schema_mismatch_carries_its_details() -> None:
    store = _new_store()
    wider = BehaviorModule(entities=[Account, Note], actions=[freeze, touch_note])
    with pytest.raises(CommitRefused) as e:
        store.evaluate(wider, "freeze", bindings={"account": "a1"}, commit_time=T0)
    d = e.value.details
    assert d is not None
    assert d["store"] == model.schema_hash and d["module"] == wider.schema_hash
    assert d["differing"] == [{"entity": "Note", "store": None,
                               "module": d["differing"][0]["module"]}]
