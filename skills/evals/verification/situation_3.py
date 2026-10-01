"""Domain rules: order amounts are never negative. After placing an order, the customer's order
total is at least the new order's amount."""

from decimal import Decimal

from behavior import (
    BehaviorModule, Id, Input, action, create, ensures, entity, field, nominal, requires, select,
    sum_,
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
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):
    requires(amount >= Money(Decimal("0")))
    create(Order, id=order_id, customer=customer.id, amount=amount)
    ensures(sum_(select(Order).where(lambda o: o.customer == customer.id),
                 lambda o: o.amount) >= amount)


model = BehaviorModule(entities=[Customer, Order], actions=[place_order])
