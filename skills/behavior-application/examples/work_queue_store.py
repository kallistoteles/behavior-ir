"""A committed work lifecycle and durable intent; no scheduler or physical execution."""
from behavior import (
    BehaviorModule, Id, InMemoryBackend, Store, WorkQueue, any_, command, emit, entity, field,
    nominal, replay_behavior, replay_data, select, set_,
)

Time = nominal("Time", int, ops={"order"})


@entity
class Worker:
    active = field(bool)


@entity
class Document:
    processed = field(bool)


@command
class DocumentProcessed:
    document = field(Id[Document])


queue = WorkQueue("document_work", worker=Worker, time=Time, max_attempts=3,
                  payload={"document": Id[Document]})
queue.enqueue(authorized=lambda **_: True)  # intentionally open to the trusted host
queue.claim(eligible=lambda w, *, worker: any_(select(Worker), lambda x: (x.id == worker) & x.active))
queue.renew()
queue.retry()
queue.fail()
# All nine capabilities must be declared; these administrative operations are disabled here.
queue.recover(authorized=lambda w: False)
queue.exhaust(authorized=lambda w: False)
queue.cancel(authorized=lambda w: False)
queue.complete(state={"doc": Document},
               requires=[lambda w, *, doc: doc.id == w.document],
               effects=[lambda w, *, doc: set_(doc.processed, True)],
               emits=[lambda w, *, doc: emit(DocumentProcessed, document=doc.id)])
model = BehaviorModule(entities=[Worker, Document], models=[queue], commands=[DocumentProcessed])
seed = [{"entity": "Worker", "value": {"id": "worker-1", "active": True}},
        {"entity": "Document", "value": {"id": "doc-1", "processed": False}}]
# Explicit public v2 policy: this example does not require signed governance evidence.
policy = {"format": "behavior.evidence_policy.v2", "require": "none",
          "trusted_authorities": [], "execution_policies": []}
store = Store.create(InMemoryBackend(), model, Store.genesis_v2_for(model, seed, policy))
start = store.current_history()
COMMIT_TIME = "2026-10-17T00:00:00Z"


def commit(capability, *, bindings=None, input=None, context=None):
    ev = store.evaluate(model, queue.action_name(capability), bindings=bindings or {},
                        input=input or {}, context=context or {}, commit_time=COMMIT_TIME)
    assert ev.decision.result == "ALLOW" and ev.bundle is not None, ev.decision.reasons
    store.commit(model, ev.bundle)


commit("enqueue", input={"id": "w1", "document": "doc-1", "available_at": 10})
assert [r["id"] for r in store.read(model, queue.read_name("ready"), context={"now": 10}).value] == ["w1"]
# worker, now and lease_until come from the trusted host, not an untrusted capability intent.
commit("claim", bindings={"document_work": "w1"}, input={"expected_generation": 0},
       context={"worker": "worker-1", "now": 10, "lease_until": 20})
assert store.load("DocumentWork", "w1")["value"]["attempts"] == 1
commit("complete", bindings={"document_work": "w1", "doc": "doc-1"}, input={"generation": 1},
       context={"worker": "worker-1", "now": 11})
assert store.current().position == 3
assert store.load("DocumentWork", "w1")["value"]["status"] == "SUCCEEDED"
assert store.load("Document", "doc-1")["value"]["processed"] is True
stream = store.commands_since({"format": "behavior.command_stream_request.v1",
                               "after": start.as_dict(), "max_records": 10})
assert len(stream.items) == 1
assert stream.items[0]["intent"]["payload"] == {"document": "doc-1"}
assert replay_data(store).ok and replay_behavior(store, [model]).ok
# The intent is durable. A host adapter still owns delivery, deduplication and physical effects.
