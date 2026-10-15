"""Declaration staging, Python side (feature 502, T006: law D2; feature 503, T005: law D9).

The build protocol runs P2 (`declare_models`) after the authored declarations. Feature 503
generalizes P2 to a topological pass over the model-declaration DAG, with enums and
definitions. Law D9: for every model set without edges and without definitions (every 501 and
502 set), the builder sees exactly the calls of feature 502, so every pinned reference case
keeps its BehaviorHash, its ModelHashes, its embedded hashes and its generated entity names.

The reference-case builders are Python-only (models/fixtures/reference_cases.py), so the
regression lives here; each entry is computed exactly as the `models compat` gate computes it."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
from typing import Any

import pytest

from models.fixtures.reference_cases import CASES

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = json.loads((ROOT / "models" / "fixtures" / "compat.json").read_text())
PINNED: dict[str, tuple[str, dict[str, Any]]] = {
    name: (t["model_kind"], case)
    for t in MANIFEST["tuples"]
    if t["model_kind"] in ("state_machine", "request_flow")
    for name, case in t["cases"].items()
}


def _compat() -> Any:
    spec = importlib.util.spec_from_file_location(
        "check_models_compat", ROOT / "scripts" / "check-models-compat.py"
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_every_501_and_502_tuple_is_covered() -> None:
    kinds = {kind for kind, _ in PINNED.values()}
    assert kinds == {"state_machine", "request_flow"}


@pytest.mark.parametrize("name", sorted(PINNED))
def test_pinned_cases_keep_their_hashes(name: str) -> None:
    module = CASES[name]()
    receipts = [r.to_dict() for r in module.model_receipts]
    # The same entry the gate pins: BehaviorHash, ModelHashes, embedded hashes, entity names.
    assert _compat().case_entry(module, receipts) == PINNED[name][1]
