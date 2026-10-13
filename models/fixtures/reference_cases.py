"""The reference cases of the compatibility manifest (lowering contract §7). Each builds a
module through the public `behavior` API; the Rust model authority lowers it."""

from __future__ import annotations

from enum import Enum
from typing import Callable

from behavior import BehaviorModule, Id, Option, StateMachine, entity, field, set_

from ..examples.state_machine import machine as order_example
from . import request_flow
from . import rf_purchase_request as rf


class RequestStatus(Enum):
    DRAFT = "Draft"
    SUBMITTED = "Submitted"
    IN_REVIEW = "InReview"
    APPROVED = "Approved"
    REJECTED = "Rejected"


@entity
class Actor:
    name = field(str)


@entity
class PurchaseRequest:
    status = field(RequestStatus)
    approver = field(Id[Actor])
    approved_by = field(Option[Id[Actor]])


def purchase_request() -> BehaviorModule:
    s = RequestStatus
    m = StateMachine("purchase_request", subject=PurchaseRequest, state_field="status",
                     entry_states={s.DRAFT})
    m.transition("submit", from_={s.DRAFT}, to=s.SUBMITTED)
    m.transition("start_review", from_={s.SUBMITTED}, to=s.IN_REVIEW)
    m.transition("approve", from_={s.IN_REVIEW}, to=s.APPROVED, context={"actor": Id[Actor]},
                 requires=[lambda r, actor: actor == r.approver],
                 effects=[lambda r, actor: set_(r.approved_by, actor)])
    m.transition("reject", from_={s.IN_REVIEW}, to=s.REJECTED)
    return BehaviorModule(entities=[Actor, PurchaseRequest], enums=[s], models=[m])


class DocStatus(Enum):
    OPEN = "Open"
    SENT = "Sent"


@entity
class Quote:
    status = field(DocStatus)


@entity
class Invoice:
    status = field(DocStatus)


def two_machines_same_local_ids() -> BehaviorModule:
    machines = []
    for model_id, subject in (("quote", Quote), ("invoice", Invoice)):
        m = StateMachine(model_id, subject=subject, state_field="status",
                         entry_states={DocStatus.OPEN})
        m.transition("submit", from_={DocStatus.OPEN}, to=DocStatus.SENT)
        machines.append(m)
    return BehaviorModule(entities=[Quote, Invoice], enums=[DocStatus], models=machines)


CASES: dict[str, Callable[[], BehaviorModule]] = {
    "order": order_example.module,
    "purchase_request": purchase_request,
    "request_flow_reference": request_flow.module,
    "two_machines_same_local_ids": two_machines_same_local_ids,
    "rf_purchase_request": rf.rf_purchase_request,
    "rf_two_flows_local_submit": rf.rf_two_flows_local_submit,
    "rf_with_standalone_machine": rf.rf_with_standalone_machine,
}
