"""Commit authorization through the Python API: native dicts in, engine decision out."""

from __future__ import annotations

import json
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest

from behavior import BehaviorError, Profile, authorize, evaluate, sign_waiver, verify, waiver_hash

from examples.tryout.model import Status, model

GOV = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "governance"
NOW = "2026-09-25T12:00:00Z"


def fixture(name: str) -> dict[str, Any]:
    data: dict[str, Any] = json.loads((GOV / name).read_text())
    return data


def allowed_decision():  # type: ignore[no-untyped-def]
    purchase = {"id": "po-1", "amount": Decimal("100"), "status": Status.PENDING, "approved_by": None}
    project = {"id": "p7", "budget": Decimal("1000"), "spent": Decimal("0")}
    actor = {"id": "anna", "role": "manager", "approval_limit": Decimal("500")}
    d = evaluate(model, "approve", state={"purchase": purchase, "project": project},
                 context={"actor": actor}, data_version="1")
    assert d.result == "ALLOW"
    return d


def test_inconclusive_findings_waived_by_a_trusted_key() -> None:
    a = verify(model, profile=Profile(checks=["preservation"], rlimit=1))
    assert a.result == "not_verified"
    seed = fixture("keys.json")["A"]["seed"]
    waivers = [
        {
            "behavior_version": a.data["behavior_version"],
            "finding_hash": f["hash"],
            "profile_hash": a.data["profile"]["hash"],
            "verifier_version": a.data["verifier_version"],
            "rationale": "budget too small in this test",
        }
        for f in a.findings
    ]
    signatures = [sign_waiver(w, seed=seed) for w in waivers]
    assert [s["waiver_hash"] for s in signatures] == [waiver_hash(w) for w in waivers]

    decision = allowed_decision()
    auth = authorize(model, decision, policy=fixture("verified_or_waived.json"), attestation=a,
                     waivers=waivers, signatures=signatures, now=NOW)
    assert auth.decision == "allow" and auth.reasons == []
    assert {u["principal"] for u in auth.waivers_used} == {"test-reviewer-a"}
    assert json.loads(auth.json)["verification"]["result"] == "not_verified"

    refused = authorize(model, decision, policy=fixture("require_verified.json"), attestation=a,
                        waivers=waivers, signatures=signatures, now=NOW)
    assert refused.decision == "refuse"
    assert {r["code"] for r in refused.reasons} == {"not_verified"}


def test_invalid_governance_input_raises() -> None:
    with pytest.raises(BehaviorError):
        authorize(model, allowed_decision(), policy={"policy_version": "9", "require": "verified"},
                  now=NOW)
