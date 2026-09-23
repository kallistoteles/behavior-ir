"""Declarative statements inside behavior bodies: `requires`, `ensures`, `set_`."""

from __future__ import annotations

from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import Any

from .errors import BehaviorDefinitionError, BehaviorTypeError
from .expr import Expr, lift
from .location import caller_loc
from .types import BOOL, UNKNOWN, coerce


@dataclass
class Condition:
    expr: Expr
    loc: tuple[str, int]


@dataclass
class Effect:
    param: str
    field: str
    value: Expr
    loc: tuple[str, int]


@dataclass
class Frame:
    """The body currently being traced."""

    kind: str  # action | derived | rule | invariant
    preconditions: list[Condition] = field(default_factory=list)
    effects: list[Effect] = field(default_factory=list)
    postconditions: list[Condition] = field(default_factory=list)


CURRENT: ContextVar[Frame | None] = ContextVar("behavior_frame", default=None)


def _action_frame(what: str, loc: tuple[str, int]) -> Frame:
    frame = CURRENT.get()
    if frame is None or frame.kind != "action":
        raise BehaviorDefinitionError(f"{what}() is only valid inside an @action body", *loc)
    return frame


def _condition(e: Any, what: str, loc: tuple[str, int]) -> Condition:
    expr = lift(e)
    if expr.type not in (BOOL, UNKNOWN):
        raise BehaviorTypeError(f"{what} must be Bool, found `{expr.type}`", "NOT_BOOLEAN", *loc)
    return Condition(expr, loc)


def requires(e: Any) -> None:
    """A precondition, checked on the current state before the transition."""
    loc = caller_loc()
    frame = _action_frame("requires", loc)
    frame.preconditions.append(_condition(e, "a precondition", loc))


def ensures(e: Any) -> None:
    """A postcondition, checked on the proposed state after the transition."""
    loc = caller_loc()
    frame = _action_frame("ensures", loc)
    frame.postconditions.append(_condition(e, "a postcondition", loc))


def set_(target: Any, value: Any) -> None:
    """An effect: the proposed new value of a field of a state parameter."""
    loc = caller_loc()
    frame = _action_frame("set_", loc)
    if not isinstance(target, Expr) or target.op != "field":
        raise BehaviorDefinitionError("set_() needs a field such as `invoice.status`", *loc)
    param, name = target.data["param"], target.data["field"]
    if target.role != "state":
        raise BehaviorDefinitionError(
            f"`{param}` is read-only ({target.role}); only state parameters can change", *loc
        )
    if name == "id":
        raise BehaviorDefinitionError("an entity's `id` cannot change", *loc)
    v = lift(value, target.type)
    if coerce(target.type, v.type) is None:
        raise BehaviorTypeError(
            f"cannot assign `{v.type}` to `{param}.{name}: {target.type}`", "TYPE_MISMATCH", *loc
        )
    frame.effects.append(Effect(param, name, v, loc))
