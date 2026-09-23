"""Behavior types and the typing rules (data-model.md → Typing rules).

These rules mirror the engine's so authors get errors where they write an expression. The
engine checks everything again on admission; the shared table
`tests/fixtures/typing_cases.json` keeps the two checkers in agreement.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from decimal import Decimal
from enum import Enum
from typing import TYPE_CHECKING, Any

from .errors import BehaviorDefinitionError, BehaviorTypeError
from .location import caller_loc

if TYPE_CHECKING:
    from .expr import Expr

ORDER, ADD, SCALE, RATIO = "order", "add", "scale", "ratio"
OPS = frozenset({ORDER, ADD, SCALE, RATIO})


class BType:
    """Base class of behavior types."""

    def wire(self) -> dict[str, Any]:
        raise NotImplementedError

    def __str__(self) -> str:
        return self.display()

    def display(self) -> str:
        raise NotImplementedError


@dataclass(frozen=True)
class PrimT(BType):
    name: str  # bool | int | decimal | string

    def wire(self) -> dict[str, Any]:
        return {"t": self.name}

    def display(self) -> str:
        return {"bool": "Bool", "int": "Int", "decimal": "Decimal", "string": "String"}[self.name]


BOOL = PrimT("bool")
INT = PrimT("int")
DECIMAL = PrimT("decimal")
STRING = PrimT("string")


@dataclass(frozen=True)
class OptionT(BType):
    of: BType

    def wire(self) -> dict[str, Any]:
        return {"t": "option", "of": self.of.wire()}

    def display(self) -> str:
        return f"Option<{self.of}>"


@dataclass(frozen=True)
class EnumT(BType):
    name: str
    values: tuple[str, ...]
    loc: tuple[str, int] | None = field(default=None, compare=False)
    py_enum: type[Enum] | None = field(default=None, compare=False)

    def wire(self) -> dict[str, Any]:
        return {"t": "enum", "name": self.name}

    def display(self) -> str:
        return self.name


@dataclass(frozen=True)
class IdT(BType):
    entity: str

    def wire(self) -> dict[str, Any]:
        return {"t": "id", "entity": self.entity}

    def display(self) -> str:
        return f"Id<{self.entity}>"


@dataclass(frozen=True)
class EntityT(BType):
    name: str

    def wire(self) -> dict[str, Any]:
        return {"t": "entity", "name": self.name}

    def display(self) -> str:
        return self.name


@dataclass(frozen=True)
class UnknownT(BType):
    """Placeholder for a derived value on a cycle; the engine reports the cycle."""

    def wire(self) -> dict[str, Any]:
        raise BehaviorDefinitionError("internal: unknown type has no wire form")

    def display(self) -> str:
        return "?"


UNKNOWN = UnknownT()


@dataclass(frozen=True)
class NominalT(BType):
    """A nominal type over a primitive, with its declared operations."""

    name: str
    underlying: PrimT
    ops: frozenset[str]
    loc: tuple[str, int] | None = field(default=None, compare=False)

    def wire(self) -> dict[str, Any]:
        return {"t": "nominal", "name": self.name}

    def display(self) -> str:
        return self.name

    def has(self, op: str) -> bool:
        return op in self.ops

    def __call__(self, value: Any) -> Expr:
        """`Money(Decimal("5"))` is a literal; `Money(expr)` wraps an expression."""
        from .expr import Expr, lit, wrap

        if isinstance(value, Expr):
            return wrap(self, value)
        return lit(self, value)


def nominal(name: str, underlying: Any, ops: set[str] | frozenset[str] = frozenset()) -> NominalT:
    """Declares a nominal type, e.g. `Money = nominal("Money", Decimal, ops={"add", ...})`."""
    loc = caller_loc()
    prim = to_type(underlying)
    if not isinstance(prim, PrimT):
        raise BehaviorDefinitionError(f"nominal `{name}` must wrap bool, int, Decimal, or str", *loc)
    unknown = set(ops) - OPS
    if unknown:
        raise BehaviorDefinitionError(f"unknown operations {sorted(unknown)}", *loc)
    if ops and prim not in (INT, DECIMAL):
        raise BehaviorDefinitionError(f"operations on `{name}` require a numeric type", *loc)
    return NominalT(name, prim, frozenset(ops), loc)


class _OptionFactory:
    def __getitem__(self, item: Any) -> OptionT:
        inner = to_type(item)
        if isinstance(inner, OptionT):
            raise BehaviorTypeError("options cannot be nested", "NESTED_OPTION", *caller_loc())
        return OptionT(inner)


class _IdFactory:
    def __getitem__(self, item: Any) -> IdT:
        if isinstance(item, str):
            return IdT(item)
        decl = getattr(item, "__behavior_entity__", None)
        if decl is None:
            raise BehaviorDefinitionError(f"Id[...] needs an @entity class, got {item!r}", *caller_loc())
        return IdT(decl.name)


Option = _OptionFactory()
Id = _IdFactory()


@dataclass(frozen=True)
class RoleSpec:
    role: str  # input | context
    spec: Any


class _RoleFactory:
    def __init__(self, role: str) -> None:
        self.role = role

    def __getitem__(self, item: Any) -> RoleSpec:
        return RoleSpec(self.role, item)


Context = _RoleFactory("context")
Input = _RoleFactory("input")

_ENUM_TYPES: dict[type[Enum], EnumT] = {}


def enum_type(cls: type[Enum]) -> EnumT:
    cached = _ENUM_TYPES.get(cls)
    if cached is not None:
        return cached
    values = tuple(m.value for m in cls)
    if not all(isinstance(v, str) for v in values):
        raise BehaviorDefinitionError(f"enum `{cls.__name__}` must have str values", *caller_loc())
    import inspect

    try:
        loc: tuple[str, int] | None = (inspect.getfile(cls), cls.__firstlineno__)  # type: ignore[attr-defined]
    except (TypeError, AttributeError):
        loc = None
    t = EnumT(cls.__name__, values, loc, cls)
    _ENUM_TYPES[cls] = t
    return t


def to_type(spec: Any) -> BType:
    """Converts a Python type annotation or DSL type to a behavior type."""
    if isinstance(spec, BType):
        return spec
    if spec is bool:
        return BOOL
    if spec is int:
        return INT
    if spec is Decimal:
        return DECIMAL
    if spec is str:
        return STRING
    if spec is float:
        raise BehaviorDefinitionError(
            "float is not allowed: use decimal.Decimal for exact numbers", *caller_loc()
        )
    if isinstance(spec, type) and issubclass(spec, Enum):
        return enum_type(spec)
    decl = getattr(spec, "__behavior_entity__", None)
    if decl is not None:
        return EntityT(decl.name)
    raise BehaviorDefinitionError(f"unsupported type {spec!r}", *caller_loc())


# --- typing rules ------------------------------------------------------------------------

KEEP, TO_DECIMAL, SOME = "keep", "to_decimal", "some"


def _numeric(t: BType) -> bool:
    return t in (INT, DECIMAL)


def _promote(a: BType, b: BType) -> list[str]:
    if a == INT and b == INT:
        return [KEEP, KEEP]
    return [TO_DECIMAL if t == INT else KEEP for t in (a, b)]


def _mismatch(op: str, operands: list[BType]) -> BehaviorTypeError:
    listed = " and ".join(f"`{t}`" for t in operands)
    return BehaviorTypeError(f"cannot apply `{op}` to {listed}", "TYPE_MISMATCH", *caller_loc())


def _not_allowed(op: str, operands: list[BType]) -> BehaviorTypeError:
    listed = " and ".join(f"`{t}`" for t in operands)
    return BehaviorTypeError(f"`{op}` is not declared for {listed}", "OP_NOT_ALLOWED", *caller_loc())


def check_type(t: BType) -> None:
    if isinstance(t, OptionT):
        if isinstance(t.of, OptionT):
            raise BehaviorTypeError("options cannot be nested", "NESTED_OPTION", *caller_loc())
        check_type(t.of)


def coerce(expected: BType, actual: BType) -> str | None:
    """How a value of type `actual` can be used where `expected` is required."""
    if expected == actual or actual == UNKNOWN or expected == UNKNOWN:
        return KEEP
    if expected == DECIMAL and actual == INT:
        return TO_DECIMAL
    if isinstance(expected, OptionT) and expected.of == actual:
        return SOME
    return None


COMPARISONS = {"eq", "ne", "lt", "le", "gt", "ge"}


def type_of_op(
    op: str,
    operands: list[BType],
    *,
    count: int | None = None,
    target: BType | None = None,
) -> tuple[BType, list[str]]:
    """Result type of `op` and the conversions for its operands; raises BehaviorTypeError."""
    n = len(operands)
    if UNKNOWN in operands:
        boolish = op in COMPARISONS | {"and", "or", "not", "in", "is_none", "is_some"}
        return (BOOL if boolish else UNKNOWN), [KEEP] * n
    if op in ("eq", "ne"):
        a, b = operands
        if isinstance(a, EntityT) or isinstance(b, EntityT):
            raise _mismatch(op, operands)
        if a == b:
            return BOOL, [KEEP, KEEP]
        if _numeric(a) and _numeric(b):
            return BOOL, _promote(a, b)
        if isinstance(a, OptionT) and a.of == b:
            return BOOL, [KEEP, SOME]
        if isinstance(b, OptionT) and b.of == a:
            return BOOL, [SOME, KEEP]
        raise _mismatch(op, operands)
    if op in ("lt", "le", "gt", "ge"):
        a, b = operands
        if _numeric(a) and _numeric(b):
            return BOOL, _promote(a, b)
        if isinstance(a, NominalT) and a == b:
            if a.has(ORDER):
                return BOOL, [KEEP, KEEP]
            raise _not_allowed(op, operands)
        raise _mismatch(op, operands)
    if op in ("add", "sub"):
        a, b = operands
        if a == INT and b == INT:
            return INT, [KEEP, KEEP]
        if _numeric(a) and _numeric(b):
            return DECIMAL, _promote(a, b)
        if isinstance(a, NominalT) and a == b:
            if a.has(ADD):
                return a, [KEEP, KEEP]
            raise _not_allowed(op, operands)
        raise _mismatch(op, operands)
    if op == "mul":
        a, b = operands
        if a == INT and b == INT:
            return INT, [KEEP, KEEP]
        if _numeric(a) and _numeric(b):
            return DECIMAL, _promote(a, b)
        if isinstance(a, NominalT) and _numeric(b):
            nom, scalar, first = a, b, True
        elif isinstance(b, NominalT) and _numeric(a):
            nom, scalar, first = b, a, False
        else:
            raise _mismatch(op, operands)
        if not nom.has(SCALE):
            raise _not_allowed(op, operands)
        if scalar == DECIMAL and nom.underlying != DECIMAL:
            raise _mismatch(op, operands)
        conv = TO_DECIMAL if nom.underlying == DECIMAL and scalar == INT else KEEP
        return nom, ([KEEP, conv] if first else [conv, KEEP])
    if op == "div":
        a, b = operands
        if _numeric(a) and _numeric(b):
            return DECIMAL, [TO_DECIMAL if t == INT else KEEP for t in (a, b)]
        if isinstance(a, NominalT) and a == b:
            if a.has(RATIO):
                return DECIMAL, [KEEP, KEEP]
            raise _not_allowed(op, operands)
        if isinstance(a, NominalT) and _numeric(b):
            if not a.has(SCALE):
                raise _not_allowed(op, operands)
            if a.underlying != DECIMAL:
                raise _mismatch(op, operands)
            return a, [KEEP, TO_DECIMAL if b == INT else KEEP]
        raise _mismatch(op, operands)
    if op in ("and", "or"):
        if n >= 2 and all(t == BOOL for t in operands):
            return BOOL, [KEEP] * n
        raise _mismatch(op, operands)
    if op == "not":
        if operands == [BOOL]:
            return BOOL, [KEEP]
        raise _mismatch(op, operands)
    if op == "in":
        if not count:
            raise BehaviorTypeError("`in` needs at least one value", "EMPTY_IN", *caller_loc())
        if isinstance(operands[0], (OptionT, EntityT)):
            raise _mismatch(op, operands)
        return BOOL, [KEEP]
    if op in ("is_none", "is_some"):
        if isinstance(operands[0], OptionT):
            return BOOL, [KEEP]
        raise _mismatch(op, operands)
    if op == "value_or":
        a, b = operands
        if isinstance(a, OptionT):
            c = coerce(a.of, b)
            if c in (KEEP, TO_DECIMAL):
                return a.of, [KEEP, c]
        raise _mismatch(op, operands)
    if op == "some":
        a = operands[0]
        if isinstance(a, OptionT):
            raise BehaviorTypeError("options cannot be nested", "NESTED_OPTION", *caller_loc())
        if isinstance(a, EntityT):
            raise _mismatch(op, operands)
        return OptionT(a), [KEEP]
    if op == "to_decimal":
        if operands == [INT]:
            return DECIMAL, [KEEP]
        raise _mismatch(op, operands)
    if op == "wrap":
        if not isinstance(target, NominalT):
            raise _mismatch(op, operands)
        c = coerce(target.underlying, operands[0])
        if c in (KEEP, TO_DECIMAL):
            return target, [c]
        raise _mismatch(target.name, operands)
    if op == "unwrap":
        a = operands[0]
        if isinstance(a, NominalT):
            return a.underlying, [KEEP]
        raise _mismatch(op, operands)
    raise BehaviorDefinitionError(f"unknown operation `{op}`", *caller_loc())
