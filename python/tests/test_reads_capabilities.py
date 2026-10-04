"""Declared reads as capabilities from Python (feature 010, US5): reads are entry points, never
building blocks; read intents return only the declared result and the record identity."""

from __future__ import annotations

from typing import Any

import pytest

from behavior import (
    BehaviorDefinitionError, BehaviorModule, InMemoryBackend, IntentRejected, ReadExecution,
    Store, action, derived, read, read_intent, requires, select, set_, count,
)
from examples.lab_reads import model as lab


def _module(**extra: Any) -> BehaviorModule:
    return BehaviorModule(entities=[lab.Culture, lab.Customer, lab.Order],
                          enums=[lab.Stage, lab.OrderStatus],
                          derived=[lab.ph_avg, lab.exposure, lab.standing, *extra.get("derived", [])],
                          actions=extra.get("actions", []), reads=extra.get("reads", []))


@read
def how_many():
    return count(select(lab.Culture))


@pytest.mark.parametrize("site", ["action", "derived", "read"])
def test_a_read_cannot_be_called_from_behavior(site: str) -> None:
    if site == "action":
        @action
        def counted(c: lab.Culture):
            requires(how_many() >= 0)
            set_(c.active, False)

        kwargs: dict[str, Any] = {"actions": [counted], "reads": [how_many]}
    elif site == "derived":
        @derived
        def counted(c: lab.Culture):
            return how_many()

        kwargs = {"derived": [counted], "reads": [how_many]}
    else:
        @read
        def twice():
            return how_many() * 2

        kwargs = {"reads": [how_many, twice]}
    with pytest.raises(BehaviorDefinitionError, match="READ_CALL_NOT_ALLOWED.*derived value"):
        _module(**kwargs)


def _store() -> Store:
    seed = [{"entity": "Customer", "value": {"id": "k1", "name": "Ada", "credit_limit": 100}},
            {"entity": "Order", "value": {"id": "o1", "customer": "k1", "amount": 30,
                                          "status": "OPEN"}}]
    return Store.create(InMemoryBackend(), lab.model, Store.genesis_for(lab.model, seed=seed))


def test_a_read_intent_returns_only_the_declared_result() -> None:
    x = _store().read_intent(lab.model, {"capability": "open_total",
                                         "targets": {"customer": "k1"}})
    assert isinstance(x, ReadExecution)
    assert x.response.result == "VALUE" and x.response.value == 30
    assert x.response.record_id == x.record.record_id
    assert set(x.response.as_dict()) == {"result", "value", "record_id"}
    for evidence in ("record", "facts", "observed", "state"):
        assert not hasattr(x.response, evidence)


def test_a_malformed_read_intent_lists_every_problem() -> None:
    with pytest.raises(IntentRejected) as e:
        _store().read_intent(lab.model, {"capability": "open_total",
                                         "targets": {"customer": "nobody"}, "input": {"x": 1}})
    assert [(p["code"], p["path"]) for p in e.value.errors] == [
        ("EXTRA_ARGUMENT", "input.x"), ("UNKNOWN_TARGET", "targets.customer")]


def test_a_plain_read_intent_takes_the_host_side() -> None:
    host = {"data_version": "h", "state": {}, "context": {},
            "facts": {"universe": [{"entity": "Culture", "members": []}]}}
    x = read_intent(lab.model, {"capability": "active_count"}, host=host)
    assert x.response.value == 0
    with pytest.raises(IntentRejected):
        read_intent(lab.model, {"capability": "measure"}, host=host)
