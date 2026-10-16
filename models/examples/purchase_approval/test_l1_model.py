"""Level 1, model conformance: admission (C01) and the construction-time negative controls
(VO-005, FR-004) and the checked initial state (VO-001).

This file deliberately has no `from __future__ import annotations`: the negative controls define
actions inside test functions, so their annotations must be evaluated eagerly."""

import pytest

from behavior import Approval, BehaviorError, Store, action, set_
from behavior.errors import BehaviorDefinitionError
from behavior.errors import CommitRefused
from models.examples.purchase_approval import harness, workflow

ORDINARY = {"action", "command", "derived", "entity", "enum", "invariant", "read"}


@pytest.mark.scenario("C01", level=1)
def test_c01_the_workflow_is_admitted_with_only_ordinary_items() -> None:
    m = workflow.module()
    admission = m.engine.admission()
    assert admission["ok"], admission["errors"]
    kinds = {k.split(":", 1)[0] for k in admission["items"]}
    assert kinds <= ORDINARY, kinds
    names = {k.split(":", 1)[1] for k in admission["items"]}
    # The three model kinds lowered into ordinary items.
    assert {"purchase_request__finalize_approved", "budget__close",
            "purchase_request_approval__approve", "purchase_request_approval__approved",
            "PurchaseRequestApprovalDecision"} <= names


@pytest.mark.scenario("VO-005", level=1)
def test_vo005_a_hand_written_decision_write_is_refused() -> None:
    def forge(ap: Approval) -> list:  # type: ignore[type-arg]
        decision = ap.decision_entity

        @action
        def overwrite_choice(d: decision):  # type: ignore[valid-type,no-untyped-def]
            set_(d.actor, d.actor)

        return [overwrite_choice]

    with pytest.raises(BehaviorError) as e:
        workflow.module(extra_actions=forge)
    want = harness.EXPECTED["scenarios"]["VO-005"]["error"]
    assert want in str(getattr(e.value, "errors", e.value)), e.value


@pytest.mark.scenario("FR-004", level=1, check="FR-004.references")
def test_fr004_unresolvable_references_are_refused_at_build_time() -> None:
    class NotAnEntity:
        pass

    for subject, actor, offending in ((NotAnEntity, workflow.User, "NotAnEntity"),
                                      (workflow.PurchaseRequest, workflow.Role, "Role")):
        with pytest.raises(BehaviorDefinitionError) as e:
            ap = Approval("bad_approval", subject=subject, actor=actor, approve_threshold=1)
            ap.approve(actor=lambda decision_id, *rest: rest[-1], context={})
            workflow.BehaviorModule(entities=[workflow.User], models=[workflow.flow(), ap])
        # A structured definition error naming the reference that does not resolve to an entity.
        assert "is not an @entity class" in str(e.value) and offending in str(e.value), e.value


@pytest.mark.scenario("VO-001", level=3)
def test_vo001_the_initial_state_is_checked_and_an_invalid_one_refused() -> None:
    m = workflow.module()
    store = harness.fresh_store(m)
    rows = harness.decision_rows(store, m)
    assert harness.unique_per_actor(rows) and harness.thresholds_exclusive(rows)
    bad = [dict(e) for e in workflow.SEED]
    bad[-1] = {"entity": "Budget", "value": {"id": "b1", "status": "Open", "limit": -1}}
    with pytest.raises(CommitRefused) as e:
        Store.create(harness.InMemoryBackend(), m, harness.genesis(m, bad))
    assert harness.EXPECTED["scenarios"]["VO-001"]["error"] in str(e.value)
