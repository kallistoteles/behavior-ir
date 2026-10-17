"""US4: the lifecycle cannot be bypassed (W16, W22, W25; the asynchronous RequestFlow
composition).

This file deliberately has no `from __future__ import annotations`: the negative cases define
actions inside test functions, so their annotations must be evaluated eagerly."""

from typing import Any

import pytest

from behavior import (
    BehaviorModule, ModelBuildError, WorkQueue, action, entity, field, remove, set_,
)
from models.examples.work_queue import harness, workflow


def codes(fn: Any) -> list[str]:
    with pytest.raises(ModelBuildError) as e:
        workflow.module(extra_actions=fn)
    return sorted({x["code"] for x in e.value.errors})


@pytest.mark.scenario("W16", level=1)
def test_every_forbidden_writer_fails_the_build() -> None:
    def writer(field_name: str) -> Any:
        def make(q: WorkQueue) -> list[Any]:
            item_type = q.work_entity

            @action
            def tamper(item: item_type):  # type: ignore[valid-type,no-untyped-def]
                # A self-assignment is still a write of the owned field.
                set_(getattr(item, field_name), getattr(item, field_name))

            return [tamper]
        return make

    for name in ("attempts", "available_at", "owner", "lease_until", "document", "request"):
        got = codes(writer(name))
        assert got == ["UNAUTHORIZED_WORK_WRITE"], (name, got)

    def remover(q: WorkQueue) -> list[Any]:
        item_type = q.work_entity

        @action
        def purge(item: item_type):  # type: ignore[valid-type,no-untyped-def]
            remove(item)

        return [purge]

    assert codes(remover) == ["UNAUTHORIZED_WORK_REMOVAL"]

    def status_writer(q: WorkQueue) -> list[Any]:
        item_type = q.work_entity

        @action
        def reopen(item: item_type):  # type: ignore[valid-type,no-untyped-def]
            set_(item.status, item.status)

        return [reopen]

    assert codes(status_writer) == ["UNAUTHORIZED_OWNED_FIELD_WRITE"]


@pytest.mark.scenario("W16", level=1, check="W16.binding")
def test_a_capability_cannot_bind_its_own_work_entity() -> None:
    q = workflow.queue()
    q.complete()
    q.renew(state={"other": q.work_entity})
    with pytest.raises(ModelBuildError) as e:
        BehaviorModule(entities=[workflow.Worker, workflow.User, workflow.Document],
                       models=[workflow.flow(), q])
    assert [x["code"] for x in e.value.errors] == ["WORK_ITEM_BINDING"]


@pytest.mark.scenario("W22", level=1)
def test_a_colliding_name_fails_composition() -> None:
    def clash(q: WorkQueue) -> list[Any]:
        @action
        def document_work__claim(doc: workflow.Document):  # type: ignore[no-untyped-def]
            set_(doc.processed, True)

        return [document_work__claim]

    with pytest.raises(ModelBuildError) as e:
        workflow.module(extra_actions=clash)
    assert "document_work__claim" in str(e.value.errors), e.value.errors

    @entity
    class DocumentWork:
        note = field(str)

    with pytest.raises(Exception) as e:
        BehaviorModule(entities=[workflow.Worker, workflow.User, workflow.Document,
                                 DocumentWork], models=[workflow.flow(), workflow.queue()])
    assert "DocumentWork" in str(getattr(e.value, "errors", e.value))


@pytest.mark.scenario("W25", level=3)
def test_two_queues_are_independent() -> None:
    a = workflow.queue(queue_id="queue_a")
    b = workflow.queue(queue_id="queue_b", m=2)
    for q in (a, b):
        q.complete()
    m = BehaviorModule(entities=[workflow.Worker, workflow.User, workflow.Document],
                       models=[workflow.flow(), a, b])
    receipts = {r.model["queue_id"]: r for r in m.model_receipts if r.model_kind == "work_queue"}
    assert receipts["queue_a"].model_hash != receipts["queue_b"].model_hash
    store = harness.fresh_store(m)

    def run(q: str, cap: str, b_: dict[str, str], i: dict[str, Any], c: dict[str, Any]) -> str:
        ev = store.evaluate(m, f"{q}__{cap}", bindings=b_, input=i, context=c,
                            commit_time=workflow.COMMIT_TIME)
        if ev.decision.result == "ALLOW":
            store.commit(m, ev.bundle)
        return str(ev.decision.result)

    for q, wid in (("queue_a", "a1"), ("queue_b", "b1")):
        assert run(q, "enqueue", {}, {"id": wid, "document": "d1", "request": "r1",
                                       "available_at": 0}, {"actor": "admin"}) == "ALLOW"
    assert run("queue_a", "claim", {"queue_a": "a1"}, {"expected_generation": 0},
               {"worker": "A", "now": 1, "lease_until": 9}) == "ALLOW"
    a1, b1 = store.load("QueueA", "a1")["value"], store.load("QueueB", "b1")["value"]
    assert (a1["status"], b1["status"]) == ("LEASED", "READY")
    assert harness.valid_work(a1, 3) and harness.valid_work(b1, 2)

    # Queue b's claim cannot bind and write queue a's items.
    c = workflow.queue(queue_id="queue_c")
    c.complete()
    d = workflow.queue(queue_id="queue_d")
    d.complete()
    d.renew(state={"other": c.work_entity},
            effects=[lambda w, *, other, **_: set_(other.attempts, 0)])
    with pytest.raises(ModelBuildError) as e:
        BehaviorModule(entities=[workflow.Worker, workflow.User, workflow.Document],
                       models=[workflow.flow(), c, d])
    assert [x["code"] for x in e.value.errors] == ["UNAUTHORIZED_WORK_WRITE"]


@pytest.mark.scenario("W24", level=3, check="W24.async_flow")
def test_a_request_asks_for_work_and_an_adapter_enqueues_it(history: Any) -> None:
    # US4 AS6: the flow's submit emits ProcessDocument; the enqueue is a separate commit.
    submit, enqueued = history.step("0b"), history.step("1")
    assert (submit.result, enqueued.result) == ("ALLOW", "ALLOW")
    assert enqueued.position == submit.position + 1
    cmd = history.commands[0]
    assert cmd == {"command": "ProcessDocument", "request": "r1", "document": "d1"}
    assert harness.item(history.store)["request"] == cmd["request"]
