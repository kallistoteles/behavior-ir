"""Several models compose deterministically (feature 501, US4: T064)."""

from __future__ import annotations

import pytest

from behavior import BehaviorModule, ModelBuildError, StateMachine, action, set_

from .test_state_machine import Actor, PurchaseRequest, RequestStatus, purchase_request
from models.fixtures.reference_cases import two_machines_same_local_ids


def test_independent_machines_reuse_local_ids() -> None:
    m = two_machines_same_local_ids()
    assert sorted(r.model["model_id"] for r in m.model_receipts) == ["invoice", "quote"]
    assert m.behavior_version == two_machines_same_local_ids().behavior_version


def _build(*models: StateMachine, actions: tuple = ()) -> BehaviorModule:
    return BehaviorModule(entities=[Actor, PurchaseRequest], enums=[RequestStatus],
                          actions=list(actions), models=list(models))


def test_supply_order_does_not_matter() -> None:
    a = purchase_request()
    other = StateMachine("another", subject=PurchaseRequest, state_field="status",
                         entry_states={RequestStatus.DRAFT})
    other.transition("submit", from_={RequestStatus.DRAFT}, to=RequestStatus.SUBMITTED)
    errors = []
    for models in ([a, other], [other, a]):
        with pytest.raises(ModelBuildError) as e:
            _build(*models)
        errors.append(e.value.errors)
    assert errors[0] == errors[1]
    assert [(x["code"], x["models"]) for x in errors[0]] == [
        ("CONFLICTING_OWNERSHIP", ["another", "purchase_request"])]


@action
def purchase_request__reject(request: PurchaseRequest):  # noqa: N802 - collides on purpose
    set_(request.amount, 0)


def test_a_hand_written_name_collision_is_an_error() -> None:
    with pytest.raises(ModelBuildError) as e:
        _build(purchase_request(), actions=(purchase_request__reject,))
    assert [(x["code"], x["name"]) for x in e.value.errors] == [
        ("GENERATED_NAME_COLLISION", "purchase_request__reject")]
