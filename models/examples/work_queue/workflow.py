"""The feature-507 reference application (spec, Reference history): document processing with a
WorkQueue, built only from the published model kinds.

- WorkQueue `document_work`: payload `{document: Id<Document>, request: Id<DocumentRequest>}`,
  `M = 3`, worker `Worker`, time `Time`. Enqueue, recover, exhaust and cancel are authorized for
  an `admin` user (context `actor`); claim is allowed for an active worker. `complete` binds the
  document, marks it processed and emits `DocumentProcessed` atomically with the completion.
- RequestFlow `document_request`: a user submits a document for processing; `submit` emits
  `ProcessDocument`, which an adapter turns into a separate enqueue (asynchronous composition).

Nothing here decides anything: every rule is authored through the model APIs and evaluated by
Core. The queue defines which changes of work state are valid; it schedules and executes nothing.
"""

from __future__ import annotations

from enum import Enum
from typing import Any

from behavior import (
    BehaviorModule, Id, RequestFlow, WorkQueue, any_, command, emit, entity, field, nominal,
    select, set_,
)

COMMIT_TIME = "2026-10-17T00:00:00Z"
M = 3

#: The trusted time domain: whole seconds of the host clock (spec, Time).
Time = nominal("Time", int, ops={"order"})


@entity
class Worker:
    name = field(str)
    active = field(bool)


@entity
class User:
    name = field(str)
    admin = field(bool)


@entity
class Document:
    title = field(str)
    processed = field(bool)


class RequestStatus(Enum):
    OPEN = "Open"
    SUBMITTED = "Submitted"


def flow() -> RequestFlow:
    f = RequestFlow("document_request", actor=User, status=RequestStatus,
                    payload={"document": Id[Document]})
    f.create("open", to=RequestStatus.OPEN, inputs={"document": Id[Document]},
             context={"actor": Id[User]},
             values=lambda request_id, document, actor: {"requester": actor,
                                                         "document": document})
    return f


#: The flow's generated request entity, at module level so annotations can name it.
DocumentRequest = flow().request_entity


def is_admin(actor: Any) -> Any:
    return any_(select(User), lambda u: (u.id == actor) & u.admin)


def is_active(worker: Any) -> Any:
    return any_(select(Worker), lambda x: (x.id == worker) & x.active)


def queue(*, m: int = M, queue_id: str = "document_work",
          extra_payload: dict[str, Any] | None = None) -> WorkQueue:
    q = WorkQueue(queue_id, worker=Worker, time=Time, max_attempts=m,
                  payload={"document": Id[Document], "request": Id[DocumentRequest],
                           **(extra_payload or {})})
    admin = {"actor": Id[User]}
    q.enqueue(authorized=lambda *, actor, **_: is_admin(actor), context=admin)
    q.claim(eligible=lambda w, *, worker, **_: is_active(worker))
    q.renew()
    q.retry()
    q.fail()
    q.recover(authorized=lambda w, *, actor, **_: is_admin(actor), context=admin)
    q.exhaust(authorized=lambda w, *, actor, **_: is_admin(actor), context=admin)
    q.cancel(authorized=lambda w, *, actor, **_: is_admin(actor), context=admin)
    return q


def module(*, m: int = M, extra_actions: Any = None,
           extra_payload: dict[str, Any] | None = None) -> BehaviorModule:
    """The admitted reference module; every call authors it afresh. `extra_actions` adds
    hand-written actions built against this build's queue (negative controls); `extra_payload`
    adds payload fields (a schema change of the work entity, for the migration controls)."""
    f = flow()
    q = queue(m=m, extra_payload=extra_payload)

    @command
    class ProcessDocument:
        request = field(Id[f.request_entity])
        document = field(Id[Document])

    @command
    class DocumentProcessed:
        document = field(Id[Document])

    f.transition("submit", from_={RequestStatus.OPEN}, to=RequestStatus.SUBMITTED,
                 context={"actor": Id[User]},
                 requires=[lambda r, actor: actor == r.requester],
                 emits=[lambda r, actor: emit(ProcessDocument, request=r.id,
                                              document=r.document)])
    q.complete(state={"doc": Document},
               requires=[lambda w, *, doc, **_: doc.id == w.document],
               effects=[lambda w, *, doc, **_: set_(doc.processed, True)],
               emits=[lambda w, *, doc, **_: emit(DocumentProcessed, document=doc.id)])
    return BehaviorModule(
        entities=[Worker, User, Document],
        enums=[RequestStatus],
        actions=list(extra_actions(q) if extra_actions else ()),
        commands=[ProcessDocument, DocumentProcessed],
        models=[f, q],
    )


#: S₀: two active workers, one inactive worker, an admin and a user, two documents.
SEED: list[dict[str, Any]] = [
    {"entity": "Worker", "value": {"id": "A", "name": "a", "active": True}},
    {"entity": "Worker", "value": {"id": "B", "name": "b", "active": True}},
    {"entity": "Worker", "value": {"id": "Z", "name": "z", "active": False}},
    {"entity": "User", "value": {"id": "admin", "name": "admin", "admin": True}},
    {"entity": "User", "value": {"id": "u1", "name": "u1", "admin": False}},
    {"entity": "Document", "value": {"id": "d1", "title": "report", "processed": False}},
    {"entity": "Document", "value": {"id": "d2", "title": "memo", "processed": False}},
]
