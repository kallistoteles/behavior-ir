"""Entity constraints in the DSL: validity of every incoming entity, by role."""

from __future__ import annotations

from decimal import Decimal

import pytest

from behavior import (
    BehaviorDefinitionError, BehaviorModule, Context, action, constraint, entity, evaluate,
    field, nominal, requires,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Employee:
    role = field(str)
    approval_limit = field(Money)


@entity
class Account:
    balance = field(Money)


@constraint
def non_negative_limit(e: Employee):
    return e.approval_limit >= Money(Decimal("0"))


@action
def review(account: Account, *, actor: Context[Employee]):
    requires(actor.role == "manager")


MODEL = BehaviorModule(entities=[Employee, Account], constraints=[non_negative_limit],
                       actions=[review])


def run(limit: object):  # type: ignore[no-untyped-def]
    return evaluate(MODEL, "review", state={"account": {"id": "a1", "balance": Decimal("1")}},
                    context={"actor": {"id": "e1", "role": "manager", "approval_limit": limit}},
                    data_version="1")


def test_valid_context_is_allowed() -> None:
    assert run(Decimal("10")).result == "ALLOW"


def test_invalid_context_is_reported_by_role() -> None:
    d = run(Decimal("-1"))
    assert d.result == "INVALID_CONTEXT"
    assert d.reasons[0]["code"] == "CONSTRAINT_VIOLATED"


def test_constraint_takes_one_parameter() -> None:
    with pytest.raises(BehaviorDefinitionError):
        @constraint
        def bad(a: Employee, b: Employee):
            return a.approval_limit >= b.approval_limit
