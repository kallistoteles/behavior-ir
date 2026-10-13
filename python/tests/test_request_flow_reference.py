"""The foundation carries a future RequestFlow without Core changes (feature 501, US5: T066).

A test-only generator emits the request lifecycle as an ordinary StateMachine plus ordinary
request creation; Core runs the whole cycle, including return and resubmit."""

from __future__ import annotations

from typing import Any

from behavior import evaluate

from models.fixtures import request_flow


def test_the_request_lifecycle_runs_end_to_end() -> None:
    m = request_flow.module()
    created = evaluate(m, "create_request", state={}, context={},
                       input={"id": "q1", "title": "Laptop"}, data_version="1",
                       facts={"identities": [{"entity": "Request", "id": "q1", "used": False}]})
    assert created.result == "ALLOW"
    assert created.lifecycle[0]["value"]["state"] == "Draft"

    state = "Draft"
    path = ["submit", "start_review", "return_for_changes", "resubmit", "start_review", "approve"]
    expected = ["Submitted", "InReview", "NeedsChanges", "Submitted", "InReview", "Approved"]
    for transition, target in zip(path, expected):
        request: dict[str, Any] = {"id": "q1", "state": state, "title": "Laptop"}
        d = evaluate(m, f"request__{transition}", state={"request": request}, context={},
                     input={}, data_version="1")
        assert d.result == "ALLOW", transition
        assert [(c.field, c.new) for c in d.changes] == [("state", target)]
        state = target


def test_creation_outside_the_entry_state_would_be_rejected() -> None:
    # The generated create action starts every request in Draft, the only entry state; the
    # model authority checked that statically (a receipt exists only for an accepted build).
    m = request_flow.module()
    [receipt] = m.model_receipts
    assert receipt.model["entry_states"] == ["Draft"]
