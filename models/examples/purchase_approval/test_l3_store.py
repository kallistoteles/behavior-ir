"""Level 3, Store conformance: contention (C10), stale observations (C11), atomic publication
(FR-013), creation retry (C17), backend conformance (FR-018) and an infrastructure failure that
stays distinguishable from a denial and from invalid input (FR-009)."""

from __future__ import annotations

from typing import Any

import pytest

from behavior import InMemoryBackend, Store, run_conformance
from python.tests.test_store_conformance import DictBackend

from models.examples.purchase_approval import harness, workflow
from models.examples.purchase_approval.report import Unsupported, full_mode

STEPS = harness.EXPECTED["steps"]
SCENARIOS = harness.EXPECTED["scenarios"]


@pytest.mark.scenario("C10", level=3)
def test_c10_two_candidates_on_one_head_commit_at_most_once() -> None:
    model = workflow.module()
    store, _ = harness.run_history(model, stop_before="7")
    x4 = harness.evaluate(store, model, harness.op("7", 0))
    x5 = harness.evaluate(store, model, harness.op("7", 1))
    assert x4.decision.result == x5.decision.result == "ALLOW"
    start = store.current().position
    assert harness.commit(store, model, x4) is None
    after_first = harness.snapshot(store, model)
    want = SCENARIOS["C10"]
    assert harness.commit(store, model, x5) == want["second"]
    assert harness.snapshot(store, model) == after_first  # FR-013: nothing published
    assert store.current().position - start == want["history_delta"]
    again = harness.evaluate(store, model, harness.op("9"))
    assert again.decision.result == want["reevaluated"]  # terminal: APPROVED


@pytest.mark.scenario("C11", level=3)
def test_c11_a_stale_eligibility_observation_authorizes_nothing() -> None:
    model = workflow.module()
    store, _ = harness.run_history(model, stop_before="12")
    candidate = harness.evaluate(store, model, harness.op("12"))
    assert candidate.decision.result == STEPS["12"]["result"]  # dave is still an Approver
    demote = harness.evaluate(store, model, harness.op("13"))
    assert harness.commit(store, model, demote) is None
    before = harness.snapshot(store, model)
    want = SCENARIOS["C11"]
    assert harness.commit(store, model, candidate) == want["commit"]
    assert harness.snapshot(store, model) == before
    assert harness.evaluate(store, model, harness.op("15")).decision.result == want["reevaluated"]


@pytest.mark.scenario("FR-013", level=3, check="FR-013.atomic_publication")
def test_every_refused_attempt_of_h_publishes_nothing(history) -> None:  # type: ignore[no-untyped-def]
    """Every DENY and every refused commit of H left the exact head (position, state identity,
    record) as it was before the attempt (recorded during the walk), and the command stream holds
    only the committed intent."""
    refused = [r for r in history.results if r.unchanged is not None]
    steps = sorted({r.step for r in refused})
    assert steps == ["14", "15", "17", "18", "5", "6", "8", "9"], steps
    assert all(r.unchanged for r in refused), [r for r in refused if not r.unchanged]
    # Commands are published only with a commit record, so an unchanged head published none;
    # the whole stream is checked once: exactly the expected intent, nothing from a refusal.
    assert harness.commands(history.store, history.model) == harness.EXPECTED["final"]["commands"]


@pytest.mark.scenario("C17", level=3)
def test_c17_repeating_store_creation_recovers_the_same_store() -> None:
    """Run on the Python reference backend: its handle is the backend itself. (An
    `InMemoryBackend` handle is copied into each Store, so a second `Store.create` on it starts a
    separate store; its creation recovery is covered by `run_conformance`, FR-018.)"""
    model = workflow.module()
    backend = DictBackend()
    genesis = harness.genesis(model)
    first = Store.create(backend, model, genesis)
    assert harness.evaluate(first, model, harness.op("1")).bundle is not None
    harness.commit(first, model, harness.evaluate(first, model, harness.op("1")))
    again = Store.create(backend, model, genesis)
    assert (again.store_id, again.current()) == (first.store_id, first.current())
    other = harness.genesis(model, [*workflow.SEED,
                                    {"entity": "Budget", "value": {"id": "b2", "status": "Open",
                                                                   "limit": 5}}])
    with pytest.raises(Exception):  # noqa: B017 - Core's refusal type is its own
        Store.create(backend, model, other)


@pytest.mark.scenario("FR-018", level=3)
def test_fr018_the_backend_passes_the_releases_conformance_suite() -> None:
    if not full_mode():
        raise Unsupported("the backend conformance suite runs in full mode", "environment")
    report = run_conformance(InMemoryBackend)
    assert report.ok, report.failed


class FailingBackend(DictBackend):  # type: ignore[misc]
    """The public backend protocol whose atomic commit fails: an infrastructure failure."""

    def commit(self, *args: Any, **kwargs: Any) -> str:
        raise OSError("disk unavailable")


@pytest.mark.scenario("FR-009", level=3, check="FR-009.infrastructure")
def test_fr009_an_infrastructure_failure_is_neither_a_denial_nor_invalid_input() -> None:
    model = workflow.module()
    backend = FailingBackend()
    store = Store.create(backend, model, harness.genesis(model))
    ev = harness.evaluate(store, model, harness.op("1"))
    assert ev.decision.result == "ALLOW"  # the decision itself is not refused
    before = (store.current(), list(store.history()))
    with pytest.raises(Exception) as e:  # noqa: B017
        store.commit(model, ev.bundle)
    assert "disk unavailable" in str(e.value) or "backend" in str(e.value).lower(), e.value
    assert not isinstance(e.value, AssertionError)
    assert (store.current(), list(store.history())) == before  # nothing published
