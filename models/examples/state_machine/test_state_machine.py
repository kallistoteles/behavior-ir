"""A Behavior Model lowers completely to ordinary Behavior IR (features 011 US5 and 501): the
`submit` transition of a state machine becomes one ordinary action, and Core admits the result
without knowing it came from a state machine. The lowering itself lives in the Rust model
authority (`crates/behavior-models`); nothing here lowers."""

from __future__ import annotations

import json
from typing import Any

from behavior import BehaviorModule, action, admit, ensures, requires, set_

from .machine import Order, OrderStatus, module


@action
def order__submit(order: Order):  # noqa: N802 - the generated action's name
    requires(order.status.in_([OrderStatus.DRAFT]))
    set_(order.status, OrderStatus.SUBMITTED)
    ensures(order.status == OrderStatus.SUBMITTED)


def _strip(v: Any) -> Any:
    if isinstance(v, dict):
        return {k: _strip(x) for k, x in v.items() if k != "loc"}
    if isinstance(v, list):
        return [_strip(x) for x in v]
    return v


def test_a_transition_lowers_to_requires_set_ensures() -> None:
    """The lowered module is the hand-written one: same Behavior IR, same BehaviorHash."""
    by_hand = BehaviorModule(entities=[Order], enums=[OrderStatus], actions=[order__submit])
    lowered = module()
    assert lowered.behavior_version == by_hand.behavior_version
    assert _strip(json.loads(lowered.to_wire_json())) == _strip(json.loads(by_hand.to_wire_json()))


def test_the_core_admits_it_without_knowing_the_model() -> None:
    lowered = module()
    assert admit(lowered).ok
    wire = json.loads(lowered.to_wire_json())
    assert [a["name"] for a in wire["actions"]] == ["order__submit"]


def test_lowering_is_deterministic() -> None:
    assert module().to_wire_json() == module().to_wire_json()
    assert module().model_receipts == module().model_receipts
