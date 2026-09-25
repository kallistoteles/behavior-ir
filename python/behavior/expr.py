"""Operator overloading over engine nodes. Nothing here type-checks or evaluates behavior.

Each operation calls the engine's builder, which checks the new node and returns it typed, or
raises; the error is reported at the author's line (research R10, R17).
"""

from __future__ import annotations

import json
from decimal import Decimal
from enum import Enum
from typing import Any

from .errors import BehaviorDefinitionError, BehaviorTypeError
from .location import caller_loc
from .types import BOOL, DECIMAL, INT, STRING, BType, NominalT, enum_type


class _NoneLiteral:
    """The `none` literal of an option; its type comes from the other operand."""

    def __repr__(self) -> str:
        return "none"


none = _NoneLiteral()

_CONTROL_FLOW = (
    "behavior expressions cannot drive Python control flow (`if`, `and`, `or`, `not`, or "
    "chained comparisons like `a < b < c`); use `&`, `|`, `~`, and_(), or_(), not_() instead"
)

# Engine error codes that describe misuse of the DSL rather than an ill-typed expression.
_DEFINITION_CODES = {"EFFECT_ON_READONLY", "RESERVED_NAME", "DECODE_ERROR", "DUPLICATE_NAME"}


def engine_error(err: ValueError, loc: tuple[str, int]) -> Exception:
    """Turns an engine error (`ValueError` with a JSON payload) into a DSL exception."""
    try:
        payload = json.loads(str(err))
        code, message = payload["code"], payload["message"]
    except (ValueError, KeyError, TypeError):
        return BehaviorDefinitionError(str(err), *loc)
    if code in _DEFINITION_CODES:
        return BehaviorDefinitionError(f"{code}: {message}", *loc)
    return BehaviorTypeError(message, code, *loc)


def builder() -> Any:
    from .module import current_session

    session = current_session()
    if session is None:
        raise BehaviorDefinitionError(
            "behavior expressions can only be built inside behavior bodies", *caller_loc()
        )
    return session


def call(method: str, *args: Any, loc: tuple[str, int]) -> Any:
    """Calls a builder method with the author's location, converting engine errors."""
    session = builder()
    try:
        return getattr(session.builder, method)(*args, loc[0], loc[1])
    except ValueError as e:
        raise engine_error(e, loc) from None


class Expr:
    """An engine node plus what the Python layer needs (location, field target, operands)."""

    __slots__ = ("node", "loc", "op", "args", "target")
    __hash__ = None  # type: ignore[assignment]  # `==` builds a node, so expressions are unhashable

    def __init__(
        self,
        node: Any,
        loc: tuple[str, int],
        op: str,
        args: tuple[Expr, ...] = (),
        target: tuple[str, str] | None = None,
    ) -> None:
        self.node = node
        self.loc = loc
        self.op = op
        self.args = args
        self.target = target  # (param, field) for field references

    @property
    def type_json(self) -> dict[str, Any] | None:
        """The engine-assigned type in wire form (None below an unresolved cycle reference)."""
        t = self.node.type_json
        return None if t is None else json.loads(t)

    @property
    def role(self) -> str | None:
        return self.node.role  # type: ignore[no-any-return]

    def __repr__(self) -> str:
        return f"Expr({self.op}: {self.type_json})"

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
        encoded = json.dumps([encode_literal(v) for v in values])
        return Expr(call("in_", self.node, encoded, loc=loc), loc, "in", (self,))

    def is_none(self) -> Expr:
        return build("is_none", [self])

    def is_some(self) -> Expr:
        return build("is_some", [self])

    def value_or(self, default: object) -> Expr:
        return build("value_or", [self, lift(default)])


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


def encode_literal(value: Any) -> Any:
    """JSON form of a Python literal; floats are rejected before they reach the engine."""
    if isinstance(value, float):
        raise BehaviorDefinitionError(
            "float literals are not allowed: use decimal.Decimal for exact numbers", *caller_loc()
        )
    if value is None or value is none:
        return None
    if isinstance(value, Enum):
        return encode_literal(value.value)
    if isinstance(value, Decimal):
        return normalize_decimal(value)
    if isinstance(value, (bool, int, str)):
        return value
    raise BehaviorTypeError(f"{value!r} cannot be used in behavior", "TYPE_MISMATCH", *caller_loc())


def _literal_type(value: Any) -> BType | None:
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


def lit(t: BType, value: Any) -> Expr:
    """A literal of type `t` (e.g. a nominal literal `Money(Decimal("5"))`)."""
    loc = caller_loc()
    builder().declare(t)
    node = call("lit", json.dumps(t.wire()), json.dumps(encode_literal(value)), loc=loc)
    return Expr(node, loc, "lit")


def lift(value: object, expected: dict[str, Any] | None = None) -> Expr:
    """Turns a Python literal into a literal node; `expected` gives `none` its option type."""
    if isinstance(value, Expr):
        return value
    loc = caller_loc()
    encoded = encode_literal(value)
    if encoded is None:
        if expected is None or expected.get("t") != "option":
            raise BehaviorTypeError(
                "`none` needs an optional value to compare with", "TYPE_MISMATCH", *loc
            )
        return Expr(call("lit", json.dumps(expected), "null", loc=loc), loc, "lit")
    t = _literal_type(value)
    if t is None:
        raise BehaviorTypeError(f"{value!r} cannot be used in behavior", "TYPE_MISMATCH", *loc)
    builder().declare(t)
    return Expr(call("lit", json.dumps(t.wire()), json.dumps(encoded), loc=loc), loc, "lit")


def build(op: str, operands: list[Expr]) -> Expr:
    loc = caller_loc()
    node = call("op", op, [o.node for o in operands], loc=loc)
    return Expr(node, loc, op, tuple(operands))


def binary(op: str, left: object, right: object) -> Expr:
    left_hint = left.type_json if isinstance(left, Expr) else None
    right_hint = right.type_json if isinstance(right, Expr) else None
    return build(op, [lift(left, right_hint), lift(right, left_hint)])


def _combine(op: str, items: tuple[object, ...]) -> Expr:
    operands: list[Expr] = []
    for item in items:
        e = lift(item)
        # Flatten nested and/or of the same kind: `a & b & c` is one node.
        if e.op == op:
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
    builder().declare(t)
    return Expr(call("wrap", t.name, e.node, loc=loc), loc, "wrap", (e,))


def underlying(e: Expr) -> Expr:
    """Explicit conversion from a nominal value to its underlying primitive."""
    return build("unwrap", [e])


def param_ref(name: str) -> Expr:
    loc = caller_loc()
    return Expr(call("param", name, loc=loc), loc, "param")


def field_ref(param: str, field: str) -> Expr:
    loc = caller_loc()
    return Expr(call("field", param, field, loc=loc), loc, "field", target=(param, field))
