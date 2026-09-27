"""BehaviorModule: drives the engine builder through PyO3 (research R12, R17).

Tracing a body calls the builder for every node; `finish` admits the module through the same
pipeline as wire JSON. The Python layer never emits or parses the behavior itself.
"""

from __future__ import annotations

from contextvars import ContextVar
from typing import Any, Sequence

from . import _engine
from .decl import (
    ActionFn, BehaviorFn, ConstraintFn, DerivedFn, EntityDecl, InvariantFn, ParamInfo, entity_decl,
)
from .errors import BehaviorDefinitionError, BehaviorInvalid
from .expr import engine_error, lift
from .statements import CURRENT, Frame, check_condition
from .types import BType, EnumT, NominalT, OptionT


class CompileSession:
    """One module compilation: the engine builder plus which bodies are traced."""

    def __init__(self) -> None:
        self.builder = _engine.Builder()
        self.started: set[int] = set()
        self._declared: set[str] = set()

    def declare(self, t: BType) -> None:
        """Declares enum and nominal types to the builder (idempotent)."""
        if isinstance(t, OptionT):
            self.declare(t.of)
            return
        if not isinstance(t, (EnumT, NominalT)) or t.name in self._declared:
            return
        file, line = t.loc or ("<unknown>", 1)
        try:
            if isinstance(t, EnumT):
                self.builder.declare_enum(t.name, list(t.values), file, line)
            else:
                self.builder.declare_nominal(
                    t.name, t.underlying.engine(), sorted(t.ops), file, line
                )
        except _engine.EngineError as e:
            raise engine_error(e, (file, line)) from None
        self._declared.add(t.name)

    def push_scope(self, fn: BehaviorFn, params: list[ParamInfo]) -> None:
        for p in params:
            self.declare(p.type)
        site = "action" if fn.kind == "action" else "derived"
        try:
            self.builder.push_scope(site, fn.engine_params(params), *fn.loc)
        except _engine.EngineError as e:
            raise engine_error(e, fn.loc) from None

    def trace_derived(self, fn: DerivedFn) -> None:
        """Traces a derived value or rule once; a re-entrant call (a cycle) returns at once."""
        if id(fn) in self.started:
            return
        self.started.add(id(fn))
        params = fn.params()
        self.push_scope(fn, params)
        token = CURRENT.set(Frame(fn.kind))
        try:
            body = lift(fn.call_symbolic(params))
        finally:
            CURRENT.reset(token)
            self.builder.pop_scope()
        if fn.kind == "rule":
            check_condition(body, f"rule `{fn.name}`", body.loc)
        try:
            self.builder.add_derived(fn.name, fn.kind, fn.engine_params(params), body.node, *fn.loc)
        except _engine.EngineError as e:
            raise engine_error(e, body.loc) from None


_SESSION: ContextVar[CompileSession | None] = ContextVar("behavior_session", default=None)


def current_session() -> CompileSession | None:
    return _SESSION.get()


class BehaviorModule:
    """A behavior module compiled by the engine. Construction traces every body."""

    def __init__(
        self,
        entities: Sequence[type],
        derived: Sequence[DerivedFn] = (),
        invariants: Sequence[InvariantFn] = (),
        actions: Sequence[ActionFn] = (),
        constraints: Sequence[ConstraintFn] = (),
        enums: Sequence[Any] = (),
        nominals: Sequence[NominalT] = (),
        root: str | None = None,
    ) -> None:
        decls: list[EntityDecl] = []
        for cls in entities:
            d = entity_decl(cls)
            if d is None:
                raise BehaviorDefinitionError(f"{cls!r} is not an @entity class")
            decls.append(d)
        self.entities = decls

        session = CompileSession()
        token = _SESSION.set(session)
        try:
            from .types import enum_type

            for e in enums:
                session.declare(e if isinstance(e, EnumT) else enum_type(e))
            for n in nominals:
                session.declare(n)
            for d in decls:
                for _, t, _ in d.fields:
                    session.declare(t)
            for d in decls:
                fields = [(n, t.engine(), l[0], l[1]) for n, t, l in d.fields]
                try:
                    session.builder.declare_entity(d.name, fields, *d.loc)
                except _engine.EngineError as e:
                    raise engine_error(e, d.loc) from None
            for fn in derived:
                session.trace_derived(fn)
            for inv in invariants:
                self._trace_invariant(session, inv)
            for con in constraints:
                self._trace_invariant(session, con)
            for act in actions:
                self._trace_action(session, act)
        finally:
            _SESSION.reset(token)

        self._module, self._admission = session.builder.finish(root)

    @staticmethod
    def _trace_invariant(session: CompileSession, fn: InvariantFn | ConstraintFn) -> None:
        params = fn.params()
        session.push_scope(fn, params)
        token = CURRENT.set(Frame("invariant"))
        try:
            body = lift(fn.call_symbolic(params))
        finally:
            CURRENT.reset(token)
            session.builder.pop_scope()
        p = params[0]
        add = (session.builder.add_constraint if fn.kind == "constraint"
               else session.builder.add_invariant)
        try:
            add(fn.name, p.type.display(), p.name, body.node, *fn.loc)
        except _engine.EngineError as e:
            raise engine_error(e, body.loc) from None

    @staticmethod
    def _trace_action(session: CompileSession, fn: ActionFn) -> None:
        params = fn.params()
        session.push_scope(fn, params)
        frame = Frame("action")
        token = CURRENT.set(frame)
        try:
            result = fn.call_symbolic(params)
        finally:
            CURRENT.reset(token)
            session.builder.pop_scope()
        if result is not None:
            raise BehaviorDefinitionError(
                f"action `{fn.name}` must not return a value; use requires/set_/ensures", *fn.loc
            )
        try:
            session.builder.add_action(
                fn.name,
                fn.engine_params(params),
                [(c.expr.node, *c.loc) for c in frame.preconditions],
                [(e.param, e.field, e.value.node, *e.loc) for e in frame.effects],
                [(c.expr.node, *c.loc) for c in frame.postconditions],
                *fn.loc,
            )
        except _engine.EngineError as e:
            raise engine_error(e, fn.loc) from None

    @property
    def engine(self) -> Any:
        """The admitted engine module; raises BehaviorInvalid if admission failed."""
        if self._module is None:
            from .results import AdmissionResult

            raise BehaviorInvalid(AdmissionResult.from_dict(self._admission))
        return self._module

    def to_wire_json(self) -> str:
        """Canonical wire IR, serialized by the engine from the admitted module."""
        return str(self.engine.wire_json())

    @property
    def behavior_version(self) -> str | None:
        return None if self._module is None else str(self._module.behavior_version)
