"""Actions: preconditions, effects and postconditions, plus creating and removing entities.
Identities are inputs chosen by the host; the engine never invents them."""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, Input, Ref, action, admit, constraint, create, ensures, entity, evaluate,
    exists, field, nominal, not_, referenced, remove, requires, set_,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    name = field(str)


@entity
class Account:
    owner = field(Ref[Customer])
    balance = field(Money)


@constraint
def non_negative_balance(a: Account):
    return a.balance >= Money(Decimal("0"))


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


@action
def remove_customer(customer: Customer):
    requires(not_(referenced(customer.id)))
    remove(customer)


model = BehaviorModule(
    entities=[Customer, Account],
    constraints=[non_negative_balance],
    actions=[open_account, deposit, close_account, remove_customer],
)
assert admit(model).ok

# Plain evaluation takes the complete parent snapshot: every declared entity type.
ada = {"id": "c1", "name": "Ada"}
no_accounts = {"universe": [{"entity": "Customer", "members": [ada]},
                            {"entity": "Account", "members": []}]}

# A creation needs an identity that was never used; history is told so through facts.
opened = evaluate(model, "open_account", state={"owner": ada},
                  input={"account_id": "a1", "initial": Decimal("10.00")}, data_version="1",
                  facts={**no_accounts,
                         "identities": [{"entity": "Account", "id": "a1", "used": False}]})
assert opened.result == "ALLOW", opened.reasons
assert opened.lifecycle[0]["op"] == "create"

reused = evaluate(model, "open_account", state={"owner": ada},
                  input={"account_id": "a1", "initial": Decimal("10.00")}, data_version="1",
                  facts={**no_accounts,
                         "identities": [{"entity": "Account", "id": "a1", "used": True}]})
assert reused.result == "ENTITY_ID_ALREADY_USED"

# A customer still referenced by an account cannot be removed.
with_account = {"universe": [
    {"entity": "Customer", "members": [ada]},
    {"entity": "Account", "members": [{"id": "a1", "owner": "c1", "balance": "0.00"}]},
]}
blocked = evaluate(model, "remove_customer", state={"customer": ada}, data_version="1",
                   facts=with_account)
assert blocked.result == "DENY"
