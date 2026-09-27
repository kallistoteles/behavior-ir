"""Verification through the Python API."""

from __future__ import annotations

import json

from behavior import Profile, verify

from examples.tryout.model import model


def test_demo_approve_can_break_the_budget() -> None:
    a = verify(model, profile=Profile(checks=["preservation"]))
    assert a.result == "not_verified"
    finding = a.findings[0]
    assert finding["kind"] == "preservation" and finding["severity"] == "blocking"
    assert finding["counterexample"]["record"]["result"] == "DENY"
    assert json.loads(a.json)["hash"] == a.hash
