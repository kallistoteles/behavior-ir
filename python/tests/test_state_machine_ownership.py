"""A state machine cannot be silently bypassed (feature 501, US2: T037)."""

from __future__ import annotations

from typing import Any

import pytest

from behavior import (
    BehaviorModule, Id, Input, ModelBuildError, action, create, remove, requires, set_,
)

from .test_state_machine import Actor, PurchaseRequest, RequestStatus, purchase_request


def build(*actions: Any) -> BehaviorModule:
    return BehaviorModule(
        entities=[Actor, PurchaseRequest], enums=[RequestStatus], actions=list(actions),
        models=[purchase_request()],
    )


def errors(*actions: Any) -> list[dict[str, Any]]:
    with pytest.raises(ModelBuildError) as e:
        build(*actions)
    return e.value.errors


@action
def force_approve(request: PurchaseRequest):
    set_(request.status, RequestStatus.APPROVED)


@action
def edit_amount(request: PurchaseRequest, *, amount: Input[int]):
    requires(request.status == RequestStatus.DRAFT)
    set_(request.amount, amount)


@action
def discard(request: PurchaseRequest):
    remove(request)


def opening(status: Any) -> Any:
    @action
    def open_request(
        *, id: Input[Id[PurchaseRequest]], approver: Input[Id[Actor]],
        initial: Input[RequestStatus],
    ):
        value = initial if status is None else status
        create(PurchaseRequest, id=id, status=value, approver=approver, approved_by=None,
               amount=0)

    return open_request


def test_hand_written_writer_is_rejected() -> None:
    [e] = errors(force_approve)
    assert e["code"] == "UNAUTHORIZED_OWNED_FIELD_WRITE"
    assert (e["action"], e["entity"], e["field"], e["owner"]) == (
        "force_approve", "PurchaseRequest", "status", "purchase_request")


def test_reading_other_writes_and_removal_are_fine() -> None:
    m = build(edit_amount, discard)
    assert m.behavior_version is not None


def test_creation_in_an_entry_state() -> None:
    build(opening(RequestStatus.DRAFT))


def test_creation_in_another_state() -> None:
    [e] = errors(opening(RequestStatus.APPROVED))
    assert (e["code"], e["state"]) == ("ENTRY_STATE_NOT_ALLOWED", "Approved")


def test_creation_from_an_input() -> None:
    [e] = errors(opening(None))
    assert e["code"] == "ENTRY_STATE_NOT_STATIC"
