"""Accounts and transfers: the behavior behind the persistence example (feature 005)."""

from __future__ import annotations

from decimal import Decimal

from behavior import BehaviorModule, action, constraint, derived, entity, field, nominal, requires, set_
from behavior.types import Input

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Account:
    active = field(bool)
    balance = field(Money)


@constraint
def non_negative_balance(a: Account):
    return a.balance >= Money(Decimal("0"))


@derived
def available(account: Account):
    return account.balance


@action
def transfer(from_: Account, to: Account, *, amount: Input[Money]):
    requires(amount > Money(Decimal("0")))
    requires(from_.active & (available(from_) >= amount))
    set_(from_.balance, from_.balance - amount)
    set_(to.balance, to.balance + amount)


@action
def freeze(account: Account):
    set_(account.active, False)


model = BehaviorModule(
    entities=[Account], constraints=[non_negative_balance], derived=[available],
    actions=[transfer, freeze],
)
