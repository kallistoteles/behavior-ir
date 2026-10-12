"""A test-only RequestFlow generator (feature 501, US5): it emits a `StateMachine` for the request
lifecycle plus ordinary request creation. It shows that a richer model can be built on
StateMachine without Core changes and without request concepts inside StateMachine."""

from __future__ import annotations

from enum import Enum
from typing import Any

from behavior import (
    BehaviorModule, Id, Input, StateMachine, action, create, entity, field,
)


class RequestState(Enum):
    DRAFT = "Draft"
    SUBMITTED = "Submitted"
    IN_REVIEW = "InReview"
    APPROVED = "Approved"
    REJECTED = "Rejected"
    NEEDS_CHANGES = "NeedsChanges"


@entity
class Request:
    state = field(RequestState)
    title = field(str)


def request_flow(subject: type, state_field: str) -> tuple[StateMachine, Any]:
    """The RequestFlow lifecycle as an ordinary StateMachine, and request creation as an
    ordinary action that starts every request in Draft."""
    s = RequestState
    machine = StateMachine("request", subject=subject, state_field=state_field,
                           entry_states={s.DRAFT})
    machine.transition("submit", from_={s.DRAFT}, to=s.SUBMITTED)
    machine.transition("start_review", from_={s.SUBMITTED}, to=s.IN_REVIEW)
    machine.transition("approve", from_={s.IN_REVIEW}, to=s.APPROVED)
    machine.transition("reject", from_={s.IN_REVIEW}, to=s.REJECTED)
    machine.transition("return_for_changes", from_={s.IN_REVIEW}, to=s.NEEDS_CHANGES)
    machine.transition("resubmit", from_={s.NEEDS_CHANGES}, to=s.SUBMITTED)

    @action
    def create_request(*, id: Input[Id[Request]], title: Input[str]):
        create(Request, id=id, state=RequestState.DRAFT, title=title)

    return machine, create_request


def module() -> BehaviorModule:
    machine, create_request = request_flow(Request, "state")
    return BehaviorModule(entities=[Request], enums=[RequestState], actions=[create_request],
                          models=[machine])
