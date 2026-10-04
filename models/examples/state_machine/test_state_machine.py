"""A Behavior Model lowers completely to ordinary Behavior IR (feature 011, US5, FR-009): the
`submit` transition of a state machine becomes a precondition, an effect and a postcondition,
and the core admits the result without knowing it came from a state machine."""

from __future__ import annotations

import json
from typing import Any

from behavior import admit

from .lower import lower

ORDER_MACHINE = {
    "entity": "Order",
    "field": "status",
    "states": ["DRAFT", "SUBMITTED"],
    "transitions": [{"name": "submit", "from": "DRAFT", "to": "SUBMITTED"}],
}


def _strip(v: Any) -> Any:
    if isinstance(v, dict):
        return {k: _strip(x) for k, x in v.items() if k != "loc"}
    if isinstance(v, list):
        return [_strip(x) for x in v]
    return v


def test_a_transition_lowers_to_requires_set_ensures() -> None:
    wire = _strip(json.loads(lower(ORDER_MACHINE).to_wire_json()))
    [submit] = wire["actions"]
    assert submit["name"] == "submit"
    lit = {"op": "lit", "type": {"name": "OrderStatus", "t": "enum"}}
    status = {"op": "field", "param": "order", "field": "status"}
    assert submit["preconditions"] == [
        {"expr": {"op": "eq", "args": [status, {**lit, "value": "DRAFT"}]}}]
    assert submit["effects"] == [{"target": {"param": "order", "field": "status"},
                                  "value": {**lit, "value": "SUBMITTED"}}]
    assert submit["postconditions"] == [
        {"expr": {"op": "eq", "args": [status, {**lit, "value": "SUBMITTED"}]}}]


def test_the_core_admits_it_without_knowing_the_model() -> None:
    module = lower(ORDER_MACHINE)
    assert admit(module).ok
    text = module.to_wire_json().lower()
    assert "state machine" not in text and "state_machine" not in text and "transition" not in text


def test_lowering_is_deterministic() -> None:
    assert lower(ORDER_MACHINE).to_wire_json() == lower(ORDER_MACHINE).to_wire_json()
