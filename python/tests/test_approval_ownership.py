"""Approval decisions are immutable audit facts owned by the approval (feature 503, US2: T031).

Everything runs through the public Store: decisions never change the subject, eligibility is
judged at decision time, Core refuses a reused decision id, and decisions survive the removal of
their subject and actor (`Id`, never `Ref`).

Terminology: DENIED = an attempt that was not permitted; REJECT = a recorded choice; REJECTED =
the derived outcome.
"""

from __future__ import annotations

from typing import Any

import pytest

from behavior import (
    Approval, BehaviorModule, Id, InMemoryBackend, Input, Store, action, entity, field, not_,
    read, remove, set_,
)

T = "2026-10-15T00:00:00Z"


@entity
class User:
    name = field(str)


@entity
class Document:
    title = field(str)
    requester = field(Id[User])


AP = Approval(
    "doc_approval", subject=Document, actor=User, approve_threshold=2, reject_threshold=1,
    eligible=lambda subject, actor: not_(actor == subject.requester),
)
AP.approve(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[User]})
AP.reject(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[User]})


@action
def reassign(document: Document, *, requester: Input[Id[User]]):
    set_(document.requester, requester)


@action
def discard(document: Document):
    remove(document)


@action
def leave(user: User):
    remove(user)


@read
def approvals(document: Document):
    return AP.approve_count(document)


@read
def is_approved(document: Document):
    return AP.approved(document)


M = BehaviorModule(entities=[User, Document], models=[AP],
                   actions=[reassign, discard, leave], reads=[approvals, is_approved])
X = AP.decision_entity.__name__


def store() -> Store:
    seed: list[dict[str, Any]] = [
        {"entity": "User", "value": {"id": u, "name": u}} for u in ("u1", "u2", "u3", "u9")
    ]
    seed.append({"entity": "Document", "value": {"id": "d1", "title": "Plan", "requester": "u9"}})
    return Store.create(InMemoryBackend(), M, Store.genesis_for(M, seed))


def decide(s: Store, capability: str, actor: str, decision_id: str) -> Any:
    """Evaluates `doc_approval__<capability>` on `d1`; commits an allowed attempt."""
    ev = s.evaluate(M, f"doc_approval__{capability}", bindings={"subject": "d1"},
                    input={"decision_id": decision_id}, context={"actor": actor},
                    commit_time=T)
    if ev.bundle is not None:
        s.commit(M, ev.bundle)
    return ev.decision


def run(s: Store, name: str, bindings: dict[str, str], **input: Any) -> Any:
    ev = s.evaluate(M, name, bindings=bindings, input=input, context={}, commit_time=T)
    assert ev.bundle is not None, ev.decision.reasons
    s.commit(M, ev.bundle)
    return ev.decision


def count(s: Store) -> Any:
    return s.read(M, approvals, bindings={"document": "d1"}).value


def test_subject_untouched_by_every_decision() -> None:
    s = store()
    for capability, actor, decision_id in [("approve", "u1", "x1"), ("reject", "u2", "x2")]:
        before = s.load("Document", "d1")
        decision = decide(s, capability, actor, decision_id)
        assert decision.result == "ALLOW", decision.reasons
        assert s.load("Document", "d1") == before
        assert [c for c in decision.changes if c.entity == "Document"] == []


def test_eligibility_change_after_decision_still_counts() -> None:
    s = store()
    assert decide(s, "approve", "u1", "x1").result == "ALLOW"
    run(s, "reassign", {"document": "d1"}, requester="u1")
    # u1 is the requester now, so u1 is no longer eligible; the earlier decision still counts.
    assert count(s) == 1
    assert decide(s, "approve", "u1", "x2").result == "DENY"
    assert decide(s, "approve", "u2", "x3").result == "ALLOW"
    assert count(s) == 2
    assert s.read(M, is_approved, bindings={"document": "d1"}).value is True


def test_reused_decision_id_is_denied_by_core() -> None:
    s = store()
    assert decide(s, "approve", "u1", "x1").result == "ALLOW"
    again = decide(s, "approve", "u2", "x1")
    assert again.result == "ENTITY_ID_ALREADY_USED"
    assert count(s) == 1


def test_actor_without_entity_is_recorded() -> None:
    s = store()
    decision = decide(s, "approve", "ghost", "x1")
    assert decision.result == "ALLOW", decision.reasons
    assert s.load(X, "x1")["value"]["actor"] == "ghost"
    assert count(s) == 1


def test_decision_survives_subject_and_actor_removal() -> None:
    s = store()
    assert decide(s, "approve", "u1", "x1").result == "ALLOW"
    run(s, "leave", {"user": "u1"})
    run(s, "discard", {"document": "d1"})
    recorded = s.load(X, "x1")["value"]
    assert (recorded["subject"], recorded["actor"], recorded["choice"]) == ("d1", "u1", "APPROVE")


@pytest.mark.parametrize("case", ["ap_document_review", "ap_purchase_request", "ap_two_approvals"])
def test_only_the_decision_actions_touch_decisions_in_every_reference_module(case: str) -> None:
    """T030 over the reference modules (T048): in the admitted module, every create, write or
    removal of a decision entity `X_p` is the single creation of `p__approve` or `p__reject`.
    This is the premise of the inductive step (T021)."""
    import json

    from models.fixtures import approval as fixtures

    wire = json.loads(getattr(fixtures, case)().to_wire_json())
    decisions = {e["name"] for e in wire["entities"] if e["name"].endswith("Decision")}
    assert decisions
    touching: dict[str, list[str]] = {}
    for action in wire["actions"]:
        entity_of = {p["name"]: p["type"].get("name") for p in action["params"]
                     if p["type"]["t"] == "entity"}
        for effect in action["effects"]:
            if effect.get("create") in decisions:
                touching.setdefault(action["name"], []).append(f"create {effect['create']}")
            elif "target" in effect and entity_of.get(effect["target"]["param"]) in decisions:
                touching.setdefault(action["name"], []).append("write")
            elif entity_of.get(effect.get("remove", "")) in decisions:
                touching.setdefault(action["name"], []).append("remove")
    assert touching, "the decision actions create decisions"
    for name, touches in touching.items():
        approval_id, _, capability = name.rpartition("__")
        assert capability in ("approve", "reject"), name
        expected = next(d for d in decisions
                        if d == "".join(w.capitalize() for w in approval_id.split("_")) + "Decision")
        assert touches == [f"create {expected}"], (name, touches)
    assert len(touching) == 2 * len(decisions)
