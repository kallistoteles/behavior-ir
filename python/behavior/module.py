"""BehaviorModule: drives the engine builder through PyO3 (research R12, R17).

Tracing a body calls the builder for every node; `finish` admits the module through the same
pipeline as wire JSON. The Python layer never emits or parses the behavior itself.
"""

from __future__ import annotations

from contextvars import ContextVar
from typing import Any, Sequence

from . import _engine
from .decl import (
    ActionFn, BehaviorFn, ConstraintFn, DerivedFn, EntityDecl, InvariantFn, ParamInfo, ReadFn,
    entity_decl,
)
from .errors import BehaviorDefinitionError, BehaviorInvalid
from .expr import engine_error, lift
from .statements import CURRENT, Frame, check_condition
from .types import BType, EnumT, ExactT, NominalT, OptionT


class CompileSession:
    """One module compilation: the engine builder plus which bodies are traced."""

    def __init__(self) -> None:
        self.builder = _engine.Builder()
        self.started: set[int] = set()
        self._declared: set[str] = set()
        #: Every enum and nominal type declared to the builder, by name (feature 009: a
        #: migration tells the two schemas' types apart with them).
        self.types: dict[str, BType] = {}

    def declare(self, t: BType) -> None:
        """Declares enum and nominal types to the builder (idempotent)."""
        if isinstance(t, OptionT):
            self.declare(t.of)
            return
        if isinstance(t, ExactT):
            if t.of is not None:
                self.declare(t.of)
            return
        if not isinstance(t, (EnumT, NominalT)) or t.name in self._declared:
            return
        self.types[t.name] = t
        file, line = t.loc or ("<unknown>", 1)
        try:
            if isinstance(t, EnumT):
                self.builder.declare_enum(t.name, list(t.values), file, line)
            else:
                self.builder.declare_nominal(
                    t.name, t.underlying.engine(), sorted(t.ops), file, line, t.scale
                )
        except _engine.EngineError as e:
            raise engine_error(e, (file, line)) from None
        self._declared.add(t.name)

    def engine_type(self, t: BType) -> Any:
        """The engine type of `t` (a migration session marks target-side types)."""
        return t.engine()

    def nominal_name(self, t: NominalT) -> str:
        """The engine name of a nominal type (a migration session marks the target side)."""
        return t.name

    def push_scope(self, fn: BehaviorFn, params: list[ParamInfo]) -> None:
        for p in params:
            self.declare(p.type)
        site = fn.kind if fn.kind in ("action", "read") else "derived"
        if fn.kind == "invariant" and not params:
            site = "closed"  # a module invariant (feature 007)
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
        declared = fn.declared_type()
        try:
            self.builder.add_derived(
                fn.name, fn.kind, fn.engine_params(params), body.node, *fn.loc,
                declared.engine() if declared is not None else None,
            )
        except _engine.EngineError as e:
            raise engine_error(e, body.loc) from None


_SESSION: ContextVar[CompileSession | None] = ContextVar("behavior_session", default=None)


def current_session() -> CompileSession | None:
    return _SESSION.get()


def trace_read(session: CompileSession, fn: ReadFn, declared: bool) -> Any:
    """Traces a read (feature 010): a declared read is added to the session's module; an ad-hoc
    read is admitted against the session's module and returned (an engine read item)."""
    from .query import Projection

    params = fn.params()
    session.push_scope(fn, params)
    token = CURRENT.set(Frame("read"))
    try:
        result = fn.call_symbolic(params)
        body = None if isinstance(result, Projection) else lift(result)
    finally:
        CURRENT.reset(token)
        session.builder.pop_scope()
    ps = fn.engine_params(params)
    try:
        if isinstance(result, Projection):
            over = result.query.node if result.query is not None else None
            args = (fn.name, ps, over, result.param, result.member, result.items, *fn.loc)
            if declared:
                session.builder.add_read_projection(*args)
                return None
            return session.builder.adhoc_read_projection(*args)
        assert body is not None
        if declared:
            session.builder.add_read_value(fn.name, ps, body.node, *fn.loc)
            return None
        return session.builder.adhoc_read_value(fn.name, ps, body.node, *fn.loc)
    except _engine.EngineError as e:
        raise engine_error(e, fn.loc if body is None else body.loc) from None


class BehaviorModule:
    """A behavior module admitted by the engine, from Python authoring or wire JSON."""

    @classmethod
    def from_wire_json(cls, text: str) -> BehaviorModule:
        """Admits a behavior document through Core, retaining its explicit semantic profile.

        Raw JSON text reaches the checked decoder unchanged. A refused document raises
        BehaviorInvalid with the engine's admission result. Imported modules retain core
        semantics without reconstructing Python declarations; use Migration.from_json for
        migrations between imported schemas.
        """
        from .results import AdmissionResult

        engine, admission = _engine.Module.from_wire(text)
        if engine is None:
            raise BehaviorInvalid(AdmissionResult.from_dict(admission))
        model = cls.__new__(cls)
        model.entities = []
        model.reads = []
        model.declared_types = {}
        model._module = engine
        model._admission = admission
        return model

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
        reads: Sequence[ReadFn] = (),
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
            for r in reads:
                trace_read(session, r, declared=True)
        finally:
            _SESSION.reset(token)

        #: The declared reads (feature 010): the module's read capabilities.
        self.reads: list[ReadFn] = list(reads)
        #: The enum and nominal types this module declares, by name (feature 009).
        self.declared_types: dict[str, BType] = dict(session.types)
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
        try:
            if not params:
                # A module invariant (feature 007): no parameter, a closed state expression.
                session.builder.add_global_invariant(fn.name, body.node, *fn.loc)
                return
            p = params[0]
            add = (session.builder.add_constraint if fn.kind == "constraint"
                   else session.builder.add_invariant)
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
                [
                    (lc.kind, lc.name, lc.id.node if lc.id is not None else None,
                     [(f, v.node) for f, v in lc.fields], *lc.loc)
                    for lc in frame.lifecycle
                ],
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

    @property
    def schema_hash(self) -> str:
        """The SchemaHash of the store schema this module declares (feature 009): its entity
        declarations, independent of actions, rules and derived values."""
        return str(self.engine.schema_hash)
