"""US5: replay, integrity, identity, lost acknowledgements and atomic effects (W17, W18, W19,
W21, W23a, W23b, W24)."""

from __future__ import annotations

import copy
import json
from typing import Any

import pytest

from behavior import invoke, replay_behavior, replay_data, replay_invocation

from models.examples.work_queue import harness, workflow
from models.examples.work_queue.harness import claim, complete, enqueue, work


@pytest.mark.scenario("W17", level=4)
def test_both_replays_reproduce_the_reference_history(history: Any) -> None:
    data = replay_data(history.store)
    assert data.ok, data.divergence
    assert data.checked == history.store.current().position
    behavior = replay_behavior(history.store, [history.model])
    assert behavior.ok, behavior.divergence


def _record(history: Any) -> tuple[Any, dict[str, Any]]:
    """A portable record of the reference claim (step 7: worker A, generation 2 → 3), against
    the walked history's parent state of that step (no second walk)."""
    op = next(o for o in harness.HISTORY if o.step == "7")
    parent = history.step("7").position - 1
    rec = invoke(history.model, harness.invocation_document(history.model, op),
                 harness.invocation_snapshot(history.store, history.model, parent))
    return rec, copy.deepcopy(rec.data)


def _detected(model: Any, data: dict[str, Any]) -> bool:
    try:
        return not replay_invocation(model, json.dumps(data, sort_keys=True)).matches
    except Exception:  # noqa: BLE001 - a refusal is detection
        return True


def _set_member(data: dict[str, Any], entity: str, item_id: str, field: str, value: Any) -> None:
    for u in data["outcome"]["record"]["facts"]["universe"]:
        if u["entity"] == entity:
            for m in u["members"]:
                if m["id"] == item_id:
                    m[field] = value


@pytest.mark.scenario("W18", level=4)
def test_altered_lease_evidence_or_context_is_detected(history: Any) -> None:
    model = history.model
    rec, original = _record(history)
    assert rec.data["outcome"]["record"]["result"] == "ALLOW"
    assert replay_invocation(model, rec).matches
    controls = {
        "now": lambda d: d["context"].__setitem__("now", 5),
        "worker": lambda d: d["context"].__setitem__("worker", "Z"),
        "available_at": lambda d: _set_member(d, "DocumentWork", "w1", "available_at", 99),
        "attempts": lambda d: _set_member(d, "DocumentWork", "w1", "attempts", 0),
    }
    missed = []
    for name, change in controls.items():
        data = copy.deepcopy(original)
        change(data)
        if not _detected(model, data):
            missed.append(name)
    assert not missed, missed


@pytest.mark.scenario("W19", level=1)
def test_a_policy_change_changes_the_identities() -> None:
    a, b = workflow.module(), workflow.module(m=4)
    qa = next(r for r in a.model_receipts if r.model_kind == "work_queue")
    qb = next(r for r in b.model_receipts if r.model_kind == "work_queue")
    assert qa.model_hash != qb.model_hash
    assert a.behavior_version != b.behavior_version
    again = next(r for r in workflow.module().model_receipts if r.model_kind == "work_queue")
    assert again.model_hash == qa.model_hash


@pytest.mark.scenario("W21", level=2)
def test_an_incomplete_parent_snapshot_is_rejected(model: Any) -> None:
    store = harness.seeded(model, work("w1", "READY", 0))
    op = claim("c", "w1", "A", 0, now=1, until=9)
    snapshot = json.loads(harness.invocation_snapshot(store, model))
    snapshot["facts"]["universe"] = [u for u in snapshot["facts"]["universe"]
                                     if u["entity"] != "Worker"]
    snapshot["entities"] = [e for e in snapshot["entities"] if e["entity"] != "Worker"]
    rec = invoke(model, harness.invocation_document(model, op), json.dumps(snapshot))
    record = rec.data["outcome"]["record"]
    assert record["result"] == "INVALID_INPUT", record["result"]
    assert [r["code"] for r in record["reasons"]] == ["INVALID_STATE_SNAPSHOT"]


@pytest.mark.scenario("W23a", level=3)
def test_resubmitting_the_original_bundle_recovers_the_original_event(model: Any) -> None:
    store = harness.seeded(model, work("w1", "READY", 0))
    ev = harness.evaluate(store, model, claim("c", "w1", "A", 0, now=1, until=9))
    first = store.commit(model, ev.bundle)
    position = store.current().position
    again = store.commit(model, ev.bundle)  # the acknowledgement was lost; resubmit
    assert again.already and again.record_id == first.record_id
    assert store.current().position == position


@pytest.mark.scenario("W23b", level=3)
def test_re_evaluating_is_a_new_attempt(model: Any) -> None:
    store = harness.fresh_store(model)
    harness.do(store, model, enqueue("1", "w1", "d1", "r1", 0))
    assert harness.do(store, model, enqueue("1", "w1", "d1", "r1", 0)).result == \
        "ENTITY_ID_ALREADY_USED"
    harness.do(store, model, claim("c", "w1", "A", 0, now=1, until=9))
    assert harness.do(store, model, claim("c", "w1", "A", 0, now=1, until=9)).result == "DENY"


@pytest.mark.scenario("W24", level=3)
def test_complete_commits_its_effect_and_command_atomically(model: Any) -> None:
    store = harness.seeded(model, work("w1", "LEASED", 1, "A", 20))
    ev = harness.evaluate(store, model, complete("c", "w1", "A", 1, now=5))
    assert ev.decision.result == "ALLOW"
    changes = {(c.param, c.field) for c in ev.decision.changes}
    assert {("document_work", "status"), ("doc", "processed")} <= changes, changes
    assert [c["payload"] for c in ev.decision.commands] == [{"document": "d1"}]
    # Evaluation executed nothing: no state change, no published command.
    assert store.load("Document", "d1")["value"]["processed"] is False
    assert harness.commands(store, model) == []
    assert harness.commit(store, model, ev) is None
    assert store.load("Document", "d1")["value"]["processed"] is True
    assert harness.commands(store, model) == [{"command": "DocumentProcessed", "document": "d1"}]


@pytest.mark.scenario("W24", level=3, check="W24.no_partial")
def test_a_refused_commit_publishes_nothing(model: Any) -> None:
    store = harness.seeded(model, work("w1", "LEASED", 1, "A", 20), work("w2", "READY", 0))
    ev = harness.evaluate(store, model, complete("c", "w1", "A", 1, now=5))
    harness.do(store, model, claim("x", "w2", "B", 0, now=1, until=9))
    position = store.current().position
    assert harness.commit(store, model, ev) == "STATE_CONFLICT"
    assert store.current().position == position
    assert store.load("Document", "d1")["value"]["processed"] is False
    assert harness.commands(store, model) == []
