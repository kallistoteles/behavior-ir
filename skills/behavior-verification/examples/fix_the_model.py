"""A confirmed counterexample is fixed in the model: add the missing guard. The violated rule is
never removed or weakened to make verification pass."""

from decimal import Decimal

from behavior import (
    BehaviorModule, Input, Profile, action, constraint, entity, field, nominal, requires, set_,
    verify,
)

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


before = verify(BehaviorModule(entities=[Account], constraints=[non_negative_balance],
                               actions=[withdraw]), Profile(checks=["preservation"]))
assert [c["outcome"] for c in before.checks] == ["counterexample"]
finding = before.findings[0]
assert finding["kind"] == "preservation"  # withdraw can break non_negative_balance


# The fix states what withdraw needs; the constraint stays exactly as it was.
@action
def withdraw_guarded(account: Account, *, amount: Input[Money]):
    requires(amount > Money(Decimal("0")))
    requires(amount <= account.balance)
    set_(account.balance, account.balance - amount)


after = verify(BehaviorModule(entities=[Account], constraints=[non_negative_balance],
                              actions=[withdraw_guarded]), Profile(checks=["preservation"]))
assert [c["outcome"] for c in after.checks] == ["proven"]
assert after.verified
