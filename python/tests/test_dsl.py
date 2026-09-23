"""Construction-time behavior of the DSL: typed expression trees and immediate errors.

Each error test marks the offending line with `# <- name` and asserts that the error points
at exactly that line of this file.
"""

from __future__ import annotations

import json
import pathlib
from decimal import Decimal
from enum import Enum

import pytest

from behavior import (
    BehaviorModule, Context, Id, Option, action, entity, field, invariant, nominal, requires,
    set_,
)
from behavior.errors import BehaviorDefinitionError, BehaviorTypeError

THIS = pathlib.Path(__file__)
LINES = THIS.read_text().splitlines()

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"})


class Status(Enum):
    PENDING = "pending"
    APPROVED = "approved"


@entity
class User:
    role = field(str)
    approval_limit = field(Money)


@entity
class Project:
    name = field(str)


@entity
class Invoice:
    amount = field(Money)
    status = field(Status)
    approved_by = field(Option[Id[User]])


def line_of(marker: str) -> int:
    for i, text in enumerate(LINES, start=1):
        if text.rstrip().endswith(f"# <- {marker}"):
            return i
    raise AssertionError(marker)


def compile_action(fn: object) -> BehaviorModule:
    return BehaviorModule(entities=[User, Project, Invoice], actions=[fn])


def expect_error(exc: type[Exception], marker: str, build: object) -> Exception:
    with pytest.raises(exc) as info:
        build()  # type: ignore[operator]
    err = info.value
    assert err.file is not None and pathlib.Path(err.file).name == THIS.name, err  # type: ignore[attr-defined]
    assert err.line == line_of(marker), (err, line_of(marker))  # type: ignore[attr-defined]
    return err


def test_comparison_builds_a_tree_not_a_value() -> None:
    @action
    def approve(invoice: Invoice, *, actor: Context[User]):
        requires(invoice.amount <= actor.approval_limit)
        set_(invoice.status, Status.APPROVED)

    wire = json.loads(compile_action(approve).to_wire_json())
    expr = wire["actions"][0]["preconditions"][0]["expr"]
    assert expr["op"] == "le"
    assert [(a["op"], a["param"], a["field"]) for a in expr["args"]] == [
        ("field", "invoice", "amount"),
        ("field", "actor", "approval_limit"),
    ]


def test_money_plus_status_is_a_type_error() -> None:
    @action
    def bad(invoice: Invoice):
        requires(invoice.amount + invoice.status > Money(Decimal("1")))  # <- plus_status
        set_(invoice.status, Status.APPROVED)

    err = expect_error(BehaviorTypeError, "plus_status", lambda: compile_action(bad))
    assert err.code == "TYPE_MISMATCH"  # type: ignore[attr-defined]


def test_money_plus_decimal_is_a_type_error() -> None:
    @action
    def bad(invoice: Invoice):
        set_(invoice.amount, invoice.amount + Decimal("10"))  # <- plus_decimal

    expect_error(BehaviorTypeError, "plus_decimal", lambda: compile_action(bad))


def test_ids_of_different_entities_do_not_mix() -> None:
    @action
    def bad(invoice: Invoice, *, project: Context[Project]):
        requires(invoice.approved_by == project.id)  # <- id_mix
        set_(invoice.status, Status.APPROVED)

    expect_error(BehaviorTypeError, "id_mix", lambda: compile_action(bad))


def test_option_cannot_be_used_as_its_value() -> None:
    @action
    def bad(invoice: Invoice, *, actor: Context[User]):
        requires(invoice.approved_by <= actor.id)  # <- option_value
        set_(invoice.status, Status.APPROVED)

    expect_error(BehaviorTypeError, "option_value", lambda: compile_action(bad))


def test_python_if_on_symbolic_value_fails() -> None:
    @action
    def bad(invoice: Invoice):
        if invoice.amount > Money(Decimal("100")):  # <- if_stmt
            set_(invoice.status, Status.APPROVED)

    expect_error(BehaviorDefinitionError, "if_stmt", lambda: compile_action(bad))


def test_chained_comparison_fails() -> None:
    @action
    def bad(invoice: Invoice, *, actor: Context[User]):
        requires(Money(Decimal("0")) < invoice.amount < actor.approval_limit)  # <- chained
        set_(invoice.status, Status.APPROVED)

    expect_error(BehaviorDefinitionError, "chained", lambda: compile_action(bad))


def test_python_not_fails() -> None:
    @action
    def bad(invoice: Invoice):
        requires(not (invoice.status == Status.PENDING))  # <- py_not
        set_(invoice.status, Status.APPROVED)

    expect_error(BehaviorDefinitionError, "py_not", lambda: compile_action(bad))


def test_context_is_read_only() -> None:
    @action
    def bad(invoice: Invoice, *, actor: Context[User]):
        set_(actor.role, "x")  # <- set_context

    expect_error(BehaviorDefinitionError, "set_context", lambda: compile_action(bad))


def test_id_cannot_change() -> None:
    @action
    def bad(invoice: Invoice):
        set_(invoice.id, invoice.id)  # <- set_id

    expect_error(BehaviorDefinitionError, "set_id", lambda: compile_action(bad))


def test_float_field_is_rejected() -> None:
    def build() -> None:
        @entity
        class Bad:
            ratio = field(float)  # <- float_field

    expect_error(BehaviorDefinitionError, "float_field", build)


def test_field_named_id_is_rejected() -> None:
    def build() -> None:
        @entity
        class Bad:
            id = field(str)  # <- id_field

    expect_error(BehaviorDefinitionError, "id_field", build)


def test_requires_outside_action_fails() -> None:
    def build() -> None:
        requires(True)  # <- outside

    expect_error(BehaviorDefinitionError, "outside", build)


def test_invariant_takes_one_parameter() -> None:
    def build() -> None:
        @invariant  # <- two_params
        def bad(a: Invoice, b: Invoice):
            return a.amount >= b.amount

    with pytest.raises(BehaviorDefinitionError):
        build()


def test_float_literal_is_rejected() -> None:
    @action
    def bad(invoice: Invoice):
        requires(invoice.amount > Money(1.5))  # <- float_literal
        set_(invoice.status, Status.APPROVED)

    expect_error(BehaviorDefinitionError, "float_literal", lambda: compile_action(bad))


def test_unknown_field_is_reported() -> None:
    @action
    def bad(invoice: Invoice):
        requires(invoice.title == "x")  # <- unknown_field
        set_(invoice.status, Status.APPROVED)

    err = expect_error(BehaviorTypeError, "unknown_field", lambda: compile_action(bad))
    assert err.code == "UNKNOWN_FIELD"  # type: ignore[attr-defined]


def test_precondition_must_be_boolean() -> None:
    @action
    def bad(invoice: Invoice):
        requires(invoice.amount)  # <- not_bool
        set_(invoice.status, Status.APPROVED)

    err = expect_error(BehaviorTypeError, "not_bool", lambda: compile_action(bad))
    assert err.code == "NOT_BOOLEAN"  # type: ignore[attr-defined]
