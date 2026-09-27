"""Invoice approval model (contracts/python-api.md) used by the DSL tests."""

from __future__ import annotations

from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Context, Id, Input, Option, action, ensures, entity, field, invariant,
    nominal, requires, set_,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


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


def build_model(**overrides) -> BehaviorModule:
    parts = dict(
        entities=[User, Invoice],
        invariants=[non_negative_amount],
        actions=[approve_invoice, apply_discount],
    )
    parts.update(overrides)
    return BehaviorModule(**parts)


model = build_model()
