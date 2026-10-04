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
from .expr import DerivedCall, Expr, call, field_ref, param_ref
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
            if kind not in ("action", "read"):
                raise BehaviorDefinitionError(
                    "Context[...]/Input[...] are for actions and reads", *loc
                )
            role = spec.role
            spec = spec.spec
        else:
            # Entities are bound by identity in actions and reads (`state`), read by derived
            # values, rules and invariants.
            role = "state" if kind in ("action", "read") else "read"
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
        with_role = self.kind in ("action", "read")
        return [(p.name, p.role if with_role else None, p.type.engine()) for p in params]

    def params(self) -> list[ParamInfo]:
        kind = self.kind if self.kind in ("action", "read") else "derived"
        return _resolve_params(self.fn, kind)

    def declared_type(self) -> BType | None:
        """The return annotation of a derived value or rule, if any (checked by the engine)."""
        try:
            hints = inspect.get_annotations(self.fn, eval_str=True)
        except NameError as e:
            raise BehaviorDefinitionError(f"cannot resolve annotation: {e}", *self.loc) from e
        spec = hints.get("return")
        return None if spec is None else to_type(spec)

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
        return DerivedCall(call("derived_ref", self.name, names, loc=loc), loc, self.name, names)


class RuleFn(DerivedFn):
    kind = "rule"


class InvariantFn(BehaviorFn):
    kind = "invariant"


class ConstraintFn(BehaviorFn):
    kind = "constraint"


class ActionFn(BehaviorFn):
    kind = "action"


class ReadFn(BehaviorFn):
    """A read (feature 010): a pure, typed observation of one state. Listed in a module's
    `reads`, it is a declared read (a capability); passed directly to `evaluate_read` or
    `Store.read`, it is an ad-hoc read for trusted host code."""

    kind = "read"

    def __call__(self, *args: Any) -> Any:
        """Reads are entry points, never building blocks: behavior cannot call one."""
        raise BehaviorDefinitionError(
            f"READ_CALL_NOT_ALLOWED: `{self.name}` is a declared read, a capability entry point "
            "that behavior cannot call; move the shared computation into a derived value and use "
            "it from both", *caller_loc()
        )


def derived(fn: Callable[..., Any]) -> DerivedFn:
    return DerivedFn(fn)


def rule(fn: Callable[..., Any]) -> RuleFn:
    return RuleFn(fn)


def invariant(fn: Callable[..., Any]) -> InvariantFn:
    """An invariant over one entity (one entity parameter), or a module invariant (no
    parameters, feature 007): a closed expression over entity sets, checked at genesis and on
    every resulting state it can be affected by."""
    if len(inspect.signature(fn).parameters) > 1:
        raise BehaviorDefinitionError(
            "an invariant takes one entity parameter, or none for a module invariant",
            *InvariantFn(fn).loc,
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


def read(fn: Callable[..., Any]) -> ReadFn:
    """A read: returns a value expression (or a projection) and changes nothing. Entity
    parameters are bound by identity; `Input[...]` and `Context[...]` work as for actions."""
    return ReadFn(fn)


__all__ = ["field", "entity", "derived", "rule", "invariant", "constraint", "action", "read"]
