"""The reference RequestFlow (feature 502, request-flow contract §5): a purchase request with
two creations (`create` → Draft, `import_submitted` → Submitted), so `Q_entry = {Draft,
Submitted}`, and the review cycle with return and resubmit. Authority is authored explicitly:
the requester submits and cancels, the assignee approves."""

from __future__ import annotations

from enum import Enum
from typing import Any

from behavior import (
    BehaviorModule, Id, Input, RequestFlow, StateMachine, action, command, emit, entity, field, set_,
)


class PurchaseRequestStatus(Enum):
    DRAFT = "Draft"
    SUBMITTED = "Submitted"
    IN_REVIEW = "InReview"
    APPROVED = "Approved"
    REJECTED = "Rejected"
    NEEDS_CHANGES = "NeedsChanges"
    CANCELLED = "Cancelled"


S = PurchaseRequestStatus


@entity
class User:
    name = field(str)


def standard(request_id: Any, amount: Any, title: Any, actor: Any) -> dict[str, Any]:
    return {"requester": actor, "title": title, "amount": amount, "assignee": None}


def flow(flow_id: str = "purchase_request") -> RequestFlow:
    f = RequestFlow(
        flow_id, actor=User, status=S, payload={"title": str, "amount": int},
        assignee=True, display_name="Purchase request",
    )
    for creation, target in (("create", S.DRAFT), ("import_submitted", S.SUBMITTED)):
        f.create(
            creation, to=target,
            inputs={"title": str, "amount": int}, context={"actor": Id[User]},
            values=standard,
            requires=[lambda request_id, amount, title, actor: amount > 0],
        )
    f.transition("submit", from_={S.DRAFT, S.NEEDS_CHANGES}, to=S.SUBMITTED,
                 context={"actor": Id[User]},
                 requires=[lambda r, actor: actor == r.requester])
    f.transition("start_review", from_={S.SUBMITTED}, to=S.IN_REVIEW,
                 inputs={"reviewer": Id[User]},
                 effects=[lambda r, reviewer: set_(r.assignee, reviewer)])
    f.transition("approve", from_={S.IN_REVIEW}, to=S.APPROVED,
                 context={"actor": Id[User]},
                 requires=[lambda r, actor: actor == r.assignee])
    f.transition("reject", from_={S.IN_REVIEW}, to=S.REJECTED)
    f.transition("return_for_changes", from_={S.IN_REVIEW}, to=S.NEEDS_CHANGES)
    f.transition("cancel", from_={S.DRAFT, S.SUBMITTED, S.NEEDS_CHANGES}, to=S.CANCELLED,
                 context={"actor": Id[User]},
                 requires=[lambda r, actor: actor == r.requester])
    return f


#: `R_T`, written out for tests: (from, to) for every transition edge.
EDGES = {
    ("Draft", "Submitted"), ("NeedsChanges", "Submitted"), ("Submitted", "InReview"),
    ("InReview", "Approved"), ("InReview", "Rejected"), ("InReview", "NeedsChanges"),
    ("Draft", "Cancelled"), ("Submitted", "Cancelled"), ("NeedsChanges", "Cancelled"),
}
ENTRY = {"create": "Draft", "import_submitted": "Submitted"}


def module(f: RequestFlow | None = None, **kw: Any) -> BehaviorModule:
    return BehaviorModule(entities=[User], enums=[S], models=[f or flow()], **kw)


# --- the compatibility reference cases (request-flow contract §5) ---------------------------

PR = flow().request_entity


@command
class Notify:
    request = field(Id[PR])
    who = field(Id[User])


def notified_flow(flow_id: str = "purchase_request") -> RequestFlow:
    """The reference flow plus an `announced` creation and an `approve` that emit `Notify`."""
    f = flow(flow_id)
    f.create("announced", to=S.DRAFT, inputs={"title": str, "amount": int},
             context={"actor": Id[User]}, values=standard,
             emits=[lambda request_id, amount, title, actor:
                    emit(Notify, request=request_id, who=actor)])
    f._machine._transitions = [t for t in f._machine._transitions
                               if t["transition_id"] != "approve"]
    f.transition("approve", from_={S.IN_REVIEW}, to=S.APPROVED, context={"actor": Id[User]},
                 requires=[lambda r, actor: actor == r.assignee],
                 emits=[lambda r, actor: emit(Notify, request=r.id, who=r.requester)])
    return f


def rf_purchase_request() -> BehaviorModule:
    return module(notified_flow(), commands=[Notify])


def rf_two_flows_local_submit() -> BehaviorModule:
    return BehaviorModule(entities=[User], enums=[S],
                          models=[flow("purchase_request"), flow("leave_request")])


class BudgetState(Enum):
    OPEN = "Open"
    FROZEN = "Frozen"


@entity
class Budget:
    state = field(BudgetState)
    reserved = field(int)


@entity
class Comment:
    on = field(Id[PR])
    text = field(str)


@action
def retitle(r: PR, *, title: Input[str]):  # hand-written behavior over the generated entity
    set_(r.title, title)


def rf_with_standalone_machine() -> BehaviorModule:
    budget = StateMachine("budget", subject=Budget, state_field="state",
                          entry_states={BudgetState.OPEN})
    budget.transition("freeze", from_={BudgetState.OPEN}, to=BudgetState.FROZEN)
    return BehaviorModule(entities=[User, Budget, Comment], enums=[S, BudgetState],
                          models=[flow(), budget], actions=[retitle])
