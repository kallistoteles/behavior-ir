"""Relational queries (feature 007): typed, read-only set comprehensions over one entity type.

A query is a behavior expression, never a Python collection: it cannot be iterated, measured or
tested for truth. Its meaning is the set of existing entities of its type that satisfy its
predicates; `count`, `any_`, `all_`, `sum_`, `min_`, `max_` and `unique` turn it into a value.
Predicates are candidate-local: they read the candidate, literals and the enclosing parameters,
never another query or another entity's existence.
"""

from __future__ import annotations

import inspect
from typing import Any, Callable

from . import _engine
from .decl import EntityDecl, EntityVar, entity_decl
from .errors import BehaviorDefinitionError, BehaviorTypeError
from .expr import Expr, builder, call, engine_error, lift
from .location import caller_loc

_NOT_A_COLLECTION = (
    "a query is a set of entities described by behavior, not a Python collection; use count(), "
    "any_(), all_(), sum_(), min_(), max_() or unique()"
)


class Query:
    """A query node over one entity type (built by `select`, `where` and set algebra)."""

    __slots__ = ("node", "loc", "decl")
    __hash__ = None  # type: ignore[assignment]

    def __init__(self, node: Any, loc: tuple[str, int], decl: EntityDecl) -> None:
        self.node = node
        self.loc = loc
        self.decl = decl

    def __repr__(self) -> str:
        return f"Query({self.decl.name})"

    def where(self, predicate: Callable[[Any], Any]) -> Query:
        """The members whose candidate satisfies `predicate` (a candidate-local lambda)."""
        loc = caller_loc()
        return Query(_lambda("where", self, predicate, loc), loc, self.decl)

    def union(self, other: Query) -> Query:
        return self._set("union", other)

    def intersection(self, other: Query) -> Query:
        return self._set("intersection", other)

    def difference(self, other: Query) -> Query:
        return self._set("difference", other)

    def _set(self, op: str, other: Query) -> Query:
        loc = caller_loc()
        if not isinstance(other, Query):
            raise BehaviorTypeError(f"`{op}` combines two queries", "TYPE_MISMATCH", *loc)
        node = call("op", op, [self.node, other.node], loc=loc)
        return Query(node, loc, self.decl)

    def __iter__(self) -> Any:
        raise BehaviorDefinitionError(_NOT_A_COLLECTION, *caller_loc())

    def __len__(self) -> int:
        raise BehaviorDefinitionError(_NOT_A_COLLECTION, *caller_loc())

    def __bool__(self) -> bool:
        raise BehaviorDefinitionError(_NOT_A_COLLECTION, *caller_loc())


def select(entity_cls: Any) -> Query:
    """Every existing entity of the `@entity` class `entity_cls`."""
    loc = caller_loc()
    decl = entity_decl(entity_cls)
    if decl is None:
        raise BehaviorDefinitionError(f"{entity_cls!r} is not an @entity class", *loc)
    return Query(call("select", decl.name, loc=loc), loc, decl)


def _query(q: object, what: str, loc: tuple[str, int]) -> Query:
    if not isinstance(q, Query):
        raise BehaviorTypeError(f"`{what}` needs a query, got {q!r}", "TYPE_MISMATCH", *loc)
    return q


def _lambda(op: str, q: Query, fn: Callable[[Any], Any], loc: tuple[str, int]) -> Any:
    """Traces `fn` over a symbolic candidate and builds `op(q, param → body)`."""
    if not callable(fn):
        raise BehaviorDefinitionError(f"`{op}` needs a lambda over the candidate", *loc)
    params = list(inspect.signature(fn).parameters)
    if len(params) != 1:
        raise BehaviorDefinitionError(f"the lambda of `{op}` takes exactly one parameter", *loc)
    name = params[0]
    session = builder()
    try:
        session.builder.push_lambda(name, q.decl.name)
    except _engine.EngineError as e:
        raise engine_error(e, loc) from None
    try:
        body = lift(fn(EntityVar(name, q.decl, "read")))
    finally:
        session.builder.pop_scope()
    return call("lambda_", op, q.node, name, body.node, loc=loc)


def _fold(op: str, q: object, fn: Callable[[Any], Any]) -> Expr:
    loc = caller_loc()
    query = _query(q, op, loc)
    return Expr(_lambda(op, query, fn, loc), loc, op)


def count(q: Query) -> Expr:
    """The number of members (0 for the empty set)."""
    loc = caller_loc()
    query = _query(q, "count", loc)
    return Expr(call("op", "count", [query.node], loc=loc), loc, "count")


def any_(q: Query, predicate: Callable[[Any], Any]) -> Expr:
    """Whether some member satisfies `predicate` (false for the empty set)."""
    return _fold("any", q, predicate)


def all_(q: Query, predicate: Callable[[Any], Any]) -> Expr:
    """Whether every member satisfies `predicate` (true for the empty set)."""
    return _fold("all", q, predicate)


def sum_(q: Query, value: Callable[[Any], Any]) -> Expr:
    """The exact sum of `value` over the members (the exact zero for the empty set)."""
    return _fold("sum", q, value)


def min_(q: Query, value: Callable[[Any], Any]) -> Expr:
    """The least `value` over the members, as an option (`none` for the empty set)."""
    return _fold("min", q, value)


def max_(q: Query, value: Callable[[Any], Any]) -> Expr:
    """The greatest `value` over the members, as an option (`none` for the empty set)."""
    return _fold("max", q, value)


def unique(q: Query, *, by: Callable[[Any], Any]) -> Expr:
    """Whether no two distinct members have the same `by` value."""
    return _fold("unique", q, by)


__all__ = ["Query", "select", "count", "any_", "all_", "sum_", "min_", "max_", "unique"]
