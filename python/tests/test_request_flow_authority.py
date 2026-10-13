"""RequestFlow adds no authority, clock or delivery semantics (feature 502, US3: T031–T033).

Who may act is exactly what the authored guards say; commands, time and atomic creation are
ordinary Core behavior inside the generated actions."""

from __future__ import annotations

import json
from typing import Any


from behavior import (
    BehaviorModule, Id, action, entity, evaluate,
    field, remove, set_,
)

from models.fixtures.rf_purchase_request import (
    S, User, flow, module, rf_purchase_request, standard,
)

FRESH = {"identities": [{"entity": "PurchaseRequest", "id": "q1", "used": False}]}


def request(status: str, assignee: str | None = "bob", **extra: Any) -> dict[str, Any]:
    return {"id": "q1", "requester": "alice", "assignee": assignee, "status": status,
            "title": "Laptop", "amount": 900, **extra}


def run(m: BehaviorModule, transition: str, r: dict[str, Any], actor: str | None = None,
        **kw: Any) -> Any:
    return evaluate(m, f"purchase_request__{transition}", state={"purchase_request": r},
                    context={} if actor is None else {"actor": actor}, input={},
                    data_version="1", **kw)


def create(m: BehaviorModule, amount: int = 900, context: dict[str, Any] | None = None,
           creation: str = "create", **kw: Any) -> Any:
    return evaluate(m, f"purchase_request__{creation}", state=kw.pop("state", {}),
                    context=context or {"actor": "alice"},
                    input={"request_id": "q1", "title": "Laptop", "amount": amount,
                           **kw.pop("input", {})},
                    data_version="1", facts=kw.pop("facts", FRESH), **kw)


# --- T031: authority is explicit ------------------------------------------------------------

def test_without_a_guard_anyone_may_approve() -> None:
    f = flow()
    f._machine._transitions = [t for t in f._machine._transitions
                               if t["transition_id"] != "approve"]
    f.transition("approve", from_={S.IN_REVIEW}, to=S.APPROVED, context={"actor": Id[User]})
    m = module(f)
    for actor in ("alice", "bob", "carol"):
        assert run(m, "approve", request("InReview"), actor).result == "ALLOW", actor


def test_with_an_assignee_guard_only_the_assignee_may_approve() -> None:
    m = module()
    assert run(m, "approve", request("InReview"), "bob").result == "ALLOW"
    for actor in ("alice", "carol"):
        assert run(m, "approve", request("InReview"), actor).result == "DENY", actor


def test_a_requester_guard_on_cancel() -> None:
    m = module()
    assert run(m, "cancel", request("Draft"), "alice").result == "ALLOW"
    for actor in ("bob", "carol"):
        assert run(m, "cancel", request("Draft"), actor).result == "DENY", actor


def test_guards_have_core_outcomes_and_failed_creations_create_nothing() -> None:
    m = module()
    assert create(m).result == "ALLOW"
    denied = create(m, amount=0)
    assert denied.result == "DENY" and denied.lifecycle == []
    f = flow()
    f.create("divide", to=S.DRAFT, inputs={"title": str, "amount": int},
             context={"actor": Id[User]}, values=standard,
             requires=[lambda request_id, amount, title, actor: amount / (amount - amount) > 0])
    d = create(module(f), creation="divide")
    # Core's ordinary evaluation error (the task text's `DIVISION_BY_ZERO` is not a Core code).
    assert d.result == "ERROR" and d.lifecycle == []
    assert [r["code"] for r in d.reasons] == ["EVALUATION_ERROR"]
    assert "division by zero" in d.reasons[0]["message"]


# --- T032: commands and time ----------------------------------------------------------------

def with_commands() -> BehaviorModule:
    return rf_purchase_request()


def universe(*requests: dict[str, Any]) -> dict[str, Any]:
    """Complete snapshot facts, which the command profile requires (ordinary Core)."""
    users = [{"id": u, "name": u} for u in ("alice", "bob", "carol")]
    return {"universe": [{"entity": "PurchaseRequest", "members": list(requests)},
                         {"entity": "User", "members": users}]}


def test_commands_are_ordinary_intents_of_the_same_commit() -> None:
    m = with_commands()
    created = create(m, creation="announced", facts={**FRESH, **universe()})
    assert created.result == "ALLOW", created.reasons
    assert [c["payload"] for c in created.commands] == [{"request": "q1", "who": "alice"}]
    r = request("InReview")
    approved = run(m, "approve", r, "bob", facts=universe(r))
    assert approved.result == "ALLOW", approved.reasons
    assert [c["payload"] for c in approved.commands] == [{"request": "q1", "who": "alice"}]
    denied = run(m, "approve", r, "carol", facts=universe(r))
    assert denied.result == "DENY" and denied.commands == []


def test_time_comes_from_context_only() -> None:
    f = flow_with_created_at()
    m = module(f)
    d = create(m, context={"actor": "alice", "now": 1_700_000_000})
    assert d.result == "ALLOW"
    assert d.lifecycle[0]["value"]["created_at"] == 1_700_000_000
    wire = m.to_wire_json().lower()
    for word in ("clock", "random", "uuid", "\"now\"("):
        assert word not in wire, word


def flow_with_created_at() -> Any:
    from behavior import RequestFlow

    f = RequestFlow("purchase_request", actor=User, status=S, assignee=True,
                    payload={"title": str, "amount": int, "created_at": int})
    f.create("create", to=S.DRAFT, inputs={"title": str, "amount": int},
             context={"actor": Id[User], "now": int},
             values=lambda request_id, amount, title, actor, now: {
                 **standard(request_id, amount, title, actor), "created_at": now})
    f.transition("submit", from_={S.DRAFT}, to=S.SUBMITTED)
    return f


# --- T033: atomic creation and Id semantics -------------------------------------------------

@entity
class Budget:
    reserved = field(int)


def with_budget(post: bool = True, aliasing: bool = False) -> BehaviorModule:
    f = flow()
    state: dict[str, Any] = {"budget": Budget}
    if aliasing:
        state["other"] = Budget
    f.create(
        "reserve", to=S.DRAFT, inputs={"title": str, "amount": int}, state=state,
        context={"actor": Id[User]},
        values=lambda request_id, amount, title, *rest: standard(
            request_id, amount, title, rest[-1]),
        effects=[lambda request_id, amount, title, budget, *rest:
                 set_(budget.reserved, budget.reserved + amount)],
        ensures=[(lambda request_id, amount, title, budget, *rest: budget.reserved < 1000)
                 if post else (lambda *a: True)],
    )
    return BehaviorModule(entities=[User, Budget], enums=[S], models=[f])


def test_creation_and_its_effects_commit_together() -> None:
    d = create(with_budget(), creation="reserve",
               state={"budget": {"id": "b1", "reserved": 50}})
    assert d.result == "ALLOW"
    assert d.lifecycle[0]["value"]["status"] == "Draft"
    assert [(c.param, c.field, c.new) for c in d.changes] == [("budget", "reserved", 950)]


def test_a_failing_postcondition_rolls_back_both() -> None:
    d = create(with_budget(), creation="reserve",
               state={"budget": {"id": "b1", "reserved": 500}})
    assert d.result != "ALLOW"
    assert d.lifecycle == [] and d.changes == []


def test_aliasing_state_bindings_are_cores_ordinary_case() -> None:
    m = with_budget(aliasing=True)
    budget = {"id": "b1", "reserved": 50}
    d = create(m, creation="reserve", state={"budget": budget, "other": budget})
    assert d.result != "ALLOW"
    assert not any("REQUEST" in json.dumps(r) for r in d.reasons)


def test_requester_and_assignee_are_identities_not_references() -> None:
    @action
    def remove_user(u: User):
        remove(u)

    m = module(actions=[remove_user])
    # Removing a user that requests reference is allowed: Id adds no existence constraint.
    d = evaluate(m, "remove_user", state={"u": {"id": "alice", "name": "A"}},
                 context={}, input={}, data_version="1")
    assert d.result == "ALLOW", d.reasons
    assert "exists" not in m.to_wire_json()
