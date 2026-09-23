"""BehaviorModule: traces all bodies and emits canonical wire IR (contracts/ir-encoding.md)."""

from __future__ import annotations

import json
import os
from contextvars import ContextVar
from dataclasses import dataclass
from typing import Any, Iterable, Sequence

from .decl import (
    ActionFn, BehaviorFn, DerivedFn, EntityDecl, InvariantFn, ParamInfo, RuleFn, entity_decl,
)
from .errors import BehaviorDefinitionError, BehaviorTypeError
from .expr import Expr, lift
from .statements import CURRENT, Condition, Effect, Frame
from .types import BOOL, UNKNOWN, BType, EnumT, NominalT, OptionT


@dataclass
class TracedDerived:
    fn: DerivedFn
    params: list[ParamInfo]
    body: Expr | None  # None while tracing (cycle placeholder)


class CompileSession:
    """Memoizes traced derived values so each body is traced once per module."""

    def __init__(self) -> None:
        self.derived: dict[int, TracedDerived] = {}

    def result_type(self, fn: DerivedFn) -> BType:
        traced = self.derived.get(id(fn))
        if traced is None:
            traced = self.trace_derived(fn)
        return UNKNOWN if traced.body is None else traced.body.type

    def trace_derived(self, fn: DerivedFn) -> TracedDerived:
        existing = self.derived.get(id(fn))
        if existing is not None:
            return existing
        params = fn.params()
        traced = TracedDerived(fn, params, None)
        self.derived[id(fn)] = traced
        token = CURRENT.set(Frame(fn.kind))
        try:
            body = lift(fn.call_symbolic(params))
        finally:
            CURRENT.reset(token)
        if isinstance(fn, RuleFn) and body.type not in (BOOL, UNKNOWN):
            raise BehaviorTypeError(
                f"rule `{fn.name}` must return Bool, found `{body.type}`", "NOT_BOOLEAN", *body.loc
            )
        traced.body = body
        return traced


_SESSION: ContextVar[CompileSession | None] = ContextVar("behavior_session", default=None)


def current_session() -> CompileSession | None:
    return _SESSION.get()


def _loc(loc: tuple[str, int], root: str) -> dict[str, Any]:
    rel = os.path.relpath(loc[0], root).replace(os.sep, "/")
    return {"file": rel, "line": loc[1]}


def _relocate(node: Any, root: str) -> Any:
    """Rewrites absolute `loc` files in a wire tree to paths relative to `root`."""
    if isinstance(node, dict):
        out = {}
        for k, v in node.items():
            if k == "loc" and isinstance(v, dict):
                out[k] = _loc((v["file"], v["line"]), root)
            else:
                out[k] = _relocate(v, root)
        return out
    if isinstance(node, list):
        return [_relocate(v, root) for v in node]
    return node


def _types_in(t: BType, out: list[BType]) -> None:
    out.append(t)
    if isinstance(t, OptionT):
        _types_in(t.of, out)


def _expr_types(e: Expr, out: list[BType]) -> None:
    _types_in(e.type, out)
    for a in e.args:
        _expr_types(a, out)


class BehaviorModule:
    """A compiled behavior module. Construction traces every body and fails fast on errors."""

    def __init__(
        self,
        entities: Sequence[type],
        derived: Sequence[DerivedFn] = (),
        invariants: Sequence[InvariantFn] = (),
        actions: Sequence[ActionFn] = (),
        enums: Iterable[Any] = (),
        nominals: Iterable[NominalT] = (),
        root: str | None = None,
    ) -> None:
        decls: list[EntityDecl] = []
        for cls in entities:
            d = entity_decl(cls)
            if d is None:
                raise BehaviorDefinitionError(f"{cls!r} is not an @entity class")
            decls.append(d)
        self.entities = decls
        self.derived_fns = list(derived)
        self.invariant_fns = list(invariants)
        self.action_fns = list(actions)

        session = CompileSession()
        token = _SESSION.set(session)
        try:
            traced = [session.trace_derived(fn) for fn in self.derived_fns]
            invs = [self._trace_invariant(fn) for fn in self.invariant_fns]
            acts = [self._trace_action(fn) for fn in self.action_fns]
        finally:
            _SESSION.reset(token)

        seen: list[BType] = []
        for d in decls:
            for _, t, _ in d.fields:
                _types_in(t, seen)
        for td in traced:
            for p in td.params:
                _types_in(p.type, seen)
            if td.body is not None:
                _expr_types(td.body, seen)
        for _, _, body in invs:
            _expr_types(body, seen)
        for params, frame in acts:
            for p in params:
                _types_in(p.type, seen)
            for c in frame.preconditions + frame.postconditions:
                _expr_types(c.expr, seen)
            for e in frame.effects:
                _expr_types(e.value, seen)
        from .types import enum_type, to_type

        explicit_enums = [enum_type(e) if not isinstance(e, EnumT) else e for e in enums]
        enum_types = {t.name: t for t in explicit_enums}
        enum_types.update({t.name: t for t in seen if isinstance(t, EnumT)})
        nominal_types = {t.name: t for t in nominals}
        nominal_types.update({t.name: t for t in seen if isinstance(t, NominalT)})
        _ = to_type

        files = [d.loc[0] for d in decls]
        files += [fn.loc[0] for fn in [*self.derived_fns, *self.invariant_fns, *self.action_fns]]
        named: list[EnumT | NominalT] = [*enum_types.values(), *nominal_types.values()]
        files += [t.loc[0] for t in named if t.loc]
        self.root = os.path.abspath(root) if root else os.path.commonpath(
            [os.path.dirname(f) for f in files]
        ) if files else os.getcwd()

        wire: dict[str, Any] = {
            "ir_version": "0.1",
            "enums": [
                {"name": t.name, "values": list(t.values), "loc": self._loc(t.loc)}
                for t in sorted(enum_types.values(), key=lambda t: t.name)
            ],
            "nominals": [
                {"name": t.name, "underlying": t.underlying.wire(), "ops": sorted(t.ops),
                 "loc": self._loc(t.loc)}
                for t in sorted(nominal_types.values(), key=lambda t: t.name)
            ],
            "entities": [
                {"name": d.name, "loc": self._loc(d.loc),
                 "fields": [{"name": n, "type": t.wire(), "loc": self._loc(l)} for n, t, l in d.fields]}
                for d in decls
            ],
            "derived": [self._derived_wire(td) for td in traced],
            "invariants": [
                {"name": fn.name, "entity": p.type.display(), "param": p.name,
                 "body": body.to_wire(), "loc": self._loc(fn.loc)}
                for fn, p, body in invs
            ],
            "actions": [self._action_wire(fn, params, frame)
                        for fn, (params, frame) in zip(self.action_fns, acts)],
        }
        self._wire = _relocate(wire, self.root)
        self._wire_json = json.dumps(
            self._wire, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        )
        self._admission: Any = None

    def _loc(self, loc: tuple[str, int] | None) -> dict[str, Any]:
        if loc is None:
            return {"file": "<unknown>", "line": 1}
        return {"file": loc[0], "line": max(loc[1], 1)}

    def _trace_invariant(self, fn: InvariantFn) -> tuple[InvariantFn, ParamInfo, Expr]:
        params = fn.params()
        token = CURRENT.set(Frame("invariant"))
        try:
            body = lift(fn.call_symbolic(params))
        finally:
            CURRENT.reset(token)
        if body.type not in (BOOL, UNKNOWN):
            raise BehaviorTypeError(
                f"invariant `{fn.name}` must return Bool, found `{body.type}`", "NOT_BOOLEAN",
                *body.loc,
            )
        return fn, params[0], body

    def _trace_action(self, fn: ActionFn) -> tuple[list[ParamInfo], Frame]:
        params = fn.params()
        frame = Frame("action")
        token = CURRENT.set(frame)
        try:
            result = fn.call_symbolic(params)
        finally:
            CURRENT.reset(token)
        if result is not None:
            raise BehaviorDefinitionError(
                f"action `{fn.name}` must not return a value; use requires/set_/ensures", *fn.loc
            )
        return params, frame

    def _derived_wire(self, t: TracedDerived) -> dict[str, Any]:
        assert t.body is not None
        return {
            "name": t.fn.name,
            "kind": t.fn.kind,
            "params": [{"name": p.name, "type": p.type.wire()} for p in t.params],
            "body": t.body.to_wire(),
            "loc": self._loc(t.fn.loc),
        }

    def _action_wire(self, fn: BehaviorFn, params: list[ParamInfo], frame: Frame) -> dict[str, Any]:
        def cond(c: Condition) -> dict[str, Any]:
            return {"expr": c.expr.to_wire(), "loc": self._loc(c.loc)}

        def eff(e: Effect) -> dict[str, Any]:
            return {"target": {"param": e.param, "field": e.field}, "value": e.value.to_wire(),
                    "loc": self._loc(e.loc)}

        return {
            "name": fn.name,
            "params": [{"name": p.name, "role": p.role, "type": p.type.wire()} for p in params],
            "preconditions": [cond(c) for c in frame.preconditions],
            "effects": [eff(e) for e in frame.effects],
            "postconditions": [cond(c) for c in frame.postconditions],
            "loc": self._loc(fn.loc),
        }

    def to_wire_json(self) -> str:
        """Canonical wire IR (sorted keys, compact, no trailing newline)."""
        return self._wire_json

    @property
    def behavior_version(self) -> str | None:
        """The engine-computed behavior version (None if the engine refuses the module)."""
        from . import admit

        return admit(self).behavior_version
