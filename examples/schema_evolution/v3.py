"""Cultures, schema generation V3: narrowed. Every order has a region."""

from __future__ import annotations

from decimal import Decimal

from behavior import BehaviorModule, Input, Ref, action, constraint, entity, field, set_

from .v2 import Culture, Customer, Money, MediumKind, Status, kill, ph_not_negative  # noqa: F401


@entity
class Order:
    customer = field(Ref[Customer])
    region = field(str)
    qty = field(int)


@action
def set_region(order: Order, *, region: Input[str]):
    set_(order.region, region)


model = BehaviorModule(
    entities=[Customer, Culture, Order],
    constraints=[ph_not_negative],
    actions=[set_region, kill],
)
