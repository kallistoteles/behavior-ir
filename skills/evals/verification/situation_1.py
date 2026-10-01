"""Domain rules: an account balance is never negative. A withdrawal takes money out of an account."""

from decimal import Decimal

from behavior import BehaviorModule, Input, action, constraint, entity, field, nominal, set_

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Account:
    balance = field(Money)


@constraint
def non_negative_balance(a: Account):
    return a.balance >= Money(Decimal("0"))


@action
def withdraw(account: Account, *, amount: Input[Money]):
    set_(account.balance, account.balance - amount)


model = BehaviorModule(entities=[Account], constraints=[non_negative_balance], actions=[withdraw])
