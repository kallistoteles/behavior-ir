"""Work state authoring, with an atomic effect on a separately bound document."""
from behavior import (
    BehaviorModule, Id, WorkQueue, admit, any_, entity, field, nominal, select, set_,
)

Time = nominal("Time", int, ops={"order"})


@entity
class Worker:
    active = field(bool)


@entity
class Document:
    processed = field(bool)


queue = WorkQueue("document_work", worker=Worker, time=Time, max_attempts=3,
                  payload={"document": Id[Document]})
# This example deliberately allows any trusted host to enqueue; authorization is explicit.
queue.enqueue(authorized=lambda **_: True)
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
               effects=[lambda w, *, doc: set_(doc.processed, True)])
Work = queue.work_entity
model = BehaviorModule(entities=[Worker, Document], models=[queue])
assert admit(model).ok
assert Work.__name__ == "DocumentWork"
assert queue.read_name("ready") == "document_work__ready"
assert queue.read_name("expired") == "document_work__expired"
assert queue.action_name("complete") == "document_work__complete"
assert len(model.model_receipts) == 1 and model.model_receipts[0].model_kind == "work_queue"
