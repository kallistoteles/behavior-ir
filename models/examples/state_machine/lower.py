"""A worked example of a Behavior Model (feature 011, US5): a state machine lowered completely to
ordinary Behavior IR through the public `behavior` API.

A state machine names an entity, the enum field that holds its state, its states and its
transitions. Each transition `name: FROM -> TO` lowers to one action over the entity:

    requires(entity.field == FROM)
    set_(entity.field, TO)
    ensures(entity.field == TO)

The core admits the result as any other module; it never learns that a state machine existed.
This is the one authoritative lowering of the model (FR-010): a binding offering state-machine
syntax would call it rather than reimplement it.
"""

from __future__ import annotations

import inspect
from enum import Enum
from typing import Any

from behavior import BehaviorModule, action, ensures, entity, field, requires, set_


def lower(machine: dict[str, Any]) -> BehaviorModule:
    """The Behavior module of a state machine. Deterministic: the same machine always gives the
    same module, byte for byte."""
    name, state_field = machine["entity"], machine["field"]
    states = list(machine["states"])
    for t in machine["transitions"]:
        for end in (t["from"], t["to"]):
            if end not in states:
                raise ValueError(f"transition {t['name']}: unknown state {end}")

    status = Enum(f"{name}{state_field.capitalize()}", {s: s for s in states})  # type: ignore[misc]
    ent = entity(type(name, (), {"__module__": __name__, state_field: field(status)}))
    param = name.lower()

    def transition(t: dict[str, str]) -> Any:
        source, target = status[t["from"]], status[t["to"]]

        def body(*args: Any, **kwargs: Any) -> None:
            current = getattr(kwargs[param] if param in kwargs else args[0], state_field)
            requires(current == source)
            set_(current, target)
            ensures(current == target)

        body.__name__ = body.__qualname__ = t["name"]
        body.__annotations__ = {param: ent}
        body.__signature__ = inspect.Signature(  # type: ignore[attr-defined]
            [inspect.Parameter(param, inspect.Parameter.POSITIONAL_OR_KEYWORD, annotation=ent)])
        return action(body)

    return BehaviorModule(entities=[ent], enums=[status],
                          actions=[transition(t) for t in machine["transitions"]])
