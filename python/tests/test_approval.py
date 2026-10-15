"""Approval end to end through the public Store (feature 503, US1; quickstart 1–4).

An attempt that is not permitted is DENIED; REJECT is a recorded choice; REJECTED is the derived
outcome. The decision actions' preconditions sit at fixed positions (data model §6): `[0]` Open,
`[1]` the terminal guard `pending(subject)`, `[2]` Eligible, `[3]` one decision per actor."""

from __future__ import annotations

import json
import os
from typing import Any

import pytest

from behavior import BehaviorModule, admit
from models.fixtures.approval import Run, build, doc_approval

TERMINAL_GUARD = 1
ONE_PER_ACTOR = 3

#: `(capability, actor, allowed, (approve_count, reject_count, outcome) after)`
Step = tuple[str, str, bool, tuple[int, int, str]]


def denied_at(ev: Any) -> int:
    """The position of the precondition that denied the attempt."""
    assert ev.decision.result == "DENY", ev.decision.result
    assert ev.decision.reasons == [{"code": "PRECONDITION_FAILED"}]
    outcomes = [s.outcome for s in ev.decision.trace if s.phase == "precondition"]
    return outcomes.index(False)


def run_steps(k_a: int, k_r: int | None, steps: list[Step]) -> None:
    r = Run(build(doc_approval(k_a, k_r)))
    assert r.outcome() == (0, 0, "PENDING")
    for capability, who, allowed, after in steps:
        assert r.decide(capability, who) is allowed, (capability, who)
        assert r.outcome() == after, (capability, who)


# --- quickstart 1: the Outcome table --------------------------------------------------------------


def test_thresholds_one_without_reject_threshold() -> None:
    run_steps(1, None, [
        ("approve", "u1", True, (1, 0, "APPROVED")),
        ("approve", "u2", False, (1, 0, "APPROVED")),
    ])


def test_thresholds_two_without_reject_threshold() -> None:
    run_steps(2, None, [
        ("approve", "u1", True, (1, 0, "PENDING")),
        ("approve", "u2", True, (2, 0, "APPROVED")),
        ("reject", "u3", False, (2, 0, "APPROVED")),
    ])


def test_thresholds_two_with_veto() -> None:
    run_steps(2, 1, [
        ("approve", "u1", True, (1, 0, "PENDING")),
        ("reject", "u2", True, (1, 1, "REJECTED")),
        ("approve", "u3", False, (1, 1, "REJECTED")),
    ])


def test_thresholds_three_two_reaches_rejected() -> None:
    run_steps(3, 2, [
        ("approve", "u1", True, (1, 0, "PENDING")),
        ("reject", "u2", True, (1, 1, "PENDING")),
        ("approve", "u3", True, (2, 1, "PENDING")),
        ("reject", "u4", True, (2, 2, "REJECTED")),
        ("approve", "u5", False, (2, 2, "REJECTED")),
    ])


def test_thresholds_three_two_reaches_approved() -> None:
    run_steps(3, 2, [
        ("approve", "u1", True, (1, 0, "PENDING")),
        ("approve", "u2", True, (2, 0, "PENDING")),
        ("reject", "u3", True, (2, 1, "PENDING")),
        ("approve", "u4", True, (3, 1, "APPROVED")),
        ("reject", "u5", False, (3, 1, "APPROVED")),
    ])


@pytest.mark.parametrize("first", ["approve", "reject"])
def test_thresholds_one_one_first_committed_decision_decides(first: str) -> None:
    outcome = (1, 0, "APPROVED") if first == "approve" else (0, 1, "REJECTED")
    run_steps(1, 1, [
        (first, "u1", True, outcome),
        ("approve", "u2", False, outcome),
        ("reject", "u3", False, outcome),
    ])


# --- quickstart 2: one decision per actor ---------------------------------------------------------


def test_one_decision_per_actor_second_approve_is_denied() -> None:
    r = Run(build(doc_approval(2, None)))
    assert r.decide("approve", "u1")
    again = r.evaluate("approve", "u1")
    assert denied_at(again) == ONE_PER_ACTOR
    assert again.bundle is None
    assert r.outcome() == (1, 0, "PENDING")


def test_approve_then_reject_is_denied() -> None:
    r = Run(build(doc_approval(2, 2)))
    assert r.decide("approve", "u1")
    assert denied_at(r.evaluate("reject", "u1")) == ONE_PER_ACTOR
    assert r.outcome() == (1, 0, "PENDING")
    assert r.decisions() == [("d1", "u1", "APPROVE")]


# --- quickstart 3 and 4: veto, no reject threshold, terminality -------------------------------


def test_veto_first_reject_is_rejected() -> None:
    run_steps(3, 1, [("reject", "u1", True, (0, 1, "REJECTED"))])


def test_no_reject_threshold_rejections_count_but_stay_pending() -> None:
    run_steps(2, None, [
        ("reject", "u1", True, (0, 1, "PENDING")),
        ("reject", "u2", True, (0, 2, "PENDING")),
        ("reject", "u3", True, (0, 3, "PENDING")),
        ("approve", "u4", True, (1, 3, "PENDING")),
        ("approve", "u5", True, (2, 3, "APPROVED")),
    ])


def test_decision_after_approved_is_denied_terminal() -> None:
    r = Run(build(doc_approval(1, 2)))
    assert r.decide("approve", "u1")
    for capability in ("approve", "reject"):
        ev = r.evaluate(capability, "u2")
        assert denied_at(ev) == TERMINAL_GUARD
    assert r.outcome() == (1, 0, "APPROVED")


def test_decision_after_rejected_is_denied_terminal() -> None:
    r = Run(build(doc_approval(2, 1)))
    assert r.decide("reject", "u1")
    for capability in ("approve", "reject"):
        assert denied_at(r.evaluate(capability, "u2")) == TERMINAL_GUARD
    assert r.outcome() == (0, 1, "REJECTED")


def test_subjects_are_independent() -> None:
    r = Run(build(doc_approval(1, None)), docs=("d1", "d2"))
    assert r.decide("approve", "u1", doc="d1")
    assert r.outcome("d1") == (1, 0, "APPROVED")
    assert r.outcome("d2") == (0, 0, "PENDING")
    assert r.decide("approve", "u1", doc="d2")


# --- spec US1 scenario 10: only ordinary Core items -----------------------------------------

#: The keys an ordinary Core item may carry in the admitted wire module.
TOP = {"ir_version", "enums", "nominals", "entities", "derived", "invariants", "constraints",
       "reads", "actions", "commands"}
ITEM_KEYS = {
    "enums": {"name", "values", "loc"},
    "entities": {"name", "fields", "loc"},
    "derived": {"name", "kind", "params", "type", "body", "loc"},
    "actions": {"name", "params", "preconditions", "effects", "postconditions", "loc",
                "command_effects"},
}


def test_lowered_module_has_no_approval_constructs() -> None:
    b = build(doc_approval(2, 1))
    wire = json.loads(b.module.to_wire_json())
    assert set(wire) <= TOP
    for kind, allowed in ITEM_KEYS.items():
        for item in wire[kind]:
            assert set(item) <= allowed, (kind, item["name"], set(item) - allowed)
    # The generated items are an enum, an entity, five derived values and two actions, all with
    # ordinary names (which may contain the approval id).
    assert {e["name"] for e in wire["enums"]} == {"DocApprovalDecisionChoice"}
    assert "DocApprovalDecision" in {e["name"] for e in wire["entities"]}
    assert {d["name"] for d in wire["derived"]} == {
        f"doc_approval__{n}" for n in ("approve_count", "reject_count", "approved", "rejected",
                                       "pending")}
    assert {a["name"] for a in wire["actions"]} == {"doc_approval__approve",
                                                     "doc_approval__reject"}
    # Every action only creates: no state effects, no removals.
    for a in wire["actions"]:
        assert all("create" in e for e in a["effects"]), a["name"]
    # The public engine admits the document as is.
    again = BehaviorModule.from_wire_json(json.dumps(wire))
    assert admit(again).ok
    assert again.behavior_version == b.module.behavior_version


# --- SC-002: every decision sequence, exhaustively ------------------------------------------------

CONFIGS = [(1, None), (2, None), (2, 1), (3, 2), (1, 1)]
#: All four actors of SC-002 in the full tier (CI and release, `BEHAVIOR_FULL_PROPERTIES=1`);
#: three by default, which keeps the local run short.
ACTORS = ("u1", "u2", "u3", "u4")[:4 if os.environ.get("BEHAVIOR_FULL_PROPERTIES") == "1" else 3]
CHOICES = ("approve", "reject")


def expected(k_a: int, k_r: int | None, state: dict[str, str]) -> tuple[int, int, str]:
    """The Outcome definition, with rejection precedence."""
    a = sum(1 for c in state.values() if c == "approve")
    r = sum(1 for c in state.values() if c == "reject")
    if k_r is not None and r >= k_r:
        return a, r, "REJECTED"
    if a >= k_a:
        return a, r, "APPROVED"
    return a, r, "PENDING"


@pytest.mark.parametrize("k_a,k_r", CONFIGS)
def test_every_sequence_matches_the_outcome_definition(k_a: int, k_r: int | None) -> None:
    """SC-002: every sequence of up to 6 attempts by up to 4 actors. A denied attempt leaves
    the state unchanged and Core's evaluation is a function of the state, so every such
    sequence is a walk through the reachable decision sets (at most one decision per actor,
    hence at most 3^4 of them, each reachable in at most 4 commits). Each reachable set is
    reached through the Store by committing its path, its outcome is compared with the
    definition, and every attempt from it is checked against the definition."""
    module = build(doc_approval(k_a, k_r))
    paths: dict[tuple[tuple[str, str], ...], list[tuple[str, str]]] = {(): []}
    frontier: list[tuple[tuple[str, str], ...]] = [()]
    visited = 0
    while frontier:
        path = paths[frontier.pop()]
        state = dict(path)
        r = Run(module)
        for who, choice in path:
            assert r.decide(choice, who), (path, who, choice)
        want = expected(k_a, k_r, state)
        assert r.outcome() == want, path
        visited += 1
        for who in ACTORS:
            for choice in CHOICES:
                allowed = want[2] == "PENDING" and who not in state
                ev = r.evaluate(choice, who)
                assert (ev.bundle is not None) is allowed, (path, who, choice)
                child = tuple(sorted({**state, who: choice}.items()))
                if allowed and child not in paths:
                    paths[child] = [*path, (who, choice)]
                    frontier.append(child)
    assert visited == len(paths) > 1
