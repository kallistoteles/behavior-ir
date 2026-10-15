"""Approval policy, payload and commands through the public Store (feature 503, US3).

Open, Eligible and the per-choice guards are ordinary Core predicates; the payload is typed data
that never affects the counts; commands are ordinary intents of the same commit. Nothing is
implicit: no authorization, no clock. Preconditions sit at fixed positions (data model §6): `[0]`
Open, `[1]` pending, `[2]` Eligible, `[3]` one decision per actor, `[4..]` the guards."""

from __future__ import annotations

from typing import Any

from behavior import (
    Approval, BehaviorModule, Id, InMemoryBackend, Ref, Store, command, emit, entity, field,
    project, read, select,
)
from models.fixtures.approval import T0, core_fixtures

OPEN, ELIGIBLE, FIRST_GUARD = 0, 2, 4


@entity
class Member:
    name = field(str)


@entity
class Paper:
    title = field(str)
    open = field(bool)
    level = field(int)
    requester = field(Id[Member])


@command
class Notify:
    paper = field(Id[Paper])


def by_actor(decision_id: Any, subject: Any, actor: Any) -> Any:
    return actor


def approval(**kwargs: Any) -> Approval:
    """`paper_review` with thresholds (2, 1); `approve=` / `reject=` configure a capability,
    by default with `actor` from the trusted context."""
    approve = kwargs.pop("approve", {})
    reject = kwargs.pop("reject", {})
    ap = Approval("paper_review", subject=Paper, actor=Member, approve_threshold=2,
                  reject_threshold=1, **kwargs)
    ap.approve(**{"actor": by_actor, "context": {"actor": Id[Member]}, **approve})
    ap.reject(**{"actor": by_actor, "context": {"actor": Id[Member]}, **reject})
    return ap


class Run:
    def __init__(self, ap: Approval, paper: dict[str, Any] | None = None,
                 commands: bool = False) -> None:
        self.ap = ap
        self.module = BehaviorModule(entities=[Member, Paper], models=[ap],
                                     commands=[Notify] if commands else [])
        rows = [{"entity": "Member", "value": {"id": m, "name": m}}
                for m in ("alice", "bob", "carol")]
        rows.append({"entity": "Paper", "value": {
            "id": "p1", "title": "t", "open": True, "level": 1, "requester": "alice",
            **(paper or {})}})
        if commands:
            policy = (core_fixtures() / "governance-v2" / "evidence-policy-none.json").read_text()
            genesis = Store.genesis_v2_for(self.module, rows, policy)
        else:
            genesis = Store.genesis_for(self.module, rows)
        self.store = Store.create(InMemoryBackend(), self.module, genesis)
        self.n = 0

        @read
        def decisions():  # type: ignore[no-untyped-def]
            return project(select(ap.decision_entity), lambda d: [d.actor, d.choice])

        self._decisions = decisions

    def evaluate(self, capability: str, **context: Any) -> Any:
        self.n += 1
        return self.store.evaluate(
            self.module, f"paper_review__{capability}", bindings={"subject": "p1"},
            input={"decision_id": f"x{self.n}"}, context=context, commit_time=T0)

    def decide(self, capability: str, **context: Any) -> Any:
        ev = self.evaluate(capability, **context)
        if ev.bundle is not None:
            self.store.commit(self.module, ev.bundle)
        return ev

    def decisions(self) -> list[dict[str, Any]]:
        rows: list[dict[str, Any]] = self.store.read(self.module, self._decisions).value
        return rows

    def stream(self, after: Any) -> list[Any]:
        request = {"format": "behavior.command_stream_request.v1", "after": after.as_dict(),
                   "max_records": 64}
        return list(self.store.commands_since(request).items)


def denied_at(ev: Any) -> int:
    assert ev.decision.result == "DENY", (ev.decision.result, ev.decision.reasons)
    assert ev.bundle is None
    outcomes = [s.outcome for s in ev.decision.trace if s.phase == "precondition"]
    return outcomes.index(False)


def test_open_false_is_denied() -> None:
    ap = approval(open_when=lambda paper: paper.open)
    closed = Run(ap, {"open": False})
    assert denied_at(closed.evaluate("approve", actor="bob")) == OPEN
    assert closed.decisions() == []
    assert Run(approval(open_when=lambda paper: paper.open)).decide(
        "approve", actor="bob").decision.result == "ALLOW"


def test_eligible_false_is_denied() -> None:
    r = Run(approval(eligible=lambda paper, actor, *rest: actor != paper.requester))
    assert denied_at(r.evaluate("approve", actor="alice")) == ELIGIBLE
    assert r.decide("approve", actor="bob").decision.result == "ALLOW"
    assert [d["actor"] for d in r.decisions()] == ["bob"]


def test_eligible_evaluation_error_is_core_evaluation_error() -> None:
    r = Run(approval(eligible=lambda paper, actor, *rest: 10 / paper.level > 0), {"level": 0})
    ev = r.evaluate("approve", actor="bob")
    assert ev.decision.result == "ERROR" and ev.bundle is None
    assert [x["code"] for x in ev.decision.reasons] == ["EVALUATION_ERROR"]
    assert ev.decision.reasons[0]["details"]["code"] == "DIVISION_BY_ZERO"
    # The error is Eligible's own (precondition [2]); nothing is recorded.
    step = [s.outcome for s in ev.decision.trace if s.phase == "precondition"][ELIGIBLE]
    assert step["error"]["code"] == "DIVISION_BY_ZERO"
    assert r.decisions() == []


def test_reject_guard_only_applies_to_reject() -> None:
    r = Run(approval(
        approve={"requires": [lambda decision_id, paper, actor: paper.level >= 0]},
        reject={"requires": [lambda decision_id, paper, actor: paper.level > 100]},
    ))
    assert denied_at(r.evaluate("reject", actor="carol")) == FIRST_GUARD
    assert r.decide("approve", actor="bob").decision.result == "ALLOW"


def test_unrestricted_approval_accepts_any_actor() -> None:
    # No implicit `actor != requester` or `actor == assignee`: the requester may decide.
    r = Run(approval())
    assert r.decide("approve", actor="alice").decision.result == "ALLOW"
    # A context parameter named `actor` has no special meaning: the author's actor expression
    # names the actor.
    ap = approval(approve={"actor": lambda decision_id, paper, actor, who: who,
                           "context": {"actor": Id[Member], "who": Id[Member]}})
    r2 = Run(ap)
    assert r2.decide("approve", actor="alice", who="carol").decision.result == "ALLOW"
    assert r2.decisions() == [{"id": "x1", "actor": "carol", "choice": "APPROVE"}]


def test_reference_payload_builds_and_records_the_reference() -> None:
    """A payload may hold a reference (`Ref[Member]`): the decision entity P2 declares merges
    with itself, and a committed decision records the referenced member (review 2026-10-15)."""
    ap = approval(payload={"reviewer": Ref[Member]}, approve={
        "actor": lambda decision_id, paper, actor: actor,
        "values": lambda decision_id, paper, actor: {"reviewer": actor},
    }, reject={"values": lambda decision_id, paper, actor: {"reviewer": actor}})
    r = Run(ap)
    ev = r.decide("approve", actor="bob")
    assert ev.decision.result == "ALLOW"
    assert ev.decision.lifecycle[0]["value"]["reviewer"] == "bob"


def test_payload_does_not_affect_counts() -> None:
    ap = approval(payload={"reason": str}, approve={
        "inputs": {"reason": str},
        "actor": lambda decision_id, reason, paper, actor: actor,
        "values": lambda decision_id, reason, paper, actor: {"reason": reason},
    }, reject={"values": lambda decision_id, paper, actor: {"reason": "no"}})
    r = Run(ap)
    r.n += 1
    first = r.store.evaluate(r.module, "paper_review__approve", bindings={"subject": "p1"},
                             input={"decision_id": "a", "reason": "looks good"},
                             context={"actor": "bob"}, commit_time=T0)
    assert first.bundle is not None
    r.store.commit(r.module, first.bundle)
    [created] = first.decision.lifecycle
    assert created["value"]["reason"] == "looks good"
    second = r.store.evaluate(r.module, "paper_review__approve", bindings={"subject": "p1"},
                              input={"decision_id": "b", "reason": ""},
                              context={"actor": "carol"}, commit_time=T0)
    assert second.decision.result == "ALLOW" and second.bundle is not None
    r.store.commit(r.module, second.bundle)
    # Two approvals with different reasons: k_A = 2 is reached, whatever the reasons say.
    assert denied_at(r.evaluate("reject", actor="alice")) == 1


def test_decided_at_from_context() -> None:
    ap = approval(payload={"decided_at": int}, approve={
        "context": {"actor": Id[Member], "now": int},
        "actor": lambda decision_id, paper, actor, now: actor,
        "values": lambda decision_id, paper, actor, now: {"decided_at": now},
    }, reject={
        "context": {"actor": Id[Member], "now": int},
        "actor": lambda decision_id, paper, actor, now: actor,
        "values": lambda decision_id, paper, actor, now: {"decided_at": now},
    })
    r = Run(ap)
    times = [r.decide("approve", actor=who, now=now).decision.lifecycle[0]["value"]["decided_at"]
             for who, now in (("bob", 100), ("carol", 200))]
    assert times == [100, 200]
    # The time is the host's: the module reads no clock.
    assert "clock" not in r.module.to_wire_json() and "\"now\"(" not in r.module.to_wire_json()


def _notifying() -> Approval:
    return approval(approve={"emits": [lambda decision_id, paper, actor:
                                       emit(Notify, paper=paper.id)]})


def test_command_emitted_only_on_commit() -> None:
    r = Run(_notifying(), commands=True)
    start = r.store.current_history()
    ev = r.evaluate("approve", actor="bob")
    assert ev.decision.result == "ALLOW"
    assert [c["payload"] for c in ev.decision.commands] == [{"paper": "p1"}]
    assert r.stream(start) == [], "a candidate's intent is not an occurrence"
    r.store.commit(r.module, ev.bundle)
    [occurrence] = r.stream(start)
    assert occurrence["intent"]["payload"] == {"paper": "p1"}


def test_denied_decision_emits_no_command() -> None:
    r = Run(_notifying(), commands=True)
    assert r.decide("approve", actor="bob").decision.result == "ALLOW"
    after = r.store.current_history()
    again = r.evaluate("approve", actor="bob")
    assert denied_at(again) == 3
    assert again.decision.commands == []
    assert r.stream(after) == []
