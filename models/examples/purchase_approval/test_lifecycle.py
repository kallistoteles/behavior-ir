"""US1: the purchase approval workflow runs end to end (C03–C06), with the invariants of
contracts/reference-workflow.md §2 checked after every commit. Every expectation comes from
expected.json."""

from __future__ import annotations

import pytest

from models.examples.purchase_approval import harness

STEPS = harness.EXPECTED["steps"]


def _last(history, step):  # type: ignore[no-untyped-def]
    return history.step(step)[-1]


@pytest.mark.scenario("C03", level=3)
def test_c03_creating_a_request_commits_a_valid_state(history) -> None:  # type: ignore[no-untyped-def]
    r = _last(history, "1")
    assert (r.result, r.conflict, r.position) == (STEPS["1"]["result"], None,
                                                  STEPS["1"]["position"])
    assert not history.invariant_failures, history.invariant_failures


@pytest.mark.scenario("C04", level=2)
def test_c04_an_eligible_approval_is_committed_and_pending(history) -> None:  # type: ignore[no-untyped-def]
    r = _last(history, "4")
    assert (r.result, r.position) == (STEPS["4"]["result"], STEPS["4"]["position"])
    at = history.store.state_at(STEPS["4"]["position"])
    assert harness.outcome(history.store, history.model, "r1", at) == STEPS["4"]["outcome"]["r1"]


@pytest.mark.scenario("C05", level=2)
def test_c05_two_approvals_reach_approved_and_finalize_emits_one_intent(history) -> None:  # type: ignore[no-untyped-def]
    commit_x4 = [r for r in history.step("7") if r.result is None][0]
    assert (commit_x4.conflict, commit_x4.position) == (None, STEPS["7"]["position"])
    final = _last(history, "10")
    assert (final.result, final.position) == (STEPS["10"]["result"], STEPS["10"]["position"])
    expected = harness.EXPECTED["final"]
    at = history.store.state_at(STEPS["7"]["position"])
    assert harness.outcome(history.store, history.model, "r1", at) == STEPS["7"]["outcome"]["r1"]
    assert harness.status(history.store, "r1") == expected["status"]["r1"]
    assert harness.commands(history.store, history.model) == expected["commands"]


@pytest.mark.scenario("C06", level=2)
def test_c06_a_rejection_reaches_rejected_and_finalize_rejected_commits(history) -> None:  # type: ignore[no-untyped-def]
    for step in ("16", "19"):
        r = _last(history, step)
        assert (r.result, r.position) == (STEPS[step]["result"], STEPS[step]["position"]), step
    at = history.store.state_at(STEPS["16"]["position"])
    assert harness.outcome(history.store, history.model, "r2", at) == STEPS["16"]["outcome"]["r2"]
    assert harness.status(history.store, "r2") == harness.EXPECTED["final"]["status"]["r2"]
    # finalize_rejected emits nothing: the stream still holds only r1's intent.
    assert harness.commands(history.store, history.model) == harness.EXPECTED["final"]["commands"]


@pytest.mark.scenario("C03", level=3, check="H.every_step")
def test_every_step_of_h_matches_its_expectation(history) -> None:  # type: ignore[no-untyped-def]
    """Each operation of H: its result, any commit refusal and the position after it."""
    for r in history.results:
        want = STEPS[r.step]
        if r.kind == "commit":  # the commit of a kept candidate
            assert (r.conflict, r.position) == (want.get("conflict"), want["position"]), r
        elif r.kind == "eval":  # a kept candidate; its commit is checked above
            assert r.result == want["result"], r
        else:  # evaluated and, when allowed, committed
            assert (r.result, r.conflict, r.position) == \
                (want["result"], want.get("conflict"), want["position"]), r
    assert not history.invariant_failures, history.invariant_failures


@pytest.mark.scenario("C05", level=2, check="H.final_state")
def test_the_final_state_matches(history) -> None:  # type: ignore[no-untyped-def]
    final = harness.EXPECTED["final"]
    store, model = history.store, history.model
    assert store.current().position == final["position"]
    assert [list(x) for x in harness.decision_rows(store, model)] == final["decisions"]
    for user, role in final["roles"].items():
        assert store.load("User", user)["value"]["role"] == role
    for budget, state in final["budget"].items():
        assert store.load("Budget", budget)["value"]["status"] == state


@pytest.mark.scenario("C05", level=2, check="H.approval_never_moves_the_request")
def test_approval_decisions_leave_the_request_status_unchanged(history) -> None:  # type: ignore[no-untyped-def]
    """After steps 4, 7 and 16 the request is still InReview (reference-workflow.md §2)."""
    store = history.store
    for step in ("4", "7", "16"):
        want = STEPS[step]["status"]
        position = STEPS[step]["position"]
        at = store.state_at(position)
        for rid, status in want.items():
            assert store.load("PurchaseRequest", rid, at=at)["value"]["status"] == status, step
