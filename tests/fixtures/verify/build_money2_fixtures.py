"""Builds the fixed-scale (feature 003) verification fixtures in tests/fixtures/verify/.

Run inside the dev shell from the repository root:

    python tests/fixtures/verify/build_money2_fixtures.py

Kept apart from build_verify_fixtures.py so the feature 002 fixtures keep their bytes. The
`.expected.json` files next to the generated modules are written by hand.
"""

from __future__ import annotations

import pathlib
from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Context, Id, Input, Option, Rounding, action, constraint, derived, ensures,
    entity, field, invariant, nominal, requires, rescale, set_,
)

OUT = pathlib.Path(__file__).resolve().parent
Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


class Status(Enum):
    PENDING = "pending"
    APPROVED = "approved"
    REJECTED = "rejected"


# --- purchase approval with two-decimal Money ----------------------------------------------------


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


@constraint
def non_negative_project(project: Project):
    return (project.budget >= Money(Decimal("0"))) & (project.spent >= Money(Decimal("0")))


@constraint
def non_negative_purchase(purchase: Purchase):
    return purchase.amount >= Money(Decimal("0"))


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
        # Decided when the module is built. With fixed-scale Money both forms are proven.
        if check == "invariant":
            requires(project.spent + purchase.amount <= project.budget)
        elif check == "remaining":
            requires(purchase.amount <= remaining(project))
        set_(purchase.status, Status.APPROVED)
        set_(purchase.approved_by, actor.id)
        set_(project.spent, project.spent + purchase.amount)
        ensures(purchase.approved_by == actor.id)

    return approve


def purchase(check: str | None) -> BehaviorModule:
    return BehaviorModule(
        entities=[Employee, Project, Purchase],
        constraints=[non_negative_project, non_negative_purchase],
        derived=[remaining] if check == "remaining" else [],
        invariants=[within_budget],
        actions=[approve_action(check)],
        root=str(OUT),
    )


# --- the invoice example of feature 001 with two-decimal Money -----------------------------------


class InvoiceStatus(Enum):
    PENDING = "pending"
    APPROVED = "approved"


@entity
class User:
    role = field(str)
    approval_limit = field(Money)


@entity
class Invoice:
    amount = field(Money)
    status = field(InvoiceStatus)
    approved_by = field(Option[Id[User]])


@invariant
def non_negative_amount(invoice: Invoice):
    return invoice.amount >= Money(Decimal("0"))


@action
def approve_invoice(invoice: Invoice, *, actor: Context[User]):
    requires(invoice.status == InvoiceStatus.PENDING)
    requires(actor.role == "manager")
    requires(invoice.amount <= actor.approval_limit)
    set_(invoice.status, InvoiceStatus.APPROVED)
    set_(invoice.approved_by, actor.id)
    ensures(invoice.approved_by == actor.id)


@action
def apply_discount(invoice: Invoice, *, discount: Input[Money]):
    requires(invoice.status == InvoiceStatus.PENDING)
    set_(invoice.amount, invoice.amount - discount)


# --- a discount computed with an explicit rescale -----------------------------------------------


@entity
class Bill:
    amount = field(Money)


@invariant
def non_negative_bill(bill: Bill):
    return bill.amount >= Money(Decimal("0"))


@action
def discount(bill: Bill, *, rate: Input[Decimal]):
    requires(rate >= Decimal("0"))
    requires(rate <= Decimal("1"))
    set_(bill.amount, rescale(bill.amount * (Decimal("1") - rate), Money, Rounding.FLOOR))


@action
def discount_over(bill: Bill, *, rate: Input[Decimal]):
    requires(rate >= Decimal("0"))
    requires(rate <= Decimal("2"))
    set_(bill.amount, rescale(bill.amount * (Decimal("1") - rate), Money, Rounding.FLOOR))


# --- properties that hold only for a specific rounding mode --------------------------------------


@entity
class Split:
    x = field(Money)
    y = field(Money)


@action
def split_floor(split: Split):
    set_(split.y, rescale(split.x / 3, Money, Rounding.FLOOR))
    ensures(split.y * 3 <= split.x)


@action
def split_ceiling(split: Split):
    set_(split.y, rescale(split.x / 3, Money, Rounding.CEILING))
    ensures(split.y * 3 <= split.x)


@action
def split_half_even(split: Split):
    set_(split.y, rescale(split.x / 3, Money, Rounding.HALF_EVEN))
    ensures(split.y * 3 <= split.x + Money(Decimal("0.02")))
    ensures(split.y * 3 >= split.x - Money(Decimal("0.02")))


def write(name: str, model: BehaviorModule) -> None:
    (OUT / f"{name}.json").write_text(model.to_wire_json())


def main() -> None:
    write("purchase_money2", purchase(None))
    write("purchase_money2_remaining", purchase("remaining"))
    write("purchase_money2_fixed", purchase("invariant"))
    write("invoice_money2", BehaviorModule(
        entities=[User, Invoice], invariants=[non_negative_amount],
        actions=[approve_invoice, apply_discount], root=str(OUT)))
    write("discount_rescale", BehaviorModule(
        entities=[Bill], invariants=[non_negative_bill], actions=[discount, discount_over],
        root=str(OUT)))
    write("rescale_modes", BehaviorModule(
        entities=[Split], actions=[split_floor, split_ceiling, split_half_even], root=str(OUT)))


if __name__ == "__main__":
    main()
