"""RequestFlow ownership through the Python binding (feature 502, US2: T027).

Mirrors crates/behavior-models/tests/request_flow_ownership.rs and the initializer tests:
every bypass of the flow is a build error with the model authority's typed payload."""

from __future__ import annotations

from typing import Any

import pytest

from behavior import (
    BehaviorModule, Id, Input, ModelBuildError, Option, RequestFlow, action, create, set_,
)

from models.fixtures.rf_purchase_request import S, User, flow, standard

F = flow()
PR = F.request_entity


def build(*actions: Any, f: Any = None) -> BehaviorModule:
    return BehaviorModule(entities=[User], enums=[S], models=[f or flow()], actions=list(actions))


def errors(*actions: Any, f: Any = None) -> list[dict[str, Any]]:
    with pytest.raises(ModelBuildError) as e:
        build(*actions, f=f)
    return e.value.errors


@action
def sneaky_create(*, id: Input[Id[PR]], u: Input[Id[User]]):
    create(PR, id=id, requester=u, status=S.DRAFT, title="x", amount=1, assignee=None)


@action
def force_approve(r: PR):
    set_(r.status, S.APPROVED)


@action
def steal(r: PR, *, u: Input[Id[User]]):
    set_(r.requester, u)


@action
def retitle(r: PR, *, title: Input[str]):
    set_(r.title, title)


@action
def unassign(r: PR):
    set_(r.assignee, None)


def test_hand_written_creation_of_the_request_is_rejected() -> None:
    assert errors(sneaky_create) == [{
        "code": "UNAUTHORIZED_REQUEST_CREATION",
        "action": "sneaky_create", "entity": "PurchaseRequest", "owner": "purchase_request",
        "message": "`sneaky_create` creates `PurchaseRequest`, which only flow "
                   "`purchase_request` may create",
    }]


def test_hand_written_status_and_requester_writes_are_rejected() -> None:
    assert [e["code"] for e in errors(force_approve)] == ["UNAUTHORIZED_OWNED_FIELD_WRITE"]
    assert [e["code"] for e in errors(steal)] == ["UNAUTHORIZED_REQUESTER_WRITE"]


def test_assignee_and_payload_writes_are_ordinary() -> None:
    build(retitle, unassign)


def test_transition_writes() -> None:
    f = flow()
    f.transition("reassign_requester", from_={S.DRAFT}, to=S.DRAFT,
                 inputs={"u": Id[User]}, effects=[lambda r, u: set_(r.requester, u)])
    assert [e["code"] for e in errors(f=f)] == ["UNAUTHORIZED_REQUESTER_WRITE"]
    f = flow()
    f.transition("force", from_={S.DRAFT}, to=S.SUBMITTED,
                 effects=[lambda r: set_(r.status, S.APPROVED)])
    assert [e["code"] for e in errors(f=f)] == ["TRANSITION_WRITES_OWNED_FIELD"]
    f = flow()
    f.transition("assign", from_={S.SUBMITTED}, to=S.SUBMITTED, inputs={"u": Option[Id[User]]},
                 effects=[lambda r, u: set_(r.assignee, u)])
    build(f=f)


def with_values(values: Any) -> list[str]:
    f = flow()
    f.create("other", to=S.DRAFT, inputs={"title": str, "amount": int},
             context={"actor": Id[User]}, values=values)
    return [e["code"] for e in errors(f=f)]


def test_initializer_rules() -> None:
    def without(field: str) -> Any:
        return lambda *a: {k: v for k, v in standard(*a).items() if k != field}

    assert with_values(without("requester")) == ["INITIALIZER_MISSING_FIELD"]
    assert with_values(without("title")) == ["INITIALIZER_MISSING_FIELD"]
    assert with_values(without("assignee")) == ["INITIALIZER_MISSING_FIELD"]
    assert with_values(lambda *a: {**standard(*a), "colour": "red"}) \
        == ["INITIALIZER_UNKNOWN_FIELD"]
    assert with_values(lambda *a: {**standard(*a), "status": S.APPROVED}) \
        == ["INITIALIZER_SETS_STATUS"]


def test_creation_effects_over_a_request_binding() -> None:
    f = flow()
    f.create("supersede", to=S.DRAFT, inputs={"title": str, "amount": int},
             state={"previous": PR}, context={"actor": Id[User]},
             values=lambda request_id, amount, title, previous, actor:
                 standard(request_id, amount, title, actor),
             effects=[lambda request_id, amount, title, previous, actor:
                      set_(previous.status, S.CANCELLED)])
    assert [e["code"] for e in errors(f=f)] == ["UNAUTHORIZED_OWNED_FIELD_WRITE"]


def leave_flow(*, assignee_in_values: Any = ..., effect: bool = False) -> Any:
    """A flow without assignee support (spec Edge Cases: "Assignee disabled")."""
    f = RequestFlow("leave_request", actor=User, status=S, payload={"title": str})

    def values(request_id: Any, title: Any, actor: Any) -> dict[str, Any]:
        v = {"requester": actor, "title": title}
        return v if assignee_in_values is ... else {**v, "assignee": assignee_in_values}

    f.create("create", to=S.DRAFT, inputs={"title": str}, context={"actor": Id[User]},
             values=values)
    if effect:
        f.transition("assign", from_={S.DRAFT}, to=S.DRAFT, inputs={"u": Option[Id[User]]},
                     effects=[lambda r, u: set_(r.assignee, u)])
    return f


def test_without_assignee_support_assignee_is_an_unknown_field() -> None:
    assert "assignee" not in [n for n, _, _ in leave_flow().request_entity
                              .__behavior_entity__.fields]
    build(f=leave_flow())
    for value in (None, "alice"):
        errs = errors(f=leave_flow(assignee_in_values=value))
        assert [(e["code"], e["field"]) for e in errs] \
            == [("INITIALIZER_UNKNOWN_FIELD", "assignee")], errs
    errs = errors(f=leave_flow(effect=True))
    assert [(e["code"], e.get("core_code"), e["model_ref"]) for e in errs] \
        == [("FRAGMENT_INVALID", "UNKNOWN_FIELD", "leave_request/transition/assign")], errs
