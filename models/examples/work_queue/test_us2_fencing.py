"""US2: fence stale and unauthorized workers (W04, W06, W07, W08; the Time boundary)."""

from __future__ import annotations

import json
from typing import Any

import pytest

from models.examples.work_queue import harness
from models.examples.work_queue.report import full_mode
from models.examples.work_queue.harness import Op, W, action, claim, complete, work


def leased(model: Any, attempts: int = 1, owner: str = "A", until: int = 20) -> Any:
    return harness.seeded(model, work("w1", "LEASED", attempts, owner, until))


@pytest.mark.scenario("W04", level=3)
def test_two_claims_from_one_head_commit_exactly_once(model: Any) -> None:
    for i in range(20 if full_mode() else 4):
        store = harness.seeded(model, work("w1", "READY", 0))
        first, second = ("A", "B") if i % 2 == 0 else ("B", "A")
        a = harness.evaluate(store, model, claim("a", "w1", first, 0, now=1, until=9))
        b = harness.evaluate(store, model, claim("b", "w1", second, 0, now=1, until=9))
        assert (a.decision.result, b.decision.result) == ("ALLOW", "ALLOW")
        outcomes = [harness.commit(store, model, a), harness.commit(store, model, b)]
        assert outcomes == [None, "STATE_CONFLICT"], (i, outcomes)
        assert harness.item(store)["owner"] == first and harness.item(store)["attempts"] == 1


@pytest.mark.scenario("W04", level=3, check="W04.moved_head")
def test_a_claim_against_a_moved_head_is_refused(model: Any) -> None:
    store = harness.seeded(model, work("w1", "READY", 0), work("w2", "READY", 0))
    ev = harness.evaluate(store, model, claim("a", "w1", "A", 0, now=1, until=9))
    harness.do(store, model, claim("b", "w2", "B", 0, now=1, until=9))
    assert harness.commit(store, model, ev) == "STATE_CONFLICT"
    assert harness.item(store)["status"] == "READY"


@pytest.mark.scenario("W06", level=2)
def test_another_worker_cannot_complete(model: Any) -> None:
    store = leased(model)
    assert harness.do(store, model, complete("x", "w1", "B", 1, now=5)).result == "DENY"


@pytest.mark.scenario("W07", level=2)
def test_a_stale_generation_is_denied_even_for_the_same_worker(model: Any, history: Any) -> None:
    assert history.step("8").result == "DENY"  # worker B, generation 2, after A's generation 3
    store = leased(model, attempts=1, owner="A", until=20)
    steps = harness.run(
        store, model,
        Op("r", action("recover"), {W: "w1"}, {}, {"now": 20, "available_at": 20,
                                                   "actor": "admin"}),
        claim("c", "w1", "A", 1, now=21, until=30),
        complete("old", "w1", "A", 1, now=22),
        complete("new", "w1", "A", 2, now=23))
    assert [s.result for s in steps] == ["ALLOW", "ALLOW", "DENY", "ALLOW"]


@pytest.mark.scenario("W07", level=2, check="W07.stale_claim")
def test_a_claim_with_a_stale_expected_generation_is_denied(model: Any) -> None:
    store = harness.seeded(model, work("w1", "READY", 1))
    assert harness.do(store, model, claim("x", "w1", "A", 0, now=1, until=9)).result == "DENY"
    assert harness.do(store, model, claim("y", "w1", "A", 1, now=1, until=9)).result == "ALLOW"


@pytest.mark.scenario("W08", level=2)
def test_completion_at_or_after_the_deadline_is_denied(model: Any) -> None:
    for now in (20, 21):
        store = leased(model, until=20)
        assert harness.do(store, model, complete("x", "w1", "A", 1, now=now)).result == "DENY"
    store = leased(model, until=20)
    assert harness.do(store, model, complete("x", "w1", "A", 1, now=19)).result == "ALLOW"


@pytest.mark.scenario("W08", level=3, check="W08.boundary")
def test_the_guarantee_is_relative_to_the_evaluation_time(model: Any) -> None:
    # Spec, Time: a completion evaluated while the lease is live commits later with an unchanged
    # head, whatever the clock says by then; the decision records the evaluation `now`.
    store = leased(model, until=20)
    ev = harness.evaluate(store, model, complete("x", "w1", "A", 1, now=19))
    assert ev.decision.result == "ALLOW"
    assert json.loads(ev.decision.record_json)["context"]["now"] == 19
    assert harness.commit(store, model, ev) is None
    assert harness.item(store)["status"] == "SUCCEEDED"


@pytest.mark.scenario("W13", level=3, check="W13.race")
def test_recover_and_a_late_complete_cannot_both_commit(model: Any) -> None:
    store = leased(model, until=20)
    rec = harness.evaluate(store, model, Op("r", action("recover"), {W: "w1"}, {},
                                            {"now": 25, "available_at": 25, "actor": "admin"}))
    late = harness.evaluate(store, model, complete("c", "w1", "A", 1, now=25))
    assert (rec.decision.result, late.decision.result) == ("ALLOW", "DENY")
    assert harness.commit(store, model, rec) is None
    assert harness.item(store)["status"] == "READY"
