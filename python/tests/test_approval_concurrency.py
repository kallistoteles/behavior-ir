"""Approval under concurrent decisions through the public Store (feature 503, US1; quickstart 5).

Layer 2 of terminality is the Store's compare-and-set: a decision evaluated on a parent that has
since moved is not committed (`STATE_CONFLICT`), and its retry on the new head meets the layer-1
guards. Nothing here is Approval-specific runtime: it is Core's ordinary commit."""

from __future__ import annotations

import pytest

from behavior import StateConflict
from models.fixtures.approval import Run, build, doc_approval

TERMINAL_GUARD = 1
ONE_PER_ACTOR = 3


def denied_at(ev) -> int:  # type: ignore[no-untyped-def]
    assert ev.decision.result == "DENY", ev.decision.result
    outcomes = [s.outcome for s in ev.decision.trace if s.phase == "precondition"]
    return outcomes.index(False)


def test_stale_commit_conflicts_and_retry_is_denied_by_the_terminal_guard() -> None:
    # A = k_A - 1 and R = k_R - 1: either decision alone would make the outcome terminal.
    r = Run(build(doc_approval(2, 2)))
    assert r.decide("approve", "u1")
    assert r.decide("reject", "u2")
    assert r.outcome() == (1, 1, "PENDING")

    approve = r.evaluate("approve", "u3")
    reject = r.evaluate("reject", "u4")
    assert approve.bundle is not None and reject.bundle is not None

    r.store.commit(r.b.module, approve.bundle)
    head = r.store.current()
    with pytest.raises(StateConflict, match="STATE_CONFLICT"):
        r.store.commit(r.b.module, reject.bundle)
    assert r.store.current() == head, "nothing applied"
    assert r.outcome() == (2, 1, "APPROVED")

    retry = r.evaluate("reject", "u4")
    assert denied_at(retry) == TERMINAL_GUARD
    assert r.outcome() == (2, 1, "APPROVED")


def test_stale_commit_same_actor_parallel_decisions() -> None:
    r = Run(build(doc_approval(3, None)))
    first = r.evaluate("approve", "u1", decision_id="p1")
    second = r.evaluate("approve", "u1", decision_id="p2")
    assert first.bundle is not None and second.bundle is not None

    r.store.commit(r.b.module, first.bundle)
    with pytest.raises(StateConflict, match="STATE_CONFLICT"):
        r.store.commit(r.b.module, second.bundle)

    retry = r.evaluate("approve", "u1", decision_id="p2")
    assert denied_at(retry) == ONE_PER_ACTOR
    assert r.outcome() == (1, 0, "PENDING")
    assert r.decisions() == [("d1", "u1", "APPROVE")]
