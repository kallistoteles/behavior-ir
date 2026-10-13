"""Reference input of the worked example (feature 501, FR-036): an `Order` state machine
authored with `StateMachine`. The Rust model authority lowers it; this file holds no lowering."""

from __future__ import annotations

from enum import Enum

from behavior import BehaviorModule, StateMachine, entity, field


class OrderStatus(Enum):
    DRAFT = "DRAFT"
    SUBMITTED = "SUBMITTED"


@entity
class Order:
    status = field(OrderStatus)


def order_machine() -> StateMachine:
    machine = StateMachine(
        "order", subject=Order, state_field="status", entry_states={OrderStatus.DRAFT}
    )
    machine.transition("submit", from_={OrderStatus.DRAFT}, to=OrderStatus.SUBMITTED)
    return machine


def module() -> BehaviorModule:
    return BehaviorModule(entities=[Order], enums=[OrderStatus], models=[order_machine()])
