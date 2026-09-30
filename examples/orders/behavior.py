"""Customers, orders and employees decided over sets (feature 007): relational queries in
action conditions and a module invariant over all employees."""

from __future__ import annotations

from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Id, Input, action, all_, any_, constraint, count, create, derived, ensures,
    entity, field, invariant, max_, min_, nominal, not_, remove, requires, select, set_, sum_,
    unique,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)
ZERO = Decimal("0")


class OrderStatus(Enum):
    OPEN = "open"
    CLOSED = "closed"
    BLOCKED = "blocked"


@entity
class Customer:
    name = field(str)
    credit_limit = field(Money)
    region = field(str)


@entity
class Order:
    customer = field(Id[Customer])  # a plain identity: closed orders may outlive their customer
    amount = field(Money)
    status = field(OrderStatus)
    region = field(str)


@entity
class Employee:
    personnel_number = field(str)


@constraint
def non_negative_amount(o: Order):
    return o.amount >= Money(ZERO)


@derived
def open_order_count(customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    return count(orders.where(lambda o: o.status == OrderStatus.OPEN))


# Local invariants describe entities; global invariants describe relations.
@invariant
def personnel_numbers_unique():
    return unique(select(Employee), by=lambda e: e.personnel_number)


@action
def close_customer(customer: Customer):
    requires(open_order_count(customer) == 0)
    remove(customer)


@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(amount >= Money(ZERO))
    requires(sum_(orders, lambda o: o.amount) + amount <= customer.credit_limit)
    create(Order, id=order_id, customer=customer.id, amount=amount, status=OrderStatus.OPEN,
           region=customer.region)
    ensures(sum_(orders, lambda o: o.amount) <= customer.credit_limit)


@action
def check_orders(customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(not_(any_(orders, lambda o: o.status == OrderStatus.BLOCKED)))
    requires(all_(orders, lambda o: o.amount > Money(ZERO)))
    requires(max_(orders, lambda o: o.amount).value_or(Money(ZERO)) <= customer.credit_limit)
    requires(min_(orders, lambda o: o.amount).value_or(Money(ZERO)) >= Money(ZERO))


@action
def raise_limit(customer: Customer, *, limit: Input[Money]):
    over = select(Order).where(lambda o: o.amount > customer.credit_limit)
    requires(limit >= customer.credit_limit)
    requires(count(over) == 0)
    set_(customer.credit_limit, limit)
    ensures(count(over) == 0)


@action
def hire(*, employee_id: Input[Id[Employee]], number: Input[str]):
    requires(not_(any_(select(Employee), lambda e: e.personnel_number == number)))
    create(Employee, id=employee_id, personnel_number=number)


@action
def renumber(employee: Employee, *, number: Input[str]):
    set_(employee.personnel_number, number)


model = BehaviorModule(
    entities=[Customer, Order, Employee],
    derived=[open_order_count],
    invariants=[personnel_numbers_unique],
    constraints=[non_negative_amount],
    actions=[close_customer, place_order, check_orders, raise_limit, hire, renumber],
)
