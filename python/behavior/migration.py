"""Migrations (feature 009): an explicit, verifiable change of a store's schema.

A migration relates two modules, the source and the target schema. For every entity type whose
declaration changed, a transform says how an old entity becomes a new one; fields with the same
name and type are copied automatically, everything else is explicit. Removed fields need an
explicit drop, removed entity types an explicit retirement, and narrowing (`strict_unwrap`,
`strict_enum_map`) is allowed only where the source requirements prove it safe.

The Python layer only traces the transforms and requirements through the engine's builder; the
engine admits, hashes, verifies and applies the migration.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Callable, Mapping, Sequence

from . import _engine
from .decl import EntityVar, entity_decl
from .errors import BehaviorDefinitionError, BehaviorInvalid
from .expr import Expr, call, engine_error, lift
from .location import caller_loc
from .module import _SESSION, BehaviorModule
from .results import AdmissionError, AdmissionResult
from .types import BType, EnumT, NominalT, OptionT, enum_type
from .verify import Attestation, Profile


class MigrationSession:
    """The builder session of one migration: expressions are typed over both schemas."""

    def __init__(self, source: BehaviorModule, target: BehaviorModule) -> None:
        self.builder = _engine.Builder.for_migration(source.engine, target.engine)
        self.source_types = source.declared_types
        self.target_types = target.declared_types
        self.started: set[int] = set()

    def declare(self, t: BType) -> None:
        """Types come from the two modules; a type of neither is refused by the engine."""

    def is_target(self, t: BType) -> bool:
        """A named type is the target's when the target module declares it as given and the
        source module does not."""
        if not isinstance(t, (EnumT, NominalT)):
            return False
        return self.source_types.get(t.name) != t and self.target_types.get(t.name) == t

    def engine_type(self, t: BType) -> Any:
        if isinstance(t, OptionT):
            return _engine.Type.option(self.engine_type(t.of))
        e = t.engine()
        return _engine.Type.on_target(e) if self.is_target(t) else e

    def nominal_name(self, t: NominalT) -> str:
        return f"{_engine.TARGET_SIDE}{t.name}" if self.is_target(t) else t.name


@dataclass(frozen=True)
class MigrationAdmission:
    ok: bool
    errors: list[AdmissionError]
    hash: str | None
    summary: dict[str, Any] | None


def _fn_loc(fn: Callable[..., Any]) -> tuple[str, int]:
    code = getattr(fn, "__code__", None)
    if code is None:
        return caller_loc()
    return code.co_filename, code.co_firstlineno


def _entity_name(cls: Any, what: str) -> str:
    d = entity_decl(cls)
    if d is None:
        raise BehaviorDefinitionError(f"{what} needs an @entity class, got {cls!r}", *caller_loc())
    return d.name


class Migration:
    """A migration from `source` to `target`.

    - `transforms`: entity class (of the source module) → `lambda old: {field: value}`, where
      `old` is the old entity and each value an expression over it, a literal or `None`. Fields
      not given are copied when name and type are unchanged.
    - `requires`: name → `lambda: predicate`, closed predicates over the source state that must
      hold for the migration to apply (they make narrowing safe).
    - `drops`: entity class → names of removed fields whose information is deliberately lost.
    - `retire`: entity classes the target no longer declares (they must be empty).
    """

    def __init__(
        self,
        source: BehaviorModule,
        target: BehaviorModule,
        transforms: Mapping[Any, Callable[[Any], Mapping[str, object]]] | None = None,
        requires: Mapping[str, Callable[[], object]] | None = None,
        drops: Mapping[Any, Sequence[str]] | None = None,
        retire: Sequence[Any] = (),
        name: str = "migration",
    ) -> None:
        self.source = source
        self.target = target
        self.name = name
        session = MigrationSession(source, target)
        b = session.builder
        token = _SESSION.set(session)  # type: ignore[arg-type]
        try:
            for rname, predicate in (requires or {}).items():
                loc = _fn_loc(predicate)
                b.push_requirement()
                try:
                    body = lift(predicate())
                finally:
                    b.pop_scope()
                try:
                    b.add_requirement(rname, body.node, *loc)
                except _engine.EngineError as e:
                    raise engine_error(e, loc) from None
            for cls, transform in (transforms or {}).items():
                entity = _entity_name(cls, "a transform")
                decl = entity_decl(cls)
                assert decl is not None
                loc = _fn_loc(transform)
                try:
                    b.push_transform(entity, *loc)
                except _engine.EngineError as e:
                    raise engine_error(e, loc) from None
                try:
                    result = transform(EntityVar("old", decl, "read"))
                    if not isinstance(result, Mapping):
                        raise BehaviorDefinitionError(
                            f"the transform of `{entity}` must return a dict of target fields", *loc
                        )
                    target_types = self._target_fields(entity)
                    for fname, value in result.items():
                        t = target_types.get(fname)
                        expected = session.engine_type(t) if t is not None else None
                        node = lift(value, expected)
                        try:
                            b.set_field(entity, fname, node.node)
                        except _engine.EngineError as e:
                            raise engine_error(e, node.loc) from None
                finally:
                    b.pop_scope()
            for cls, names in (drops or {}).items():
                entity = _entity_name(cls, "a drop")
                for n in names:
                    b.drop_field(entity, n)
            for cls in retire:
                b.retire(_entity_name(cls, "a retirement"))
        finally:
            _SESSION.reset(token)
        self._inner, self._admission = b.finish_migration(name)

    def _target_fields(self, entity: str) -> dict[str, BType]:
        for d in self.target.entities:
            if d.name == entity:
                return {n: t for n, t, _ in d.fields}
        return {}

    def admit(self) -> MigrationAdmission:
        """The admission result: ok, or every error with its code."""
        a = self._admission
        if self._inner is None:
            r = AdmissionResult.from_dict(a)
            return MigrationAdmission(False, list(r.errors), None, None)
        return MigrationAdmission(True, [], a["hash"], a["summary"])

    @property
    def engine(self) -> Any:
        if self._inner is None:
            raise BehaviorInvalid(AdmissionResult.from_dict(self._admission))
        return self._inner

    @property
    def hash(self) -> str:
        """The migration's identity: the hash of its resolved form."""
        return str(self.engine.hash)

    @property
    def source_schema(self) -> str:
        return str(self.engine.source_schema)

    @property
    def target_schema(self) -> str:
        return str(self.engine.target_schema)

    def summary(self) -> dict[str, Any]:
        """Per migrated type: copied, transformed, new and dropped fields; and retired types."""
        return dict(self.engine.summary())

    def to_json(self) -> str:
        """The resolved migration document (migration IR 0.1)."""
        return str(self.engine.resolved_json())

    @staticmethod
    def from_json(source: BehaviorModule, target: BehaviorModule, text: str) -> Migration:
        """Admits a migration document between two modules."""
        m = Migration.__new__(Migration)
        m.source, m.target = source, target
        m._inner, m._admission = _engine.Migration.from_json(source.engine, target.engine, text)
        m.name = str(m._inner.name) if m._inner is not None else "migration"
        return m


def strict_unwrap(value: object) -> Expr:
    """The value of an option, in a migration transform: a narrowing that must be proven safe
    (by the source schema or a source requirement); an absent value refuses the migration."""
    loc = caller_loc()
    arg = lift(value)
    return Expr(call("strict_unwrap", arg.node, loc=loc), loc, "strict_unwrap", (arg,))


def _enum_map(value: object, mapping: Mapping[Enum, Enum], strict: bool) -> Expr:
    loc = caller_loc()
    if not mapping:
        raise BehaviorDefinitionError("an enum map needs at least one pair", *loc)
    targets = {type(t) for t in mapping.values()}
    if len(targets) != 1 or not all(isinstance(t, Enum) for t in mapping.values()):
        raise BehaviorDefinitionError("an enum map maps to the values of one enum", *loc)
    arg = lift(value)
    session = _SESSION.get()
    if not isinstance(session, MigrationSession):
        raise BehaviorDefinitionError("enum maps are available only in migrations", *loc)
    to = enum_type(next(iter(targets)))
    pairs = [(s.value, t.value) for s, t in mapping.items()]
    node = call("enum_map", arg.node, session.engine_type(to), pairs, strict, loc=loc)
    op = "strict_enum_map" if strict else "enum_map"
    return Expr(node, loc, op, (arg,))


def enum_map(value: object, mapping: Mapping[Enum, Enum]) -> Expr:
    """Maps every value of a source enum to a value of a target enum (a total map)."""
    return _enum_map(value, mapping, False)


def strict_enum_map(value: object, mapping: Mapping[Enum, Enum]) -> Expr:
    """Maps some values of a source enum (a narrowing that must be proven safe; an unmapped
    value refuses the migration)."""
    return _enum_map(value, mapping, True)


def apply_migration(m: Migration, entities: Sequence[dict[str, Any]]) -> dict[str, Any]:
    """Applies a migration to a supplied source universe (`{"entity", "value"}` dicts), without
    a store. `result` is "MIGRATED" with the target entities, or the refusal code."""
    out: dict[str, Any] = _engine.apply_migration(
        m.engine, m.source.engine, m.target.engine, list(entities)
    )
    return out


def verify_migration(m: Migration, profile: Profile | None = None) -> Attestation:
    """Verifies a migration with the solver: for every migrated type, narrowing sites, evaluation
    errors and target rules on the transformed value, under the source rules and the migration's
    requirements (a proof that needs them lists them in `under`); whole-state properties outside
    the model are inconclusive. Raises BehaviorError if the solver is unavailable."""
    import warnings

    from .errors import BehaviorError

    p = profile or Profile()
    notice = _engine.solver_notice()
    if notice is not None:
        warnings.warn(notice, stacklevel=2)
    try:
        a = m.engine.verify(
            m.source.engine, m.target.engine, p.checks, p.rlimit, p.wall_clock_guard_ms
        )
    except _engine.EngineError as e:
        raise BehaviorError(str(e.args[1])) from None
    return Attestation(a.result, a.hash, a.data, a.json)
