"""Declarations: entities, fields, derived values, rules, invariants, and actions.

Decorators register functions without running them. Bodies are traced when a
`BehaviorModule` is compiled, which allows forward references and lets cycles between
derived values reach the engine (research R11).
"""

from __future__ import annotations

import inspect
import os
from dataclasses import dataclass
from typing import Any, Callable

from .errors import BehaviorDefinitionError, BehaviorTypeError
from .expr import Expr, call, field_ref, param_ref
from .location import caller_loc
from .types import BType, EntityT, RoleSpec, to_type


@dataclass
class FieldSpec:
    type: BType
    loc: tuple[str, int]


def field(spec: Any) -> FieldSpec:
    """Declares an entity field of type `spec` (bool, int, Decimal, str, Enum, nominal, Id, Option)."""
    loc = caller_loc()
    t = to_type(spec)
    if isinstance(t, EntityT):
        raise BehaviorDefinitionError("fields cannot hold entities; use Id[...]", *loc)
    return FieldSpec(t, loc)


@dataclass
class EntityDecl:
    name: str
    fields: list[tuple[str, BType, tuple[str, int]]]
    loc: tuple[str, int]


def entity(cls: type) -> type:
    """Marks a class whose `field(...)` attributes describe an entity."""
    # The caller's frame works for classes defined anywhere (modules, REPL, exec, notebooks).
    file, line = caller_loc()
    loc = (file, getattr(cls, "__firstlineno__", line))
    fields: list[tuple[str, BType, tuple[str, int]]] = []
    for name, value in vars(cls).items():
        if not isinstance(value, FieldSpec):
            continue
        if name == "id":
            raise BehaviorDefinitionError("`id` is reserved for the entity's identity", *value.loc)
        fields.append((name, value.type, value.loc))
    cls.__behavior_entity__ = EntityDecl(cls.__name__, fields, loc)  # type: ignore[attr-defined]
    return cls


def entity_decl(spec: Any) -> EntityDecl | None:
    return getattr(spec, "__behavior_entity__", None)


class EntityVar:
    """A symbolic entity parameter; attribute access builds field references."""

    def __init__(self, name: str, decl: EntityDecl, role: str) -> None:
        self._name = name
        self._decl = decl
        self._role = role

    def __getattr__(self, item: str) -> Expr:
        if item.startswith("_"):
            raise AttributeError(item)
        return field_ref(self._name, item)

    def __bool__(self) -> bool:
        raise BehaviorDefinitionError("entities cannot drive Python control flow", *caller_loc())


@dataclass
class ParamInfo:
    name: str
    role: str  # read | state | input | context
    type: BType
    decl: EntityDecl | None
    keyword: bool


def _fn_loc(fn: Callable[..., Any]) -> tuple[str, int]:
    return os.path.abspath(fn.__code__.co_filename), fn.__code__.co_firstlineno


def _resolve_params(fn: Callable[..., Any], kind: str) -> list[ParamInfo]:
    loc = _fn_loc(fn)
    try:
        hints = inspect.get_annotations(fn, eval_str=True)
    except NameError as e:
        raise BehaviorDefinitionError(f"cannot resolve annotation: {e}", *loc) from e
    out = []
    for p in inspect.signature(fn).parameters.values():
        if p.name not in hints:
            raise BehaviorDefinitionError(f"parameter `{p.name}` needs a type annotation", *loc)
        spec = hints[p.name]
        keyword = p.kind == inspect.Parameter.KEYWORD_ONLY
        if isinstance(spec, RoleSpec):
            if kind != "action":
                raise BehaviorDefinitionError("Context[...]/Input[...] are for actions", *loc)
            role = spec.role
            spec = spec.spec
        else:
            role = "state" if kind == "action" else "read"
        decl = entity_decl(spec)
        t = to_type(spec)
        if role in ("state", "read") and decl is None:
            raise BehaviorDefinitionError(f"parameter `{p.name}` must be an @entity class", *loc)
        out.append(ParamInfo(p.name, role, t, decl, keyword))
    return out


def _symbol(p: ParamInfo) -> Any:
    if p.decl is not None:
        return EntityVar(p.name, p.decl, p.role)
    return param_ref(p.name)


class BehaviorFn:
    """A registered behavior function (derived, rule, invariant, or action)."""

    kind = "?"

    def __init__(self, fn: Callable[..., Any]) -> None:
        self.fn = fn
        self.name = fn.__name__
        self.loc = _fn_loc(fn)
        self.__doc__ = fn.__doc__

    def engine_params(self, params: list[ParamInfo]) -> list[tuple[str, str | None, Any]]:
        """Parameters as `(name, role or None, Type)` tuples for the engine builder."""
        action = self.kind == "action"
        return [(p.name, p.role if action else None, p.type.engine()) for p in params]

    def params(self) -> list[ParamInfo]:
        return _resolve_params(self.fn, "action" if self.kind == "action" else "derived")

    def call_symbolic(self, params: list[ParamInfo]) -> Any:
        args = [_symbol(p) for p in params if not p.keyword]
        kwargs = {p.name: _symbol(p) for p in params if p.keyword}
        return self.fn(*args, **kwargs)


class DerivedFn(BehaviorFn):
    kind = "derived"

    def __call__(self, *args: Any) -> Expr:
        """Inside a traced body: a reference to this derived value (never inlined)."""
        from .module import current_session

        loc = caller_loc()
        session = current_session()
        if session is None:
            raise BehaviorDefinitionError(
                f"`{self.name}` can only be used inside a behavior body", *loc
            )
        names = []
        for arg in args:
            if not isinstance(arg, EntityVar):
                raise BehaviorTypeError(
                    f"`{self.name}` takes entity parameters, got {arg!r}", "TYPE_MISMATCH", *loc
                )
            names.append(arg._name)
        # Trace the referenced body first (unless it is being traced: a cycle); the engine then
        # checks arity and parameter types against it.
        session.trace_derived(self)
        return Expr(call("derived_ref", self.name, names, loc=loc), loc, "derived")


class RuleFn(DerivedFn):
    kind = "rule"


class InvariantFn(BehaviorFn):
    kind = "invariant"


class ConstraintFn(BehaviorFn):
    kind = "constraint"


class ActionFn(BehaviorFn):
    kind = "action"


def derived(fn: Callable[..., Any]) -> DerivedFn:
    return DerivedFn(fn)


def rule(fn: Callable[..., Any]) -> RuleFn:
    return RuleFn(fn)


def invariant(fn: Callable[..., Any]) -> InvariantFn:
    if len(inspect.signature(fn).parameters) != 1:
        raise BehaviorDefinitionError(
            "an invariant takes exactly one entity parameter", *InvariantFn(fn).loc
        )
    return InvariantFn(fn)


def constraint(fn: Callable[..., Any]) -> ConstraintFn:
    """An entity constraint: what a valid instance of the parameter's entity type is. It is
    checked on every incoming entity of that type, whatever its role, and on new state."""
    if len(inspect.signature(fn).parameters) != 1:
        raise BehaviorDefinitionError(
            "an entity constraint takes exactly one entity parameter", *ConstraintFn(fn).loc
        )
    return ConstraintFn(fn)


def action(fn: Callable[..., Any]) -> ActionFn:
    return ActionFn(fn)


__all__ = ["field", "entity", "derived", "rule", "invariant", "constraint", "action"]
