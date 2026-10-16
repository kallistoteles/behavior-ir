"""The feature-506 conformance suite: the report hooks and shared fixtures."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import pytest

from models.examples.purchase_approval import report
from models.examples.purchase_approval.report import *  # noqa: F401,F403 - the report hooks
from models.examples.purchase_approval import harness, workflow


@dataclass
class History:
    """One walk of reference history H, with the invariant checks of reference-workflow.md §2
    recorded after every commit."""

    model: Any
    store: Any
    results: list[Any]
    records: dict[str, Any]
    invariant_failures: list[str]

    def step(self, step: str) -> list[Any]:
        return [r for r in self.results if r.step == step]


def walk_history() -> History:
    model = workflow.module()
    failures: list[str] = []

    def check(store: Any) -> None:
        rows = harness.decision_rows(store, model)
        position = store.current().position
        if not harness.unique_per_actor(rows):
            failures.append(f"VO-003 at position {position}: {rows}")
        if not harness.thresholds_exclusive(rows):
            failures.append(f"VO-004 at position {position}: {rows}")

    records: dict[str, Any] = {}
    store, results = harness.run_history(model, on_commit=check, records=records,
                                         track_unchanged=True)
    return History(model, store, results, records, failures)


@pytest.fixture(scope="session")
def history() -> History:
    return walk_history()


@pytest.fixture(scope="session")
def prefix() -> Any:
    """`prefix(step)`: one model and a store that has walked H up to (not including) `step`,
    shared by the tests that only evaluate against that head. A test that commits builds its
    own store."""
    cache: dict[str, tuple[Any, Any]] = {}
    model = workflow.module()

    def at(step: str) -> tuple[Any, Any]:
        if step not in cache:
            store, _ = harness.run_history(model, stop_before=step)
            cache[step] = (model, store)
        return cache[step]

    return at


@pytest.fixture(scope="session", autouse=True)
def _report_facts() -> None:
    """Model versions and profiles for the report (data-model §7)."""
    model = workflow.module()
    report.REPORT_EXTRA["models"] = {
        r.model_kind: [r.model_semantic_version, r.lowerer_version]
        for r in model.model_receipts}
    report.REPORT_EXTRA["profiles"] = {"evaluation": "command_intents",
                                       "genesis": "store_genesis.v2, evidence policy none"}
