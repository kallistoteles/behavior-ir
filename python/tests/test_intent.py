"""Structured intents through the Python API: capability, targets, and input only."""

from __future__ import annotations

import json

import pytest

from behavior import IntentRejected, evaluate_intent

from .conftest import FIXTURES
from .fixtures import invoice_model

HOST = json.loads((FIXTURES / "host" / "invoice_1042.json").read_text())


def run(intent: dict[str, object]):  # type: ignore[no-untyped-def]
    return evaluate_intent(invoice_model.model, intent, state=HOST["state"],
                           context=HOST["context"], data_version=HOST["data_version"])


def test_valid_intent_is_evaluated() -> None:
    d = run({"capability": "approve_invoice", "targets": {"invoice": "1042"}, "input": {}})
    assert d.result == "ALLOW"


def test_invalid_intent_lists_all_problems() -> None:
    with pytest.raises(IntentRejected) as info:
        run({"capability": "apply_discount", "targets": {"invoice": "9999"},
             "input": {"discount": "abc", "note": "x"}})
    assert [(e["code"], e["path"]) for e in info.value.errors] == [
        ("WRONG_TYPE", "input.discount"),
        ("EXTRA_ARGUMENT", "input.note"),
        ("TARGET_MISMATCH", "targets.invoice"),
    ]


def test_intent_cannot_supply_the_actor() -> None:
    with pytest.raises(IntentRejected):
        run({"capability": "approve_invoice", "targets": {"invoice": "1042"}, "input": {},
             "context": {"actor": {"id": "mallory", "role": "manager",
                                   "approval_limit": "99999999"}}})
