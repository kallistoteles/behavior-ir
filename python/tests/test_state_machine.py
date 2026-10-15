"""StateMachine through the Python binding (feature 501, US1: T020, T021).

A machine is authored in Python, lowered by the Rust model authority to ordinary actions, and
run by Core. Every outcome here is an ordinary Core outcome.
"""

from __future__ import annotations

import itertools
import json
from enum import Enum
from typing import Any

import pytest

from behavior import (
    BehaviorModule, Id, ModelBuildError, Option, StateMachine, entity, evaluate, field, replay,
    set_,
)


class RequestStatus(Enum):
    DRAFT = "Draft"
    SUBMITTED = "Submitted"
    IN_REVIEW = "InReview"
    APPROVED = "Approved"
    REJECTED = "Rejected"


@entity
class Actor:
    name = field(str)


@entity
class PurchaseRequest:
    status = field(RequestStatus)
    approver = field(Id[Actor])
    approved_by = field(Option[Id[Actor]])
    amount = field(int)


def purchase_request(**over: Any) -> StateMachine:
    m = StateMachine(
        "purchase_request",
        subject=PurchaseRequest,
        state_field="status",
        entry_states={RequestStatus.DRAFT},
        display_name="Purchase request",
    )
    m.transition("submit", from_={RequestStatus.DRAFT}, to=RequestStatus.SUBMITTED)
    m.transition("start_review", from_={RequestStatus.SUBMITTED}, to=RequestStatus.IN_REVIEW)
    m.transition(
        "approve",
        from_={RequestStatus.IN_REVIEW},
        to=RequestStatus.APPROVED,
        context={"actor": Id[Actor]},
        requires=[over.get("guard", lambda r, actor: actor == r.approver)],
        effects=[lambda r, actor: set_(r.approved_by, actor)],
        display_name="Approve request",
    )
    m.transition("reject", from_={RequestStatus.IN_REVIEW}, to=RequestStatus.REJECTED)
    return m


def build(*models: StateMachine) -> BehaviorModule:
    return BehaviorModule(
        entities=[Actor, PurchaseRequest], enums=[RequestStatus], models=list(models)
    )


M = build(purchase_request())

TRANSITIONS = {
    "submit": ({"Draft"}, "Submitted"),
    "start_review": ({"Submitted"}, "InReview"),
    "approve": ({"InReview"}, "Approved"),
    "reject": ({"InReview"}, "Rejected"),
}


def request(status: str, **over: Any) -> dict[str, Any]:
    base: dict[str, Any] = {
        "id": "r1", "status": status, "approver": "anna", "approved_by": None, "amount": 10,
    }
    base.update(over)
    return base


def universe(req: dict[str, Any]) -> dict[str, Any]:
    """The complete parent snapshot (Core 017): every declared entity type, here the one request
    and the approver."""
    return {"universe": [
        {"entity": "PurchaseRequest", "members": [req]},
        {"entity": "Actor", "members": [{"id": "anna", "name": "Anna"}]},
    ]}


def invoke(module: BehaviorModule, transition: str, status: str, actor: str = "anna") -> Any:
    context = {"actor": actor} if transition == "approve" else {}
    req = request(status)
    return evaluate(
        module, f"purchase_request__{transition}",
        state={"purchase_request": req}, context=context, input={},
        data_version="1", facts=universe(req),
    )


@pytest.mark.parametrize(
    ("state", "transition"),
    list(itertools.product([s.value for s in RequestStatus], TRANSITIONS)),
)
def test_transition_matrix(state: str, transition: str) -> None:
    """SC-001: a transition commits exactly from its source states; otherwise Core's ordinary
    precondition denial, with no state-machine-specific outcome."""
    sources, target = TRANSITIONS[transition]
    d = invoke(M, transition, state)
    if state in sources:
        assert d.result == "ALLOW"
        assert ("status", state, target) in [(c.field, c.old, c.new) for c in d.changes]
    else:
        assert d.result == "DENY"
        assert d.changes == []
        pre = [s for s in d.trace if s.phase == "precondition"]
        assert pre[0].outcome is False  # the generated source-state requirement
        assert all(s.outcome == "skipped" for s in pre[1:])


def test_guard_true_and_false_and_extra_effect() -> None:
    ok = invoke(M, "approve", "InReview", actor="anna")
    assert ok.result == "ALLOW"
    assert [(c.field, c.new) for c in ok.changes] == [
        ("status", "Approved"), ("approved_by", "anna")]
    denied = invoke(M, "approve", "InReview", actor="bob")
    assert denied.result == "DENY"
    assert denied.changes == []


def test_guard_evaluation_error_is_cores() -> None:
    m = build(purchase_request(guard=lambda r, actor: (r.amount / 0) > 0))
    d = invoke(m, "approve", "InReview")
    assert d.result not in ("ALLOW", "DENY")
    assert d.changes == []


def test_core_consumer_round_trip() -> None:
    """The lowered module is ordinary Core behavior: wire JSON admits back to the same
    module, and decisions replay, through the public API only."""
    again = BehaviorModule.from_wire_json(M.to_wire_json())
    assert again.behavior_version == M.behavior_version
    d = invoke(M, "submit", "Draft")
    assert replay(M, d.record_json).matches


def test_no_model_items_survive() -> None:
    """Erasure: the module holds the declarations plus one ordinary action per transition."""
    wire = json.loads(M.to_wire_json())
    assert sorted(a["name"] for a in wire["actions"]) == sorted(
        f"purchase_request__{t}" for t in TRANSITIONS)
    assert sorted(e["name"] for e in wire["entities"]) == ["Actor", "PurchaseRequest"]


def test_python_names_and_collection_order_are_not_semantic() -> None:
    """T021: renamed lambda parameters and reordered or duplicated Python collections give the
    identical admitted module (Python passes raw input; Rust decides)."""
    other = StateMachine(
        "purchase_request", subject=PurchaseRequest, state_field="status",
        entry_states=[RequestStatus.DRAFT, RequestStatus.DRAFT],
    )
    other.transition("reject", from_=[RequestStatus.IN_REVIEW], to=RequestStatus.REJECTED)
    other.transition(
        "approve", from_=[RequestStatus.IN_REVIEW, RequestStatus.IN_REVIEW],
        to=RequestStatus.APPROVED, context=[("actor", Id[Actor])],
        requires=[lambda req, who: who == req.approver],
        effects=[lambda req, who: set_(req.approved_by, who)],
    )
    other.transition("start_review", from_=[RequestStatus.SUBMITTED], to=RequestStatus.IN_REVIEW)
    other.transition("submit", from_=[RequestStatus.DRAFT], to=RequestStatus.SUBMITTED)
    assert build(other).behavior_version == M.behavior_version


def test_input_and_context_declaration_order_is_not_semantic() -> None:
    def with_inputs(inputs: dict[str, Any]) -> str | None:
        m = purchase_request()
        m.transition(
            "note", from_={RequestStatus.DRAFT}, to=RequestStatus.SUBMITTED, inputs=inputs,
            effects=[lambda r, amount, reason: set_(r.amount, amount)],
        )
        return build(m).behavior_version

    # Positional binding follows Rust's canonical (sorted) order: amount, reason.
    assert with_inputs({"reason": str, "amount": int}) == with_inputs(
        {"amount": int, "reason": str})


def test_build_errors_are_the_model_authoritys() -> None:
    m = StateMachine(
        "PurchaseRequest", subject=PurchaseRequest, state_field="status",
        entry_states={"Ghost"},
    )
    with pytest.raises(ModelBuildError) as e:
        build(m)
    assert [x["code"] for x in e.value.errors] == ["INVALID_SEMANTIC_ID", "UNKNOWN_STATE"]


def test_a_core_denial_is_explained_through_the_source_map() -> None:
    """T050: Core's record names only the Core action; the receipt's source map lets tooling
    explain the denial in model terms."""
    d = invoke(M, "approve", "Draft")
    assert d.result == "DENY"
    assert "purchase_request/transition" not in d.record_json
    [receipt] = M.model_receipts
    entry = next(e for e in receipt.source_map["entries"]
                 if e["core"].get("name") == "purchase_request__approve")
    assert entry["model"] == "purchase_request/transition/approve"
    assert entry["display"] == {"display_name": "Approve request"}
    assert receipt.behavior_hash == M.behavior_version
    assert receipt.model["transitions"][0]["from"] == ["InReview"]


def test_diagnostics_are_warnings() -> None:
    assert {(d["code"], d["severity"]) for d in M.model_diagnostics} == {
        ("NO_OUTGOING_TRANSITIONS", "warning")}


# --- Commands on transitions (T075, FR-022, US1/AC1) -------------------------------------------

from behavior import command, emit  # noqa: E402


@command
class NotifyRequester:
    request_approver = field(Id[Actor])
    amount = field(int)


def with_commands(emits: list[Any]) -> BehaviorModule:
    m = purchase_request()
    m.transition(
        "escalate", from_={RequestStatus.SUBMITTED}, to=RequestStatus.IN_REVIEW,
        emits=emits, display_name="Escalate",
    )
    return BehaviorModule(entities=[Actor, PurchaseRequest], enums=[RequestStatus],
                          commands=[NotifyRequester], models=[m])


def invoke_commands(m: BehaviorModule, status: str) -> Any:
    req = request(status)
    return evaluate(m, "purchase_request__escalate", state={"purchase_request": req},
                    context={}, input={}, data_version="1", facts=universe(req))


def test_a_transition_emits_a_command_in_the_same_commit() -> None:
    m = with_commands([
        lambda r: emit(NotifyRequester, request_approver=r.approver, amount=r.amount),
        lambda r: emit(NotifyRequester, when=r.amount > 100, request_approver=r.approver,
                       amount=0),
    ])
    d = invoke_commands(m, "Submitted")
    assert d.result == "ALLOW"
    assert [(c.field, c.new) for c in d.changes] == [("status", "InReview")]
    # One intent: the second emission's `when` (amount > 100) is false. Core names the command
    # by its declaration hash.
    assert [c["payload"] for c in d.commands] == [{"amount": 10, "request_approver": "anna"}]


def test_a_denied_transition_emits_nothing() -> None:
    m = with_commands([lambda r: emit(NotifyRequester, request_approver=r.approver, amount=1)])
    d = invoke_commands(m, "Draft")
    assert d.result == "DENY"
    assert d.commands == []


def test_emits_must_build_an_emission() -> None:
    with pytest.raises(ModelBuildError) as e:
        with_commands([lambda r: r.amount])
    assert [x["code"] for x in e.value.errors] == ["FRAGMENT_INVALID"]


def test_commands_enter_the_model_hash() -> None:
    a = with_commands([lambda r: emit(NotifyRequester, request_approver=r.approver, amount=1)])
    b = with_commands([lambda r: emit(NotifyRequester, request_approver=r.approver, amount=2)])
    assert a.model_receipts[0].model_hash != b.model_receipts[0].model_hash


# --- Receipts across checkouts (T076, Constitution IV) -----------------------------------------

def test_receipts_are_identical_across_checkouts(tmp_path: Any) -> None:
    import importlib.util
    import pathlib

    source = (pathlib.Path(__file__).parents[2] / "models/examples/state_machine/machine.py")
    receipts = []
    for i, where in enumerate(("one/checkout", "ci/build/elsewhere")):
        d = tmp_path / where
        d.mkdir(parents=True)
        (d / "machine.py").write_text(source.read_text())
        spec = importlib.util.spec_from_file_location(f"_checkout_{i}", d / "machine.py")
        assert spec is not None and spec.loader is not None
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        receipts.append(mod.module().model_receipts[0])
    a, b = (json.dumps(r.to_dict(), sort_keys=True) for r in receipts)
    assert a == b
    assert "/" not in receipts[0].source_map["entries"][0]["source"]["file"]


# --- Typed receipts (T078) ----------------------------------------------------------------------

def test_receipts_are_typed_views_of_the_authoritys_receipt() -> None:
    from dataclasses import FrozenInstanceError

    from behavior import LoweringReceipt

    [r] = M.model_receipts
    assert isinstance(r, LoweringReceipt)
    assert r.model_kind == "state_machine"
    assert (r.model_semantic_version, r.lowerer_version) == (1, 1)
    assert r.behavior_hash == M.behavior_version
    assert r.model_hash.startswith("sha256:")
    assert LoweringReceipt.from_dict(r.to_dict()) == r
    with pytest.raises(FrozenInstanceError):
        r.model_hash = "x"  # type: ignore[misc]
    drifted = {**r.to_dict(), "unexpected": 1}
    with pytest.raises(RuntimeError):
        LoweringReceipt.from_dict(drifted)
