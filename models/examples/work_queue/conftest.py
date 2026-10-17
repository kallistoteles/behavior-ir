"""The feature-507 conformance suite: the report hooks and shared fixtures."""

from __future__ import annotations

from dataclasses import dataclass
from functools import cached_property
from typing import Any

import pytest

from models.examples.work_queue import report
from models.examples.work_queue.report import *  # noqa: F401,F403 - the report hooks
from models.examples.work_queue import harness, workflow


@dataclass
class History:
    """One walk of the reference history, with `ValidWork` checked after every commit."""

    model: Any
    store: Any
    results: list[Any]
    invalid: list[str]

    def step(self, step: str) -> Any:
        return next(r for r in self.results if r.step == step)

    @cached_property
    def commands(self) -> list[dict[str, Any]]:
        """One export of the completed history's immutable command stream."""
        return harness.commands(self.store, self.model)


@pytest.fixture(scope="session")
def history(model: Any) -> History:
    invalid: list[str] = []

    def check(store: Any) -> None:
        try:
            value = harness.item(store)
        except Exception:  # noqa: BLE001 - before enqueue there is no item
            return
        if not harness.valid_work(value):
            invalid.append(f"position {store.current().position}: {value}")

    store, results = harness.run_history(model, on_commit=check)
    return History(model, store, results, invalid)


@pytest.fixture(scope="session")
def model() -> Any:
    return workflow.module()


@pytest.fixture(scope="session", autouse=True)
def _report_facts(model: Any) -> None:
    report.REPORT_EXTRA["models"] = {
        r.model_kind: [r.model_semantic_version, r.lowerer_version] for r in model.model_receipts}
    report.REPORT_EXTRA["profiles"] = {"evaluation": "command_intents",
                                       "genesis": "store_genesis.v2, evidence policy none"}
