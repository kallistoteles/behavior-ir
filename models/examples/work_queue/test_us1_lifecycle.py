"""US1: author a queue and run work to completion (W01, W02, W03, W05; FR-024, FR-029)."""

from __future__ import annotations

from typing import Any

import pytest

from models.examples.work_queue import harness
from models.examples.work_queue.harness import EXPECTED, claim, enqueue


def _item_matches(actual: dict[str, Any] | None, want: dict[str, Any]) -> bool:
    return actual is not None and all(actual[k] == v for k, v in want.items())


@pytest.mark.scenario("W01", level=1)
def test_a_built_queue_has_a_receipt_and_enqueue_creates_ready_work(history: Any) -> None:
    r = history.model.model_receipts
    receipt = next(x for x in r if x.model_kind == "work_queue")
    assert receipt.model_hash.startswith("sha256:") and receipt.behavior_hash
    assert receipt.embedded and receipt.embedded[0]["kind"] == "state_machine"
    assert receipt.source_map["entries"]
    step = history.step("1")
    assert step.result == "ALLOW" and _item_matches(step.item, EXPECTED["steps"]["1"]["item"])


@pytest.mark.scenario("W01", level=2, check="W01.reuse")
def test_a_used_id_is_refused(model: Any) -> None:
    store = harness.fresh_store(model)
    first = harness.do(store, model, enqueue("1", "w1", "d1", "r1", 10))
    again = harness.do(store, model, enqueue("1", "w1", "d2", "r1", 0))
    assert (first.result, again.result) == ("ALLOW", "ENTITY_ID_ALREADY_USED")
    assert harness.item(store)["document"] == "d1"


@pytest.mark.scenario("W02", level=2)
def test_work_is_not_eligible_before_its_time(model: Any) -> None:
    store = harness.fresh_store(model)
    harness.do(store, model, enqueue("1", "w1", "d1", "r1", 10))
    reads = EXPECTED["reads"]
    for key in ("ready_before_available", "ready_when_available"):
        assert harness.ready(store, model, reads[key]["now"]) == reads[key]["ids"], key
    early = harness.do(store, model, claim("x", "w1", "A", 0, now=5, until=20))
    assert early.result == "DENY"


@pytest.mark.scenario("W02", level=2, check="W02.read_only")
def test_reading_does_not_reserve_work(model: Any) -> None:
    store = harness.fresh_store(model)
    harness.do(store, model, enqueue("1", "w1", "d1", "r1", 10))
    head = store.current().position
    assert harness.ready(store, model, 10) == harness.ready(store, model, 10) == ["w1"]
    assert store.current().position == head
    a = harness.evaluate(store, model, claim("a", "w1", "A", 0, now=10, until=20))
    b = harness.evaluate(store, model, claim("b", "w1", "B", 0, now=10, until=20))
    assert (a.decision.result, b.decision.result) == ("ALLOW", "ALLOW")
    assert harness.commit(store, model, a) is None
    assert harness.commit(store, model, b) == "STATE_CONFLICT"


@pytest.mark.scenario("W03", level=2)
def test_an_eligible_worker_claims_generation_one(history: Any) -> None:
    step = history.step("3")
    assert step.result == "ALLOW" and _item_matches(step.item, EXPECTED["steps"]["3"]["item"])


@pytest.mark.scenario("W03", level=2, check="W03.ineligible")
def test_an_inactive_worker_cannot_claim(model: Any) -> None:
    store = harness.fresh_store(model)
    harness.do(store, model, enqueue("1", "w1", "d1", "r1", 0))
    assert harness.do(store, model, claim("x", "w1", "Z", 0, now=1, until=9)).result == "DENY"


@pytest.mark.scenario("W05", level=2)
def test_the_owner_completes_with_the_current_generation(history: Any) -> None:
    step = history.step("9")
    assert step.result == "ALLOW" and _item_matches(step.item, EXPECTED["steps"]["9"]["item"])


@pytest.mark.scenario("W05", level=3, check="W05.history")
def test_every_reference_step_has_its_expected_outcome(history: Any) -> None:
    bad = []
    for r in history.results:
        want = EXPECTED["steps"][r.step]
        if r.result != want["result"] or r.position != want["position"] or r.conflict:
            bad.append((r.step, r.result, r.position, r.conflict))
        if "item" in want and not _item_matches(r.item, want["item"]):
            bad.append((r.step, r.item))
    assert not bad, bad
    assert not history.invalid, history.invalid
    final = EXPECTED["final"]
    assert history.store.current().position == final["position"]
    assert _item_matches(harness.item(history.store), final["item"])
    for d, processed in final["documents"].items():
        assert history.store.load("Document", d)["value"]["processed"] is processed
    assert history.store.load("DocumentRequest", "r1")["value"]["status"] == final["request_status"]
    assert history.commands == final["commands"]


@pytest.mark.scenario("W01", level=2, check="W01.unauthorized")
def test_enqueue_needs_its_authorization(model: Any) -> None:
    store = harness.fresh_store(model)
    r = harness.do(store, model, enqueue("1", "w1", "d1", "r1", 10, actor="u1"))
    assert r.result == "DENY"
