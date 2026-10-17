"""The negative controls of `contracts/safety-arguments.md` (FR-036, research R11).

Each control shows that its premise matters: on a variant without the premise the safety
conclusion fails, while the intact construction keeps it. The variants are hand-authored mirrors
of the lowering (`mirror.py`) or real Core migrations on the reference module; the shipped
lowering has no switch.

This file deliberately has no `from __future__ import annotations`: the writer control defines an
action inside a function, so its annotations must be evaluated eagerly."""

from typing import Any

import pytest

from behavior import (
    BehaviorModule, InMemoryBackend, Migration, ModelBuildError, Store, action, set_,
)
from behavior.expr import lit
from behavior.types import IdT, OptionT
from models.examples.work_queue import harness, mirror, workflow
from models.examples.work_queue.harness import complete, work

CT = workflow.COMMIT_TIME


def _store(model: BehaviorModule) -> Store:
    seed = [{"entity": "Worker", "value": {"id": w, "name": w, "active": True}} for w in "AB"]
    return Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))


def _do(store: Store, model: BehaviorModule, act: str, bindings: dict[str, str] | None = None,
        inp: dict[str, Any] | None = None, ctx: dict[str, Any] | None = None) -> str:
    ev = store.evaluate(model, act, bindings=bindings or {}, input=inp or {},
                        context=ctx or {}, commit_time=CT)
    if ev.decision.result == "ALLOW":
        store.commit(model, ev.bundle)
    return str(ev.decision.result)


W = {"mirror_work": "w1"}


def _claim(store: Store, m: BehaviorModule, g: int, now: int, until: int, who: str = "A") -> str:
    return _do(store, m, "mirror__claim", W, {"expected_generation": g},
               {"worker": who, "now": now, "lease_until": until})


def _retry(store: Store, m: BehaviorModule, g: int, now: int, who: str = "A") -> str:
    return _do(store, m, "mirror__retry", W, {"generation": g},
               {"worker": who, "now": now, "available_at": now})


def _complete(store: Store, m: BehaviorModule, g: int, now: int, who: str = "A") -> str:
    return _do(store, m, "mirror__complete", W, {"generation": g}, {"worker": who, "now": now})


def _two_generations(m: BehaviorModule) -> Store:
    """w1 claimed by A twice: generation 2 current, generation 1 stale."""
    store = _store(m)
    assert _do(store, m, "mirror_enqueue", {}, {"id": "w1", "available_at": 0}) == "ALLOW"
    assert [_claim(store, m, 0, 1, 50), _retry(store, m, 1, 2), _claim(store, m, 1, 3, 50)] == \
        ["ALLOW"] * 3
    return store


@pytest.mark.scenario("W16", level=4, check="NC-writer")
def test_without_ownership_a_writer_revalidates_an_old_generation() -> None:
    @action
    def reset(w: mirror.MirrorWork):  # type: ignore[no-untyped-def]
        set_(w.attempts, w.attempts - 1)

    variant = mirror.mirror(extra=(reset,))
    store = _two_generations(variant)
    assert _do(store, variant, "reset", {"w": "w1"}) == "ALLOW"
    assert _complete(store, variant, 1, 4) == "ALLOW"  # the stale token now completes

    def writer(q: Any) -> list[Any]:
        item_type = q.work_entity

        @action
        def reset_attempts(item: item_type):  # type: ignore[valid-type,no-untyped-def]
            set_(item.attempts, item.attempts)

        return [reset_attempts]

    with pytest.raises(ModelBuildError) as e:
        workflow.module(extra_actions=writer)
    assert [x["code"] for x in e.value.errors] == ["UNAUTHORIZED_WORK_WRITE"]


@pytest.mark.scenario("W07", level=4, check="NC-generation")
def test_a_weakened_generation_comparison_lets_an_old_token_complete() -> None:
    intact, variant = mirror.mirror(), mirror.mirror(frozenset({"generation_eq"}))
    assert _complete(_two_generations(intact), intact, 1, 4) == "DENY"
    assert _complete(_two_generations(variant), variant, 1, 4) == "ALLOW"


@pytest.mark.scenario("W08", level=4, check="NC-expiry")
def test_without_the_live_lease_guard_a_late_completion_passes() -> None:
    for drop, want in ((frozenset(), "DENY"), (frozenset({"live"}), "ALLOW")):
        m = mirror.mirror(drop)
        store = _store(m)
        _do(store, m, "mirror_enqueue", {}, {"id": "w1", "available_at": 0})
        _claim(store, m, 0, 1, 20)
        assert _complete(store, m, 1, 25) == want, drop


@pytest.mark.scenario("W12", level=4, check="NC-bound")
def test_the_bound_rests_on_valid_work() -> None:
    def claims(m: BehaviorModule) -> list[str]:
        store = _store(m)
        _do(store, m, "mirror_enqueue", {}, {"id": "w1", "available_at": 0})
        out = []
        for g in range(4):
            out.append(_claim(store, m, g, 10 * g + 1, 10 * g + 5))
            out.append(_retry(store, m, g + 1, 10 * g + 2))
        return out

    intact = claims(mirror.mirror())
    assert intact[:5] == ["ALLOW"] * 5 and "ALLOW" not in intact[5:]  # the third retry is denied
    guards_only = claims(mirror.mirror(frozenset({"claim_bound", "retry_bound"})))
    assert guards_only[6] != "ALLOW"  # ValidWork refuses READY at attempts = M
    unbounded = claims(mirror.mirror(frozenset({"claim_bound", "retry_bound",
                                                 "invariant_bound"})))
    assert unbounded[6] == "ALLOW"  # the fourth claim commits: attempts = 4 > M


def _migrate(store: Store, src: BehaviorModule, change: Any) -> BehaviorModule:
    """A real Core migration that adds a payload field and applies `change(queue)` (a mapping of
    typed values) to every item."""
    target = workflow.module(extra_payload={"priority": int})
    q = workflow.queue()
    m = Migration(source=src, target=target,
                  transforms={q.work_entity: lambda old: {**change(q), "priority": 0}})
    assert m.admit().ok
    store.migrate(m, commit_time=CT)
    return target


@pytest.mark.scenario("W17", level=4, check="NC-mig-reset")
def test_a_migration_that_resets_attempts_revalidates_an_old_generation() -> None:
    src = workflow.module()
    store = harness.seeded(src, work("w1", "LEASED", 3, "A", 100))
    assert harness.do(store, src, complete("x", "w1", "A", 1, now=5)).result == "DENY"
    target = _migrate(store, src, lambda q: {"attempts": 1})
    value = harness.item(store)
    assert harness.valid_work(value)  # Core's validity still holds
    assert harness.do(store, target, complete("x", "w1", "A", 1, now=5)).result == "ALLOW"


@pytest.mark.scenario("W17", level=4, check="NC-mig-reactivate")
def test_a_migration_that_reactivates_a_lease_revalidates_an_old_generation() -> None:
    src = workflow.module()
    # A held generation 2, its lease expired and was recovered: READY with attempts 2.
    store = harness.seeded(src, work("w1", "READY", 2))
    target = _migrate(store, src, lambda q: {
        "status": lit(q.status, "LEASED"), "owner": lit(OptionT(IdT("Worker")), "A"),
        "lease_until": lit(OptionT(q.time), 100)})
    value = harness.item(store)
    assert value["attempts"] == 2 and harness.valid_work(value)
    assert harness.do(store, target, complete("x", "w1", "A", 2, now=5)).result == "ALLOW"
