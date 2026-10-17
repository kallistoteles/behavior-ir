"""WorkQueue authoring through Python (feature 507, T025): the reference queue builds and admits,
build errors surface as typed codes, and the receipt is the model authority's."""

from __future__ import annotations

from typing import Any

import pytest

from behavior import (
    BehaviorModule, Id, InMemoryBackend, ModelBuildError, Store, WorkQueue, entity, field, nominal,
    set_,
)

Time = nominal("Time", int, ops={"order"})
Plain = nominal("Plain", int)
CT = "2026-10-17T00:00:00Z"


@entity
class Worker:
    name = field(str)


@entity
class Document:
    processed = field(bool)


def queue(*, auth: bool = True, time: Any = Time, m: int = 3) -> WorkQueue:
    q = WorkQueue("document_work", worker=Worker, time=time, max_attempts=m,
                  payload={"document": Id[Document]})
    yes = (lambda *a, **_: True) if auth else None
    q.enqueue(authorized=yes)
    q.claim(eligible=yes)
    q.renew()
    q.complete(state={"doc": Document}, effects=[lambda w, doc, **_: set_(doc.processed, True)])
    q.retry()
    q.fail()
    q.recover(authorized=yes)
    q.exhaust(authorized=yes)
    q.cancel(authorized=yes)
    return q


def module(q: WorkQueue) -> BehaviorModule:
    return BehaviorModule(entities=[Worker, Document], models=[q])


def test_the_reference_queue_builds_and_admits() -> None:
    m = module(queue())
    r = m.model_receipts[0]
    assert r.model_kind == "work_queue"
    assert r.model_semantic_version == 1
    assert r.embedded is not None and r.embedded[0]["model_semantic_version"] == 3


def test_build_errors_are_typed() -> None:
    with pytest.raises(ModelBuildError) as e:
        module(queue(auth=False))
    assert {x["code"] for x in e.value.errors} == {"MISSING_AUTHORIZATION"}
    with pytest.raises(ModelBuildError) as e:
        module(queue(time=Plain))
    assert [x["code"] for x in e.value.errors] == ["TIME_NOT_ORDERED"]
    with pytest.raises(ModelBuildError) as e:
        module(queue(m=0))
    assert [x["code"] for x in e.value.errors] == ["INVALID_MAX_ATTEMPTS"]


@pytest.mark.parametrize("capability", ("renew", "complete", "retry", "fail", "recover", "exhaust", "cancel"))
@pytest.mark.parametrize("keyword", ("guards", "require"))
def test_unknown_keywords_cannot_silently_drop_a_guard(capability: str, keyword: str) -> None:
    q = queue()
    original = q._capabilities[capability]
    with pytest.raises(TypeError, match="unexpected keyword"):
        getattr(q, capability)(**{keyword: [lambda w, **_: False]})
    assert q._capabilities[capability] is original


def test_a_short_lifecycle_runs_on_the_store() -> None:
    q = queue()
    m = module(q)
    seed = [{"entity": "Worker", "value": {"id": "A", "name": "a"}},
            {"entity": "Document", "value": {"id": "d1", "processed": False}}]
    st = Store.create(InMemoryBackend(), m, Store.genesis_for(m, seed))

    def do(cap: str, **kw: Any) -> str:
        ev = st.evaluate(m, q.action_name(cap), commit_time=CT, **kw)
        if ev.decision.result == "ALLOW":
            st.commit(m, ev.bundle)
        return str(ev.decision.result)

    assert do("enqueue", bindings={}, input={"id": "w1", "document": "d1", "available_at": 5}) == "ALLOW"
    assert st.read(m, q.read_name("ready"), context={"now": 5}).value == [{"id": "w1"}]
    assert do("claim", bindings={"document_work": "w1"}, input={"expected_generation": 0},
              context={"worker": "A", "now": 5, "lease_until": 10}) == "ALLOW"
    assert do("complete", bindings={"document_work": "w1", "doc": "d1"},
              input={"generation": 1}, context={"worker": "A", "now": 6}) == "ALLOW"
    item = st.load("DocumentWork", "w1")["value"]
    assert (item["status"], item["attempts"], item["owner"]) == ("SUCCEEDED", 1, None)
    assert st.load("Document", "d1")["value"]["processed"] is True
