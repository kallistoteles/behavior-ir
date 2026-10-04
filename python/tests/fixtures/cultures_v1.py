"""The Python form of the core's migration conformance module `cultures_v1`
(`tests/fixtures/migration/modules/cultures_v1.json` of the pinned Core Release), for the
equivalence test (feature 011, SC-004)."""

from __future__ import annotations

from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Id, Input, Option, Ref, action, constraint, create, entity, field, nominal,
    remove, set_,
)


class MediumKind(Enum):
    MS = "MS"
    WPM = "WPM"


class Status(Enum):
    ACTIVE = "ACTIVE"
    DEAD = "DEAD"


Money = nominal("Money", Decimal, ops={"add", "order"}, scale=2)
Fine = nominal("Fine", Decimal, ops={"add", "order"}, scale=4)


@entity
class Customer:
    name = field(str)
    email = field(str)


@entity
class Culture:
    medium = field(MediumKind)
    status = field(Status)
    ph = field(int)
    legacy_code = field(str)
    price = field(Money)
    fee = field(Fine)


@entity
class Order:
    customer = field(Ref[Customer])
    region = field(Option[str])
    qty = field(int)


@entity
class AuditNote:
    text = field(str)


@constraint
def ph_not_negative(c: Culture):
    return c.ph >= 0


@action
def set_region(order: Order, *, region: Input[str]):
    set_(order.region, region)


@action
def kill(culture: Culture):
    set_(culture.status, Status.DEAD)


@action
def register_customer(*, customer_id: Input[Id[Customer]], name: Input[str], email: Input[str]):
    create(Customer, id=customer_id, name=name, email=email)


@action
def forget_customer(customer: Customer):
    remove(customer)


model = BehaviorModule(
    entities=[Customer, Culture, Order, AuditNote],
    enums=[MediumKind, Status],
    nominals=[Money, Fine],
    constraints=[ph_not_negative],
    actions=[set_region, kill, register_customer, forget_customer],
)
