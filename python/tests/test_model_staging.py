"""Declaration staging, Python side (feature 502, T006: law D2).

The build protocol now runs P2 (`declare_models`) after the authored declarations. For a model
set that contributes no declarations P2 does nothing, so every pinned StateMachine reference
case keeps its BehaviorHash and ModelHashes exactly."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from models.fixtures.reference_cases import CASES

MANIFEST = json.loads(
    (Path(__file__).resolve().parents[2] / "models" / "fixtures" / "compat.json").read_text()
)
PINNED = next(t for t in MANIFEST["tuples"] if t["model_kind"] == "state_machine")["cases"]


@pytest.mark.parametrize("name", sorted(PINNED))
def test_state_machine_cases_keep_their_pinned_hashes(name: str) -> None:
    module = CASES[name]()
    assert module.behavior_version == PINNED[name]["behavior_hash"]
    assert {r.model["model_id"]: r.model_hash for r in module.model_receipts} \
        == PINNED[name]["models"]
