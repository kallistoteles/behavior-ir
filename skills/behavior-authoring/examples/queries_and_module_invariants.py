"""Deciding over sets: queries over one entity type with candidate-local filters, the relational
operators, module invariants over the whole state, and derived values and rules for reuse."""

from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Id, Input, action, admit, all_, any_, count, create, derived, entity, evaluate,
    field, invariant, max_, min_, nominal, not_, remove, requires, rule, select, sum_, unique,
)

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


class OrderStatus(Enum):
    OPEN = "open"
    CLOSED = "closed"
    BLOCKED = "blocked"


@entity
class Customer:
    name = field(str)
    credit_limit = field(Money)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)
    status = field(OrderStatus)


@entity
class Employee:
    personnel_number = field(str)


# Reuse goes through derived values: they return values, never queries.
@derived
def open_order_count(customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    return count(orders.where(lambda o: o.status == OrderStatus.OPEN))


# A rule is a Bool derived value; the verifier warns if it is always true or always false.
@rule
def has_open_orders(customer: Customer):
    return open_order_count(customer) > 0


# A module invariant has no parameters: it describes the whole state.
@invariant
def personnel_numbers_unique():
    return unique(select(Employee), by=lambda e: e.personnel_number)


@action
def close_customer(customer: Customer):
    requires(not_(has_open_orders(customer)))
    remove(customer)


@action
def check_orders(customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(not_(any_(orders, lambda o: o.status == OrderStatus.BLOCKED)))
    requires(all_(orders, lambda o: o.amount > Money(Decimal("0"))))
    requires(sum_(orders, lambda o: o.amount) <= customer.credit_limit)
    requires(max_(orders, lambda o: o.amount).value_or(Money(Decimal("0"))) <= customer.credit_limit)
    requires(min_(orders, lambda o: o.amount).is_some() | (count(orders) == 0))


@action
def hire(*, employee_id: Input[Id[Employee]], number: Input[str]):
    create(Employee, id=employee_id, personnel_number=number)


model = BehaviorModule(
    entities=[Customer, Order, Employee],
    derived=[open_order_count, has_open_orders],
    invariants=[personnel_numbers_unique],
    actions=[close_customer, check_orders, hire],
)
assert admit(model).ok

customer = {"id": "c1", "name": "Ada", "credit_limit": Decimal("100.00")}


def orders(*members: dict[str, str]) -> dict[str, object]:
    # A `universe` lists every existing entity of a type; a store supplies this automatically.
    return {"universe": [{"entity": "Order", "members": list(members)}]}


closed = {"id": "o1", "customer": "c1", "amount": "10.00", "status": "closed"}
still_open = {"id": "o2", "customer": "c1", "amount": "5.00", "status": "open"}
assert evaluate(model, "close_customer", state={"customer": customer}, data_version="1",
                facts=orders(closed)).result == "ALLOW"
denied = evaluate(model, "close_customer", state={"customer": customer}, data_version="1",
                  facts=orders(closed, still_open))
assert denied.result == "DENY"
# The decision records exactly which set it depended on.
assert [q["members"] for q in denied.facts["queries"]] == [[{"id": "o2"}]]

assert evaluate(model, "check_orders", state={"customer": customer}, data_version="1",
                facts=orders()).result == "ALLOW"  # empty set: any=false, all=true, sum=0

# The module invariant refuses a second employee with a taken number, on the resulting state.
employees = {"universe": [{"entity": "Employee", "members": [{"id": "e1", "personnel_number": "N1"}]},
                          {"entity": "Order", "members": []}],
             "identities": [{"entity": "Employee", "id": "e2", "used": False}]}
dup = evaluate(model, "hire", state={}, input={"employee_id": "e2", "number": "N1"},
               data_version="1", facts=employees)
assert dup.result == "DENY" and dup.reasons[0]["code"] == "INVARIANT_VIOLATED"
