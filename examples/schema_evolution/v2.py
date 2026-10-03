"""Cultures, schema generation V2: broadened. `MediumKind` gains `B5`, `medium` is renamed
`medium_type`, `ph` becomes a decimal, `notes` is a new optional field, `legacy_code` is gone,
`Money` widens to four decimals, and `AuditNote` is retired."""

from __future__ import annotations

from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Id, Input, Option, Ref, action, constraint, create, entity, field, nominal, set_,
)


class MediumKind(Enum):
    MS = "MS"
    WPM = "WPM"
    B5 = "B5"


class Status(Enum):
    ACTIVE = "ACTIVE"
    DEAD = "DEAD"


Money = nominal("Money", Decimal, ops={"add", "order"}, scale=4)


@entity
class Customer:
    name = field(str)


@entity
class Culture:
    medium_type = field(MediumKind)
    status = field(Status)
    ph = field(Decimal)
    notes = field(Option[str])
    price = field(Money)


@entity
class Order:
    customer = field(Ref[Customer])
    region = field(Option[str])
    qty = field(int)


@constraint
def ph_not_negative(c: Culture):
    return c.ph >= Decimal("0")


@action
def set_region(order: Order, *, region: Input[str]):
    set_(order.region, region)


@action
def kill(culture: Culture):
    set_(culture.status, Status.DEAD)


@action
def register_customer(*, customer_id: Input[Id[Customer]], name: Input[str]):
    create(Customer, id=customer_id, name=name)


model = BehaviorModule(
    entities=[Customer, Culture, Order],
    constraints=[ph_not_negative],
    actions=[set_region, kill, register_customer],
)
