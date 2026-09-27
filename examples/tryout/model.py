"""Purchase approval against a project budget: the behavior used by the try-out demo."""

from __future__ import annotations

from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Context, Id, Option, action, derived, ensures, entity, field, invariant,
    nominal, requires, rule, set_,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"})


class Status(Enum):
    PENDING = "pending"
    APPROVED = "approved"


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


@rule
def fits_budget(project: Project):
    return remaining(project) >= Money(Decimal("0"))


@invariant
def within_budget(project: Project):
    return project.spent <= project.budget


@action
def approve(purchase: Purchase, project: Project, *, actor: Context[Employee]):
    requires(purchase.status == Status.PENDING)
    requires(actor.role == "manager")
    requires(purchase.amount <= actor.approval_limit)
    set_(purchase.status, Status.APPROVED)
    set_(purchase.approved_by, actor.id)
    set_(project.spent, project.spent + purchase.amount)
    ensures(purchase.approved_by == actor.id)


model = BehaviorModule(
    entities=[Employee, Project, Purchase],
    derived=[remaining, fits_budget],
    invariants=[within_budget],
    actions=[approve],
)
