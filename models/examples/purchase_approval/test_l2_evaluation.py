"""Level 2, evaluation conformance: refusals at their boundary (C07, C08, C09, C09.input) and
the child-state laws (C12). A business denial (DENY), invalid input (INVALID_INPUT) and an
infrastructure failure (an exception, test_l3_store.py) stay distinguishable."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

import pytest

from behavior import evaluate

from models.examples.purchase_approval import harness, workflow
from models.examples.purchase_approval.report import Unsupported, report_dir

STEPS = harness.EXPECTED["steps"]
SCENARIOS = harness.EXPECTED["scenarios"]


def _refused_without_trace(prefix: Any, step: str) -> None:
    """Attempts `step` on a store that walked H up to it, and checks DENY with nothing
    changed."""
    model, store = prefix(step)
    before = harness.snapshot(store, model)
    ev = harness.evaluate(store, model, harness.op(step))
    assert ev.decision.result == STEPS[step]["result"]
    assert ev.bundle is None
    assert harness.snapshot(store, model) == before


@pytest.mark.scenario("C07", level=2)
def test_c07_a_second_decision_by_one_approver_is_denied_without_trace(prefix: Any) -> None:
    _refused_without_trace(prefix, "5")


@pytest.mark.scenario("C08", level=2)
def test_c08_an_ineligible_actor_is_denied_without_trace(prefix: Any) -> None:
    _refused_without_trace(prefix, "6")


def _direct(store: Any, model: Any, drop: str | None) -> Any:
    """Step 4 evaluated directly against the store's own complete parent snapshot, optionally
    with the entity type `drop` removed from the universe."""
    o = harness.op("4")
    snap = json.loads(harness.invocation_snapshot(store, model, o))
    facts = snap["facts"]
    if drop is not None:
        facts["universe"] = [u for u in facts["universe"] if u["entity"] != drop]
    subject = store.load("PurchaseRequest", "r1")["value"]
    return evaluate(model, o.capability, state={"subject": subject}, input=o.input,
                    context=o.context, data_version=snap["data_version"], facts=facts)


@pytest.mark.scenario("C09", level=2)
def test_c09_an_incomplete_snapshot_is_refused_and_never_completed(prefix: Any) -> None:
    model, store = prefix("4")
    complete = _direct(store, model, None)
    assert complete.result == STEPS["4"]["result"], complete.reasons
    incomplete = _direct(store, model, "User")
    want = SCENARIOS["C09"]
    assert incomplete.result == want["result"]
    assert want["reason"] in json.dumps(incomplete.reasons), incomplete.reasons


@pytest.mark.scenario("C09", level=2, check="C09.input")
def test_c09_invalid_input_is_refused_as_input_not_denied(prefix: Any) -> None:
    model, store = prefix("4")
    before = harness.snapshot(store, model)
    o = harness.op("4")
    ev = store.evaluate(model, o.capability, bindings=o.bindings, input={}, context=o.context,
                        commit_time=workflow.COMMIT_TIME)
    want = SCENARIOS["C09.input"]
    assert ev.decision.result == want["result"] != "DENY"
    assert [r["code"] for r in ev.decision.reasons] == [want["reason"]]
    assert ev.bundle is None and harness.snapshot(store, model) == before


@pytest.mark.scenario("C12", level=2)
def test_c12_the_child_state_laws_refuse_invalid_child_states() -> None:
    """The uniqueness and threshold-consistency postconditions are reachable only with the
    generated guards omitted, which no public interface allows (the guards deny first). The
    model authority's own child-state tests evaluate exactly that layout through Core;
    scripts/conformance.sh runs them and records the result (research, risk 1)."""
    path = report_dir() / "c12-rust.json"
    if not path.exists():
        raise Unsupported("the child-state tests were not run (scripts/conformance.sh runs "
                          "them)", "ecosystem")
    result = json.loads(path.read_text())
    assert result["passed"], result
    assert result["tests"] >= 3, result
