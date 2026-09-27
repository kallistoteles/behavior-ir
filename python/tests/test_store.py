"""The persistence contract from Python (feature 005): evaluate against a store, commit, conflict,
replay."""

from __future__ import annotations

from decimal import Decimal

import pytest

from behavior import (
    CommitRefused, InMemoryBackend, StateConflict, Store, replay_behavior, replay_data,
)
from examples.ledger.behavior import model

T0 = "2026-09-27T12:00:00Z"
SEED = [
    {"entity": "Account", "value": {"id": "a1", "active": True, "balance": Decimal("100.00")}},
    {"entity": "Account", "value": {"id": "a2", "active": True, "balance": Decimal("5.00")}},
]


def new_store(policy: dict | None = None) -> Store:
    return Store.create(InMemoryBackend(), model, Store.genesis_for(model, SEED, policy))


def transfer(store: Store, frm: str, to: str, amount: str):  # type: ignore[no-untyped-def]
    return store.evaluate(model, "transfer", bindings={"from_": frm, "to": to},
                          input={"amount": Decimal(amount)}, commit_time=T0)


def test_commit_conflict_and_retry() -> None:
    store = new_store()
    s0 = store.current()
    assert s0.position == 0
    first, second = transfer(store, "a1", "a2", "10.00"), transfer(store, "a2", "a1", "1.00")
    assert first.decision.result == "ALLOW" and first.bundle is not None
    r = store.commit(model, first.bundle)
    assert r.result_state.position == 1 and not r.already
    assert store.load("Account", "a1")["value"]["balance"] == "90.00"
    assert store.load("Account", "a1", s0)["value"]["balance"] == "100.00"
    with pytest.raises(StateConflict) as e:
        assert second.bundle is not None
        store.commit(model, second.bundle)
    assert e.value.current["position"] == 1
    assert {c["id"] for c in e.value.changed} == {"a1", "a2"}
    retried = transfer(store, "a2", "a1", "1.00")
    assert retried.bundle is not None
    assert store.commit(model, retried.bundle).result_state.position == 2
    # Resubmitting a committed transition is recognized, never applied twice.
    assert store.commit(model, first.bundle).already


def test_denied_decisions_have_no_bundle() -> None:
    store = new_store()
    ev = transfer(store, "a2", "a1", "500.00")
    assert ev.decision.result == "DENY"
    assert ev.bundle is None


def test_replay_and_history() -> None:
    store = new_store()
    for frm, to in [("a1", "a2"), ("a2", "a1"), ("a1", "a2")]:
        ev = transfer(store, frm, to, "1.00")
        assert ev.bundle is not None
        store.commit(model, ev.bundle)
    assert [r["position"] for r in store.history()] == [1, 2, 3]
    assert replay_data(store).ok
    b = replay_behavior(store, [model])
    assert b.ok and b.checked == 3


def test_evidence_policy_requires_authorization() -> None:
    store = new_store({"format": "behavior.evidence_policy.v1", "require": "commit_authorization"})
    ev = transfer(store, "a1", "a2", "1.00")
    assert ev.bundle is not None
    with pytest.raises(CommitRefused) as e:
        store.commit(model, ev.bundle)
    assert e.value.code == "EVIDENCE_REQUIRED"
