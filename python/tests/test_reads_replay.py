"""Replaying read records from Python (feature 010, US4): from the record's own facts and
against the store; a tampered record does not replay."""

from __future__ import annotations

import json

from behavior import InMemoryBackend, Store, evaluate_read, replay_read
from examples.lab_reads.model import model

CULTURES = {"universe": [{"entity": "Culture", "members": [
    {"id": "c1", "name": "Basil", "stage": None, "active": True, "measurements": 2,
     "ph_total": 14}]}]}


def test_a_record_replays_from_its_facts() -> None:
    r = evaluate_read(model, "active_count", data_version="t", facts=CULTURES)
    assert replay_read(model, r.record).matches
    assert replay_read(model, r.record.to_json()).matches
    tampered = r.record.as_dict()
    tampered["value"] = 5
    result = replay_read(model, json.dumps(tampered))
    assert not result.matches
    assert result.diff is not None


def test_a_record_replays_against_the_store() -> None:
    seed = [{"entity": "Customer", "value": {"id": "k1", "name": "Ada", "credit_limit": 100}}]
    store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed=seed))
    r = store.read(model, "open_total", bindings={"customer": "k1"})
    assert store.replay_read(model, r.record).matches
    tampered = r.record.as_dict()
    tampered["state"]["customer"]["credit_limit"] = 1
    assert not store.replay_read(model, json.dumps(tampered)).matches
