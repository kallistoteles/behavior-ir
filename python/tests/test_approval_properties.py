"""Approval properties over random decision sequences (feature 503, SC-003; quickstart 13).

Seeded: 1,000 random sequences over 1–4 actors, random choices, random thresholds, and random
Open and Eligible results, including concurrent pairs evaluated on one parent. Each step is
decided by Core through plain evaluation against a complete parent snapshot; the harness only
applies the creation Core reports (as a Store commit would) and reads the derived values back
through declared reads. A concurrent pair models the Store's compare-and-set: the second
candidate was evaluated on the old parent, so it is not applied; its retry is evaluated on the
new state (test_approval_concurrency.py checks the `STATE_CONFLICT` itself).

After every step:
- no `(subject, actor)` pair has two decisions;
- never `A ≥ k_A ∧ R ≥ k_R`;
- exactly one of approved, rejected, pending holds, and the counts are the recorded decisions;
- once terminal, the outcome never changes;
- `approved ⇒ A ≥ k_A` with at least `k_A` distinct approving actors;
- `rejected ⇒ k_R ≠ ⊥ ∧ R ≥ k_R`;
- a decision is allowed exactly when Open, pending, Eligible and the actor has no decision.
"""

from __future__ import annotations

import os
import random
from functools import cache
from typing import Any

from behavior import Approval, BehaviorModule, Id, entity, evaluate, evaluate_read, field, read

# A quick regression by default; the full 1,000 sequences when BEHAVIOR_FULL_PROPERTIES=1 (CI and
# the release check set it). The safety argument is the induction; this is its empirical check.
SEQUENCES = 1000 if os.environ.get("BEHAVIOR_FULL_PROPERTIES") == "1" else 32
SEED = 503
THRESHOLDS: list[tuple[int, int | None]] = [(1, None), (2, None), (2, 1), (3, 2), (1, 1)]
ACTORS = ("a1", "a2", "a3", "a4")


@entity
class PropUser:
    name = field(str)


@entity
class PropDoc:
    title = field(str)
    open = field(bool)


def _actor(decision_id: Any, subject: Any, actor: Any, ok: Any) -> Any:
    return actor


@cache
def module(k_a: int, k_r: int | None) -> tuple[BehaviorModule, str]:
    """The module for one threshold pair, and its decision entity's name. Open is the document's
    `open` field; Eligible is the trusted context flag `ok`."""
    ap = Approval("prop_approval", subject=PropDoc, actor=PropUser, approve_threshold=k_a,
                  reject_threshold=k_r, open_when=lambda doc: doc.open,
                  eligible=lambda doc, actor, decision_id, actor_param, ok: ok)
    for capability in (ap.approve, ap.reject):
        capability(actor=_actor, context={"actor": Id[PropUser], "ok": bool})
    reads = []
    for local in ("approve_count", "reject_count", "approved", "rejected", "pending"):
        reads.append(_read(ap, local))
    m = BehaviorModule(entities=[PropUser, PropDoc], models=[ap], reads=reads)
    return m, ap.decision_entity.__name__


def _read(ap: Approval, local: str) -> Any:
    handle = getattr(ap, local)

    def value(doc: PropDoc):  # type: ignore[no-untyped-def]
        return handle(doc)

    value.__name__ = value.__qualname__ = local
    return read(value)


class World:
    """The host's complete state: users, one document and the recorded decisions."""

    def __init__(self, k_a: int, k_r: int | None) -> None:
        self.k_a, self.k_r = k_a, k_r
        self.m, self.entity = module(k_a, k_r)
        self.doc: dict[str, Any] = {"id": "d1", "title": "t", "open": True}
        self.decisions: list[dict[str, Any]] = []
        self.next_id = 0

    def facts(self, new_id: str | None = None) -> dict[str, Any]:
        f: dict[str, Any] = {"universe": [
            {"entity": "PropUser", "members": [{"id": a, "name": a} for a in ACTORS]},
            {"entity": "PropDoc", "members": [self.doc]},
            {"entity": self.entity, "members": list(self.decisions)},
        ]}
        if new_id is not None:
            f["identities"] = [{"entity": self.entity, "id": new_id, "used": False}]
        return f

    def evaluate(self, capability: str, who: str, ok: bool) -> Any:
        self.next_id += 1
        new_id = f"n{self.next_id}"
        return evaluate(self.m, f"prop_approval__{capability}", state={"subject": self.doc},
                        input={"decision_id": new_id}, context={"actor": who, "ok": ok},
                        data_version="1", facts=self.facts(new_id))

    def apply(self, d: Any) -> None:
        """What a commit of an allowed decision does: exactly one creation."""
        assert d.changes == []
        [created] = d.lifecycle
        assert created["op"] == "create" and created["entity"] == self.entity
        self.decisions.append(created["value"])

    def read(self, local: str) -> Any:
        return evaluate_read(self.m, local, state={"doc": self.doc}, data_version="1",
                             facts=self.facts()).value

    def counts(self) -> tuple[int, int]:
        a = sum(d["choice"] == "APPROVE" for d in self.decisions)
        return a, len(self.decisions) - a

    def expect_allowed(self, who: str, ok: bool) -> bool:
        a, r = self.counts()
        terminal = (self.k_r is not None and r >= self.k_r) or a >= self.k_a
        decided = any(d["actor"] == who for d in self.decisions)
        return bool(self.doc["open"]) and not terminal and ok and not decided

    def outcome(self) -> str:
        approved, rejected, pending = (self.read(n) for n in ("approved", "rejected", "pending"))
        assert [approved, rejected, pending].count(True) == 1, (approved, rejected, pending)
        return "APPROVED" if approved else "REJECTED" if rejected else "PENDING"

    def check(self, previous: str) -> str:
        a, r = self.counts()
        assert (self.read("approve_count"), self.read("reject_count")) == (a, r)
        pairs = [(d["subject"], d["actor"]) for d in self.decisions]
        assert len(pairs) == len(set(pairs)), "one decision per (subject, actor)"
        assert not (self.k_r is not None and a >= self.k_a and r >= self.k_r)
        outcome = self.outcome()
        if previous != "PENDING":
            assert outcome == previous, "a terminal outcome never changes"
        if outcome == "APPROVED":
            approvers = {d["actor"] for d in self.decisions if d["choice"] == "APPROVE"}
            assert a >= self.k_a and len(approvers) >= self.k_a
        if outcome == "REJECTED":
            assert self.k_r is not None and r >= self.k_r
        return outcome


def _attempt(rng: random.Random, actors: tuple[str, ...]) -> tuple[str, str, bool]:
    return rng.choice(("approve", "reject")), rng.choice(actors), rng.random() < 0.85


def _step(w: World, capability: str, who: str, ok: bool) -> None:
    expected = w.expect_allowed(who, ok)
    d = w.evaluate(capability, who, ok)
    assert (d.result == "ALLOW") is expected, (d.result, d.reasons, capability, who, ok)
    if d.result == "ALLOW":
        w.apply(d)


def test_random_decision_sequences_keep_the_approval_invariants() -> None:
    rng = random.Random(SEED)
    terminal_seen = 0
    for _ in range(SEQUENCES):
        w = World(*rng.choice(THRESHOLDS))
        actors = ACTORS[: rng.randint(1, 4)]
        outcome = w.check("PENDING")
        for _ in range(rng.randint(1, 6)):
            if rng.random() < 0.15:
                w.doc = {**w.doc, "open": not w.doc["open"]}
            if rng.random() < 0.2:
                # A concurrent pair on one parent: both are evaluated there; the first one
                # commits, the second meets a moved head and is retried on the new state.
                first, second = _attempt(rng, actors), _attempt(rng, actors)
                parent_allowed = w.expect_allowed(second[1], second[2])
                d2_on_parent = w.evaluate(*second)
                assert (d2_on_parent.result == "ALLOW") is parent_allowed
                _step(w, *first)
                outcome = w.check(outcome)
                _step(w, *second)
            else:
                _step(w, *_attempt(rng, actors))
            outcome = w.check(outcome)
        terminal_seen += outcome != "PENDING"
    # The generator reaches terminal outcomes often enough for terminal stability to matter.
    assert terminal_seen > SEQUENCES // 5, terminal_seen
