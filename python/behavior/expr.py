"""Operator overloading over engine nodes. Nothing here type-checks or evaluates behavior.

Each operation calls the engine's builder, which checks the new node and returns it typed, or
raises; the error is reported at the author's line (research R10, R17).
"""

from __future__ import annotations

from decimal import Decimal
from enum import Enum
from typing import Any

from . import _engine
from .errors import BehaviorDefinitionError, BehaviorTypeError
from .location import caller_loc
from .types import BOOL, DECIMAL, INT, STRING, BType, NominalT, Rounding, enum_type


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
_DEFINITION_CODES = {
    "EFFECT_ON_READONLY", "RESERVED_NAME", "DECODE_ERROR", "DUPLICATE_NAME",
    # Reads (feature 010).
    "UNKNOWN_PROJECTION_ITEM", "DUPLICATE_PROJECTION_ITEM", "INVALID_PROJECTION",
    "READ_CALL_NOT_ALLOWED", "DUPLICATE_CAPABILITY",
}


def engine_error(err: Exception, loc: tuple[str, int]) -> Exception:
    """Turns an engine error (`EngineError(code, message)`) into a DSL exception."""
    if not isinstance(err, _engine.EngineError) or len(err.args) != 2:
        return BehaviorDefinitionError(str(err), *loc)
    code, message = err.args
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
    except _engine.EngineError as e:
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
    def type(self) -> _engine.Type | None:
        """The engine-assigned type (None below an unresolved cycle reference)."""
        t: _engine.Type | None = self.node.type
        return t

    @property
    def role(self) -> str | None:
        return self.node.role  # type: ignore[no-any-return]

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
        literals = [encode_literal(v) for v in values]
        return Expr(call("in_", self.node, literals, loc=loc), loc, "in", (self,))

    def is_none(self) -> Expr:
        return build("is_none", [self])

    def is_some(self) -> Expr:
        return build("is_some", [self])

    def value_or(self, default: object) -> Expr:
        return build("value_or", [self, lift(default)])


def encode_literal(value: Any) -> Any:
    """A Python literal as passed to the engine (`none` becomes None). Floats are rejected here
    with a DSL error so the author sees their own line; the engine rejects them too."""
    if isinstance(value, float):
        raise BehaviorDefinitionError(
            "float literals are not allowed: use decimal.Decimal for exact numbers", *caller_loc()
        )
    if value is none:
        return None
    if value is None or isinstance(value, (bool, int, str, Decimal, Enum)):
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
    session = builder()
    session.declare(t)
    node = call("lit", session.engine_type(t), encode_literal(value), loc=loc)
    return Expr(node, loc, "lit")


def lift(value: object, expected: _engine.Type | None = None) -> Expr:
    """Turns a Python literal into a literal node; `expected` gives `none` its option type."""
    if isinstance(value, Expr):
        return value
    loc = caller_loc()
    encoded = encode_literal(value)
    if encoded is None:
        if expected is None or not expected.is_option():
            raise BehaviorTypeError(
                "`none` needs an optional value to compare with", "TYPE_MISMATCH", *loc
            )
        return Expr(call("lit", expected, None, loc=loc), loc, "lit")
    t = _literal_type(value)
    if t is None:
        raise BehaviorTypeError(f"{value!r} cannot be used in behavior", "TYPE_MISMATCH", *loc)
    session = builder()
    session.declare(t)
    return Expr(call("lit", session.engine_type(t), encoded, loc=loc), loc, "lit")


def build(op: str, operands: list[Expr]) -> Expr:
    loc = caller_loc()
    node = call("op", op, [o.node for o in operands], loc=loc)
    return Expr(node, loc, op, tuple(operands))


def binary(op: str, left: object, right: object) -> Expr:
    left_hint = left.type if isinstance(left, Expr) else None
    right_hint = right.type if isinstance(right, Expr) else None
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


def _identity(e: object) -> Expr:
    """An identity expression: an entity parameter stands for its `id`."""
    from .decl import EntityVar

    if isinstance(e, EntityVar):
        return field_ref(e._name, "id")
    return lift(e)


def exists(e: object) -> Expr:
    """`exists(id)` (feature 006): the entity with this identity exists — in the current state
    in preconditions, in the resulting state in postconditions. `id` may be an `Id[T]`, an
    `Option[Id[T]]` (an absent value gives false), or an entity parameter."""
    return build("exists", [_identity(e)])


def referenced(e: object) -> Expr:
    """`referenced(id)` (feature 006): some surviving `Ref` field points at the identity."""
    return build("referenced", [_identity(e)])


def wrap(t: NominalT, e: Expr) -> Expr:
    loc = caller_loc()
    session = builder()
    session.declare(t)
    return Expr(call("wrap", session.nominal_name(t), e.node, loc=loc), loc, "wrap", (e,))


def rescale(e: Any, target: NominalT, rounding: Rounding) -> Expr:
    """Narrows an exact value to the fixed-scale type `target`, rounding once with `rounding`.

    Everything inside is computed exactly; this is the only place a value is rounded.
    """
    loc = caller_loc()
    if not isinstance(target, NominalT) or target.scale is None:
        raise BehaviorDefinitionError("rescale needs a fixed-scale nominal as its target", *loc)
    if not isinstance(rounding, Rounding):
        raise BehaviorDefinitionError(
            "rescale needs an explicit rounding mode, e.g. Rounding.HALF_EVEN", *loc
        )
    arg = lift(e)
    session = builder()
    session.declare(target)
    node = call("rescale", arg.node, session.nominal_name(target), rounding.value, loc=loc)
    return Expr(node, loc, "rescale", (arg,))


def underlying(e: Expr) -> Expr:
    """Explicit conversion from a nominal value to its underlying primitive."""
    return build("unwrap", [e])


def param_ref(name: str) -> Expr:
    loc = caller_loc()
    return Expr(call("param", name, loc=loc), loc, "param")


class DerivedCall(Expr):
    """A call of a derived value: an expression that also names the call (feature 010: a
    projection item is a derived value over the member)."""

    __slots__ = ("call",)

    def __init__(self, node: Any, loc: tuple[str, int], name: str, args: list[str]) -> None:
        super().__init__(node, loc, "derived")
        self.call = (name, list(args))


def field_ref(param: str, field: str) -> Expr:
    loc = caller_loc()
    return Expr(call("field", param, field, loc=loc), loc, "field", target=(param, field))
