"""The feature-506 reference workflow (data-model §1, contract reference-workflow.md): a purchase
approval built only from the published model kinds.

- RequestFlow `purchase_request` (with its embedded StateMachine): create, submit, start_review,
  finalize_approved (requires an APPROVED outcome, emits `RequestApproved`), finalize_rejected.
- StateMachine `budget` over an authored `Budget`: close.
- Approval `purchase_request_approval` over the flow's request entity: `k_A = 2`, `k_R = 1`,
  open while the request is in review, eligible when the actor's role is Approver (a query over
  `User`, so the observation can go stale).
- Hand-written `set_role` and the declared reads `decisions` and `request_outcome`.

Nothing here decides anything: every rule is authored through the model APIs and evaluated by
Core.
"""

from __future__ import annotations

from enum import Enum
from collections.abc import Callable, Sequence
from typing import Any

from behavior import (
    Approval, BehaviorModule, Id, Input, RequestFlow, StateMachine, action, any_, command, emit,
    entity, field, invariant, project, read, select, set_,
)

COMMIT_TIME = "2026-10-16T00:00:00Z"


class Role(Enum):
    REQUESTER = "Requester"
    APPROVER = "Approver"


@entity
class User:
    name = field(str)
    role = field(Role)


class BudgetStatus(Enum):
    OPEN = "Open"
    CLOSED = "Closed"


@entity
class Budget:
    status = field(BudgetStatus)
    limit = field(int)


@invariant
def budget_limit_is_not_negative(budget: Budget):  # type: ignore[no-untyped-def]
    return budget.limit >= 0


class PurchaseRequestStatus(Enum):
    DRAFT = "Draft"
    SUBMITTED = "Submitted"
    IN_REVIEW = "InReview"
    APPROVED = "Approved"
    REJECTED = "Rejected"


S = PurchaseRequestStatus


def flow() -> RequestFlow:
    f = RequestFlow("purchase_request", actor=User, status=S,
                    payload={"title": str, "amount": int}, assignee=True)
    f.create("create", to=S.DRAFT, inputs={"title": str, "amount": int},
             context={"actor": Id[User]},
             values=lambda request_id, amount, title, actor: {
                 "requester": actor, "title": title, "amount": amount, "assignee": None})
    f.transition("submit", from_={S.DRAFT}, to=S.SUBMITTED, context={"actor": Id[User]},
                 requires=[lambda r, actor: actor == r.requester])
    f.transition("start_review", from_={S.SUBMITTED}, to=S.IN_REVIEW,
                 inputs={"reviewer": Id[User]},
                 effects=[lambda r, reviewer: set_(r.assignee, reviewer)])
    return f


#: The flow's generated request entity, at module level so annotations can name it.
PurchaseRequest = flow().request_entity


def _by_context(decision_id: Any, *rest: Any) -> Any:
    """The decision's actor: the context `actor`, the last argument of each capability."""
    return rest[-1]


def approval(f: RequestFlow) -> Approval:
    ap = Approval("purchase_request_approval", subject=f.request_entity, actor=User,
                  approve_threshold=2, reject_threshold=1,
                  open_when=lambda r: r.status == S.IN_REVIEW,
                  eligible=lambda r, who: any_(
                      select(User), lambda u: (u.id == who) & (u.role == Role.APPROVER)))
    ap.approve(actor=_by_context, context={"actor": Id[User]})
    ap.reject(actor=_by_context, context={"actor": Id[User]})
    return ap


def budget_machine() -> StateMachine:
    m = StateMachine("budget", subject=Budget, state_field="status",
                     entry_states={BudgetStatus.OPEN})
    m.transition("close", from_={BudgetStatus.OPEN}, to=BudgetStatus.CLOSED)
    return m


@action
def set_role(user: User, *, role: Input[Role]):  # type: ignore[no-untyped-def]
    set_(user.role, role)


def module(*,
           extra_actions: Callable[[Approval], Sequence[Any]] | None = None) -> BehaviorModule:
    """The admitted reference module; every call authors it afresh. `extra_actions` adds
    hand-written actions built against this build's approval (the VO-005 negative control)."""
    f = flow()
    ap = approval(f)

    @command
    class RequestApproved:
        request = field(Id[f.request_entity])
        by = field(Id[User])

    f.transition("finalize_approved", from_={S.IN_REVIEW}, to=S.APPROVED,
                 context={"actor": Id[User]},
                 requires=[lambda r, actor: ap.approved(r)],
                 emits=[lambda r, actor: emit(RequestApproved, request=r.id, by=actor)])
    f.transition("finalize_rejected", from_={S.IN_REVIEW}, to=S.REJECTED,
                 requires=[lambda r: ap.rejected(r)])
    decision = ap.decision_entity

    @read
    def decisions():  # type: ignore[no-untyped-def]
        return project(select(decision), lambda d: [d.subject, d.actor, d.choice])

    @read
    def request_outcome(request: PurchaseRequest):  # type: ignore[no-untyped-def,valid-type]
        return project(request, lambda r: [r.status, ap.approved(r), ap.rejected(r),
                                           ap.pending(r)])

    return BehaviorModule(
        entities=[User, Budget], enums=[Role, BudgetStatus, S],
        invariants=[budget_limit_is_not_negative],
        actions=[set_role, *(extra_actions(ap) if extra_actions else ())],
        reads=[decisions, request_outcome],
        commands=[RequestApproved], models=[f, budget_machine(), ap])


USERS = (("alice", Role.REQUESTER), ("bob", Role.APPROVER), ("carol", Role.APPROVER),
         ("dave", Role.APPROVER))

#: S₀ (data-model §1): four users, one open budget, no request and no decision.
SEED: list[dict[str, Any]] = [
    *({"entity": "User", "value": {"id": u, "name": u, "role": r.value}} for u, r in USERS),
    {"entity": "Budget", "value": {"id": "b1", "status": "Open", "limit": 1000}},
]
