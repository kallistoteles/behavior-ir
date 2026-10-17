"""Reference cases for the WorkQueue model kind (feature 507, research R6, R13). Each case uses one
embedded StateMachine language level: `wq_documents` binds a document in `complete` (embedded
machine 3), `wq_minimal` binds nothing (embedded machine 2), and `wq_two_queues` holds two
independent level-2 queues."""

from __future__ import annotations

from behavior import BehaviorModule, Id, WorkQueue, entity, field, nominal, set_

Time = nominal("Time", int, ops={"order"})


@entity
class Agent:
    name = field(str)


@entity
class Doc:
    processed = field(bool)


def _queue(queue_id: str, *, bind: bool) -> WorkQueue:
    q = WorkQueue(queue_id, worker=Agent, time=Time, max_attempts=3,
                  payload={"doc": Id[Doc]})
    yes = (lambda *a, **_: True)
    q.enqueue(authorized=yes)
    q.claim(eligible=yes)
    q.renew()
    if bind:
        q.complete(state={"target": Doc},
                   effects=[lambda w, *, target, **_: set_(target.processed, True)])
    else:
        q.complete()
    q.retry()
    q.fail()
    q.recover(authorized=yes)
    q.exhaust(authorized=yes)
    q.cancel(authorized=yes)
    return q


def wq_documents() -> BehaviorModule:
    return BehaviorModule(entities=[Agent, Doc], models=[_queue("documents", bind=True)])


def wq_minimal() -> BehaviorModule:
    return BehaviorModule(entities=[Agent, Doc], models=[_queue("jobs", bind=False)])


def wq_two_queues() -> BehaviorModule:
    return BehaviorModule(entities=[Agent, Doc],
                          models=[_queue("jobs", bind=False), _queue("exports", bind=False)])
