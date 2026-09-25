"""Type descriptors of the DSL: primitives, Option, Id, enums, nominal types, parameter roles.

These only *describe* types and translate them to wire form; all typing rules live in the
Rust engine (research R10, R17).
"""

from __future__ import annotations

import inspect
from dataclasses import dataclass, field
from decimal import Decimal
from enum import Enum
from typing import TYPE_CHECKING, Any

from .errors import BehaviorDefinitionError
from .location import caller_loc

if TYPE_CHECKING:
    from .expr import Expr

OPS = frozenset({"order", "add", "scale", "ratio"})


class BType:
    """Base class of type descriptors."""

    def wire(self) -> dict[str, Any]:
        raise NotImplementedError

    def display(self) -> str:
        raise NotImplementedError

    def __str__(self) -> str:
        return self.display()


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
    return NominalT(name, prim, frozenset(ops), loc)


class _OptionFactory:
    def __getitem__(self, item: Any) -> OptionT:
        return OptionT(to_type(item))


class _IdFactory:
    def __getitem__(self, item: Any) -> IdT:
        if isinstance(item, str):
            return IdT(item)
        decl = getattr(item, "__behavior_entity__", None)
        if decl is None:
            raise BehaviorDefinitionError(
                f"Id[...] needs an @entity class, got {item!r}", *caller_loc()
            )
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
    try:
        loc: tuple[str, int] | None = (inspect.getfile(cls), cls.__firstlineno__)  # type: ignore[attr-defined]
    except (TypeError, AttributeError):
        loc = None
    t = EnumT(cls.__name__, values, loc)
    _ENUM_TYPES[cls] = t
    return t


def to_type(spec: Any) -> BType:
    """Converts a Python annotation or DSL type to a type descriptor."""
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
