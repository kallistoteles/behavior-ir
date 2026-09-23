"""Typed expression trees. Operators build nodes; nothing here evaluates behavior.

Every node records its type and the author's source location, and operators check operand
types as they build, so an ill-typed expression fails at the line where it is written.
"""

from __future__ import annotations

from decimal import Decimal
from enum import Enum
from typing import Any

from .errors import BehaviorDefinitionError, BehaviorTypeError
from .location import caller_loc
from .types import (
    BOOL, DECIMAL, INT, STRING, UNKNOWN, BType, EnumT, IdT, NominalT, OptionT, PrimT, enum_type,
    type_of_op,
)


class _NoneLiteral:
    """The `none` literal of an option; its type comes from the other operand."""

    def __repr__(self) -> str:
        return "none"


none = _NoneLiteral()

_CONTROL_FLOW = (
    "behavior expressions cannot drive Python control flow (`if`, `and`, `or`, `not`, or "
    "chained comparisons like `a < b < c`); use `&`, `|`, `~`, and_(), or_(), not_() instead"
)


class Expr:
    """A typed expression node."""

    __slots__ = ("op", "type", "loc", "args", "data", "role")
    __hash__ = None  # type: ignore[assignment]  # `==` builds a node, so expressions are unhashable

    def __init__(
        self,
        op: str,
        type_: BType,
        args: tuple[Expr, ...] = (),
        data: dict[str, Any] | None = None,
        loc: tuple[str, int] | None = None,
    ) -> None:
        self.op = op
        self.type = type_
        self.args = args
        self.data = data or {}
        self.loc = loc or caller_loc()
        self.role: str | None = None  # parameter role of `field`/`param` nodes

    def __repr__(self) -> str:
        return f"Expr({self.op}: {self.type})"

    def __bool__(self) -> bool:
        raise BehaviorDefinitionError(_CONTROL_FLOW, *caller_loc())

    # comparisons
    def __eq__(self, other: object) -> Expr:  # type: ignore[override]
        return binary("eq", self, other)

    def __ne__(self, other: object) -> Expr:  # type: ignore[override]
        return binary("ne", self, other)

    def __lt__(self, other: object) -> Expr:
        return binary("lt", self, other)

    def __le__(self, other: object) -> Expr:
        return binary("le", self, other)

    def __gt__(self, other: object) -> Expr:
        return binary("gt", self, other)

    def __ge__(self, other: object) -> Expr:
        return binary("ge", self, other)

    # arithmetic
    def __add__(self, other: object) -> Expr:
        return binary("add", self, other)

    def __radd__(self, other: object) -> Expr:
        return binary("add", other, self)

    def __sub__(self, other: object) -> Expr:
        return binary("sub", self, other)

    def __rsub__(self, other: object) -> Expr:
        return binary("sub", other, self)

    def __mul__(self, other: object) -> Expr:
        return binary("mul", self, other)

    def __rmul__(self, other: object) -> Expr:
        return binary("mul", other, self)

    def __truediv__(self, other: object) -> Expr:
        return binary("div", self, other)

    def __rtruediv__(self, other: object) -> Expr:
        return binary("div", other, self)

    # boolean combinators
    def __and__(self, other: object) -> Expr:
        return and_(self, other)

    def __rand__(self, other: object) -> Expr:
        return and_(other, self)

    def __or__(self, other: object) -> Expr:
        return or_(self, other)

    def __ror__(self, other: object) -> Expr:
        return or_(other, self)

    def __invert__(self) -> Expr:
        return not_(self)

    # named operations
    def in_(self, values: list[Any] | tuple[Any, ...]) -> Expr:
        loc = caller_loc()
        literals = [lift(v, self.type) for v in values]
        type_of_op("in", [self.type], count=len(literals))
        return Expr("in", BOOL, (self,), {"values": [literal_value(v) for v in literals]}, loc)

    def is_none(self) -> Expr:
        return unary("is_none", self)

    def is_some(self) -> Expr:
        return unary("is_some", self)

    def value_or(self, default: object) -> Expr:
        expected = self.type.of if isinstance(self.type, OptionT) else None
        return build("value_or", [self, lift(default, expected)])

    def to_wire(self) -> dict[str, Any]:
        node: dict[str, Any] = {"op": self.op, "loc": {"file": self.loc[0], "line": self.loc[1]}}
        node.update(self.data)
        if self.op == "lit":
            node["type"] = self.type.wire()
        if self.args:
            node["args"] = [a.to_wire() for a in self.args]
        return node


def literal_value(e: Expr) -> Any:
    return e.data["value"]


def encode_literal(t: BType, value: Any) -> Any:
    """Wire value of a Python literal of type `t`; rejects floats and wrong kinds."""
    loc = caller_loc()
    if isinstance(value, float):
        raise BehaviorDefinitionError(
            "float literals are not allowed: use decimal.Decimal for exact numbers", *loc
        )
    if isinstance(t, NominalT):
        return encode_literal(t.underlying, value)
    if isinstance(t, OptionT):
        return None if value is none or value is None else encode_literal(t.of, value)
    if t == BOOL and isinstance(value, bool):
        return value
    if t == INT and isinstance(value, int) and not isinstance(value, bool):
        return value
    if t == DECIMAL and isinstance(value, (Decimal, int)) and not isinstance(value, bool):
        return normalize_decimal(Decimal(value))
    if t == STRING and isinstance(value, str):
        return value
    if isinstance(t, IdT) and isinstance(value, str):
        return value
    if isinstance(t, EnumT) and isinstance(value, Enum) and value.value in t.values:
        return value.value
    raise BehaviorTypeError(f"{value!r} is not a `{t}` literal", "TYPE_MISMATCH", *loc)


def normalize_decimal(d: Decimal) -> str:
    """Normalized plain notation: no exponent, no trailing zeros, `-0` → `0`."""
    if not d.is_finite():
        raise BehaviorDefinitionError(f"{d} is not a finite decimal", *caller_loc())
    if d == 0:
        return "0"
    text = format(d.normalize(), "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text


def lit(t: BType, value: Any) -> Expr:
    return Expr("lit", t, (), {"value": encode_literal(t, value)}, caller_loc())


def infer_literal_type(value: Any) -> BType | None:
    if isinstance(value, bool):
        return BOOL
    if isinstance(value, int):
        return INT
    if isinstance(value, Decimal):
        return DECIMAL
    if isinstance(value, str):
        return STRING
    if isinstance(value, Enum):
        return enum_type(type(value))
    return None


def lift(value: object, expected: BType | None = None) -> Expr:
    """Turns a Python literal into a literal node; `expected` types `none` and ints."""
    if isinstance(value, Expr):
        return value
    loc = caller_loc()
    if isinstance(value, float):
        raise BehaviorDefinitionError(
            "float literals are not allowed: use decimal.Decimal for exact numbers", *loc
        )
    if value is none or value is None:
        if isinstance(expected, OptionT):
            return Expr("lit", expected, (), {"value": None}, loc)
        raise BehaviorTypeError("`none` needs an optional value to compare with", "TYPE_MISMATCH", *loc)
    t = infer_literal_type(value)
    if t is None:
        raise BehaviorTypeError(f"{value!r} cannot be used in behavior", "TYPE_MISMATCH", *loc)
    return Expr("lit", t, (), {"value": encode_literal(t, value)}, loc)


def build(op: str, operands: list[Expr], data: dict[str, Any] | None = None) -> Expr:
    loc = caller_loc()
    result, _ = type_of_op(op, [o.type for o in operands])
    return Expr(op, result, tuple(operands), data, loc)


def binary(op: str, left: object, right: object) -> Expr:
    left_hint = left.type if isinstance(left, Expr) else None
    right_hint = right.type if isinstance(right, Expr) else None
    a = lift(left, right_hint)
    b = lift(right, left_hint)
    return build(op, [a, b])


def unary(op: str, operand: Expr) -> Expr:
    return build(op, [operand])


def _combine(op: str, items: tuple[object, ...]) -> Expr:
    operands: list[Expr] = []
    for item in items:
        e = lift(item)
        # Flatten nested and/or of the same kind: `a & b & c` is one node.
        if e.op == op and e.type == BOOL:
            operands.extend(e.args)
        else:
            operands.append(e)
    return build(op, operands)


def and_(*items: object) -> Expr:
    return _combine("and", items)


def or_(*items: object) -> Expr:
    return _combine("or", items)


def not_(item: object) -> Expr:
    return build("not", [lift(item)])


def wrap(t: NominalT, e: Expr) -> Expr:
    loc = caller_loc()
    result, _ = type_of_op("wrap", [e.type], target=t)
    return Expr("wrap", result, (e,), {"nominal": t.name}, loc)


def underlying(e: Expr) -> Expr:
    """Explicit conversion from a nominal value to its underlying primitive."""
    return build("unwrap", [e])


def param_ref(name: str, t: BType, role: str) -> Expr:
    e = Expr("param", t, (), {"param": name}, caller_loc())
    e.role = role
    return e


def field_ref(param: str, field: str, t: BType, role: str) -> Expr:
    e = Expr("field", t, (), {"param": param, "field": field}, caller_loc())
    e.role = role
    return e


__all__ = [
    "Expr", "none", "lit", "lift", "and_", "or_", "not_", "wrap", "underlying", "PrimT",
    "UNKNOWN",
]
