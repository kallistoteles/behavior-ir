"""The Python form of the core's read conformance module (`tests/fixtures/reads/modules/lab.json`
of the pinned Core Release), for the equivalence test (feature 011, SC-004): the same module,
written through the binding, must admit to the same behavior version and item hashes."""

from __future__ import annotations

from enum import Enum

from behavior import (
    BehaviorModule, Id, Input, Option, Ref, action, constraint, count, create, derived, entity,
    field, min_, project, read, select, set_, sum_,
)


class Stage(Enum):
    SEED = "SEED"
    GROWTH = "GROWTH"
    HARVEST = "HARVEST"


class OrderStatus(Enum):
    OPEN = "OPEN"
    CLOSED = "CLOSED"


@entity
class Culture:
    name = field(str)
    stage = field(Option[Stage])
    active = field(bool)
    measurements = field(int)
    ph_total = field(int)


@entity
class Customer:
    name = field(str)
    credit_limit = field(int)


@entity
class Order:
    customer = field(Ref[Customer])
    amount = field(int)
    status = field(OrderStatus)


@constraint
def measurements_not_negative(c: Culture):
    return c.measurements >= 0


@derived
def ph_avg(c: Culture):
    """The average pH reading; fails for a culture without measurements."""
    return c.ph_total / c.measurements


@derived
def exposure(c: Customer):
    """The total of a customer's open orders."""
    mine = select(Order).where(lambda o: (o.customer == c.id) & (o.status == OrderStatus.OPEN))
    return sum_(mine, lambda o: o.amount)


@derived
def standing(c: Customer):
    """Whether the open orders are within the credit limit (reads the internal limit)."""
    return exposure(c) <= c.credit_limit


@action
def start_culture(*, culture_id: Input[Id[Culture]], name: Input[str]):
    create(Culture, id=culture_id, name=name, stage=None, active=True, measurements=0, ph_total=0)


@action
def measure(culture: Culture, *, ph: Input[int]):
    set_(culture.measurements, culture.measurements + 1)
    set_(culture.ph_total, culture.ph_total + ph)


@action
def advance(culture: Culture, *, stage: Input[Stage]):
    set_(culture.stage, stage)


@action
def retire(culture: Culture):
    set_(culture.active, False)


@action
def register(*, customer_id: Input[Id[Customer]], name: Input[str], limit: Input[int]):
    create(Customer, id=customer_id, name=name, credit_limit=limit)


@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[int]):
    create(Order, id=order_id, customer=customer.id, amount=amount, status=OrderStatus.OPEN)


@action
def close_order(order: Order):
    set_(order.status, OrderStatus.CLOSED)


# --- the questions the lab asks ----------------------------------------------------------------

@read
def active_count():
    return count(select(Culture).where(lambda c: c.active))


@read
def open_total(customer: Customer):
    return exposure(customer)


@read
def smallest_order():
    return min_(select(Order), lambda o: o.amount)


@read
def average_ph():
    return sum_(select(Culture), lambda c: c.ph_total) / count(select(Culture))


@read
def order_view(order: Order):
    """The record of one order (an entity projection: exactly one record)."""
    return project(order, lambda o: [o.status, o.amount])


@read
def active_cultures():
    """The active cultures with their average pH (a query projection: zero or more records)."""
    return project(select(Culture).where(lambda c: c.active),
                   lambda c: [c.name, c.stage, ph_avg(c)])


@read
def customer_summary(customer: Customer):
    """What an agent may know about a customer: the name and whether it is in good standing,
    not the credit limit the standing is computed from."""
    return project(customer, lambda c: [c.name, standing(c)])


@read
def big_orders(*, threshold: Input[int]):
    return project(select(Order).where(lambda o: o.amount >= threshold),
                   lambda o: [o.amount, o.status, o.customer])


model = BehaviorModule(
    entities=[Culture, Customer, Order],
    enums=[Stage, OrderStatus],
    constraints=[measurements_not_negative],
    derived=[ph_avg, exposure, standing],
    actions=[start_culture, measure, advance, retire, register, place_order, close_order],
    reads=[active_count, open_total, order_view, active_cultures, customer_summary, smallest_order,
           average_ph, big_orders],
)
