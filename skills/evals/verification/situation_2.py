"""Domain rules: no order may exceed its customer's credit limit. A customer's credit limit may
only be raised. Raising it must keep every order within the (new) limit."""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, Input, action, count, ensures, entity, field, nominal, requires, select,
    set_,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


@entity
class Customer:
    credit_limit = field(Money)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)


@action
def raise_limit(customer: Customer, *, limit: Input[Money]):
    over = select(Order).where(lambda o: o.amount > customer.credit_limit)
    requires(limit >= customer.credit_limit)
    requires(count(over) == 0)
    set_(customer.credit_limit, limit)
    ensures(count(over) == 0)


model = BehaviorModule(entities=[Customer, Order], actions=[raise_limit])
