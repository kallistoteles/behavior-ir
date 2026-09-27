"""Customers and the accounts that reference them: the behavior behind the lifecycle example
(feature 006). Accounts are created and closed; a customer can be removed once nothing refers to
them."""

from __future__ import annotations

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, Input, Ref, action, constraint, create, ensures, entity, exists, field,
    nominal, not_, referenced, remove, requires, set_,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    name = field(str)


@entity
class Account:
    owner = field(Ref[Customer])  # Id[Customer] + constraint exists(owner)
    balance = field(Money)


@entity
class AuditNote:
    about = field(Id[Customer])  # a plain identity: never blocks a removal
    text = field(str)


@constraint
def non_negative_balance(a: Account):
    return a.balance >= Money(Decimal("0"))


@action
def register_customer(*, customer_id: Input[Id[Customer]], name: Input[str]):
    create(Customer, id=customer_id, name=name)


@action
def open_account(owner: Customer, *, account_id: Input[Id[Account]], initial: Input[Money]):
    requires(initial >= Money(Decimal("0")))
    create(Account, id=account_id, owner=owner.id, balance=initial)
    ensures(exists(account_id))


@action
def deposit(account: Account, *, amount: Input[Money]):
    requires(amount > Money(Decimal("0")))
    set_(account.balance, account.balance + amount)


@action
def close_account(account: Account):
    requires(account.balance == Money(Decimal("0")))
    remove(account)
    ensures(not_(exists(account.id)))


@action
def remove_customer(customer: Customer):
    requires(not_(referenced(customer.id)))
    remove(customer)


model = BehaviorModule(
    entities=[Customer, Account, AuditNote],
    constraints=[non_negative_balance],
    actions=[register_customer, open_account, deposit, close_account, remove_customer],
)
