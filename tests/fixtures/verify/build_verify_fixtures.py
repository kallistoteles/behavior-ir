"""Builds the verification fixture modules in tests/fixtures/verify/ with the DSL.

Run inside the dev shell from the repository root:

    python tests/fixtures/verify/build_verify_fixtures.py

The `.expected.json` files next to them are written by hand.
"""

from __future__ import annotations

import pathlib
from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Context, Id, Input, Option, action, constraint, derived, ensures, entity,
    field, invariant, nominal, requires, rule, set_,
)

OUT = pathlib.Path(__file__).resolve().parent
Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)
# The US4 fixture stores no computed amounts; it keeps its unscaled Money (and its identity).
PlainMoney = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"})


class Status(Enum):
    PENDING = "pending"
    APPROVED = "approved"
    REJECTED = "rejected"


@entity
class Employee:
    role = field(str)
    approval_limit = field(Money)


@entity
class Project:
    budget = field(Money)
    spent = field(Money)


@entity
class Purchase:
    amount = field(Money)
    status = field(Status)
    approved_by = field(Option[Id[Employee]])


@derived
def remaining(project: Project):
    return project.budget - project.spent


@invariant
def within_budget(project: Project):
    return project.spent <= project.budget


def approve_action(check: str | None):  # type: ignore[no-untyped-def]
    @action
    def approve(purchase: Purchase, project: Project, *, actor: Context[Employee]):
        requires(purchase.status == Status.PENDING)
        requires(actor.role == "manager")
        requires(purchase.amount <= actor.approval_limit)
        # Decided when the module is built, not a behavior condition. Both the invariant-shaped
        # check and the `remaining` form are proven: arithmetic is exact (feature 004).
        if check == "invariant":
            requires(project.spent + purchase.amount <= project.budget)
        elif check == "remaining":
            requires(purchase.amount <= remaining(project))
        set_(purchase.status, Status.APPROVED)
        set_(purchase.approved_by, actor.id)
        set_(project.spent, project.spent + purchase.amount)
        ensures(purchase.approved_by == actor.id)

    return approve


@action
def reject(purchase: Purchase, project: Project):
    requires(purchase.status == Status.PENDING)
    set_(purchase.status, Status.REJECTED)


def build(name: str, actions: list, derived_values: list) -> None:  # type: ignore[type-arg]
    model = BehaviorModule(
        entities=[Employee, Project, Purchase],
        derived=derived_values,
        invariants=[within_budget],
        actions=actions,
        root=str(OUT),
    )
    (OUT / f"{name}.json").write_text(model.to_wire_json())


# --- US3: postconditions, evaluation errors, rounding ------------------------------------------


@entity
class Account:
    balance = field(Money)


@action
def deposit(account: Account, *, amount: Input[Money]):
    set_(account.balance, account.balance + amount)
    ensures(account.balance >= Money(Decimal("0")))


@action
def deposit_checked(account: Account, *, amount: Input[Money]):
    requires(account.balance >= Money(Decimal("0")))
    requires(amount >= Money(Decimal("0")))
    set_(account.balance, account.balance + amount)
    ensures(account.balance >= Money(Decimal("0")))


@entity
class Counter:
    n = field(int)


@action
def scale(counter: Counter, *, factor: Input[int]):
    requires(factor > 0)
    set_(counter.n, counter.n * factor)


@action
def scale_bounded(counter: Counter, *, factor: Input[int]):
    requires(factor > 0)
    requires(factor <= 10)
    requires(counter.n >= 0)
    requires(counter.n <= 1000)
    set_(counter.n, counter.n * factor)


@entity
class Ratio:
    a = field(Decimal)
    b = field(Decimal)


@action
def third_tight(ratio: Ratio):
    set_(ratio.a, Decimal("1"))
    set_(ratio.b, Decimal("3"))
    ensures(ratio.a / ratio.b < Decimal("0.3333333333333333333333333334"))


@action
def third_clear(ratio: Ratio):
    set_(ratio.a, Decimal("1"))
    set_(ratio.b, Decimal("3"))
    ensures(ratio.a / ratio.b < Decimal("0.3334"))


@entity
class Pair:
    v = field(Money)
    w = field(Money)


@action
def add_positive(pair: Pair, *, amount: Input[Money]):
    # Exact, so the postcondition holds; what can fail is representability: the sum may leave
    # Money's range (26 integer digits), an evaluation error found by verification.
    requires(amount > Money(Decimal("0")))
    set_(pair.w, pair.v + amount)
    ensures(pair.w > pair.v)


# --- US4: dead actions, redundant preconditions, vacuous rules ---------------------------------


@entity
class Order:
    amount = field(PlainMoney)
    shipped = field(bool)


@constraint
def non_negative_amount(order: Order):
    return order.amount >= PlainMoney(Decimal("0"))


@rule
def has_value(order: Order):
    return order.amount >= PlainMoney(Decimal("0"))


@rule
def large(order: Order):
    return order.amount > PlainMoney(Decimal("1000"))


@action
def impossible(order: Order):
    requires(order.amount > PlainMoney(Decimal("100")))
    requires(order.amount < PlainMoney(Decimal("50")))
    set_(order.shipped, True)


@action
def ship(order: Order):
    requires(order.amount >= PlainMoney(Decimal("0")))
    requires(order.amount < PlainMoney(Decimal("1000")))
    set_(order.shipped, True)


def build_simple(name: str, entities: list, actions: list) -> None:  # type: ignore[type-arg]
    model = BehaviorModule(entities=entities, actions=actions, root=str(OUT))
    (OUT / f"{name}.json").write_text(model.to_wire_json())


def main() -> None:
    build_simple("postcondition", [Account], [deposit, deposit_checked])
    build_simple("overflow", [Counter], [scale, scale_bounded])
    build_simple("rounding", [Ratio], [third_tight, third_clear])
    build_simple("sum_rounding", [Pair], [add_positive])
    dead = BehaviorModule(
        entities=[Order], constraints=[non_negative_amount], derived=[has_value, large],
        actions=[impossible, ship], root=str(OUT),
    )
    (OUT / "dead_and_vacuous.json").write_text(dead.to_wire_json())
    build("purchase", [approve_action(None)], [])
    build("purchase_fixed", [approve_action("invariant")], [])
    build("purchase_remaining", [approve_action("remaining")], [remaining])
    build("unchanged_entity", [approve_action("invariant"), reject], [])


if __name__ == "__main__":
    main()
