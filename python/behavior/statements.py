"""Declarative statements inside behavior bodies: `requires`, `ensures`, `set_`, and the
lifecycle effects `create` and `remove` (feature 006).

The Python layer only records statements; the engine checks them (Bool conditions, effects on
state fields of the right type) and raises at the author's line.
"""

from __future__ import annotations

from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import Any

from . import _engine
from .errors import BehaviorDefinitionError
from .expr import Expr, builder, engine_error, lift
from .location import caller_loc


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
class Lifecycle:
    """A creation (`kind == "create"`, `name` the entity type) or removal (`name` the state
    parameter)."""

    kind: str
    name: str
    id: Expr | None
    fields: list[tuple[str, Expr]]
    loc: tuple[str, int]


@dataclass
class Frame:
    """The body currently being traced."""

    kind: str  # action | derived | rule | invariant
    preconditions: list[Condition] = field(default_factory=list)
    effects: list[Effect] = field(default_factory=list)
    lifecycle: list[Lifecycle] = field(default_factory=list)
    postconditions: list[Condition] = field(default_factory=list)


CURRENT: ContextVar[Frame | None] = ContextVar("behavior_frame", default=None)


def _action_frame(what: str, loc: tuple[str, int]) -> Frame:
    frame = CURRENT.get()
    if frame is None or frame.kind != "action":
        raise BehaviorDefinitionError(f"{what}() is only valid inside an @action body", *loc)
    return frame


def check_condition(expr: Expr, what: str, loc: tuple[str, int]) -> None:
    try:
        builder().builder.check_condition(expr.node, what)
    except _engine.EngineError as e:
        raise engine_error(e, loc) from None


def requires(e: Any) -> None:
    """A precondition, checked on the current state before the transition."""
    loc = caller_loc()
    frame = _action_frame("requires", loc)
    expr = lift(e)
    check_condition(expr, "a precondition", loc)
    frame.preconditions.append(Condition(expr, loc))


def ensures(e: Any) -> None:
    """A postcondition, checked on the proposed state after the transition."""
    loc = caller_loc()
    frame = _action_frame("ensures", loc)
    expr = lift(e)
    check_condition(expr, "a postcondition", loc)
    frame.postconditions.append(Condition(expr, loc))


def set_(target: Any, value: Any) -> None:
    """An effect: the proposed new value of a field of a state parameter."""
    loc = caller_loc()
    frame = _action_frame("set_", loc)
    if not isinstance(target, Expr) or target.target is None:
        raise BehaviorDefinitionError("set_() needs a field such as `invoice.status`", *loc)
    param, name = target.target
    v = lift(value, target.type)
    try:
        builder().builder.check_effect(param, name, v.node)
    except _engine.EngineError as e:
        raise engine_error(e, loc) from None
    frame.effects.append(Effect(param, name, v, loc))


def create(entity_cls: Any, *, id: Any, **fields: Any) -> None:  # noqa: A002
    """A creation effect: a new `entity_cls` with identity `id` (an `Id[entity_cls]`, usually an
    `Input`) and a complete initial value, one keyword per field. The engine checks that the
    value is complete and well typed, and at evaluation that the identity was never used."""
    from .decl import entity_decl
    from .types import IdT, RefT

    loc = caller_loc()
    frame = _action_frame("create", loc)
    decl = entity_decl(entity_cls)
    if decl is None:
        raise BehaviorDefinitionError("create() needs an @entity class", *loc)
    types = {name: t for name, t, _ in decl.fields}
    values: list[tuple[str, Expr]] = []
    for name, value in fields.items():
        t = types.get(name)
        if isinstance(t, RefT):
            t = IdT(t.entity)
        values.append((name, lift(value, t.engine() if t is not None else None)))
    frame.lifecycle.append(Lifecycle("create", decl.name, lift(id), values, loc))


def remove(entity: Any) -> None:
    """A removal effect: the entity bound to state parameter `entity` is absent from the
    resulting state. Its history is kept and its identity is never used again."""
    from .decl import EntityVar

    loc = caller_loc()
    frame = _action_frame("remove", loc)
    if not isinstance(entity, EntityVar):
        raise BehaviorDefinitionError("remove() needs a state entity parameter", *loc)
    frame.lifecycle.append(Lifecycle("remove", entity._name, None, [], loc))
