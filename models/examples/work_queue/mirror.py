"""A hand-authored mirror of the WorkQueue lowering, for the negative controls of
`contracts/safety-arguments.md` (FR-036, research R11).

The mirror is written with ordinary hand-written behavior: an entity, the `ValidWork`
invariant, a StateMachine with the generated guards, effects and postconditions of data model
§3, and enqueue as a hand-written creation. `drop` removes exactly one premise (or a named
group). It is built only by the tests; the shipped lowering has no switch. The mirror has no
WorkQueue ownership pass, which is itself the premise NC-writer removes.
"""

from __future__ import annotations

from enum import Enum
from typing import Any

from behavior import (
    BehaviorModule, Id, Input, Option, StateMachine, action, create, entity, field, invariant,
    none, set_,
)

from models.examples.work_queue.workflow import Time, Worker

M = 3


class MirrorStatus(Enum):
    READY = "READY"
    LEASED = "LEASED"
    SUCCEEDED = "SUCCEEDED"
    FAILED = "FAILED"
    CANCELLED = "CANCELLED"


S = MirrorStatus


@entity
class MirrorWork:
    status = field(S)
    attempts = field(int)
    owner = field(Option[Id[Worker]])
    lease_until = field(Option[Time])
    available_at = field(Time)


def _live(w: Any, now: Any) -> Any:
    return w.lease_until.is_some() & (now < w.lease_until.value_or(now))


def mirror(drop: frozenset[str] = frozenset(), extra: tuple[Any, ...] = ()) -> BehaviorModule:
    """The mirror module without the premises named in `drop`:

    - `invariant_bound`: the `attempts ≤ M` and `READY ⇒ attempts < M` conjuncts of ValidWork;
    - `claim_bound`, `retry_bound`: the `attempts < M` guards;
    - `generation_eq`: complete compares `attempts >= generation` instead of `==`;
    - `live`: complete does not require `now < lease_until`.
    """

    @invariant
    def mirror_valid_work(w: MirrorWork):  # type: ignore[no-untyped-def]
        leased = w.status == S.LEASED
        body = ((w.attempts >= 0)
                & (leased == (w.owner.is_some() & w.lease_until.is_some()))
                & (~leased | (w.attempts >= 1)))
        if "invariant_bound" not in drop:
            body = body & (w.attempts <= M) & (~(w.status == S.READY) | (w.attempts < M))
        return body

    @action
    def mirror_enqueue(*, id: Input[Id[MirrorWork]], available_at: Input[Time]):  # type: ignore[no-untyped-def]  # noqa: A002
        create(MirrorWork, id=id, status=S.READY, attempts=0, owner=none, lease_until=none,
               available_at=available_at)

    m = StateMachine("mirror", subject=MirrorWork, state_field="status", entry_states={S.READY})
    claim_guards = [lambda w, eg, lu, now, worker: (w.available_at <= now)
                    & (w.attempts == eg) & (lu > now)]
    if "claim_bound" not in drop:
        claim_guards.append(lambda w, eg, lu, now, worker: w.attempts < M)
    m.transition("claim", from_={S.READY}, to=S.LEASED, inputs={"expected_generation": int},
                 context={"worker": Id[Worker], "now": Time, "lease_until": Time},
                 requires=claim_guards,
                 effects=[lambda w, eg, lu, now, worker: set_(w.attempts, w.attempts + 1),
                          lambda w, eg, lu, now, worker: set_(w.owner, worker),
                          lambda w, eg, lu, now, worker: set_(w.lease_until, lu)])

    def generation(w: Any, g: Any) -> Any:
        return (w.attempts >= g) if "generation_eq" in drop else (w.attempts == g)

    def current(w: Any, g: Any, now: Any, worker: Any) -> Any:
        c = (w.owner == worker) & generation(w, g)
        return c if "live" in drop else c & _live(w, now)

    m.transition("complete", from_={S.LEASED}, to=S.SUCCEEDED, inputs={"generation": int},
                 context={"worker": Id[Worker], "now": Time},
                 requires=[lambda w, g, now, worker: current(w, g, now, worker)],
                 effects=[lambda w, g, now, worker: set_(w.owner, none),
                          lambda w, g, now, worker: set_(w.lease_until, none)])
    retry_guards = [lambda w, g, at, now, worker: current(w, g, now, worker) & (at >= now)]
    if "retry_bound" not in drop:
        retry_guards.append(lambda w, g, at, now, worker: w.attempts < M)
    m.transition("retry", from_={S.LEASED}, to=S.READY, inputs={"generation": int},
                 context={"worker": Id[Worker], "now": Time, "available_at": Time},
                 requires=retry_guards,
                 effects=[lambda w, g, at, now, worker: set_(w.owner, none),
                          lambda w, g, at, now, worker: set_(w.lease_until, none),
                          lambda w, g, at, now, worker: set_(w.available_at, at)])
    return BehaviorModule(entities=[Worker, MirrorWork], enums=[S],
                          invariants=[mirror_valid_work], actions=[mirror_enqueue, *extra],
                          models=[m])
