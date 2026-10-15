"""Feature 504 (US2): under the command profile, an action that observes a query commits.

Core 0.12.0 refused such candidates with `INVALID_CANDIDATE: invalid observed facts` (feature 503,
research R13); Core 023, released in 0.13.0, accepts them. The module is the one of the Rust
regression `core_assumptions.rs::command_profile_query_*`, as a Wire 0.8 document.
"""

from __future__ import annotations

import copy
import json
from typing import Any

import pytest

import behavior as bh

from .conftest import FIXTURES

T = "2026-10-15T00:00:00Z"
LOC = {"file": "query_command_profile", "line": 1}


def _field(param: str, field: str) -> dict[str, Any]:
    return {"op": "field", "param": param, "field": field, "loc": LOC}


# `mark(item: State Item, mark_id: Input Id<Mark>)`: precondition
# `¬any(select(Mark), m → m.item == item.id)`; creates `Mark { item: item.id }`.
WIRE: dict[str, Any] = {
    "ir_version": "0.8",
    "enums": [],
    "nominals": [],
    "entities": [
        {"name": "Item", "fields": [{"name": "name", "type": {"t": "string"}, "loc": LOC}],
         "loc": LOC},
        {"name": "Mark",
         "fields": [{"name": "item", "type": {"t": "id", "entity": "Item"}, "loc": LOC}],
         "loc": LOC},
    ],
    "derived": [],
    "invariants": [],
    "constraints": [],
    "reads": [],
    "commands": [
        {"name": "Ping", "fields": [{"name": "item", "type": {"t": "id", "entity": "Item"},
                                     "loc": LOC}], "loc": LOC},
    ],
    "actions": [
        {
            "name": "mark",
            "params": [
                {"name": "item", "role": "state", "type": {"t": "entity", "name": "Item"}},
                {"name": "mark_id", "role": "input", "type": {"t": "id", "entity": "Mark"}},
            ],
            "preconditions": [{
                "expr": {"op": "not", "loc": LOC, "args": [{
                    "op": "any", "param": "m", "loc": LOC,
                    "args": [{"op": "select", "entity": "Mark", "loc": LOC}],
                    "body": {"op": "eq", "loc": LOC,
                             "args": [_field("m", "item"), _field("item", "id")]},
                }]},
                "loc": LOC,
            }],
            "effects": [{
                "create": "Mark",
                "id": {"op": "param", "param": "mark_id", "loc": LOC},
                "fields": {"item": _field("item", "id")},
                "loc": LOC,
            }],
            "command_effects": [],
            "postconditions": [],
            "loc": LOC,
        },
    ],
}


def _module() -> Any:
    return bh.BehaviorModule.from_wire_json(json.dumps(WIRE))


def _store(module: Any) -> bh.Store:
    policy = (FIXTURES / "governance-v2/evidence-policy-none.json").read_text()
    seed = [{"entity": "Item", "value": {"id": "i1", "name": "i1"}}]
    genesis = bh.Store.genesis_v2_for(module, seed, policy)
    assert genesis["format"] == "behavior.store_genesis.v2"
    return bh.Store.create(bh.InMemoryBackend(), module, genesis)


def _mark(store: bh.Store, module: Any, mark_id: str) -> Any:
    request = {"format": "behavior.invocation.v1", "capability": "mark",
               "bindings": {"item": {"entity": "Item", "id": "i1"}},
               "input": {"mark_id": mark_id}, "context": {}}
    return store.invoke(module, request, commit_time=T)


def test_query_observing_action_commits() -> None:
    module = _module()
    store = _store(module)
    start = store.current_history()
    result = _mark(store, module, "m1")
    assert result.bundle is not None, "the first mark is allowed"
    store.commit(module, result.bundle)
    assert store.current_history().position == start.position + 1
    assert bh.replay_data(store).ok


def test_query_refusal_commits_nothing() -> None:
    module = _module()
    store = _store(module)
    store.commit(module, _mark(store, module, "m1").bundle)
    head = store.current_history()
    assert _mark(store, module, "m2").bundle is None, "the second mark is refused"
    assert store.current_history() == head


def test_altered_query_evidence_is_refused() -> None:
    module = _module()
    store = _store(module)
    bundle = copy.deepcopy(_mark(store, module, "m1").bundle)
    assert bundle is not None
    query = bundle["read_facts"]["queries"][0]
    digest = query["result_hash"]
    query["result_hash"] = digest[:-1] + ("1" if digest.endswith("0") else "0")
    head = store.current_history()
    with pytest.raises(bh.BehaviorError):
        store.commit(module, bundle)
    assert store.current_history() == head
